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
                match fetch_video_links_4chan(ids) {
                    Ok(links) => {
                        let items: Vec<MediaItem> = links
                            .into_iter()
                            .map(|url| MediaItem {
                                url,
                                media_type: MediaType::Video,
                            })
                            .collect();

                        // where to create the html
                        save_gallery(items, "faptopia_4chan.html")?;
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
            } else {
                println!("INPUT A THREAD ID");
            }
        }
        // functionality for reddit | allows you to do this: faptopia reddit [subreddit:modifier:time]
        Commands::Reddit(sub_reddit) => {
            if let Some(names) = &sub_reddit.name {
                for x in names {
                    let parts: Vec<&str> = x.split(':').collect();
                    if parts.len() == 3 {
                        // separates something like hotwife:top:month into 3 parts, subreddit, modifier and time
                        let (subreddit, modifier, time) = (parts[0], parts[1], parts[2]);
                        match fetch_media_embeds_reddit(subreddit, modifier, time) {
                            Ok(contents) => {
                                let items: Vec<MediaItem> = contents
                                    .into_iter()
                                    .flatten()
                                    .map(|url| MediaItem {
                                        url,
                                        media_type: MediaType::Iframe,
                                    })
                                    .collect();
                              
                                // saves the collection of videos to this html file
                                save_gallery(items, "faptopia_reddit.html")?;
                            }
                            Err(e) => eprintln!("Error: {}", e),
                        }
                    } else {
                        println!("Invalid format. Use format: subreddit:modifier:time");
                    }
                }
            } else {
                println!("INPUT A SUBREDDIT PAGE");
            }
        }
    }

    Ok(())
}

// with an input of links to the videos(items) and html filename to write to, creates a scrollable gallery 
fn save_gallery(items: Vec<MediaItem>, filename: &str) -> io::Result<()> {
    if items.is_empty() {
       
        println!("No media items found");
       
        return Ok(());
    }
    
    // this is where the scrollable html gallery is created
    let html = generate_gallery(items);

    fs::write(filename, html)?;

    println!("Gallery saved to {}", filename);

    Ok(())
}

// create a horizontal scrollable gallery 
fn generate_gallery(items: Vec<MediaItem>) -> String {
    let gallery_items: String = items
        .iter()
        .enumerate()
        .map(|(index, item)| match item.media_type {
            MediaType::Iframe => {
                format!(
                    r#"<div class="gallery-item" data-index="{index}">
                            <iframe 
                                src="{}" 
                                frameborder="0" 
                                allowfullscreen 
                                sandbox="allow-same-origin allow-scripts"
                                loading="lazy"
                            ></iframe>
                        </div>"#,
                    item.url
                )
            }
            MediaType::Video => {
                format!(
                    r#"<div class="gallery-item" data-index="{index}">
                            <video 
                                controls 
                                {autoplay}
                                {muted}
                                playsinline
                                data-index="{index}"
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
            }
        })
        .collect();

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <title>Media Gallery</title>
  <style>
    body {{
      margin: 0;
      overflow: hidden;
      background-color: #121212;
    }}
    .gallery-container {{
      display: flex;
      overflow-x: auto;
      scroll-snap-type: x mandatory;
      height: 100vh;
      scroll-behavior: smooth;
    }}
    .gallery-item {{
      scroll-snap-align: start;
      flex: 0 0 100vw;
      height: 100vh;
    }}
    iframe, video {{
      width: 100%;
      height: 100%;
      object-fit: contain;
      background-color: black;
    }}
    .nav-hint {{
      position: fixed;
      bottom: 20px;
      left: 50%;
      transform: translateX(-50%);
      color: white;
      background: rgba(0,0,0,0.7);
      padding: 10px 20px;
      border-radius: 20px;
      font-family: sans-serif;
      font-size: 1.2rem;
      z-index: 100;
    }}
  </style>
</head>
<body tabindex="0">
  <div class="gallery-container" id="gallery">
    {gallery_items}
  </div>
  <div class="nav-hint">Press Z ← → X to navigate</div>
  <script>
    let currentIndex = 0;
    const gallery = document.getElementById('gallery');
    const galleryItems = document.querySelectorAll('.gallery-item');
    const totalItems = galleryItems.length;
    const videos = document.querySelectorAll('video');

    // Focus management for videos
    function updateVideoFocus(index) {{
      videos.forEach((video, i) => {{
        if (i === index) {{
          video.muted = false;
          video.play().catch(e => console.log('Autoplay prevented:', e));
        }} else if (video.played.length > 0) {{
          video.muted = true;
          video.pause();
        }}
      }});
    }}

    // Scroll to item with index
    function scrollToIndex(index) {{
      gallery.scrollTo({{
        left: index * window.innerWidth,
        behavior: 'smooth'
      }});
      updateVideoFocus(index);
    }}

    // Keyboard navigation
    function handleNavigation(direction) {{
      const newIndex = direction === 'forward' 
        ? (currentIndex + 1) % totalItems 
        : (currentIndex - 1 + totalItems) % totalItems;
      
      scrollToIndex(newIndex);
      currentIndex = newIndex;
    }}

    // Intersection Observer for lazy loading and focus
    const observer = new IntersectionObserver((entries) => {{
      entries.forEach(entry => {{
        if (entry.isIntersecting) {{
          const index = parseInt(entry.target.dataset.index);
          if (index !== currentIndex) {{
            currentIndex = index;
            updateVideoFocus(index);
          }}
        }}
      }});
    }}, {{ threshold: 0.5 }});

    // Observe all gallery items
    galleryItems.forEach(item => observer.observe(item));

    // Event listeners
    document.addEventListener('keydown', (e) => {{
      if (e.key === 'x' || e.key === 'X') handleNavigation('forward');
      if (e.key === 'z' || e.key === 'Z') handleNavigation('back');
    }});

    // Focus body on load
    document.body.focus();

    // Touch support
    let touchStartX = 0;
    gallery.addEventListener('touchstart', (e) => {{
      touchStartX = e.changedTouches[0].screenX;
    }});
    
    gallery.addEventListener('touchend', (e) => {{
      const touchEndX = e.changedTouches[0].screenX;
      if (touchStartX - touchEndX > 50) handleNavigation('forward');
      if (touchEndX - touchStartX > 50) handleNavigation('back');
    }});

    // Initialize first video
    if (videos.length > 0) {{
      videos[0].muted = false;
      videos[0].play().catch(e => console.log('First video autoplay:', e));
    }}
  </script>
</body>
</html>"#
    )
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
