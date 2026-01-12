  <p align="center">
  <img src="./faptopia.png" alt="Faptopia logo" width="300">
</p>


<video src="https://github.com/user-attachments/assets/2d701d76-4562-4b5e-baae-cb0f6e0948fc" controls width="600"></video>

A command-line tool for creating gallery views of media content from Reddit and 4chan's /gif/ board.

## Features

- Create scrollable galleries from Reddit media embeds
- Create video galleries from 4chan /gif/ threads
- **Multi-tab interface** for viewing multiple sources simultaneously
- **Drag-and-drop support** to add 4chan threads dynamically
- **Dynamic tab management** with close buttons
- Keyboard navigation (Z, X, and C keys)
- Touch support for mobile viewing
- Automatic video playback management
- Lazy loading for better performance

## Installation

```bash
git clone https://github.com/Nigh-Knight/faptopia.git
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
faptopia 4chan <thread_id> [thread_id2] [thread_id3] ... [--serve] [--port <port>]
```

Parameters:
- `thread_id`: One or more thread IDs from /gif/ board
- `--serve`: (Optional) Start HTTP server after generating HTML
- `--port`: (Optional) Port to run the server on (default: 8080)

Example:
```bash
faptopia 4chan 123456789
```
This creates `faptopia_4chan.html` with a gallery of videos from the specified thread.

Multiple threads example:v
```bash
faptopia 4chan 123456789 987654321
```
This creates a multi-tab gallery with each thread in its own tab.

**With built-in server** (recommended for drag-and-drop):
```bash
faptopia 4chan 123456789 --serve
```
This generates the HTML and immediately starts a local server at `http://localhost:8080`.

**Custom port:**
```bash
faptopia 4chan 123456789 --serve --port 3000
```

### HTTP Server Mode

The built-in HTTP server enables drag-and-drop functionality without CORS issues.

**Standalone server command:**
```bash
faptopia serve [--port <port>]
```

This starts a local HTTP server without generating new HTML files. Use this when you already have generated HTML files and want to serve them.

**Examples:**

Workflow 1 - Generate and serve in one command:
```bash
faptopia 4chan 123456789 --serve
# Opens server at http://localhost:8080
# Drag-and-drop works immediately
```

Workflow 2 - Generate first, serve later:
```bash
# Step 1: Generate HTML files
faptopia 4chan 123456789
faptopia 4chan 987654321

# Step 2: Start server to view all files
faptopia serve
# Opens server at http://localhost:8080
# Navigate to any generated HTML file
```

**Server features:**
- Serves static HTML files from the current directory
- Provides API endpoint for drag-and-drop functionality
- Eliminates CORS issues when adding threads dynamically
- Graceful shutdown with Ctrl+C

## Gallery Navigation

### Keyboard Controls
- `Z` key: Go to previous media item
- `X` key: Go to next media item
- `C` key: Switch to next tab

### Tab Management
- Click on tabs to switch between different sources
- Click the `×` button on a tab to close it (minimum 1 tab required)
- Drag and drop 4chan thread URLs onto the page to add new tabs dynamically

### Touch Support
- Swipe left/right on touch devices to navigate media
- Videos autoplay when in view
- Audio automatically mutes when switching tabs or navigating away

## Dependencies

- clap: Command line argument parsing
- regex: Regular expression matching
- ureq: HTTP requests
- serde: JSON serialization/deserialization
- once_cell: Lazy static initialization
- tiny_http: Lightweight HTTP server for local serving

## Dynamic Features

Once you've generated a gallery HTML file, you can:
- **Add more 4chan threads**: Drag and drop a 4chan /gif/ thread URL (e.g., `https://boards.4chan.org/gif/thread/123456789`) directly onto the page
- **Close tabs**: Click the × button on any tab to remove it (at least one tab must remain)
- **Switch between sources**: Use tabs or the `C` key to navigate between different threads or subreddits

## Server Usage Notes

### When is the server needed?

The HTTP server is **required** for drag-and-drop functionality to work properly. When you drag a 4chan thread URL onto the page, the JavaScript needs to fetch thread data from the 4chan API. Due to browser CORS restrictions, this requires a local server to act as a proxy.

**Server required for:**
- ✅ Drag-and-drop to add new 4chan threads dynamically

**Server NOT required for:**
- ✅ Viewing initially generated galleries
- ✅ Navigating between tabs
- ✅ Keyboard controls (Z, X, C keys)
- ✅ Closing tabs

### Backward Compatibility

The generated HTML files remain **fully standalone** and work without the server:
- Open `faptopia_4chan.html` directly in your browser (`file://` protocol)
- All initially generated content displays correctly
- Tab switching, keyboard navigation, and video playback work normally
- Only drag-and-drop requires the server

### Choosing a Workflow

**Use `--serve` flag** when:
- You want to use drag-and-drop immediately
- You're working with a single HTML file
- You want the quickest setup

**Use `faptopia serve` separately** when:
- You've generated multiple HTML files
- You want to switch between different galleries
- You're iterating on HTML generation

**Skip the server** when:
- You just want to view the generated content
- You don't need drag-and-drop functionality
- You're sharing the HTML file with others

## Troubleshooting

### Linux executable won't run

If you downloaded a pre-built Linux binary and it won't run after `chmod +x faptopia`:

**Check what error you're getting:**
```bash
./faptopia
```

**Common issues:**

1. **"No such file or directory"** - This usually means missing shared library dependencies. Check with:
   ```bash
   ldd ./faptopia
   ```

2. **Glibc version mismatch** - Older Linux systems may not have the required glibc version.

**Solutions:**

- **Use the latest release** - Binaries from version 0.1.1+ are statically linked and work on all Linux distributions
- **Build from source** - This ensures compatibility with your system:
  ```bash
  git clone https://github.com/Nigh-Knight/faptopia.git
  cd faptopia
  cargo build --release
  ./target/release/faptopia
  ```
- **Use musl target** - For maximum compatibility:
  ```bash
  rustup target add x86_64-unknown-linux-musl
  cargo build --release --target x86_64-unknown-linux-musl
  ```

### Port already in use

If you see an error like "Port 8080 is already in use":
```bash
# Use a different port
faptopia serve --port 3000
# or
faptopia 4chan 123456789 --serve --port 3000
```

**Note:** The drag-and-drop feature currently only works with the default port 8080. If you use a custom port, you can still:
- View the generated galleries
- Use keyboard navigation
- Close and switch tabs
- But drag-and-drop will not work (the JavaScript is hardcoded to port 8080)

### Drag-and-drop not working

If you see "Server not running" when trying to drag-and-drop:
1. Make sure you're accessing the HTML via HTTP (e.g., `http://localhost:8080/faptopia_4chan.html`)
2. Don't open the file directly with `file://` protocol
3. Start the server with `faptopia serve` or use the `--serve` flag

### Thread not loading

If a thread fails to load when dragging:
- The thread may be archived or deleted (4chan threads expire)
- Check the browser console for specific error messages
- Verify the thread ID is correct and from the /gif/ board

### Server won't start

If the server fails to start:
- Check if another application is using the port
- Try a different port with `--port` flag
- Ensure you have network permissions (firewall/antivirus)

## Notes

- The generated HTML files are standalone and can be opened in any modern browser
- Internet connection is required to view the media content
- Drag-and-drop functionality works with 4chan /gif/ thread URLs only
- The server binds to `localhost` only (not accessible from other devices on your network)
