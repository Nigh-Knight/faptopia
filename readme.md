<img src="./faptopia.png" alt="Faptopia logo" width="300">

# Faptopia

A command-line tool for creating gallery views of media content from Reddit and 4chan's /gif/ board.

## Features

- Create scrollable galleries from Reddit media embeds
- Create video galleries from 4chan /gif/ threads
- Keyboard navigation (Z and X keys)
- Touch support for mobile viewing
- Automatic video playback management
- Lazy loading for better performance

## Installation

```bash
git clone https://github.com/yourusername/faptopia.git
cd faptopia
cargo build --release
```

The binary is be available in `target/release/faptopia`

## Usage

### Reddit Galleries

Format:
```bash
faptopia reddit <subreddit>:<modifier>:<timeframe>
```

Parameters:
- `subreddit`: Name of the subreddit
- `modifier`: Sort type (top, hot, new)
- `timeframe`: Time period (hour, day, week, month, year, all)

Example:
```bash
faptopia reddit hotwife:top:month
```
This creates `faptopia_reddit.html` with a gallery of media from r/hotwife's top posts this month.

### 4chan Galleries

Format:
```bash
faptopia 4chan <thread_id>
```

Parameters:
- `thread_id`: The thread ID from /gif/ board

Example:
```bash
faptopia 4chan 123456789
```
This creates `faptopia_4chan.html` with a gallery of videos from the specified thread.

## Gallery Navigation

- Use `Z` key to go to previous media
- Use `X` key to go to next media
- Swipe left/right on touch devices
- Videos autoplay when in view
- Audio automatically mutes when not in active view

## Dependencies

- clap: Command line argument parsing
- regex: Regular expression matching
- ureq: HTTP requests
- serde: JSON serialization/deserialization
- once_cell: Lazy static initialization

## Notes

- The generated HTML files are standalone and can be opened in any modern browser
- Internet connection is required to view the media content