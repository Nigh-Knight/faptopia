<p align="center">
  <img src="./faptopia.png" alt="Faptopia logo" width="300">
</p>

<video src="https://github.com/user-attachments/assets/2d701d76-4562-4b5e-baae-cb0f6e0948fc" controls width="600"></video>

A CLI tool that builds interactive, standalone HTML video galleries from Reddit and 4chan's /gif/ board. Single binary, no server required.

## Features

- Galleries from Reddit (via RedGifs) and 4chan /gif/ threads
- Multi-tab interface — multiple subreddits or threads in one gallery
- Keyboard navigation (Z / X to scroll, C to switch boards, P for auto-scroll, S to save)
- Touch support: swipe to navigate, wrap-around at ends
- Progress counter overlay (e.g. `3 / 47`)
- Lazy loading and autoplay management for performance
- PWA-ready: serve over local network and add to phone home screen
- Cloudflare Pages deploy (`faptopia deploy`) for sharing
- Usage stats page tracking sessions, time per source, and streaks

## Installation

### Download Pre-built Binary (Recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/Nigh-Knight/faptopia/releases):

- **Linux**: `faptopia-linux-x86_64`
- **Windows**: `faptopia-windows-x86_64.exe`
- **macOS**: `faptopia-macos-x86_64`

```bash
# Linux/macOS — make it executable:
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

Binary: `target/release/faptopia`

---

## Usage

### Reddit Galleries

```bash
# Format string
faptopia reddit r/subreddit:modifier:timeframe

# Full URL
faptopia reddit https://reddit.com/r/subreddit/top/?t=month

# Multiple subreddits (one tab each)
faptopia reddit r/sub1:top:month r/sub2:hot:day
```

| Parameter | Values |
|-----------|--------|
| modifier | `top`, `hot`, `new` |
| timeframe | `hour`, `day`, `week`, `month`, `year`, `all` |

Output: `faptopia_reddit.html`

Reddit caps anonymous traffic at a handful of requests per minute, so faptopia
pulls one Atom feed per board (100 posts each) and paces itself between them.
Two caches keep repeat runs cheap: resolved video URLs are kept 4 hours and each
board's scraped listing is kept 1 hour in `.faptopia_cache.json`, so a re-run
within the hour needs no reddit request at all. `--refresh` forces a re-scrape.

If you can get reddit API approval (`faptopia auth`, a script app), requests go
to OAuth instead: 100 per minute, which turns a 27-board run into seconds.

### 4chan Galleries

```bash
# Single thread
faptopia 4chan 123456789

# Multiple threads (one tab each)
faptopia 4chan 123456789 987654321
```

Output: `faptopia_4chan.html`

### Gallery Flags

```bash
--random      Shuffle boards/threads AND videos in random order
--reverse     Reverse board/thread AND video order (start from last)
--serve       Start local server for mobile access (default port 6969)
--port N      Custom port for --serve
--deploy      Deploy to Cloudflare Pages after generating gallery
--refresh     Ignore cached board listings and re-scrape reddit
--con N       Concurrent RedGifs lookups (default 8)
--output PATH Gallery output path
```

Credentials for OAuth live in `~/.config/faptopia/config.json` (0600) and are
managed with `faptopia auth` / `faptopia auth --logout`.

Progress is one rewritten line (`3/27 boards   41/1620 videos   waiting 12s`),
not a line per item. Redirected output gets board-level lines instead.

### Running two jobs at once

It works, with three caveats: both share the same per-IP rate budget, so reddit
requests interleave and each job slows down; give each one its own `--output` or
they overwrite the same gallery; the cache is safe (writers take a lock and merge
instead of clobbering).

Examples:

```bash
faptopia reddit r/hotwife:top:month --random
faptopia 4chan 123456789 --reverse
faptopia reddit r/hotwife:top:month --serve
faptopia 4chan 123456789 --deploy
```

---

## Commands

### `faptopia deploy`

Deploy your existing gallery files to Cloudflare Pages without regenerating them.

```bash
faptopia deploy
```

Requires `wrangler` CLI to be installed and authenticated (`wrangler login`).
Deploys to `https://faptopia.pages.dev` by default.

### `faptopia stats`

Generate a standalone stats page from your local browser history.

```bash
faptopia stats
# Creates faptopia_stats.html

# Or use --serve to share the same origin (required for cross-page localStorage access):
faptopia reddit r/hotwife:top:month --serve
# Then open http://localhost:6969/faptopia_stats.html
```

The stats page shows:
- Total sessions, time watched, average session length
- Day streak
- Most-watched sources (bar chart)
- Last 7 days activity
- Recent session history

---

## Gallery Controls

### Keyboard

| Key | Action |
|-----|--------|
| `Z` | Previous video |
| `X` | Next video |
| `C` | Switch to next board/tab |
| `P` | Toggle auto-scroll |
| `S` | Save (download) current video |

### Mobile

- Swipe up/down to navigate
- Wraps around at first/last item
- Bottom tab bar for switching boards
- Progress counter in top-right corner

### Tabs

- Click tabs to switch sources
- Click `x` on a tab to close it (minimum 1 required)

---

## 4chan Userscript

`userscript/faptopia_4chan.user.js` adds a launcher button to 4chan that automatically collects your pinned /gif/ threads and builds a `faptopia 4chan ...` command you can copy with one click.

Install with [Tampermonkey](https://www.tampermonkey.net/) or [Violentmonkey](https://violentmonkey.github.io/).

Features:
- Auto-detects pinned threads on any 4chan page
- Checkbox list to select/deselect threads
- "Copy" button (plain command) and "Copy + Deploy" button (with `--deploy`)
- Live-syncs as you pin/unpin threads

---

## Notes

- Generated HTML files are fully standalone — no server needed for basic use
- An internet connection is required to stream the actual video content
- The `--serve` flag is needed for stats to work across both gallery pages (they share `localStorage` on the same origin)
- Toggle button in each gallery links to the other platform's file (both must be in the same directory)
