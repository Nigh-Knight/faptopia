# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Encoding Requirements

**IMPORTANT**: All files in this repository, including this CLAUDE.md file, must use UTF-8 encoding only. Do not use special Unicode characters that may cause compatibility issues with editors like Zed. When creating diagrams or visual representations:
- Use ASCII characters only (e.g., `->` instead of `→`, `|` and `v` instead of `↓`)
- Avoid box-drawing characters, arrows, or other non-ASCII symbols
- Stick to standard ASCII for maximum compatibility

## Project Overview

**Faptopia** is a cross-platform Rust CLI tool that creates interactive HTML galleries of media from Reddit and 4chan. It generates responsive, feature-rich galleries with keyboard navigation and tab management for organizing multiple sources. The entire application (Rust binary + embedded frontend) ships as a single standalone executable with no external dependencies.

## High-Level Architecture

### Core Module Organization

**main.rs (443 lines)** - Single-file application containing all logic
- CLI parsing and command routing via clap
- Two main commands:
  - `reddit` - Fetches media from Reddit via RedGifs API
  - `4chan` - Extracts video links from 4chan /gif/ threads
- Gallery generation and HTML file writing
- RedGifs API integration (OAuth token fetching and video URL resolution)
- Reddit and 4chan API clients

### Frontend Architecture (Embedded Templates)

Templates are embedded at compile-time using `include_str!()` macros and rendered with `{{PLACEHOLDER}}` string substitution:

- **templates/gallery.html** - HTML scaffold with tab navigation and video containers
- **templates/styles.css** - Dark theme with horizontal scroll-snap gallery
- **templates/script.js** - Client-side interactivity:
  - Tab management (create, switch, close tabs)
  - Keyboard controls (Z/X/C for navigation)
  - Video state management (mute, autoplay, preload optimization)
  - Section-based scroll position tracking
  - Touch/swipe support
  - **Video resilience layer** (RedGifs CDN throttles per-IP with fake 404s and infinite stalls):
    - Auto-retry on `error` AND on cold-start stall (12s watchdog when no data arrives) with exponential backoff + jitter
    - Time-based HD -> SD downgrade via `data-sd-url`: an 8s grace timer drops a stuck HD video to SD (throttled fetches hang silently, so retry-count downgrades took ~50s); in throttle mode, never-played videos start on SD directly. Skipped in proxied mode (the proxy falls back server-side)
    - Burst circuit breaker: 3 failures within 30s enters throttle mode (3x backoff, preloading paused, toast shown); recovers automatically
    - Manual Retry overlay restores the original HD source for a clean slate
  - **Paced neighbor preloading**: while the focused video plays, its neighbors (wrap-around, so reverse navigation from first -> last stays warm) are warmed one at a time via `preload="auto"` (strictly 1 fetch in flight; the old eager preloader saturated the single HTTP/2 connection and was removed). Direction-aware — next warms first when going forward, prev first when going back — and the frontier extends a few videos ahead while the user lingers on a video
  - Behavior verified by `tests/script_resilience.test.js` (node harness that evals the template with DOM stubs; run with `node tests/script_resilience.test.js`)

### Data Flow

```
CLI Input (reddit or 4chan command)
    |
    v
Reddit Command                     4chan Command
    |                                  |
    v                                  v
fetch_media_embeds_reddit()       fetch_video_links_4chan()
    |                                  |
    | (calls RedGifs API)              | (calls 4chan API)
    v                                  v
fetch_redgifs_token()             Parse thread JSON
    |                                  |
    v                                  v
fetch_redgifs_video_url()         Extract .mp4/.webm URLs
    |                                  |
    v                                  v
Vec<String> (video URLs)          Vec<String> (video URLs)
    |                                  |
    +----------------------------------+
                   |
                   v
            Vec<MediaItem>
                   |
                   v
           save_gallery()
                   |
                   v
    Generates standalone .html file
    (faptopia_reddit.html or faptopia_4chan.html)
```

### Key Design Decisions

1. **Single-File Deployment** - All assets (HTML, CSS, JS) embedded in binary
2. **Simplified Architecture** - No HTTP server, all functionality in standalone HTML files
3. **Platform-Specific Galleries** - Separate output files for Reddit (`faptopia_reddit.html`) and 4chan (`faptopia_4chan.html`)
4. **Toggle Button** - Each gallery includes a button to switch to the other gallery file
5. **Section-Based Tabs** - Gallery items indexed as `{section}-{item}` for scroll position tracking per tab
6. **Lazy Loading** - Videos set `preload="none"` except first item (performance optimization)
7. **Direct API Integration** - Uses the Reddit Atom feed + RedGifs API v2 for Reddit media, 4chan JSON API for threads
8. **Progress Indicators** - Real-time progress display during RedGifs video fetching

## Platform-Specific Considerations

### Build Targets

- **Linux** - Uses `x86_64-unknown-linux-musl` target for static linking (universal glibc compatibility)
- **macOS** - Requires native macOS runner (commit a61fa04 fixed ring crate compilation issues)
- **Windows** - Built using native Windows runner with MSVC toolchain (commit 7066b05)

### Known Constraints

- Generated `.html` files are completely standalone and work via `file://` protocol
- Toggle button between Reddit/4chan galleries requires both HTML files to be in the same directory
- RedGifs API requires temporary auth token for each session
- 4chan API only supports `/gif/` board

## Development Commands

```bash
# Build and check
cargo build --release                    # Standard build
cargo build --release --target x86_64-unknown-linux-musl  # Static Linux binary
cargo check                              # Quick syntax check

# Credentials (once): ~/.config/faptopia/config.json, 0600
./target/release/faptopia auth           # interactive; 100 req/min instead of 10
./target/release/faptopia auth --logout  # back to anonymous

# Code quality
cargo fmt                                # Format code
cargo clippy                             # Lint with clippy
cargo clippy -- -D warnings              # Strict lint mode

# Testing (none currently exist - add tests to /tests or in module files)
cargo test

# Running locally
./target/release/faptopia reddit r/subreddit:top:month
./target/release/faptopia reddit https://reddit.com/r/subreddit/top/?t=month
./target/release/faptopia 4chan 123456789
./target/release/faptopia reddit r/subreddit1:top:week r/subreddit2:hot:day  # Multiple sources
./target/release/faptopia 4chan 111111111 222222222                          # Multiple threads
```

## Dependencies

Key active dependencies and their purposes:

| Crate | Purpose |
|-------|---------|
| **clap** | CLI argument parsing with derive macros |
| **ureq** | HTTP client for Reddit/4chan/RedGifs API requests |
| **serde/serde_json** | JSON deserialization for API responses |
| **regex** | Extract RedGifs IDs from Reddit embed HTML and parse Reddit URLs |
| **once_cell** | Lazy static initialization for compiled regex patterns |

**Unused Dependencies** (should be removed): scraper, maud, pulldown-cmark

## API Integration Details

### Reddit Integration

Reddit has walled off its anonymous JSON API (403 "blocked by network security")
and old.reddit.com now 302s logged-out clients to a login page, so there are two
access paths:

**OAuth (preferred, `faptopia auth`)** -- a reddit *script* app
(reddit.com/prefs/apps, redirect uri `http://localhost`) plus the account
username/password, exchanged at `/api/v1/access_token` with
`grant_type=password`. Requests go to `oauth.reddit.com/.../{top,hot,...}.json`
with a bearer token. Reddit allows **100 requests/minute** per client id here
versus ~10/minute anonymously, so this is the difference between seconds and
half an hour on a 27-board run. Tokens are cached in memory for 24h and re-minted
on a 401.

**Anonymous (fallback)** -- the public `/r/<sub>/<sort>/.rss` Atom feed. The
redgifs slug is regex-scraped out of each entry's HTML-encoded body.

Both paths share `reddit_get()`, which owns pacing:

- `ANON_GATE` / `OAUTH_GATE` (Mutex<RedditGate>) serialise request *starts*.
- A 429 sets a shared `blocked_until` deadline **and** multiplies the steady
  interval (AIMD, anon 10s floor -> 40s ceiling, oauth 0.7s -> 5s). Per-worker
  backoff alone produced 429 storms, because every worker retried into the
  next. The old code slept a flat 62s per board - roughly 6x slower than needed.
- `x-ratelimit-remaining: 0` parks the gate for `x-ratelimit-reset`.
- Requests always use `limit=100`; `--limit` only truncates afterwards, because
  a second page costs a whole rate-limit slot. Pagination caps at 3 pages
  (anonymous) / 4 (OAuth) and stops early on a page that adds no new slugs.

Pagination uses `&after=<fullname>`; the anonymous cursor comes from
`ENTRY_ID_REGEX` over the feed, the OAuth one from `data.after`.

### Progress output

`PROGRESS` (a `Mutex<Progress>`) renders one `\r`-rewritten status line: board and
video counts plus a rate-limit countdown. It only draws on a tty
(`io::stdout().is_terminal()`), throttled to 80ms, and `progress_log()` clears the
line before printing so log lines never smear. Non-tty output (pipes, logs) skips
per-item lines entirely and prints board results only. The gate sleeps in 250ms
slices so the countdown keeps ticking. Anything that prints during a run should
go through `progress_log`, not `println!`, and `progress_finish()` must run before
the final summary.

### Concurrent runs

Supported: `save_cache()` takes an exclusive `flock` on `.faptopia_cache.lock`,
reloads, merges, and writes via temp+rename, so two jobs cannot drop each other's
entries. `--output` exists because both jobs would otherwise write the same
gallery file. The rate gate is per-process, so two jobs still share one per-IP
reddit budget and interleave.

### Listing cache

`fetch_redgifs_ids()` checks `.faptopia_cache.json`'s `listings` map first, keyed
by `{r|u}/{name}/{modifier}/{time}`, TTL 1 hour, bypassed by `--refresh`. A hit
means **zero** reddit requests for that board, which is the only slow step in
the pipeline - a warm re-run of 27 boards finishes in about a second. The full
page yield is cached (not the truncated slice) so a later larger `--limit`
benefits. The startup line reports how many boards actually need reddit.

Input formats:
- Format string: `r/subreddit:modifier:time` (e.g., `r/hotwife:top:month`)
- User: `u/username:sort:time`
- Full URL: `https://reddit.com/r/subreddit/modifier/?t=time`

`fetch_media_embeds_reddit()` then resolves slugs through RedGifs with a worker
pool sized by `--con` (default 8, config `concurrency`). Boards are also fetched
through a worker pool (max 8); results land in per-index slots so tab order is
preserved regardless of completion order.

### 4chan Integration

1. Fetches thread JSON from `a.4cdn.org/gif/thread/{id}.json`
2. Filters posts for `.mp4` or `.webm` extensions
3. Constructs direct video URLs: `i.4cdn.org/gif/{tim}{ext}`

Input format:
- Thread ID: `123456789`

## Release & Version Management

Versions are managed via git tags that trigger automated GitHub Actions releases:

```bash
# Update version in Cargo.toml
vim Cargo.toml  # Change version field

# Commit and tag
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.X.Y"
git tag vX.Y.Z
git push origin main
git push origin vX.Y.Z  # Triggers CI/CD
```

GitHub Actions automatically:
- Builds for Linux (musl), Windows (MSVC), and macOS (native runners)
- Strips binaries for smaller size
- Creates GitHub release with pre-built executables

## When Modifying Code

### Adding Features or Fixes

1. The codebase is intentionally minimal - all logic is in `src/main.rs`
2. When changes affect gallery generation, test both Reddit and 4chan outputs
3. Platform-specific logic should consider Linux (musl), macOS, and Windows builds
4. Frontend changes to templates must preserve keyboard controls (Z/X/C) and tab behavior

### Template Updates

- HTML/CSS/JS are embedded at compile-time via `include_str!()`; changes require rebuild
- Placeholders use `{{UPPERCASE}}` format and are replaced in `generate_gallery()`
- Available placeholders: `{{STYLES}}`, `{{TABS}}`, `{{SECTIONS}}`, `{{SCRIPT}}`, `{{TOTAL_SECTIONS}}`
- Frontend state management uses `{section}-{item}` indexing for scroll position tracking

### API Integration Changes

- **Reddit**: Two paths, both behind `reddit_get()`. OAuth (`faptopia auth`) against
  `oauth.reddit.com` JSON lists at 100 req/min; anonymous `/rss?limit=100` as the
  fallback. Never sleep a fixed interval per board - the gate adapts from
  `x-ratelimit-remaining`/`-reset` and an AIMD penalty on 429. Do not reintroduce
  `.json` scraping against `www.reddit.com` or old.reddit.com without checking;
  both answer 403/302 for anonymous clients.
- **4chan**: Direct API endpoint call returning thread JSON with post metadata
- **RedGifs**: Requires temporary OAuth token (expires after session), returns HD/SD video URLs
- Both platforms use `MediaItem { url: String }` struct for unified handling

### Progress Indicators

Reddit fetching shows real-time progress:
```
Fetching videos: 45% (12/27)
```

Progress updates use `\r` for in-place updates and flush stdout for immediate display.

### Cross-Platform Testing

When modifying build or platform-specific code:
- Test Linux build produces musl-linked binary (check with `ldd target/release/faptopia`)
- Verify macOS build on native runner (not cross-compiled)
- Windows builds use native runner with MSVC (not cross-compilation)
- CI logs should show successful builds for all three platforms

### Input Parsing

Reddit command supports two input formats:
1. **Format string**: `r/subreddit:modifier:time` (legacy format)
2. **Full URL**: `https://reddit.com/r/subreddit/modifier/?t=time` (added in e9a0aba)

Use `parse_reddit_input()` function to handle both formats consistently.

## Recent Changes (Last 5 Commits)

1. **491225a** - Simplified README by removing obsolete sections
2. **cfbe65f** - Bumped version to 0.1.3
3. **7066b05** - Fixed Windows build to use native runner instead of cross-compilation
4. **a5dda16** - Bumped version to 0.1.2
5. **e9a0aba** - Added progress indicator and URL parsing for Reddit command

## Removed Features

The following features were removed in commit **f28c30e** (2026-01-14):
- HTTP server implementation (`server.rs` module, `tiny_http` dependency)
- `serve` command and `--serve`/`--port` flags
- Drag-and-drop functionality for adding threads dynamically
- Tab management via server API
- CORS proxy for 4chan API
- Loading indicators and error handling for dynamic features

The application now focuses on generating standalone HTML galleries without requiring a running server.
