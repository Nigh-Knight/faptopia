use clap::{Args, Parser, Subcommand};
use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use ureq;

struct MediaItem {
    url: String,
    gif_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaEmbed {
    content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostData {
    media_embed: Option<MediaEmbed>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Child {
    data: PostData,
}

#[derive(Debug, Serialize, Deserialize)]
struct Listing {
    children: Vec<Child>,
    after: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RedditResponse {
    data: Listing,
}

#[derive(Serialize, Deserialize)]
struct Post {
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
    urls: RedGifsGifUrls,
}

#[derive(Deserialize)]
struct RedGifsGifResponse {
    gif: RedGifsGif,
}

// Cache structures for storing resolved RedGifs URLs
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    url: String,
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

// extracts the redgif ID from iframe URLs
static REDGIFS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https://www\.redgifs\.com/ifr/([a-zA-Z0-9_-]+)").unwrap());

// parses Reddit URLs to extract subreddit, modifier, and time parameters
static REDDIT_URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https?://(?:www\.)?reddit\.com/r/([^/]+)/([^/?]+)(?:/?\?t=([^&]+))?").unwrap()
});

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, global = true, help = "Shuffle videos in a random order")]
    random: bool,
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

enum RedditInput {
    FormatString {
        subreddit: String,
        modifier: String,
        time: String,
    },
    Url {
        subreddit: String,
        modifier: String,
        time: String,
    },
}

fn parse_reddit_input(input: &str) -> Result<RedditInput, String> {
    if input.starts_with("http://") || input.starts_with("https://") {
        if let Some(caps) = REDDIT_URL_REGEX.captures(input) {
            let subreddit = caps
                .get(1)
                .ok_or("Missing subreddit in URL")?
                .as_str()
                .to_string();
            let modifier = caps
                .get(2)
                .ok_or("Missing modifier in URL")?
                .as_str()
                .to_string();
            let time = caps
                .get(3)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "all".to_string());

            return Ok(RedditInput::Url {
                subreddit,
                modifier,
                time,
            });
        } else {
            return Err(
                "Invalid Reddit URL format. Expected: https://reddit.com/r/SUBREDDIT/MODIFIER/?t=TIME"
                    .to_string(),
            );
        }
    }

    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() == 3 {
        Ok(RedditInput::FormatString {
            subreddit: parts[0].to_string(),
            modifier: parts[1].to_string(),
            time: parts[2].to_string(),
        })
    } else {
        Err("Invalid format. Use 'subreddit:modifier:time' or a full Reddit URL".to_string())
    }
}

fn main() -> io::Result<()> {
    let cli: Cli = Cli::parse();

    // functionality for 4chan | allows you to do this: faptopia 4chan [thread id]
    let random = cli.random;
    match &cli.command {
        Commands::FourChan(args) => {
            if let Some(ids) = &args.thread {
                let mut thread_items = Vec::new();
                for id in ids {
                    match fetch_video_links_4chan(&[*id]) {
                        Ok(links) => {
                            let mut items: Vec<MediaItem> =
                                links.into_iter().map(|url| MediaItem { url, gif_id: None }).collect();
                            if random {
                                items.shuffle(&mut thread_rng());
                            }
                            thread_items.push((format!("Thread {}", id), items));
                        }
                        Err(e) => eprintln!("Error fetching thread {}: {}", id, e),
                    }
                }
                save_gallery(thread_items, "faptopia_4chan.html")?;
            } else {
                println!("INPUT A THREAD ID");
            }
        }
        // functionality for reddit | allows you to do this: faptopia reddit [subreddit:modifier:time]
        Commands::Reddit(args) => {
            if let Some(names) = &args.name {
                let mut subreddit_items = Vec::new();
                for x in names {
                    match parse_reddit_input(x) {
                        Ok(input) => {
                            let (subreddit, modifier, time) = match input {
                                RedditInput::FormatString {
                                    subreddit,
                                    modifier,
                                    time,
                                }
                                | RedditInput::Url {
                                    subreddit,
                                    modifier,
                                    time,
                                } => (subreddit, modifier, time),
                            };

                            match fetch_media_embeds_reddit(&subreddit, &modifier, &time, args.limit) {
                                Ok(mut items) => {
                                    if random {
                                        items.shuffle(&mut thread_rng());
                                    }
                                    subreddit_items.push((format!("r/{}", subreddit), items));
                                }
                                Err(e) => eprintln!("Error fetching r/{}: {}", subreddit, e),
                            }
                        }
                        Err(e) => eprintln!("Invalid input '{}': {}", x, e),
                    }
                }
                save_gallery(subreddit_items, "faptopia_reddit.html")?;
            } else {
                println!("INPUT A SUBREDDIT PAGE");
            }
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

    println!("Deploying to Cloudflare Pages...");

    let status = Command::new("wrangler")
        .args(["pages", "deploy", deploy_dir, "--project-name", "faptopia", "--branch", "main", "--commit-dirty=true"])
        .status()?;

    let _ = fs::remove_dir_all(deploy_dir);

    if status.success() {
        println!("Deployed! Visit https://faptopia.pages.dev");
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

    // Write the proxy server script
    let server_script = format!("{}/server.py", serve_dir);
    fs::write(&server_script, include_str!("../templates/server.py"))?;

    let ip = get_local_ip();
    println!("Serving gallery at:");
    println!("  Local:   http://localhost:{}", port);
    println!("  Mobile:  http://{}:{}", ip, port);
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
    let (toggle_text, toggle_file, toggle_color) = if filename.contains("reddit") {
        ("4chan", "faptopia_4chan.html", "#0f9d58") // Green for 4chan
    } else {
        ("reddit", "faptopia_reddit.html", "#ff4500") // Reddit orange/red
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
                    let mime_type = if item.url.ends_with(".webm") {
                        "video/webm"
                    } else {
                        "video/mp4"
                    };
                    format!(
                        r#"<div class="gallery-item" data-index="{global_index}">
                            <video
                                controls
                                {autoplay}
                                {muted}
                                playsinline
                                data-index="{global_index}"{gif_id_attr}
                                preload="{preload}"
                            >
                                <source src="{url}" type="{mime_type}">
                            </video>
                        </div>"#,
                        url = item.url,
                        autoplay = if index == 0 { "autoplay" } else { "" },
                        muted = if index == 0 { "muted" } else { "" },
                        preload = if index == 0 { "auto" } else { "none" }
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
                    <div class="gallery-container" id="gallery-{}">
                        {}
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
    let script = JS_TEMPLATE.replace("{{TOTAL_SECTIONS}}", &items.len().to_string());

    // Add toggle button to switch between reddit/4chan galleries
    let toggle_button = format!(
        r#"<a href="{}" class="gallery-toggle" style="background: {}; margin-left: auto;">{}</a>"#,
        toggle_file, toggle_color, toggle_text
    );
    let tabs_with_toggle = format!("{}\n{}", tabs.join("\n"), toggle_button);

    HTML_TEMPLATE
        .replace("{{STYLES}}", CSS_TEMPLATE)
        .replace("{{TABS}}", &tabs_with_toggle)
        .replace("{{SECTIONS}}", &sections.join("\n"))
        .replace("{{SCRIPT}}", &script)
}

// gets the 4chan video links from the gif board
pub fn fetch_video_links_4chan(thread_ids: &[u64]) -> Result<Vec<String>, ureq::Error> {
    let mut all_links = Vec::new();
    for id in thread_ids {
        let url = format!("https://a.4cdn.org/gif/thread/{}.json", id);
        let response = ureq::get(&url).call()?;
        let thread: Thread = response.into_json()?;

        let thread_links: Vec<String> = thread
            .posts
            .into_iter()
            .filter_map(|post| match (post.ext.as_ref(), post.tim) {
                (Some(ext), Some(tim)) if ext == ".mp4" || ext == ".webm" => {
                    Some(format!("https://i.4cdn.org/gif/{}{}", tim, ext))
                }
                _ => None,
            })
            .collect();

        all_links.extend(thread_links);
    }
    Ok(all_links)
}

// fetches a temporary auth token from RedGifs API
fn fetch_redgifs_token() -> Result<String, ureq::Error> {
    let response = ureq::get("https://api.redgifs.com/v2/auth/temporary")
        .set("User-Agent", "rust:faptopia:v1.0")
        .call()?;
    let auth: RedGifsAuthResponse = response.into_json()?;
    Ok(auth.token)
}

// fetches the direct video URL for a RedGifs gif ID
fn fetch_redgifs_video_url(gif_id: &str, token: &str) -> Result<String, ureq::Error> {
    let url = format!("https://api.redgifs.com/v2/gifs/{}", gif_id.to_lowercase());
    let response = ureq::get(&url)
        .set("User-Agent", "rust:faptopia:v1.0")
        .set("Authorization", &format!("Bearer {}", token))
        .call()?;
    let gif_response: RedGifsGifResponse = response.into_json()?;
    Ok(gif_response.gif.urls.hd.unwrap_or(gif_response.gif.urls.sd))
}

// gets the videos from reddit via RedGifs API (with caching and parallel fetching)
fn fetch_media_embeds_reddit(
    subreddit: &str,
    modifier: &str,
    time: &str,
    limit: usize,
) -> Result<Vec<MediaItem>, Box<dyn std::error::Error>> {
    let mut gif_ids: Vec<String> = Vec::new();
    let mut after: Option<String> = None;

    // Paginate until we have enough gif IDs or run out of posts.
    // Always request 100 per page (Reddit's max) since many posts won't be RedGifs embeds.
    while gif_ids.len() < limit {
        let per_page = 100;
        let url = match &after {
            Some(cursor) => format!(
                "https://www.reddit.com/r/{}/{}/.json?t={}&limit={}&after={}",
                subreddit, modifier, time, per_page, cursor
            ),
            None => format!(
                "https://www.reddit.com/r/{}/{}/.json?t={}&limit={}",
                subreddit, modifier, time, per_page
            ),
        };

        let response = ureq::get(&url)
            .set("User-Agent", "rust:faptopia:v1.0")
            .call()?;
        let reddit_response: RedditResponse = response.into_json()?;

        let next_cursor = reddit_response.data.after.clone();
        let children = reddit_response.data.children;
        let total_posts = children.len();

        let page_ids: Vec<String> = children
            .into_iter()
            .filter_map(|child| {
                child
                    .data
                    .media_embed
                    .and_then(|embed| embed.content)
                    .and_then(|html| {
                        REDGIFS_REGEX
                            .captures(&html)
                            .and_then(|caps| caps.get(1))
                            .map(|m| m.as_str().to_string())
                    })
            })
            .collect();

        gif_ids.extend(page_ids);

        // Stop if Reddit has no more pages or returned no posts at all
        match next_cursor {
            Some(cursor) if total_posts > 0 => after = Some(cursor),
            _ => break,
        }
    }

    gif_ids.truncate(limit);

    // Deduplicate gif IDs while preserving order
    let mut seen = HashSet::new();
    gif_ids.retain(|id| seen.insert(id.to_lowercase()));

    if gif_ids.is_empty() {
        return Ok(vec![]);
    }

    // Load cache and split into cached vs uncached
    let mut cache = load_cache();
    let now = now_secs();
    let mut video_urls: Vec<Option<String>> = vec![None; gif_ids.len()];
    let mut uncached: Vec<(usize, String)> = Vec::new();
    let mut cache_hits = 0;

    for (i, gif_id) in gif_ids.iter().enumerate() {
        let id_lower = gif_id.to_lowercase();
        if let Some(entry) = cache.redgifs.get(&id_lower) {
            if now - entry.fetched_at < CACHE_TTL_SECS {
                video_urls[i] = Some(entry.url.clone());
                cache_hits += 1;
                continue;
            }
        }
        uncached.push((i, gif_id.clone()));
    }

    if cache_hits > 0 {
        println!("{} videos loaded from cache", cache_hits);
    }

    // Fetch uncached URLs in parallel (8 concurrent requests)
    if !uncached.is_empty() {
        let token = fetch_redgifs_token()?;
        let total_to_fetch = uncached.len();
        let progress = Arc::new(AtomicUsize::new(0));

        let batch_size = 8;
        let mut fetched_results: Vec<(usize, String, String)> = Vec::new();

        for chunk in uncached.chunks(batch_size) {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(idx, gif_id)| {
                    let idx = *idx;
                    let gif_id = gif_id.clone();
                    let token = token.clone();
                    let progress = Arc::clone(&progress);
                    let total = total_to_fetch;
                    let sub = subreddit.to_string();

                    thread::spawn(move || {
                        let result = fetch_redgifs_video_url(&gif_id, &token).ok();
                        let done = progress.fetch_add(1, Ordering::Relaxed) + 1;
                        let pct = (done as f32 / total as f32 * 100.0) as u8;
                        print!(
                            "\rFetching videos: {}% ({}/{}) from r/{}",
                            pct, done, total, sub
                        );
                        io::stdout().flush().ok();
                        result.map(|url| (idx, gif_id, url))
                    })
                })
                .collect();

            for h in handles {
                if let Ok(Some((idx, gif_id, url))) = h.join() {
                    fetched_results.push((idx, gif_id, url));
                }
            }
        }

        if total_to_fetch > 0 {
            println!();
        }

        // Update results and cache with fetched URLs
        for (idx, gif_id, url) in &fetched_results {
            video_urls[*idx] = Some(url.clone());
            cache.redgifs.insert(
                gif_id.to_lowercase(),
                CacheEntry {
                    url: url.clone(),
                    fetched_at: now,
                },
            );
        }

        save_cache(&cache);
    }

    // Build final MediaItems preserving order
    let fetched_count = video_urls.iter().filter(|u| u.is_some()).count() - cache_hits;
    let final_items: Vec<MediaItem> = gif_ids
        .iter()
        .enumerate()
        .filter_map(|(i, gif_id)| {
            video_urls[i].as_ref().map(|url| MediaItem {
                url: url.clone(),
                gif_id: Some(gif_id.to_lowercase()),
            })
        })
        .collect();

    println!(
        "Ready: {} videos ({} cached, {} fetched)",
        final_items.len(),
        cache_hits,
        fetched_count
    );

    Ok(final_items)
}
