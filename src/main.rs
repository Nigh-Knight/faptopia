use clap::{Args, Parser, Subcommand};
use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use ureq;

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
struct RedGifsGifsResponse {
    gifs: Vec<RedGifsGif>,
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
}

const CACHE_FILE: &str = ".faptopia_cache.json";
const CACHE_TTL_SECS: u64 = 4 * 60 * 60; // 4 hours

fn load_cache() -> UrlCache {
    fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &UrlCache) {
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = fs::write(CACHE_FILE, json);
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
static ENTRY_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<id>t3_([a-z0-9]+)</id>").unwrap());

// Anonymous reddit allows roughly one request per minute, so remember when the
// last request went out and space every subsequent one instead of eating 429s.
static LAST_REDDIT_REQUEST: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
const REDDIT_MIN_INTERVAL: Duration = Duration::from_secs(62);

fn reddit_rate_gate() {
    let mut last = LAST_REDDIT_REQUEST.lock().unwrap();
    if let Some(prev) = *last {
        let elapsed = prev.elapsed();
        if elapsed < REDDIT_MIN_INTERVAL {
            thread::sleep(REDDIT_MIN_INTERVAL - elapsed);
        }
    }
    *last = Some(Instant::now());
}

// parses Reddit URLs to extract subreddit, modifier, and time parameters
static REDDIT_URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https?://(?:www\.)?reddit\.com/r/([^/]+)/([^/?]+)(?:/?\?t=([^&]+))?").unwrap()
});

// parses Reddit user profile URLs to extract username
// sort and t params are extracted manually from the query string
static REDDIT_USER_URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https?://(?:www\.)?reddit\.com/user/([^/?]+)").unwrap()
});

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, global = true, help = "Shuffle videos AND boards/threads in a random order")]
    random: bool,
    #[arg(long, global = true, help = "Reverse video AND board/thread order (start from last)")]
    reverse: bool,
    #[arg(long, global = true, help = "Start local server for mobile access")]
    serve: bool,
    #[arg(long, global = true, default_value_t = 6969, help = "Port for local server")]
    port: u16,
    #[arg(long, global = true, help = "Deploy to Cloudflare Pages (PWA with HTTPS)")]
    deploy: bool,
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
}

#[derive(Args)]
struct ThreadId {
    thread: Option<Vec<u64>>,
}

#[derive(Args)]
struct SubReddit {
    name: Option<Vec<String>>,
    #[arg(long, default_value_t = 25, help = "Number of posts to fetch (default: 25)")]
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
                name: caps.get(1).ok_or("Missing subreddit in URL")?.as_str().to_string(),
                modifier: caps.get(2).ok_or("Missing modifier in URL")?.as_str().to_string(),
                time: caps
                    .get(3)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "all".to_string()),
            });
        }
        // Try user profile URL
        if let Some(caps) = REDDIT_USER_URL_REGEX.captures(input) {
            let username = caps.get(1).ok_or("Missing username in URL")?.as_str().to_string();
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
    match &cli.command {
        Commands::FourChan(args) => {
            if let Some(ids) = &args.thread {
                // Randomize/reverse thread order so tabs come out shuffled too
                let mut ids = ids.clone();
                if random { ids.shuffle(&mut thread_rng()); }
                if reverse { ids.reverse(); }

                // Fetch all threads in parallel
                let handles: Vec<_> = ids
                    .iter()
                    .map(|&id| thread::spawn(move || fetch_video_links_4chan(&[id]).map_err(|e| e.to_string())))
                    .collect();

                let mut thread_items = Vec::new();
                for (id, handle) in ids.iter().zip(handles) {
                    match handle.join().expect("thread panicked") {
                        Ok(threads) => {
                            for (title, links) in threads {
                                let mut items: Vec<MediaItem> =
                                    links.into_iter().map(|url| MediaItem { url, sd_url: None, gif_id: None }).collect();
                                if random { items.shuffle(&mut thread_rng()); }
                                if reverse { items.reverse(); }
                                thread_items.push((title, items));
                            }
                        }
                        Err(e) => eprintln!("Error fetching thread {}: {}", id, e),
                    }
                }
                save_gallery(thread_items, "faptopia_4chan.html")?;
            } else {
                println!("INPUT A THREAD ID");
            }
        }
        Commands::Reddit(args) => {
            if let Some(names) = &args.name {
                // Parse all inputs into RedditSource (subreddit or user)
                let mut parsed: Vec<RedditSource> = names
                    .iter()
                    .filter_map(|x| match parse_reddit_input(x) {
                        Ok(source) => Some(source),
                        Err(e) => { eprintln!("Invalid input '{}': {}", x, e); None }
                    })
                    .collect();

                // Randomize/reverse board order so tabs come out shuffled too
                if random { parsed.shuffle(&mut thread_rng()); }
                if reverse { parsed.reverse(); }

                // Load cache and fetch token once; each thread gets clones
                let cache = load_cache();
                let limit = args.limit;
                let token = match fetch_redgifs_token() {
                    Ok(t) => t,
                    Err(e) => { eprintln!("Failed to fetch RedGifs token: {}", e); return Ok(()); }
                };

                // Fetch boards one at a time: anonymous reddit only allows
                // ~1 request/minute, so parallelism just trades 429s.
                // Tab order is preserved.
                let mut subreddit_items = Vec::new();
                let mut all_new_entries: HashMap<String, CacheEntry> = HashMap::new();
                let concurrency = 1;

                if parsed.len() > 1 {
                    println!(
                        "Fetching {} boards - reddit allows ~1 request/minute, so this takes ~{} minutes",
                        parsed.len(),
                        parsed.len()
                    );
                }

                for chunk in parsed.chunks(concurrency) {
                    let handles: Vec<_> = chunk
                        .iter()
                        .map(|source| {
                            let snapshot = cache.redgifs.clone();
                            let tok = token.clone();
                            let source_clone = source.clone();
                            thread::spawn(move || {
                                let label = source_label(&source_clone);
                                fetch_media_embeds_reddit(&source_clone, limit, snapshot, tok)
                                    .map(|(items, new_entries)| (label, items, new_entries))
                            })
                        })
                        .collect();

                    for (_source, handle) in chunk.iter().zip(handles) {
                        match handle.join().expect("thread panicked") {
                            Ok((name, mut items, new_entries)) => {
                                if random { items.shuffle(&mut thread_rng()); }
                                if reverse { items.reverse(); }
                                subreddit_items.push((name, items));
                                all_new_entries.extend(new_entries);
                            }
                            Err(e) => eprintln!("Error fetching {}: {}", source_label(_source), e),
                        }
                    }

                    // Small gap between batches to keep the request rate low
                    if chunk.len() == concurrency {
                        thread::sleep(Duration::from_millis(250));
                    }
                }

                // Merge new entries back into cache and save once
                let mut final_cache = UrlCache { redgifs: cache.redgifs };
                final_cache.redgifs.extend(all_new_entries);
                save_cache(&final_cache);

                save_gallery(subreddit_items, "faptopia_reddit.html")?;
            } else {
                println!("INPUT A SUBREDDIT PAGE");
            }
        }
        Commands::Deploy => {
            deploy_to_cloudflare()?;
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
        format!(r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={}"></head></html>"#, index_target),
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
        .args(["pages", "deploy", deploy_dir, "--project-name", "faptopia", "--branch", "main", "--commit-dirty=true"])
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
        format!(r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={}"></head></html>"#, index_target),
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
const EMBLA_TEMPLATE: &str = include_str!("../templates/embla.js");
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
    let redgifs_re =
        Regex::new(r#"(?s)data-gif-id="([^"]+)"(.*?)<source src="[^"]*""#).unwrap();
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
    let platform = if filename.contains("reddit") { "reddit" } else { "4chan" };
    let script = format!(
        "{}\n{}",
        EMBLA_TEMPLATE,
        JS_TEMPLATE
            .replace("{{TOTAL_SECTIONS}}", &items.len().to_string())
            .replace("{{PLATFORM}}", platform)
    );

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
pub fn fetch_video_links_4chan(thread_ids: &[u64]) -> Result<Vec<(String, Vec<String>)>, ureq::Error> {
    let mut results = Vec::new();
    for id in thread_ids {
        let url = format!("https://a.4cdn.org/gif/thread/{}.json", id);
        let response = ureq::get(&url).call()?;
        let thread: Thread = response.into_json()?;

        let title = thread.posts.first()
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
fn fetch_redgifs_batch(gif_ids: &[String], token: &str) -> Result<HashMap<String, (String, String)>, ureq::Error> {
    let mut result = HashMap::new();
    for gif_id in gif_ids {
        let id_lower = gif_id.to_lowercase();
        let url = format!("https://api.redgifs.com/v2/gifs/{}", id_lower);
        if let Ok(response) = ureq::get(&url)
            .set("User-Agent", "rust:faptopia:v1.0")
            .set("Authorization", &format!("Bearer {}", token))
            .call()
        {
            if let Ok(gif_data) = response.into_json::<RedGifsSingleGifResponse>() {
                if let Some(gif) = gif_data.gif {
                    let canonical_id = gif.id.unwrap_or_else(|| id_lower.clone()).to_lowercase();
                    let RedGifsGifUrls { hd, sd } = gif.urls;
                    let hd = hd.unwrap_or_else(|| sd.clone());
                    result.insert(canonical_id, (hd, sd));
                }
            }
        }
    }
    Ok(result)
}

// Fetches RedGifs IDs from a subreddit/user Atom feed.
//
// Reddit has walled off its anonymous JSON API (403 "blocked by network
// security") and old.reddit.com now 302s logged-out clients to a login page,
// but the public /r/<sub>/<sort>/.rss feed still works and carries the redgifs
// link inside each entry's HTML body. The feed honours limit=100 and
// ?after=<last post id> for pagination; anonymous requests are capped at about
// one per minute, which reddit_rate_gate() enforces for us.
fn fetch_redgifs_ids_from_rss(
    source: &RedditSource,
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut gif_ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut after: Option<String> = None;
    let mut pages = 0;

    let ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";
    let per_page = limit.clamp(25, 100);

    // Cap the page count so a board whose posts never link RedGifs can't walk
    // the whole feed one minute at a time.
    while gif_ids.len() < limit && pages < 6 {
        pages += 1;
        let url = match (source, &after) {
            (RedditSource::Subreddit { name, modifier, time }, Some(cursor)) => format!(
                "https://www.reddit.com/r/{}/{}/.rss?t={}&limit={}&after=t3_{}",
                name, modifier, time, per_page, cursor
            ),
            (RedditSource::Subreddit { name, modifier, time }, None) => format!(
                "https://www.reddit.com/r/{}/{}/.rss?t={}&limit={}",
                name, modifier, time, per_page
            ),
            (RedditSource::User { name, modifier, time }, Some(cursor)) => format!(
                "https://www.reddit.com/user/{}/.rss?sort={}&t={}&limit={}&after=t3_{}",
                name, modifier, time, per_page, cursor
            ),
            (RedditSource::User { name, modifier, time }, None) => format!(
                "https://www.reddit.com/user/{}/.rss?sort={}&t={}&limit={}",
                name, modifier, time, per_page
            ),
        };

        // Retry rate limits / transient blocks with the server's own backoff hint
        let mut attempt: u32 = 0;
        let body = loop {
            attempt += 1;
            reddit_rate_gate();
            match ureq::get(&url)
                .set("User-Agent", ua)
                .set("Cookie", "over18=1")
                .set("Accept", "application/atom+xml,application/xml;q=0.9,*/*;q=0.8")
                .set("Accept-Language", "en-US,en;q=0.9")
                .call()
            {
                Ok(resp) => break resp.into_string().map_err(|e| e.to_string())?,
                Err(ureq::Error::Status(code, resp))
                    if (code == 429 || code == 403 || code == 503) && attempt < 5 =>
                {
                    let wait = resp
                        .header("x-ratelimit-reset")
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(60);
                    eprintln!(
                        "Rate-limited by reddit ({}) - retrying in {}s (attempt {}/5)",
                        code,
                        wait + 1,
                        attempt
                    );
                    thread::sleep(Duration::from_secs(wait + 1 + rand::random::<u64>() % 3));
                }
                Err(ureq::Error::Status(code, _)) => {
                    return Err(format!("reddit feed returned status {}", code))
                }
                Err(e) => return Err(e.to_string()),
            }
        };

        for cap in REDGIFS_LINK_REGEX.captures_iter(&body) {
            if let Some(id) = cap.get(1) {
                let id = id.as_str().to_lowercase();
                if seen.insert(id.clone()) {
                    gif_ids.push(id);
                }
            }
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

// Resolves numeric RedGifs IDs to canonical slugs via the single-GIF API.
// Slug IDs are returned as-is. This fixes the batch API coalescing issue
// where numeric IDs map to canonical slugs but the lookup fails.
fn resolve_gif_ids_to_canonical(
    gif_ids: Vec<String>,
    token: &str,
) -> Vec<String> {
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
fn fetch_media_embeds_reddit(
    source: &RedditSource,
    limit: usize,
    cache_snapshot: HashMap<String, CacheEntry>,
    token: String,
) -> Result<(Vec<MediaItem>, HashMap<String, CacheEntry>), String> {
    let label = source_label(source);
    let gif_ids = fetch_redgifs_ids_from_rss(source, limit)?;
    let gif_ids = resolve_gif_ids_to_canonical(gif_ids, &token);

    if gif_ids.is_empty() {
        return Ok((vec![], HashMap::new()));
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

    if cache_hits > 0 {
        println!("[{}] {} cached", label, cache_hits);
    }

    // Fetch uncached URLs via batch API (up to 100 IDs per request)
    let mut new_entries: HashMap<String, CacheEntry> = HashMap::new();

    if !uncached.is_empty() {
        let total_to_fetch = uncached.len();
        let mut fetched_results: Vec<(usize, String, String, String)> = Vec::new();
        let mut done = 0;

        for chunk in uncached.chunks(100) {
            let ids: Vec<String> = chunk.iter().map(|(_, id)| id.clone()).collect();
            match fetch_redgifs_batch(&ids, &token) {
                Ok(url_map) => {
                    done += ids.len();
                    let pct = (done as f32 / total_to_fetch as f32 * 100.0) as u8;
                    println!("[{}] fetching {}% ({}/{})", label, pct, done, total_to_fetch);
                    for (idx, gif_id) in chunk {
                        if let Some((hd, sd)) = url_map.get(&gif_id.to_lowercase()) {
                            fetched_results.push((*idx, gif_id.clone(), hd.clone(), sd.clone()));
                        }
                    }
                }
                Err(e) => eprintln!("[{}] batch fetch failed: {}", label, e),
            }
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

    println!(
        "[{}] done: {} videos ({} cached, {} fetched)",
        label,
        final_items.len(),
        cache_hits,
        fetched_count
    );

    Ok((final_items, new_entries))
}
