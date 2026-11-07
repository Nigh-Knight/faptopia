use clap::{Args, Parser, Subcommand};
use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct MediaItem {
    url: String,
    sd_url: Option<String>,
    gif_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Post {
    no: Option<u64>,
    sub: Option<String>,
    ext: Option<String>,
    tim: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct Thread {
    posts: Vec<Post>,
}

type BoardResult = Result<BoardFetch, String>;

struct BoardFetch {
    items: Vec<MediaItem>,
    new_urls: HashMap<String, CacheEntry>,
    new_listings: HashMap<String, ListingEntry>,
}

// RedGifs API response structures
#[derive(Deserialize)]
struct RedGifsAuthResponse {
    token: String,
}

#[derive(Deserialize)]
struct RedGifsGifUrls {
    hd: Option<String>,
    sd: String,
}

#[derive(Deserialize)]
struct RedGifsGif {
    id: Option<String>,
    urls: RedGifsGifUrls,
}

#[derive(Deserialize)]
struct RedGifsSingleGifResponse {
    gif: Option<RedGifsGif>,
}

// Cache structures for storing resolved RedGifs URLs
#[derive(Serialize, Deserialize, Clone)]
struct CacheEntry {
    url: String,
    #[serde(default)]
    sd_url: Option<String>,
    fetched_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct UrlCache {
    redgifs: HashMap<String, CacheEntry>,
    #[serde(default)]
    listings: HashMap<String, ListingEntry>,
}

// A board's scraped slug list. Re-running the same boards is the common case, so
// serving the list from disk removes the reddit request entirely - which is the
// only slow part of the pipeline.
#[derive(Serialize, Deserialize, Clone)]
struct ListingEntry {
    ids: Vec<String>,
    fetched_at: u64,
}

const CACHE_FILE: &str = ".faptopia_cache.json";
const CACHE_LOCK_FILE: &str = ".faptopia_cache.lock";
const CACHE_TTL_SECS: u64 = 4 * 60 * 60; // 4 hours
const LISTING_TTL_SECS: u64 = 60 * 60; // 1 hour

fn load_cache() -> UrlCache {
    fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &UrlCache) {
    // Two concurrent runs both read at start and write at the end, so the last
    // writer would drop the other's entries. Take an exclusive lock and merge
    // with whatever landed on disk meanwhile. Temp file + rename keeps readers
    // from seeing a half-written file.
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(CACHE_LOCK_FILE)
        .ok();
    if let Some(file) = &lock {
        let _ = file.lock();
    }

    let mut merged = load_cache();
    merged.redgifs.extend(cache.redgifs.clone());
    merged.listings.extend(cache.listings.clone());

    if let Ok(json) = serde_json::to_string(&merged) {
        let temp = format!("{}.{}", CACHE_FILE, std::process::id());
        if fs::write(&temp, json).is_ok() {
            let _ = fs::rename(&temp, CACHE_FILE);
        } else {
            let _ = fs::remove_file(&temp);
        }
    }

    if let Some(file) = &lock {
        let _ = file.unlock();
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// extracts a RedGifs slug from a Reddit Atom entry. The link is HTML-encoded
// inside the entry body, e.g. &quot;https://www.redgifs.com/watch/tepididealxenopus&quot;
static REDGIFS_LINK_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"redgifs\.com/(?:watch|ifr)/([a-zA-Z0-9_-]+)").unwrap());

// extracts the last post id of an Atom page, reused as the ?after= cursor
static ENTRY_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"<id>t3_([a-z0-9]+)</id>").unwrap());

// ---------------------------------------------------------------------------
// Reddit access control
//
// Reddit rate limits by source IP and, once authenticated, by OAuth client id.
// Measured behaviour (2026):
//   anonymous  -> one-request bucket, x-ratelimit-reset: 5, then hard 429s
//                 (documented ceiling: 10 requests/minute)
//   OAuth      -> 100 requests/minute per client id, averaged over 10 minutes
// Sixteen minutes ago the anonymous limit was enforced with a flat 62s sleep,
// i.e. ~10x slower than the limit requires. The gate below instead paces from
// the server's own x-ratelimit-remaining / x-ratelimit-reset headers and
// adapts: it relaxes toward `base` on clean responses and tightens on a 429.
// ---------------------------------------------------------------------------

const REDDIT_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

struct RedditGate {
    last: Option<Instant>,
    interval: Duration,
    floor: Duration,
    ceiling: Duration,
    blocked_until: Option<Instant>,
}

impl RedditGate {
    fn wait(&mut self) {
        // A 429 parks every worker on the same deadline, so they cannot retry
        // into each other and re-trip the limit (measured: the anonymous window
        // is ~60s, so independent per-worker backoff just produced 429 storms).
        if let Some(until) = self.blocked_until.take() {
            progress_block(until);
            // Slice the sleep so the countdown in the status line keeps ticking
            loop {
                let now = Instant::now();
                if until <= now {
                    break;
                }
                thread::sleep((until - now).min(Duration::from_millis(250)));
                progress_tick();
            }
            progress_unblock();
        }
        if let Some(prev) = self.last {
            let elapsed = prev.elapsed();
            // Jitter stops workers from lining up on the same window edge
            let target = self.interval + Duration::from_millis(rand::random::<u64>() % 1_500);
            if elapsed < target {
                thread::sleep(target - elapsed);
            }
        }
        self.last = Some(Instant::now());
    }

    fn block(&mut self, secs: u64) {
        let until = Instant::now() + Duration::from_secs(secs);
        self.blocked_until = Some(match self.blocked_until {
            Some(existing) if existing > until => existing,
            _ => until,
        });
    }

    // Additive-increase/multiplicative-decrease: a 429 widens the steady spacing
    // so later requests stop re-tripping the window; clean responses creep back
    // toward the floor. Reset headers alone are not enough - they announce the
    // current window, not how many requests reddit will tolerate in the next one.
    fn penalize(&mut self, wait: u64) {
        let widened = self
            .interval
            .mul_f32(1.6)
            .max(Duration::from_secs(wait))
            .min(self.ceiling);
        self.interval = widened;
    }

    fn reward(&mut self) {
        if self.interval > self.floor {
            self.interval = self.interval.mul_f32(0.9).max(self.floor);
        }
    }
}

// Anonymous reddit is a ~60s window over a handful of requests, and the exact
// budget moves with how flagged the IP is. 10s floor (6/min) with AIMD widening
// up to 40s tracks it; a fixed 62s (the old behaviour) was ~6x slower.
static ANON_GATE: Lazy<Mutex<RedditGate>> = Lazy::new(|| {
    Mutex::new(RedditGate {
        last: None,
        interval: Duration::from_secs(10),
        floor: Duration::from_secs(10),
        ceiling: Duration::from_secs(40),
        blocked_until: None,
    })
});

// 100 req/min = one every 600ms; a little slack for jitter
static OAUTH_GATE: Lazy<Mutex<RedditGate>> = Lazy::new(|| {
    Mutex::new(RedditGate {
        last: None,
        interval: Duration::from_millis(700),
        floor: Duration::from_millis(700),
        ceiling: Duration::from_millis(5_000),
        blocked_until: None,
    })
});

fn reddit_gate(oauth: bool) {
    let gate = if oauth { &OAUTH_GATE } else { &ANON_GATE };
    gate.lock().unwrap().wait();
}

// Reads the budget headers. A success always relaxes the spacing (anonymous
// reddit reports remaining=0 even on a 200, so the header cannot be the only
// signal); an empty bucket also parks the gate for the advertised reset.
fn reddit_note_budget(oauth: bool, remaining: Option<f64>, reset: Option<u64>) {
    let gate = if oauth { &OAUTH_GATE } else { &ANON_GATE };
    let mut gate = gate.lock().unwrap();
    gate.reward();
    if remaining.map(|r| r < 1.0).unwrap_or(false) {
        gate.block(reset.unwrap_or(if oauth { 2 } else { 10 }).clamp(1, 60));
    }
}

// Parks the gate after a 429 for the full window the server asks for, not just
// the caller's own retry, and widens the steady spacing.
fn reddit_penalize(oauth: bool, reset: Option<u64>) {
    let gate = if oauth { &OAUTH_GATE } else { &ANON_GATE };
    // reset: 0 means the window already rolled over - still wait a beat so a
    // retry cannot land inside the same second.
    let wait = reset
        .unwrap_or(if oauth { 5 } else { 30 })
        .max(1)
        .clamp(1, 90);
    let mut gate = gate.lock().unwrap();
    gate.penalize(wait);
    gate.block(wait);
}

// ---------------------------------------------------------------------------
// Credentials
//
// Stored at $XDG_CONFIG_HOME/faptopia/config.json (0600) because a Reddit
// "script" app needs a client id/secret plus the account password, and pasting
// four secrets per invocation is miserable. Env vars override the file so a
// throwaway account can be used without rewriting it.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default, Clone)]
struct RedditConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

impl RedditConfig {
    fn complete(&self) -> bool {
        self.client_id.is_some()
            && self.client_secret.is_some()
            && self.username.is_some()
            && self.password.is_some()
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Config {
    #[serde(default)]
    reddit: RedditConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    concurrency: Option<usize>,
}

// Reads a value from the terminal when the flag was omitted. Enter keeps the
// currently stored value so a typo'd single field can be fixed without retyping.
fn prompt(label: &str, flag: Option<String>, current: Option<String>) -> Option<String> {
    if let Some(value) = flag {
        return Some(value);
    }
    match &current {
        Some(existing) => print!("{} [{}]: ", label, existing),
        None => print!("{}: ", label),
    }
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => current,
        Ok(_) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                current
            } else {
                Some(trimmed.to_string())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Progress
//
// Boards fetch in parallel, so per-item println lines from 8 threads are noise.
// Instead one status line is rewritten in place with \r: counts plus any rate
// limit countdown. Non-tty output gets board-level lines only.
// ---------------------------------------------------------------------------

struct Progress {
    boards_total: usize,
    boards_done: usize,
    videos_total: usize,
    videos_done: usize,
    blocked_until: Option<Instant>,
    rendered: usize,
    last_draw: Option<Instant>,
}

static PROGRESS: Lazy<Mutex<Progress>> = Lazy::new(|| {
    Mutex::new(Progress {
        boards_total: 0,
        boards_done: 0,
        videos_total: 0,
        videos_done: 0,
        blocked_until: None,
        rendered: 0,
        last_draw: None,
    })
});

fn progress_enabled() -> bool {
    io::stdout().is_terminal()
}

impl Progress {
    fn status(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.boards_total > 0 {
            parts.push(format!("{}/{} boards", self.boards_done, self.boards_total));
        }
        if self.videos_total > 0 {
            parts.push(format!("{}/{} videos", self.videos_done, self.videos_total));
        }
        if let Some(until) = self.blocked_until {
            let now = Instant::now();
            if until > now {
                parts.push(format!("waiting {}s", (until - now).as_secs() + 1));
            }
        }
        parts.join("   ")
    }

    fn clear(&mut self) {
        if self.rendered > 0 {
            print!("\r{}", " ".repeat(self.rendered));
            print!("\r");
            let _ = io::stdout().flush();
            self.rendered = 0;
        }
    }

    fn draw(&mut self) {
        let line = self.status();
        let pad = self.rendered.saturating_sub(line.len());
        print!("\r{}{}", line, " ".repeat(pad));
        let _ = io::stdout().flush();
        self.rendered = line.len();
        self.last_draw = Some(Instant::now());
    }
}

// Draws at most every 80ms unless forced, so 8 threads do not thrash the tty.
fn progress_render(force: bool) {
    if !progress_enabled() {
        return;
    }
    let mut progress = PROGRESS.lock().unwrap();
    if !force {
        if let Some(last) = progress.last_draw {
            if last.elapsed() < Duration::from_millis(80) {
                return;
            }
        }
    }
    progress.draw();
}

fn progress_tick() {
    progress_render(true);
}

fn progress_start(boards: usize) {
    PROGRESS.lock().unwrap().boards_total = boards;
    progress_render(true);
}

fn progress_add_videos(count: usize) {
    PROGRESS.lock().unwrap().videos_total += count;
    progress_render(false);
}

fn progress_video_done() {
    if !progress_enabled() {
        return;
    }
    PROGRESS.lock().unwrap().videos_done += 1;
    progress_render(false);
}

fn progress_board_done() {
    PROGRESS.lock().unwrap().boards_done += 1;
    progress_render(true);
}

fn progress_board_position() -> usize {
    PROGRESS.lock().unwrap().boards_done + 1
}

fn progress_boards_total() -> usize {
    PROGRESS.lock().unwrap().boards_total
}

fn progress_block(until: Instant) {
    if !progress_enabled() {
        return;
    }
    PROGRESS.lock().unwrap().blocked_until = Some(until);
    progress_render(true);
}

fn progress_unblock() {
    if !progress_enabled() {
        return;
    }
    PROGRESS.lock().unwrap().blocked_until = None;
    progress_render(true);
}

// Finishes the status line and drops it, so the shell prompt is clean.
fn progress_finish() {
    if !progress_enabled() {
        return;
    }
    PROGRESS.lock().unwrap().clear();
}

// Prints a line above the status line without leaving it mangled.
fn progress_log(message: &str) {
    if !progress_enabled() {
        println!("{}", message);
        return;
    }
    {
        let mut progress = PROGRESS.lock().unwrap();
        progress.clear();
    }
    println!("{}", message);
    progress_render(true);
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("faptopia").join("config.json"))
}

fn load_config() -> Config {
    let mut cfg: Config = config_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    if let Some(v) = env("FAPTOPIA_REDDIT_CLIENT_ID") {
        cfg.reddit.client_id = Some(v);
    }
    if let Some(v) = env("FAPTOPIA_REDDIT_CLIENT_SECRET") {
        cfg.reddit.client_secret = Some(v);
    }
    if let Some(v) = env("FAPTOPIA_REDDIT_USERNAME") {
        cfg.reddit.username = Some(v);
    }
    if let Some(v) = env("FAPTOPIA_REDDIT_PASSWORD") {
        cfg.reddit.password = Some(v);
    }
    cfg
}

fn save_config(cfg: &Config) -> io::Result<()> {
    let path = config_path().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "no config directory (HOME unset?)")
    })?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(io::Error::other)?;
    fs::write(&path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    println!("Saved {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// OAuth
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RedditTokenResponse {
    access_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
}

static REDDIT_TOKEN: Lazy<Mutex<Option<(String, Instant)>>> = Lazy::new(|| Mutex::new(None));

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        out.push(TABLE[(b[0] >> 2) as usize] as char);
        out.push(TABLE[(((b[0] & 0b11) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b[1] & 0b1111) << 2) | (b[2] >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b[2] & 0b111111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn form_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

fn clear_reddit_token() {
    *REDDIT_TOKEN.lock().unwrap() = None;
}

// Mints (and caches) an OAuth bearer token via the password grant that reddit
// script apps use. Tokens live 24h; refresh a minute early to avoid a race.
fn reddit_token(creds: &RedditConfig) -> Result<String, String> {
    if let Some((token, expires)) = REDDIT_TOKEN.lock().unwrap().as_ref() {
        if *expires > Instant::now() {
            return Ok(token.clone());
        }
    }

    let client_id = creds.client_id.as_deref().ok_or("missing client id")?;
    let client_secret = creds
        .client_secret
        .as_deref()
        .ok_or("missing client secret")?;
    let username = creds.username.as_deref().ok_or("missing username")?;
    let password = creds.password.as_deref().ok_or("missing password")?;

    let body = format!(
        "grant_type=password&username={}&password={}",
        form_encode(username),
        form_encode(password)
    );
    let auth = format!(
        "Basic {}",
        base64_encode(format!("{}:{}", client_id, client_secret).as_bytes())
    );

    let response = ureq::post("https://www.reddit.com/api/v1/access_token")
        .set("User-Agent", REDDIT_UA)
        .set("Authorization", &auth)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&body);

    let parsed: RedditTokenResponse = match response {
        Ok(resp) => resp
            .into_json()
            .map_err(|e| format!("token response was not JSON: {}", e))?,
        Err(ureq::Error::Status(code, resp)) => {
            let detail = resp.into_string().unwrap_or_default();
            return Err(format!(
                "token request returned {}: {}",
                code,
                detail.trim()
            ));
        }
        Err(e) => return Err(e.to_string()),
    };

    let token = parsed.access_token.ok_or_else(|| {
        format!(
            "no access_token in response ({})",
            parsed.error.unwrap_or_else(|| "unknown".into())
        )
    })?;
    let ttl = parsed.expires_in.unwrap_or(3600).saturating_sub(60);
    *REDDIT_TOKEN.lock().unwrap() =
        Some((token.clone(), Instant::now() + Duration::from_secs(ttl)));
    Ok(token)
}

// Verify creds work without depending on whatever is cached.
fn verify_reddit_creds(creds: &RedditConfig) -> Result<(), String> {
    clear_reddit_token();
    reddit_token(creds).map(|_| ())
}

#[derive(Clone)]
enum RedditAuth {
    Anon,
    OAuth(RedditConfig),
}

impl RedditAuth {
    fn is_oauth(&self) -> bool {
        matches!(self, RedditAuth::OAuth(_))
    }
}

// parses Reddit URLs to extract subreddit, modifier, and time parameters
static REDDIT_URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https?://(?:www\.)?reddit\.com/r/([^/]+)/([^/?]+)(?:/?\?t=([^&]+))?").unwrap()
});

// parses Reddit user profile URLs to extract username
// sort and t params are extracted manually from the query string
static REDDIT_USER_URL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^https?://(?:www\.)?reddit\.com/user/([^/?]+)").unwrap());

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(
        long,
        global = true,
        help = "Shuffle videos AND boards/threads in a random order"
    )]
    random: bool,
    #[arg(
        long,
        global = true,
        help = "Reverse video AND board/thread order (start from last)"
    )]
    reverse: bool,
    #[arg(long, global = true, help = "Start local server for mobile access")]
    serve: bool,
    #[arg(
        long,
        global = true,
        default_value_t = 6969,
        help = "Port for local server"
    )]
    port: u16,
    #[arg(
        long,
        global = true,
        help = "Deploy to Cloudflare Pages (PWA with HTTPS)"
    )]
    deploy: bool,
    #[arg(
        long,
        global = true,
        help = "Ignore the board listing cache and re-scrape reddit"
    )]
    refresh: bool,
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Gallery output path (needed when running two jobs at once)"
    )]
    output: Option<String>,
    #[arg(
        long,
        global = true,
        value_name = "N",
        help = "Concurrent RedGifs lookups (default 8)"
    )]
    con: Option<usize>,
}

#[derive(Subcommand)]
enum Commands {
    Reddit(SubReddit),
    #[clap(name = "4chan")]
    FourChan(ThreadId),
    #[clap(about = "Deploy existing gallery files to Cloudflare Pages")]
    Deploy,
    #[clap(about = "Generate usage stats page (faptopia_stats.html)")]
    Stats,
    #[clap(about = "Store reddit OAuth credentials (100 req/min instead of 10)")]
    Auth(AuthArgs),
}

#[derive(Args)]
struct AuthArgs {
    #[arg(long, help = "OAuth client id from reddit.com/prefs/apps (script app)")]
    client_id: Option<String>,
    #[arg(long, help = "OAuth client secret from reddit.com/prefs/apps")]
    client_secret: Option<String>,
    #[arg(long, help = "Reddit account username")]
    username: Option<String>,
    #[arg(long, help = "Reddit account password")]
    password: Option<String>,
    #[arg(long, help = "Delete the stored credentials")]
    logout: bool,
}

#[derive(Args)]
struct ThreadId {
    thread: Option<Vec<u64>>,
}

#[derive(Args)]
struct SubReddit {
    name: Option<Vec<String>>,
    #[arg(
        long,
        default_value_t = 25,
        help = "Number of posts to fetch (default: 25)"
    )]
    limit: usize,
}

#[derive(Clone)]
enum RedditSource {
    Subreddit {
        name: String,
        modifier: String,
        time: String,
    },
    User {
        name: String,
        modifier: String,
        time: String,
    },
}

/// Extract a query parameter from a URL query string
fn extract_query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == name {
            return parts.next().map(|v| v.to_string());
        }
    }
    None
}

/// Returns the display label for a Reddit source (e.g. "r/hotwife" or "u/theworldwidebull")
fn source_label(source: &RedditSource) -> String {
    match source {
        RedditSource::Subreddit { name, .. } => format!("r/{}", name),
        RedditSource::User { name, .. } => format!("u/{}", name),
    }
}

fn parse_reddit_input(input: &str) -> Result<RedditSource, String> {
    if input.starts_with("http://") || input.starts_with("https://") {
        // Try subreddit URL first
        if let Some(caps) = REDDIT_URL_REGEX.captures(input) {
            return Ok(RedditSource::Subreddit {
                name: caps
                    .get(1)
                    .ok_or("Missing subreddit in URL")?
                    .as_str()
                    .to_string(),
                modifier: caps
                    .get(2)
                    .ok_or("Missing modifier in URL")?
                    .as_str()
                    .to_string(),
                time: caps
                    .get(3)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "all".to_string()),
            });
        }
        // Try user profile URL
        if let Some(caps) = REDDIT_USER_URL_REGEX.captures(input) {
            let username = caps
                .get(1)
                .ok_or("Missing username in URL")?
                .as_str()
                .to_string();
            // Parse query params manually for flexibility
            let query_part = input.split('?').nth(1).unwrap_or("");
            let sort = extract_query_param(query_part, "sort").unwrap_or_else(|| "hot".to_string());
            let time = extract_query_param(query_part, "t").unwrap_or_else(|| "all".to_string());
            return Ok(RedditSource::User {
                name: username,
                modifier: sort,
                time,
            });
        }
        return Err(
            "Invalid Reddit URL. Expected: https://reddit.com/r/SUBREDDIT/MODIFIER/?t=TIME or https://reddit.com/user/NAME/?sort=X&t=Y"
                .to_string(),
        );
    }

    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() == 3 {
        if parts[0].starts_with("u/") {
            Ok(RedditSource::User {
                name: parts[0].trim_start_matches("u/").to_string(),
                modifier: parts[1].to_string(),
                time: parts[2].to_string(),
            })
        } else {
            // Bare name or r/name — both default to subreddit
            let name = parts[0].trim_start_matches("r/");
            Ok(RedditSource::Subreddit {
                name: name.to_string(),
                modifier: parts[1].to_string(),
                time: parts[2].to_string(),
            })
        }
    } else {
        Err(
            "Invalid format. Use 'subreddit:modifier:time', 'u/username:sort:time', or a full Reddit URL"
                .to_string(),
        )
    }
}

fn main() -> io::Result<()> {
    let cli: Cli = Cli::parse();

    let random = cli.random;
    let reverse = cli.reverse;
    let output_override = cli.output.clone();
    match &cli.command {
        Commands::FourChan(args) => {
            let output = output_override
                .clone()
                .unwrap_or_else(|| "faptopia_4chan.html".to_string());
            if let Some(ids) = &args.thread {
                // Randomize/reverse thread order so tabs come out shuffled too
                let mut ids = ids.clone();
                if random {
                    ids.shuffle(&mut thread_rng());
                }
                if reverse {
                    ids.reverse();
                }

                // Fetch all threads in parallel
                let handles: Vec<_> = ids
                    .iter()
                    .map(|&id| {
                        thread::spawn(move || {
                            fetch_video_links_4chan(&[id]).map_err(|e| e.to_string())
                        })
                    })
                    .collect();

                let mut thread_items = Vec::new();
                for (id, handle) in ids.iter().zip(handles) {
                    match handle.join().expect("thread panicked") {
                        Ok(threads) => {
                            for (title, links) in threads {
                                let mut items: Vec<MediaItem> = links
                                    .into_iter()
                                    .map(|url| MediaItem {
                                        url,
                                        sd_url: None,
                                        gif_id: None,
                                    })
                                    .collect();
                                if random {
                                    items.shuffle(&mut thread_rng());
                                }
                                if reverse {
                                    items.reverse();
                                }
                                thread_items.push((title, items));
                            }
                        }
                        Err(e) => eprintln!("Error fetching thread {}: {}", id, e),
                    }
                }
                save_gallery(thread_items, &output)?;
            } else {
                println!("INPUT A THREAD ID");
            }
        }
        Commands::Reddit(args) => {
            let output = output_override
                .clone()
                .unwrap_or_else(|| "faptopia_reddit.html".to_string());
            if let Some(names) = &args.name {
                // Parse all inputs into RedditSource (subreddit or user)
                let mut parsed: Vec<RedditSource> = names
                    .iter()
                    .filter_map(|x| match parse_reddit_input(x) {
                        Ok(source) => Some(source),
                        Err(e) => {
                            eprintln!("Invalid input '{}': {}", x, e);
                            None
                        }
                    })
                    .collect();

                // Randomize/reverse board order so tabs come out shuffled too
                if random {
                    parsed.shuffle(&mut thread_rng());
                }
                if reverse {
                    parsed.reverse();
                }

                // Load cache and fetch token once; each thread gets clones
                let cache = load_cache();
                let limit = args.limit;
                let token = match fetch_redgifs_token() {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Failed to fetch RedGifs token: {}", e);
                        return Ok(());
                    }
                };

                // --- access mode -------------------------------------------------
                let config = load_config();
                let concurrency = cli.con.or(config.concurrency).unwrap_or(8).clamp(1, 32);

                let mut auth = if config.reddit.complete() {
                    RedditAuth::OAuth(config.reddit.clone())
                } else {
                    RedditAuth::Anon
                };

                // Never let broken creds look like a dead network
                if let RedditAuth::OAuth(creds) = &auth {
                    if let Err(e) = verify_reddit_creds(creds) {
                        eprintln!("OAuth failed ({}), falling back to anonymous", e);
                        auth = RedditAuth::Anon;
                    }
                }

                // Boards whose listing is still fresh need no reddit request at
                // all, so the ETA counts only the ones that actually will.
                let needs_scrape = if cli.refresh {
                    parsed.len()
                } else {
                    parsed
                        .iter()
                        .filter(|source| {
                            cache
                                .listings
                                .get(&listing_key(source))
                                .map(|entry| {
                                    now_secs().saturating_sub(entry.fetched_at) >= LISTING_TTL_SECS
                                        || entry.ids.is_empty()
                                })
                                .unwrap_or(true)
                        })
                        .count()
                };

                // One terse line: how long, not why.
                if parsed.len() > 1 {
                    if needs_scrape == 0 {
                        println!("Fetching {} boards (all cached)", parsed.len());
                    } else {
                        println!(
                            "Fetching {} boards (~{} min)",
                            parsed.len(),
                            needs_scrape.div_ceil(6).max(1)
                        );
                    }
                }
                progress_start(parsed.len());

                // Boards run through a worker pool. The rate gate serialises the
                // actual request starts, so more workers than the gate can feed
                // just keeps the pipeline from stalling on RedGifs latency.
                // Results land in a per-index slot so tab order is preserved.
                let board_workers = concurrency.clamp(1, 8).min(parsed.len().max(1));
                let slots: Vec<Mutex<Option<(String, BoardResult)>>> =
                    (0..parsed.len()).map(|_| Mutex::new(None)).collect();
                let cursor = AtomicUsize::new(0);

                thread::scope(|scope| {
                    for _ in 0..board_workers {
                        scope.spawn(|| loop {
                            let i = cursor.fetch_add(1, Ordering::Relaxed);
                            if i >= parsed.len() {
                                break;
                            }
                            let source = &parsed[i];
                            let label = source_label(source);
                            let result = fetch_media_embeds_reddit(
                                source,
                                limit,
                                cache.redgifs.clone(),
                                cache.listings.clone(),
                                token.clone(),
                                &auth,
                                concurrency,
                                cli.refresh,
                            );
                            *slots[i].lock().unwrap() = Some((label, result));
                        });
                    }
                });

                let mut subreddit_items = Vec::new();
                let mut all_new_entries: HashMap<String, CacheEntry> = HashMap::new();
                let mut all_new_listings: HashMap<String, ListingEntry> = HashMap::new();
                for slot in slots {
                    match slot.into_inner().unwrap() {
                        Some((name, Ok(fetch))) => {
                            let mut items = fetch.items;
                            if random {
                                items.shuffle(&mut thread_rng());
                            }
                            if reverse {
                                items.reverse();
                            }
                            subreddit_items.push((name, items));
                            all_new_entries.extend(fetch.new_urls);
                            all_new_listings.extend(fetch.new_listings);
                        }
                        Some((name, Err(e))) => eprintln!("Error fetching {}: {}", name, e),
                        None => {}
                    }
                }

                // Merge new entries back into cache and save once
                let mut final_cache = UrlCache {
                    redgifs: cache.redgifs,
                    listings: cache.listings,
                };
                final_cache.redgifs.extend(all_new_entries);
                final_cache.listings.extend(all_new_listings);
                save_cache(&final_cache);

                progress_finish();
                save_gallery(subreddit_items, &output)?;
            } else {
                println!("INPUT A SUBREDDIT PAGE");
            }
        }
        Commands::Deploy => {
            deploy_to_cloudflare()?;
        }
        Commands::Auth(args) => {
            if args.logout {
                let mut config = load_config();
                config.reddit = RedditConfig::default();
                save_config(&config)?;
                println!("Credentials removed - anonymous access (10 req/min) until you run `faptopia auth`");
                return Ok(());
            }

            let mut config = load_config();
            config.reddit.client_id = prompt(
                "client id (reddit.com/prefs/apps, the line under the app name)",
                args.client_id.clone(),
                config.reddit.client_id,
            );
            config.reddit.client_secret = prompt(
                "client secret (reddit.com/prefs/apps, labelled \"secret\")",
                args.client_secret.clone(),
                config.reddit.client_secret,
            );
            config.reddit.username = prompt(
                "reddit username",
                args.username.clone(),
                config.reddit.username,
            );
            config.reddit.password = prompt(
                "reddit password",
                args.password.clone(),
                config.reddit.password,
            );

            match verify_reddit_creds(&config.reddit) {
                Ok(()) => {
                    save_config(&config)?;
                    println!("OAuth works - reddit now allows 100 requests/minute");
                }
                Err(e) => {
                    eprintln!("Credentials rejected: {}", e);
                    eprintln!("Not saved. Check the app type is \"script\" and the redirect uri is http://localhost");
                }
            }
        }
        Commands::Stats => {
            generate_stats_page()?;
        }
    }

    if cli.deploy {
        deploy_to_cloudflare()?;
    }

    if cli.serve {
        serve_gallery(cli.port)?;
    }

    Ok(())
}

// with an input of links to the videos(items) and html filename to write to, creates a scrollable gallery
fn save_gallery(items: Vec<(String, Vec<MediaItem>)>, filename: &str) -> io::Result<()> {
    if items.is_empty() {
        println!("No media items found");
        return Ok(());
    }

    let html = generate_gallery(items, filename);
    fs::write(filename, html)?;
    println!("Gallery saved to {}", filename);
    Ok(())
}

// deploys gallery to Cloudflare Pages with video proxy worker
fn deploy_to_cloudflare() -> io::Result<()> {
    let deploy_dir = ".cf_deploy";
    fs::create_dir_all(deploy_dir)?;

    // Copy gallery files with proxy/on-demand URLs
    for filename in &["faptopia_reddit.html", "faptopia_4chan.html"] {
        if std::path::Path::new(filename).exists() {
            let html = fs::read_to_string(filename)?;
            let html = rewrite_html_for_proxy(&html);
            fs::write(format!("{}/{}", deploy_dir, filename), html)?;
        }
    }

    // Index page redirects to whichever gallery exists
    let index_target = if std::path::Path::new("faptopia_reddit.html").exists() {
        "faptopia_reddit.html"
    } else {
        "faptopia_4chan.html"
    };
    fs::write(
        format!("{}/index.html", deploy_dir),
        format!(
            r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={}"></head></html>"#,
            index_target
        ),
    )?;

    // PWA files
    fs::write(
        format!("{}/manifest.json", deploy_dir),
        include_str!("../templates/manifest.json"),
    )?;
    fs::write(
        format!("{}/sw.js", deploy_dir),
        include_str!("../templates/sw.js"),
    )?;
    fs::write(
        format!("{}/icon.svg", deploy_dir),
        include_str!("../templates/icon.svg"),
    )?;

    // Cloudflare Worker for video proxying
    fs::write(
        format!("{}/_worker.js", deploy_dir),
        include_str!("../templates/_worker.js"),
    )?;

    // Stats page (reads localStorage from same origin)
    fs::write(
        format!("{}/faptopia_stats.html", deploy_dir),
        include_str!("../templates/stats.html"),
    )?;

    println!("Deploying to Cloudflare Pages...");

    let status = Command::new("wrangler")
        .args([
            "pages",
            "deploy",
            deploy_dir,
            "--project-name",
            "faptopia",
            "--branch",
            "main",
            "--commit-dirty=true",
        ])
        .status()?;

    let _ = fs::remove_dir_all(deploy_dir);

    if status.success() {
        println!("Deployed! Visit https://faptopia.pages.dev");
        println!("Stats: https://faptopia.pages.dev/faptopia_stats.html");
        println!("Add to Home Screen on your phone for full PWA experience.");
    } else {
        eprintln!("Deploy failed. Run 'wrangler login' to authenticate.");
    }

    Ok(())
}

// gets the local network IP address
fn get_local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

// starts a local HTTP server for mobile PWA access
fn serve_gallery(port: u16) -> io::Result<()> {
    let serve_dir = ".faptopia_serve";
    fs::create_dir_all(serve_dir)?;

    // Copy gallery files, rewriting video URLs to go through local proxy
    for filename in &["faptopia_reddit.html", "faptopia_4chan.html"] {
        if std::path::Path::new(filename).exists() {
            let html = fs::read_to_string(filename)?;
            let html = rewrite_html_for_proxy(&html);
            fs::write(format!("{}/{}", serve_dir, filename), html)?;
        }
    }

    // Default index.html redirects to whichever gallery exists
    let index_target = if std::path::Path::new("faptopia_reddit.html").exists() {
        "faptopia_reddit.html"
    } else {
        "faptopia_4chan.html"
    };
    fs::write(
        format!("{}/index.html", serve_dir),
        format!(
            r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={}"></head></html>"#,
            index_target
        ),
    )?;

    // Write PWA files for Add to Home Screen
    fs::write(
        format!("{}/manifest.json", serve_dir),
        include_str!("../templates/manifest.json"),
    )?;
    fs::write(
        format!("{}/icon.svg", serve_dir),
        include_str!("../templates/icon.svg"),
    )?;

    // Stats page (reads localStorage from same origin)
    fs::write(
        format!("{}/faptopia_stats.html", serve_dir),
        include_str!("../templates/stats.html"),
    )?;

    // Write the proxy server script
    let server_script = format!("{}/server.py", serve_dir);
    fs::write(&server_script, include_str!("../templates/server.py"))?;

    let ip = get_local_ip();
    println!("Serving gallery at:");
    println!("  Local:   http://localhost:{}", port);
    println!("  Mobile:  http://{}:{}", ip, port);
    println!("  Stats:   http://localhost:{}/faptopia_stats.html", port);
    println!();
    println!("Open the Mobile URL on your phone and tap 'Add to Home Screen' for PWA.");
    println!("Press Ctrl+C to stop.");

    let status = Command::new("python3")
        .args([&server_script, &port.to_string(), serve_dir])
        .status()?;

    // Clean up
    let _ = fs::remove_dir_all(serve_dir);

    if !status.success() {
        eprintln!("Server exited unexpectedly.");
    }

    Ok(())
}

// Template files embedded at compile time
const HTML_TEMPLATE: &str = include_str!("../templates/gallery.html");
const CSS_TEMPLATE: &str = include_str!("../templates/styles.css");
const JS_TEMPLATE: &str = include_str!("../templates/script.js");
const STATS_TEMPLATE: &str = include_str!("../templates/stats.html");

fn generate_stats_page() -> io::Result<()> {
    let filename = "faptopia_stats.html";
    fs::write(filename, STATS_TEMPLATE)?;
    println!("Stats page saved to {}", filename);
    println!("Open it in the same browser you use the galleries.");
    println!("Tip: use --serve so all pages share the same origin.");
    Ok(())
}

// rewrites gallery HTML for proxy/deploy: RedGifs videos use on-demand resolution,
// 4chan videos use simple proxy
fn rewrite_html_for_proxy(html: &str) -> String {
    // Replace RedGifs video sources with on-demand resolution endpoint using gif IDs
    let redgifs_re = Regex::new(r#"(?s)data-gif-id="([^"]+)"(.*?)<source src="[^"]*""#).unwrap();
    let html = redgifs_re
        .replace_all(html, |caps: &regex::Captures| {
            format!(
                r#"data-gif-id="{}"{}<source src="/video/redgifs/{}""#,
                &caps[1], &caps[2], &caps[1]
            )
        })
        .to_string();
    // 4chan CDN URLs are permanent, simple proxy is fine
    html.replace("https://i.4cdn.org/", "/proxy/4cdn/")
}

// create a horizontal scrollable gallery
fn generate_gallery(items: Vec<(String, Vec<MediaItem>)>, filename: &str) -> String {
    // Determine toggle button based on current file
    let (toggle_text, toggle_file, toggle_class) = if filename.contains("reddit") {
        ("4CHAN", "faptopia_4chan.html", "gallery-toggle--4chan") // Green for 4chan
    } else {
        ("REDDIT", "faptopia_reddit.html", "gallery-toggle--reddit") // Reddit orange/red
    };

    // Generate tabs and content sections
    let (tabs, sections): (Vec<String>, Vec<String>) = items
        .iter()
        .enumerate()
        .map(|(section_index, (source_name, items))| {
            let gallery_items: String = items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let global_index = format!("{}-{}", section_index, index);
                    let gif_id_attr = match &item.gif_id {
                        Some(id) => format!(r#" data-gif-id="{}""#, id),
                        None => String::new(),
                    };
                    // SD fallback URL (only when it differs from the primary)
                    let sd_url_attr = match &item.sd_url {
                        Some(sd) if *sd != item.url => format!(r#" data-sd-url="{}""#, sd),
                        _ => String::new(),
                    };
                    let mime_type = if item.url.ends_with(".webm") {
                        "video/webm"
                    } else {
                        "video/mp4"
                    };
                    // Only the very first video of the first board autoplays and preloads.
                    // All others start as preload="none" — JS promotes them when the user
                    // is about to need them, avoiding ~1100 simultaneous network requests on load.
                    // Only the very first video's metadata is preloaded for instant
                    // player chrome. All others start with preload="none" so the browser
                    // doesn't start any network requests until the user navigates to them.
                    // This prevents saturating the single HTTP/2 connection to the CDN.
                    let is_first = section_index == 0 && index == 0;
                    format!(
                        r#"<div class="gallery-item" data-index="{global_index}">
                            <video
                                controls
                                {autoplay}
                                {muted}
                                playsinline
                                data-index="{global_index}"{gif_id_attr}{sd_url_attr}
                                preload="{preload}"
                            >
                                <source src="{url}" type="{mime_type}">
                            </video>
                        </div>"#,
                        url = item.url,
                        autoplay = if is_first { "autoplay" } else { "" },
                        muted = if is_first { "muted" } else { "" },
                        preload = if is_first { "metadata" } else { "none" }
                    )
                })
                .collect();

            let tab = format!(
                r#"<button class="tab-button{}" onclick="showSection({})">
                    <span class="tab-label">{}</span>
                    <span class="tab-close" onclick="closeTab(event, {})">×</span>
                </button>"#,
                if section_index == 0 { " active" } else { "" },
                section_index,
                source_name,
                section_index
            );

            let section = format!(
                r#"<div class="gallery-section{}" data-section="{}">
                    <div class="gallery-em" id="gallery-{}">
                        <div class="gallery-em__track">
                        {}
                        </div>
                    </div>
                </div>"#,
                if section_index == 0 {
                    " active"
                } else {
                    " hidden"
                },
                section_index,
                section_index,
                gallery_items
            );

            (tab, section)
        })
        .unzip();

    // Inject the generated content into templates
    let platform = if filename.contains("reddit") {
        "reddit"
    } else {
        "4chan"
    };
    let script = JS_TEMPLATE
        .replace("{{TOTAL_SECTIONS}}", &items.len().to_string())
        .replace("{{PLATFORM}}", platform);

    let toggle_button = format!(
        r#"<a href="{}" class="gallery-toggle {} ">{}</a>"#,
        toggle_file, toggle_class, toggle_text
    );
    let tabs_with_toggle = format!("{}\n{}", tabs.join("\n"), toggle_button);

    HTML_TEMPLATE
        .replace("{{STYLES}}", CSS_TEMPLATE)
        .replace("{{TABS}}", &tabs_with_toggle)
        .replace("{{SECTIONS}}", &sections.join("\n"))
        .replace("{{SCRIPT}}", &script)
}

// gets the 4chan video links from the gif board
pub fn fetch_video_links_4chan(
    thread_ids: &[u64],
) -> Result<Vec<(String, Vec<String>)>, ureq::Error> {
    let mut results = Vec::new();
    for id in thread_ids {
        let url = format!("https://a.4cdn.org/gif/thread/{}.json", id);
        let response = ureq::get(&url).call()?;
        let thread: Thread = response.into_json()?;

        let title = thread
            .posts
            .first()
            .and_then(|op| op.sub.clone())
            .unwrap_or_else(|| format!("Thread {}", id));

        let links: Vec<String> = thread
            .posts
            .into_iter()
            .filter_map(|post| match (post.ext.as_ref(), post.tim) {
                (Some(ext), Some(tim)) if ext == ".mp4" || ext == ".webm" => {
                    Some(format!("https://i.4cdn.org/gif/{}{}", tim, ext))
                }
                _ => None,
            })
            .collect();

        results.push((title, links));
    }
    Ok(results)
}

// fetches a temporary auth token from RedGifs API
fn fetch_redgifs_token() -> Result<String, ureq::Error> {
    let response = ureq::get("https://api.redgifs.com/v2/auth/temporary")
        .set("User-Agent", "rust:faptopia:v1.0")
        .call()?;
    let auth: RedGifsAuthResponse = response.into_json()?;
    Ok(auth.token)
}

// RedGifs deprecated the batch query endpoint (?ids= — returns 404).
// The multi-ID path form requires a user token (PATCH), unavailable to
// temporary tokens. Fall back to fetching each gif individually.
// Returns (hd, sd) URL pairs so galleries can fall back to SD when HD fails.
// Resolves one gif id to its (hd, sd) URL pair. Callers fan this out across a
// worker pool; the API is per-id anyway, so batching bought nothing but latency.
fn fetch_redgifs_one(gif_id: &str, token: &str) -> Option<(String, String)> {
    let id_lower = gif_id.to_lowercase();
    let url = format!("https://api.redgifs.com/v2/gifs/{}", id_lower);
    let response = ureq::get(&url)
        .set("User-Agent", "rust:faptopia:v1.0")
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .ok()?;
    let gif_data = response.into_json::<RedGifsSingleGifResponse>().ok()?;
    let gif = gif_data.gif?;
    let RedGifsGifUrls { hd, sd } = gif.urls;
    let hd = hd.unwrap_or_else(|| sd.clone());
    Some((hd, sd))
}

// One rate-limited reddit GET. Both the anonymous feed and the OAuth JSON API
// go through here so pacing, budget accounting and the retry ladder live once.
fn reddit_get(url: &str, auth: &RedditAuth) -> Result<String, String> {
    let oauth = auth.is_oauth();
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        reddit_gate(oauth);

        let mut request = ureq::get(url)
            .set("User-Agent", REDDIT_UA)
            .set("Cookie", "over18=1")
            .set("Accept-Language", "en-US,en;q=0.9");

        if let RedditAuth::OAuth(creds) = auth {
            let token = reddit_token(creds)?;
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        match request.call() {
            Ok(response) => {
                let remaining = response
                    .header("x-ratelimit-remaining")
                    .and_then(|v| v.parse::<f64>().ok());
                let reset = response
                    .header("x-ratelimit-reset")
                    .and_then(|v| v.parse::<u64>().ok());
                // The next caller now waits if the bucket is spent.
                reddit_note_budget(oauth, remaining, reset);
                return response.into_string().map_err(|e| e.to_string());
            }
            Err(ureq::Error::Status(401, _)) if oauth && attempt < 3 => {
                clear_reddit_token();
            }
            Err(ureq::Error::Status(code, response))
                if (code == 429 || code == 403 || code == 503) && attempt < 8 =>
            {
                let wait = response
                    .header("x-ratelimit-reset")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(if oauth { 5 } else { 30 });
                reddit_penalize(oauth, Some(wait));
                // The gate now holds the deadline; nothing to sleep here.
            }
            Err(ureq::Error::Status(code, _)) => {
                return Err(format!("reddit returned status {}", code))
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

// Collects seen slugs into `gif_ids` and reports how many were new, so a page
// that adds nothing ends the walk instead of costing another request.
fn collect_gif_ids(body: &str, gif_ids: &mut Vec<String>, seen: &mut HashSet<String>) -> usize {
    let before = gif_ids.len();
    for cap in REDGIFS_LINK_REGEX.captures_iter(body) {
        if let Some(id) = cap.get(1) {
            let id = id.as_str().to_lowercase();
            if seen.insert(id.clone()) {
                gif_ids.push(id);
            }
        }
    }
    gif_ids.len() - before
}

// Fetches RedGifs IDs from the authenticated JSON listing.
//
// url_overridden_by_dest carries the redgifs link for media posts and the
// preview blobs carry it too, so scanning the raw body for redgifs slugs (same
// as the RSS path) is both shorter and more tolerant than walking fields.
fn fetch_redgifs_ids_from_json(
    source: &RedditSource,
    limit: usize,
    auth: &RedditAuth,
) -> Result<Vec<String>, String> {
    let mut gif_ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut after: Option<String> = None;
    let mut pages = 0;
    let per_page = 100;

    // OAuth is cheap enough to walk a few more pages than the anonymous path
    while gif_ids.len() < limit && pages < 4 {
        pages += 1;
        let base = match source {
            RedditSource::Subreddit {
                name,
                modifier,
                time,
            } => format!(
                "https://oauth.reddit.com/r/{}/{}.json?t={}&limit={}&raw_json=1",
                name, modifier, time, per_page
            ),
            RedditSource::User {
                name,
                modifier,
                time,
            } => format!(
                "https://oauth.reddit.com/user/{}/submitted.json?sort={}&t={}&limit={}&raw_json=1",
                name, modifier, time, per_page
            ),
        };
        let url = match &after {
            Some(cursor) => format!("{}&after={}", base, cursor),
            None => base,
        };

        let body = reddit_get(&url, auth)?;
        if collect_gif_ids(&body, &mut gif_ids, &mut seen) == 0 {
            break;
        }

        let listing: serde_json::Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => break,
        };
        match listing["data"]["after"].as_str() {
            Some(cursor) if !cursor.is_empty() => after = Some(cursor.to_string()),
            _ => break,
        }
    }

    // No truncation here: the full page yield gets cached, so a later run with a
    // larger --limit benefits from it.
    Ok(gif_ids)
}

// Fetches RedGifs IDs from a subreddit/user Atom feed.
//
// Reddit has walled off its anonymous JSON API (403 "blocked by network
// security") and old.reddit.com now 302s logged-out clients to a login page,
// but the public /r/<sub>/<sort>/.rss feed still works and carries the redgifs
// link inside each entry's HTML body. The feed honours limit=100 and
// ?after=<last post id> for pagination; anonymous requests are capped at about
// ten per minute, which reddit_gate() paces for us.
fn fetch_redgifs_ids_from_rss(
    source: &RedditSource,
    limit: usize,
    auth: &RedditAuth,
) -> Result<Vec<String>, String> {
    let mut gif_ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut after: Option<String> = None;
    let mut pages = 0;

    // Always ask for a full page: a feed row is cheap, a second request costs a
    // whole rate-limit slot. limit is only used to truncate afterwards.
    let per_page = 100;

    // Cap the page count so a board whose posts never link RedGifs can't walk
    // the whole feed one request at a time. One page covers most boards.
    while gif_ids.len() < limit && pages < 3 {
        pages += 1;
        let url = match (source, &after) {
            (
                RedditSource::Subreddit {
                    name,
                    modifier,
                    time,
                },
                Some(cursor),
            ) => format!(
                "https://www.reddit.com/r/{}/{}/.rss?t={}&limit={}&after=t3_{}",
                name, modifier, time, per_page, cursor
            ),
            (
                RedditSource::Subreddit {
                    name,
                    modifier,
                    time,
                },
                None,
            ) => format!(
                "https://www.reddit.com/r/{}/{}/.rss?t={}&limit={}",
                name, modifier, time, per_page
            ),
            (
                RedditSource::User {
                    name,
                    modifier,
                    time,
                },
                Some(cursor),
            ) => format!(
                "https://www.reddit.com/user/{}/.rss?sort={}&t={}&limit={}&after=t3_{}",
                name, modifier, time, per_page, cursor
            ),
            (
                RedditSource::User {
                    name,
                    modifier,
                    time,
                },
                None,
            ) => format!(
                "https://www.reddit.com/user/{}/.rss?sort={}&t={}&limit={}",
                name, modifier, time, per_page
            ),
        };

        let body = reddit_get(&url, auth)?;
        if collect_gif_ids(&body, &mut gif_ids, &mut seen) == 0 && pages > 1 {
            break;
        }

        // Cursor for the next page is the last post id on this one
        match ENTRY_ID_REGEX.captures_iter(&body).last() {
            Some(cap) => after = Some(cap.get(1).unwrap().as_str().to_string()),
            None => break,
        }
    }

    gif_ids.truncate(limit);
    Ok(gif_ids)
}

fn listing_key(source: &RedditSource) -> String {
    match source {
        RedditSource::Subreddit {
            name,
            modifier,
            time,
        } => {
            format!("r/{}/{}/{}", name, modifier, time)
        }
        RedditSource::User {
            name,
            modifier,
            time,
        } => {
            format!("u/{}/{}/{}", name, modifier, time)
        }
    }
}

type ListingFetch = Result<(Vec<String>, Option<(String, ListingEntry)>), String>;

// Returns the slugs for a board, plus a cache entry to persist when they were
// actually scraped (None on a cache hit). A fresh listing means zero reddit
// requests, which is the only slow step in the whole pipeline.
fn fetch_redgifs_ids(
    source: &RedditSource,
    limit: usize,
    auth: &RedditAuth,
    listings: &HashMap<String, ListingEntry>,
    refresh: bool,
) -> ListingFetch {
    let key = listing_key(source);

    if !refresh {
        if let Some(entry) = listings.get(&key) {
            if now_secs().saturating_sub(entry.fetched_at) < LISTING_TTL_SECS {
                let mut ids = entry.ids.clone();
                ids.truncate(limit);
                if !ids.is_empty() {
                    return Ok((ids, None));
                }
            }
        }
    }

    let mut ids = if auth.is_oauth() {
        fetch_redgifs_ids_from_json(source, limit, auth)?
    } else {
        fetch_redgifs_ids_from_rss(source, limit, auth)?
    };

    let entry = ListingEntry {
        ids: ids.clone(),
        fetched_at: now_secs(),
    };
    ids.truncate(limit);
    Ok((ids, Some((key, entry))))
}

// Resolves numeric RedGifs IDs to canonical slugs via the single-GIF API.
// Slug IDs are returned as-is. This fixes the batch API coalescing issue
// where numeric IDs map to canonical slugs but the lookup fails.
fn resolve_gif_ids_to_canonical(gif_ids: Vec<String>, token: &str) -> Vec<String> {
    let mut resolved = Vec::with_capacity(gif_ids.len());
    for id in gif_ids {
        if id.chars().all(|c| c.is_ascii_digit()) {
            let url = format!("https://api.redgifs.com/v2/gifs/{}", id.to_lowercase());
            match ureq::get(&url)
                .set("User-Agent", "rust:faptopia:v1.0")
                .set("Authorization", &format!("Bearer {}", token))
                .call()
            {
                Ok(response) => {
                    if let Ok(gif_data) = response.into_json::<RedGifsSingleGifResponse>() {
                        if let Some(gif) = gif_data.gif {
                            if let Some(gif_id) = gif.id {
                                resolved.push(gif_id.to_lowercase());
                                continue;
                            }
                        }
                    }
                    resolved.push(id.to_lowercase());
                }
                Err(_) => resolved.push(id.to_lowercase()),
            }
        } else {
            resolved.push(id.to_lowercase());
        }
    }

    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    resolved.retain(|id| seen.insert(id.clone()));

    resolved
}

// gets the videos for a reddit source via the Atom feed + RedGifs API (with caching)
// Takes an owned snapshot of the cache map so it can be called from a thread.
// Returns the fetched items and any new cache entries (caller merges and saves).
#[allow(clippy::too_many_arguments)] // threading state, not a smell to chase
fn fetch_media_embeds_reddit(
    source: &RedditSource,
    limit: usize,
    cache_snapshot: HashMap<String, CacheEntry>,
    listing_snapshot: HashMap<String, ListingEntry>,
    token: String,
    auth: &RedditAuth,
    concurrency: usize,
    refresh: bool,
) -> Result<BoardFetch, String> {
    let started = Instant::now();
    let debug = std::env::var_os("FAPTOPIA_DEBUG").is_some();
    let label = source_label(source);
    let (gif_ids, new_listing) =
        fetch_redgifs_ids(source, limit, auth, &listing_snapshot, refresh)?;
    let ids_ms = started.elapsed().as_millis();
    let gif_ids = resolve_gif_ids_to_canonical(gif_ids, &token);
    if debug {
        eprintln!(
            "[{}] ids {}ms ({} ids, {}), resolve {}ms",
            label,
            ids_ms,
            gif_ids.len(),
            if new_listing.is_some() {
                "scraped"
            } else {
                "cached"
            },
            started.elapsed().as_millis() - ids_ms
        );
    }

    if gif_ids.is_empty() {
        return Ok(BoardFetch {
            items: vec![],
            new_urls: HashMap::new(),
            new_listings: new_listing.into_iter().collect(),
        });
    }

    // Split into cached vs uncached using the provided snapshot
    let now = now_secs();
    let mut video_urls: Vec<Option<(String, String)>> = vec![None; gif_ids.len()];
    let mut uncached: Vec<(usize, String)> = Vec::new();
    let mut cache_hits = 0;

    for (i, gif_id) in gif_ids.iter().enumerate() {
        let id_lower = gif_id.to_lowercase();
        if let Some(entry) = cache_snapshot.get(&id_lower) {
            if now - entry.fetched_at < CACHE_TTL_SECS {
                // Older cache entries have no SD URL; fall back to the HD URL
                // (no downgrade possible, but still a valid pair)
                video_urls[i] = Some((
                    entry.url.clone(),
                    entry.sd_url.clone().unwrap_or_else(|| entry.url.clone()),
                ));
                cache_hits += 1;
                continue;
            }
        }
        uncached.push((i, gif_id.clone()));
    }

    if new_listing.is_none() && debug {
        eprintln!("[{}] listing cached ({} ids)", label, gif_ids.len());
    }

    // Count every id, cached or not, so the status line tracks overall work
    progress_add_videos(gif_ids.len());
    for _ in 0..cache_hits {
        progress_video_done();
    }

    // Resolve uncached URLs across a worker pool. RedGifs has no useful batch
    // endpoint left, so this is ~N sequential round trips per board turned into
    // N/concurrency, and the progress line finally moves instead of jumping
    // straight to 100%.
    let mut new_entries: HashMap<String, CacheEntry> = HashMap::new();

    if !uncached.is_empty() {
        let total_to_fetch = uncached.len();
        let workers = concurrency.clamp(1, 32).min(total_to_fetch);
        let fetched_results: Mutex<Vec<(usize, String, String, String)>> = Mutex::new(Vec::new());
        let cursor = AtomicUsize::new(0);

        thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let i = cursor.fetch_add(1, Ordering::Relaxed);
                    if i >= uncached.len() {
                        break;
                    }
                    let (idx, gif_id) = &uncached[i];
                    if let Some((hd, sd)) = fetch_redgifs_one(gif_id, &token) {
                        fetched_results
                            .lock()
                            .unwrap()
                            .push((*idx, gif_id.clone(), hd, sd));
                    }
                    progress_video_done();
                });
            }
        });

        let fetched_results = fetched_results.into_inner().unwrap();

        if debug {
            eprintln!(
                "[{}] url fetch {}ms for {} ids",
                label,
                started.elapsed().as_millis(),
                total_to_fetch
            );
        }

        for (idx, gif_id, hd, sd) in &fetched_results {
            video_urls[*idx] = Some((hd.clone(), sd.clone()));
            new_entries.insert(
                gif_id.to_lowercase(),
                CacheEntry {
                    url: hd.clone(),
                    sd_url: Some(sd.clone()),
                    fetched_at: now,
                },
            );
        }
    }

    // Build final MediaItems preserving order
    let fetched_count = new_entries.len();
    let final_items: Vec<MediaItem> = gif_ids
        .iter()
        .enumerate()
        .filter_map(|(i, gif_id)| {
            video_urls[i].as_ref().map(|(url, sd_url)| MediaItem {
                url: url.clone(),
                // Only carry an SD URL when it actually differs (avoids useless fallbacks)
                sd_url: (sd_url != url).then(|| sd_url.clone()),
                gif_id: Some(gif_id.to_lowercase()),
            })
        })
        .collect();

    progress_log(&format!(
        "[{}/{}] {} -> {} videos ({} cached, {} fetched)",
        progress_board_position(),
        progress_boards_total(),
        label,
        final_items.len(),
        cache_hits,
        fetched_count
    ));
    progress_board_done();

    Ok(BoardFetch {
        items: final_items,
        new_urls: new_entries,
        new_listings: new_listing.into_iter().collect(),
    })
}
