use clap::{Args, Parser, Subcommand};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use ureq;

struct MediaItem {
    url: String,
    media_type: MediaType,
}

enum MediaType {
    Iframe,
    Video,
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

// gets the redgif link
static REDGIFS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https://www\.redgifs\.com/ifr/[a-zA-Z0-9_-]+").unwrap());

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
        Commands::FourChan(id) => {
            if let Some(ids) = &id.thread {
                let mut thread_items = Vec::new();
                for id in ids {
                    match fetch_video_links_4chan(&[*id]) {
                        Ok(links) => {
                            let items = links
                                .into_iter()
                                .map(|url| MediaItem {
                                    url,
                                    media_type: MediaType::Video,
                                })
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
        Commands::Reddit(sub_reddit) => {
            if let Some(names) = &sub_reddit.name {
                let mut subreddit_items = Vec::new();
                for x in names {
                    let parts: Vec<&str> = x.split(':').collect();
                    if parts.len() == 3 {
                        // separates something like hotwife:top:month into 3 parts, subreddit, modifier and time
                        let (subreddit, modifier, time) = (parts[0], parts[1], parts[2]);
                        match fetch_media_embeds_reddit(subreddit, modifier, time) {
                            Ok(contents) => {
                                let items = contents
                                    .into_iter()
                                    .flatten()
                                    .map(|url| MediaItem {
                                        url,
                                        media_type: MediaType::Iframe,
                                    })
                                    .collect();
                                subreddit_items.push((format!("r/{}", subreddit), items));
                            }
                            Err(e) => eprintln!("Error: {}", e),
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
    
    let html = generate_gallery(items);
    fs::write(filename, html)?;
    println!("Gallery saved to {}", filename);
    Ok(())
}

// Template files embedded at compile time
const HTML_TEMPLATE: &str = include_str!("../templates/gallery.html");
const CSS_TEMPLATE: &str = include_str!("../templates/styles.css");
const JS_TEMPLATE: &str = include_str!("../templates/script.js");

// create a horizontal scrollable gallery 
fn generate_gallery(items: Vec<(String, Vec<MediaItem>)>) -> String {
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
                    match item.media_type {
                        MediaType::Iframe => format!(
                            r#"<div class="gallery-item" data-index="{global_index}">
                                <iframe 
                                    src="{}" 
                                    frameborder="0" 
                                    allowfullscreen 
                                    sandbox="allow-same-origin allow-scripts"
                                    loading="lazy"
                                ></iframe>
                            </div>"#,
                            item.url
                        ),
                        MediaType::Video => format!(
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
                        ),
                    }
                })
                .collect();

            let tab = format!(
                r#"<button class="tab-button{}" onclick="showSection({})">{}</button>"#,
                if section_index == 0 { " active" } else { "" },
                section_index,
                source_name
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
    
    HTML_TEMPLATE
        .replace("{{STYLES}}", CSS_TEMPLATE)
        .replace("{{TABS}}", &tabs.join("\n"))
        .replace("{{SECTIONS}}", &sections.join("\n"))
        .replace("{{SCRIPT}}", &script)
}

// gets the 4chan video links from the gif board
fn fetch_video_links_4chan(thread_ids: &[u64]) -> Result<Vec<String>, ureq::Error> {
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


// gets the videos from reddit 
fn fetch_media_embeds_reddit(
    subreddit: &str,
    modifier: &str,
    time: &str,
) -> Result<Vec<Option<String>>, ureq::Error> {
    let url = format!(
        "https://www.reddit.com/r/{}/{}/.json?t={}",
        subreddit, modifier, time
    );
    let response = ureq::get(&url)
        .set("User-Agent", "rust:reddit_scraper:v1.0")
        .call()?;

    let reddit_response: RedditResponse = response.into_json()?;

    let contents: Vec<Option<String>> = reddit_response
        .data
        .children
        .into_iter()
        .map(|child| {
            child
                .data
                .media_embed
                .and_then(|embed| embed.content)
                .and_then(|html| {
                    REDGIFS_REGEX
                        .find(&html)
                        .map(|mat| mat.as_str().to_string())
                })
        })
        .collect();

    Ok(contents)
}
