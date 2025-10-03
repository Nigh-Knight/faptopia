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
            font-family: sans-serif;
        }}
        .tabs {{
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            background: rgba(0,0,0,0.8);
            padding: 10px;
            z-index: 1000;
            display: flex;
            gap: 10px;
            overflow-x: auto;
        }}
        .tab-button {{
            background: #333;
            color: white;
            border: none;
            padding: 10px 20px;
            border-radius: 5px;
            cursor: pointer;
            white-space: nowrap;
        }}
        .tab-button.active {{
            background: #666;
        }}
        .gallery-section {{
            margin-top: 50px;
            height: calc(100vh - 50px);
        }}
        .gallery-container {{
            display: flex;
            overflow-x: auto;
            scroll-snap-type: x mandatory;
            height: 100%;
            scroll-behavior: smooth;
        }}
        .gallery-item {{
            scroll-snap-align: start;
            flex: 0 0 100vw;
            height: 100%;
        }}
        .hidden {{
            display: none;
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
            font-size: 1.2rem;
            z-index: 100;
        }}
    </style>
</head>
<body tabindex="0">
    <div class="tabs">
        {tabs}
    </div>
    {sections}
    <!-- Hint updated to include the new C shortcut -->
    <div class="nav-hint">Press C to switch boards · Press Z ← → X to navigate</div>
    <script>
        // Index of the currently visible board (section)
        let currentSection = 0;
        // Per-board remembered focused item index, so switching boards restores position
        let currentIndices = new Array({total_sections}).fill(0);
        
        // showSection: reveals the board at `index`, hides others, updates active tab,
        // and ensures the correct video in that board has focus/play state.
        function showSection(index) {{
            // Hide all sections
            document.querySelectorAll('.gallery-section').forEach(section => 
                section.classList.add('hidden'));
            // Remove active class from all tabs
            document.querySelectorAll('.tab-button').forEach(tab => 
                tab.classList.remove('active'));
            
            // Show the requested section and mark the corresponding tab active
            document.querySelector(`[data-section="${{index}}"]`).classList.remove('hidden');
            document.querySelectorAll('.tab-button')[index].classList.add('active');
            currentSection = index;

            // Restore the remembered index inside that section and update video playback
            updateVideoFocus(currentSection, currentIndices[currentSection]);
            // Also ensure the section container is scrolled to the remembered item
            const gallery = document.getElementById(`gallery-${{currentSection}}`);
            gallery.scrollTo({{ left: currentIndices[currentSection] * window.innerWidth, behavior: 'instant' }});
        }}

        // updateVideoFocus: play/unmute only the focused video in the section,
        // pause and mute other videos to avoid multiple audio streams.
        function updateVideoFocus(sectionIndex, itemIndex) {{
            const videos = document.querySelectorAll(`#gallery-${{sectionIndex}} video`);
            videos.forEach((video, i) => {{
                if (i === itemIndex) {{
                    video.muted = false;
                    // Attempt to play; may be blocked by autoplay policies, so catch errors.
                    video.play().catch(e => console.log('Autoplay prevented:', e));
                }} else {{
                    // If the other video has ever played, pause and mute it.
                    if (!video.paused || video.played.length > 0) {{
                        video.muted = true;
                        try {{ video.pause(); }} catch (e) {{ /* ignore */ }}
                    }}
                }}
            }});
        }}

        // scrollToIndex: scrolls the given section to item `index` and updates remembered index + focus.
        function scrollToIndex(sectionIndex, index) {{
            const gallery = document.getElementById(`gallery-${{sectionIndex}}`);
            if (!gallery) return;
            gallery.scrollTo({{
                left: index * window.innerWidth,
                behavior: 'smooth'
            }});
            currentIndices[sectionIndex] = index;
            updateVideoFocus(sectionIndex, index);
        }}

        // handleNavigation: move forward/back within the current board, wrapping around.
        function handleNavigation(direction) {{
            const gallery = document.getElementById(`gallery-${{currentSection}}`);
            if (!gallery) return;
            const items = gallery.querySelectorAll('.gallery-item');
            const totalItems = items.length;
            if (totalItems === 0) return;

            const newIndex = direction === 'forward'
                ? (currentIndices[currentSection] + 1) % totalItems
                : (currentIndices[currentSection] - 1 + totalItems) % totalItems;
            
            scrollToIndex(currentSection, newIndex);
        }}

        // switchBoard: cycle to the next board (wraps). Uses the remembered per-board index.
        function switchBoard() {{
            const next = (currentSection + 1) % {total_sections};
            showSection(next);
        }}

        // Keyboard shortcuts:
        // X => next item, Z => previous item, C => next board
        document.addEventListener('keydown', (e) => {{
            // Ignore events when focused on inputs (safety)
            const active = document.activeElement;
            if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || active.isContentEditable)) return;

            if (e.key === 'x' || e.key === 'X') handleNavigation('forward');
            if (e.key === 'z' || e.key === 'Z') handleNavigation('back');
            if (e.key === 'c' || e.key === 'C') switchBoard();
        }});

        // Initialize the first section on load
        showSection(0);
    </script>
</body>
</html>"#,
        tabs = tabs.join("\n"),
        sections = sections.join("\n"),
        total_sections = items.len()
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
