<p align="center">
  <img src="./faptopia.png" alt="Faptopia logo" width="300">
</p>

<video src="https://github.com/user-attachments/assets/2d701d76-4562-4b5e-baae-cb0f6e0948fc" controls width="600"></video>

A command-line tool for creating gallery views of media content from Reddit and 4chan's /gif/ board.

## Features

- Create scrollable galleries from Reddit media embeds
- Create video galleries from 4chan /gif/ threads
- Multi-tab interface for viewing multiple sources simultaneously
- Keyboard navigation (Z, X, and C keys)
- Touch support for mobile viewing
- Automatic video playback management
- Lazy loading for better performance

## Installation

### Download Pre-built Binary (Recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/Nigh-Knight/faptopia/releases):

- **Linux**: `faptopia-linux-x86_64`
- **Windows**: `faptopia-windows-x86_64.exe`
- **macOS**: `faptopia-macos-x86_64`

```bash
# Linux/macOS - make it executable:
chmod +x faptopia-linux-x86_64
./faptopia-linux-x86_64 --help

# Optionally, move to PATH:
sudo mv faptopia-linux-x86_64 /usr/local/bin/faptopia
```

### Build from Source

```bash
git clone https://github.com/Nigh-Knight/faptopia.git
cd faptopia
cargo build --release
```

The binary will be available in `target/release/faptopia`

## Usage

### Reddit Galleries

```bash
faptopia reddit <subreddit>:<modifier>:<timeframe>
```

**Example:**
```bash
faptopia reddit hotwife:top:month
```

This creates `faptopia_reddit.html` with media from r/hotwife's top posts this month.

**Parameters:**
- `subreddit`: Name of the subreddit
- `modifier`: Sort type (top, hot, new)
- `timeframe`: Time period (hour, day, week, month, year, all)

### 4chan Galleries

```bash
faptopia 4chan <thread_id> [thread_id2] [thread_id3] ...
```

**Example:**
```bash
faptopia 4chan 123456789
```

This creates `faptopia_4chan.html` with videos from the specified thread.

**Multiple threads:**
```bash
faptopia 4chan 123456789 987654321
```

Creates a multi-tab gallery with each thread in its own tab.

## Gallery Navigation

### Keyboard Controls
- `Z` key: Go to previous media item
- `X` key: Go to next media item
- `C` key: Switch to next tab

### Tab Management
- Click on tabs to switch between different sources
- Click the `×` button on a tab to close it (minimum 1 tab required)

### Touch Support
- Swipe left/right on touch devices to navigate media
- Videos autoplay when in view
- Audio automatically mutes when switching tabs or navigating away

## Notes

- Generated HTML files are standalone and work in any modern browser
- Internet connection required to view media content
