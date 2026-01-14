use clap::{Args, Parser, Subcommand};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use ureq;

struct MediaItem {
    url: String,
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

// extracts the redgif ID from iframe URLs
static REDGIFS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https://www\.redgifs\.com/ifr/([a-zA-Z0-9_-]+)").unwrap());

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
}

fn main() -> io::Result<()> {
    let cli: Cli = Cli::parse();

    // functionality for 4chan | allows you to do this: faptopia 4chan [thread id]
    match &cli.command {
        Commands::FourChan(args) => {
            if let Some(ids) = &args.thread {
                let mut thread_items = Vec::new();
                for id in ids {
                    match fetch_video_links_4chan(&[*id]) {
                        Ok(links) => {
                            let items = links
                                .into_iter()
                                .map(|url| MediaItem { url })
                                .collect();
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
                    let parts: Vec<&str> = x.split(':').collect();
                    if parts.len() == 3 {
                        // separates something like hotwife:top:month into 3 parts, subreddit, modifier and time
                        let (subreddit, modifier, time) = (parts[0], parts[1], parts[2]);
                        match fetch_media_embeds_reddit(subreddit, modifier, time) {
                            Ok(urls) => {
                                let items = urls
                                    .into_iter()
                                    .map(|url| MediaItem { url })
                                    .collect();
                                subreddit_items.push((format!("r/{}", subreddit), items));
                            }
                            Err(e) => eprintln!("Error fetching r/{}: {}", subreddit, e),
                        }
                    } else {
                        println!("Invalid format. Use format: subreddit:modifier:time");
                    }
                }
                save_gallery(subreddit_items, "faptopia_reddit.html")?;
            } else {
                println!("INPUT A SUBREDDIT PAGE");
            }
        }
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

// Template files embedded at compile time
const HTML_TEMPLATE: &str = include_str!("../templates/gallery.html");
const CSS_TEMPLATE: &str = include_str!("../templates/styles.css");
const JS_TEMPLATE: &str = include_str!("../templates/script.js");

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
                    format!(
                        r#"<div class="gallery-item" data-index="{global_index}">
                            <video
                                controls
                                {autoplay}
                                {muted}
                                playsinline
                                data-index="{global_index}"
                                preload="{preload}"
                            >
                                <source src="{}" type="video/mp4">
                            </video>
                        </div>"#,
                        item.url,
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
                if section_index == 0 { " active" } else { " hidden" },
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

// gets the videos from reddit via RedGifs API
fn fetch_media_embeds_reddit(
    subreddit: &str,
    modifier: &str,
    time: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://www.reddit.com/r/{}/{}/.json?t={}",
        subreddit, modifier, time
    );
    let response = ureq::get(&url)
        .set("User-Agent", "rust:faptopia:v1.0")
        .call()?;

    let reddit_response: RedditResponse = response.into_json()?;

    // Extract gif IDs from embed HTML using capture group
    let gif_ids: Vec<String> = reddit_response
        .data
        .children
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

    if gif_ids.is_empty() {
        return Ok(vec![]);
    }

    // Get auth token once for all requests
    let token = fetch_redgifs_token()?;

    // Resolve each gif ID to a video URL, skipping failures
    let video_urls: Vec<String> = gif_ids
        .into_iter()
        .filter_map(|gif_id| {
            match fetch_redgifs_video_url(&gif_id, &token) {
                Ok(url) => Some(url),
                Err(e) => {
                    eprintln!("Warning: Failed to fetch video for {}: {}", gif_id, e);
                    None
                }
            }
        })
        .collect();

    Ok(video_urls)
}
