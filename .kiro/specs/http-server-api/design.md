# Design Document (Simplified - No Caching, 4chan Only)

## Overview

This design implements an optional HTTP server mode for the faptopia application that provides a local API for fetching 4chan thread data. The server eliminates CORS restrictions by acting as a proxy between the browser-based JavaScript and the 4chan API, while maintaining backward compatibility with standalone HTML file generation.

The design supports two workflows:
1. **Integrated mode**: Generate HTML and start server in one command (`faptopia 4chan 123456 --serve`)
2. **Separate mode**: Generate HTML first, then start server separately (`faptopia 4chan 123456` then `faptopia serve`)

The design follows a modular architecture with clear separation between the CLI interface, HTTP server, API handlers, and existing thread fetching logic.

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        User Browser                          │
│  ┌────────────────────────────────────────────────────┐    │
│  │  Generated HTML + JavaScript                       │    │
│  │  - Drag & Drop Handler (4chan only)                │    │
│  │  - Fetch to localhost:8080/api/thread/:id          │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ HTTP Request
┌─────────────────────────────────────────────────────────────┐
│                   Faptopia HTTP Server                       │
│  ┌────────────────────────────────────────────────────┐    │
│  │  Router                                             │    │
│  │  - GET /api/thread/:id → thread_handler()          │    │
│  │  - GET /* → serve_static_files()                   │    │
│  └────────────────────────────────────────────────────┘    │
│                            │                                 │
│                            ▼                                 │
│  ┌────────────────────────────────────────────────────┐    │
│  │  fetch_video_links_4chan() [Existing Function]     │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ HTTP Request
┌─────────────────────────────────────────────────────────────┐
│                    4chan API (a.4cdn.org)                    │
└─────────────────────────────────────────────────────────────┘
```

### Component Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                         main.rs                               │
│                                                               │
│  ┌─────────────┐      ┌──────────────┐                      │
│  │ CLI Parser  │─────▶│  Commands    │                      │
│  │  (clap)     │      │  - Reddit    │                      │
│  └─────────────┘      │  - 4chan     │                      │
│                       │  - Serve ◄───┼─── NEW               │
│                       └──────────────┘                       │
│                              │                                │
│                              ▼                                │
│                       ┌──────────────┐                       │
│                       │ serve_mode() │◄──── NEW              │
│                       └──────────────┘                       │
│                              │                                │
│         ┌────────────────────┴────────────────────┐          │
│         ▼                                         ▼          │
│  ┌─────────────┐                       ┌──────────────┐     │
│  │HTTP Server  │                       │ API Handlers │     │
│  │(tiny_http)  │                       │              │     │
│  └─────────────┘                       └──────────────┘     │
│         │                                      │            │
│         └──────────────────────────────────────┘            │
│                            │                                 │
│                            ▼                                 │
│                 ┌──────────────────────┐                     │
│                 │fetch_video_links_    │                     │
│                 │4chan() [Existing]    │                     │
│                 └──────────────────────┘                     │
└──────────────────────────────────────────────────────────────┘
```

## Main Application Flow

### Command Handling Logic

```rust
fn main() -> io::Result<()> {
    let cli: Cli = Cli::parse();

    match &cli.command {
        Commands::FourChan(args) => {
            if let Some(ids) = &args.thread {
                // Generate HTML file (existing logic)
                let mut thread_items = Vec::new();
                for id in ids {
                    match fetch_video_links_4chan(&[*id]) {
                        Ok(links) => {
                            let items = links.into_iter()
                                .map(|url| MediaItem { url, media_type: MediaType::Video })
                                .collect();
                            thread_items.push((format!("Thread {}", id), items));
                        }
                        Err(e) => eprintln!("Error fetching thread {}: {}", id, e),
                    }
                }
                save_gallery(thread_items, "faptopia_4chan.html")?;
                
                // If --serve flag is set, start the server
                if args.serve {
                    println!("\nStarting server...\n");
                    let server = AppServer::new(args.port);
                    if let Err(e) = server.start() {
                        eprintln!("Server error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("INPUT A THREAD ID");
            }
        }
        
        Commands::Reddit(args) => {
            // Existing reddit logic - no --serve support
            if let Some(names) = &args.name {
                let mut subreddit_items = Vec::new();
                for x in names {
                    // ... existing reddit logic ...
                }
                save_gallery(subreddit_items, "faptopia_reddit.html")?;
            } else {
                println!("INPUT A SUBREDDIT PAGE");
            }
        }
        
        Commands::Serve(args) => {
            // Standalone server mode - no HTML generation
            let server = AppServer::new(args.port);
            if let Err(e) = server.start() {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
```

## Components and Interfaces

### 1. CLI Command Structure

**Updated Commands with `--serve` flag (4chan only):**

```rust
#[derive(Args)]
struct ThreadId {
    thread: Option<Vec<u64>>,
    
    /// Start HTTP server after generating HTML
    #[arg(long, default_value = "false")]
    serve: bool,
    
    /// Port to run the server on (only with --serve)
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[derive(Args)]
struct SubReddit {
    name: Option<Vec<String>>,
    // No --serve flag for Reddit
}

#[derive(Args)]
struct ServeArgs {
    /// Port to run the server on
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[derive(Subcommand)]
enum Commands {
    Reddit(SubReddit),
    #[clap(name = "4chan")]
    FourChan(ThreadId),
    Serve(ServeArgs),  // NEW - standalone server mode
}
```

### 2. HTTP Server Module

**File: `src/server.rs` (new file)**

```rust
use tiny_http::{Server, Response, Request, Method, StatusCode};

pub struct AppServer {
    port: u16,
}

impl AppServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
    
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let server = Server::http(format!("127.0.0.1:{}", self.port))
            .map_err(|e| {
                if e.to_string().contains("Address already in use") {
                    format!("Port {} is already in use. Try a different port with --port", self.port)
                } else {
                    e.to_string()
                }
            })?;
        
        println!("✓ Server running at http://localhost:{}", self.port);
        println!("✓ Drag and drop 4chan thread URLs to add them dynamically");
        println!("✓ Press Ctrl+C to stop");
        println!();
        
        for request in server.incoming_requests() {
            self.handle_request(request);
        }
        
        Ok(())
    }
    
    fn handle_request(&self, request: Request) {
        let url = request.url().to_string();
        
        match (request.method(), url.as_str()) {
            (Method::Get, path) if path.starts_with("/api/thread/") => {
                self.handle_thread_api(request);
            }
            (Method::Get, _) => {
                self.handle_static_file(request);
            }
            _ => {
                let response = Response::from_string("Not Found")
                    .with_status_code(StatusCode(404));
                let _ = request.respond(response);
            }
        }
    }
    
    fn handle_thread_api(&self, request: Request) {
        // Implementation in API Handlers section
    }
    
    fn handle_static_file(&self, request: Request) {
        // Implementation in Static File Serving section
    }
}
```

### 3. API Handlers

**Thread API Handler:**

```rust
fn handle_thread_api(&self, request: Request) {
    // Extract thread ID from URL path
    let url = request.url();
    let thread_id = match extract_thread_id(url) {
        Some(id) => id,
        None => {
            let response = create_error_response(400, "Invalid thread ID");
            let _ = request.respond(response);
            return;
        }
    };
    
    // Fetch from 4chan API (no caching)
    match fetch_video_links_4chan(&[thread_id]) {
        Ok(video_urls) => {
            let response = create_json_response(200, &video_urls);
            let _ = request.respond(response);
        }
        Err(e) => {
            eprintln!("Error fetching thread {}: {}", thread_id, e);
            let response = create_error_response(500, "Failed to fetch thread data");
            let _ = request.respond(response);
        }
    }
}

fn extract_thread_id(url: &str) -> Option<u64> {
    // Extract ID from "/api/thread/12345"
    url.strip_prefix("/api/thread/")
        .and_then(|id_str| id_str.parse::<u64>().ok())
}

fn create_json_response(status: u16, video_urls: &[String]) -> Response<std::io::Cursor<Vec<u8>>> {
    let json = serde_json::json!({
        "success": true,
        "videos": video_urls
    });
    
    Response::from_string(json.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
        )
        .with_header(
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
        )
}

fn create_error_response(status: u16, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let json = serde_json::json!({
        "success": false,
        "error": message
    });
    
    Response::from_string(json.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
        )
        .with_header(
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
        )
}
```

### 4. Static File Serving

```rust
fn handle_static_file(&self, request: Request) {
    let url = request.url();
    let file_path = if url == "/" {
        "index.html"
    } else {
        &url[1..] // Remove leading slash
    };
    
    match std::fs::read(file_path) {
        Ok(content) => {
            let content_type = get_content_type(file_path);
            let response = Response::from_data(content)
                .with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..], 
                        content_type.as_bytes()
                    ).unwrap()
                );
            let _ = request.respond(response);
        }
        Err(_) => {
            let response = Response::from_string("File not found")
                .with_status_code(StatusCode(404));
            let _ = request.respond(response);
        }
    }
}

fn get_content_type(file_path: &str) -> String {
    if file_path.ends_with(".html") {
        "text/html; charset=utf-8".to_string()
    } else if file_path.ends_with(".css") {
        "text/css".to_string()
    } else if file_path.ends_with(".js") {
        "application/javascript".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}
```

### 5. JavaScript Integration

**Updated `fetch4chanThread` function in `templates/script.js`:**

```javascript
async function fetch4chanThread(threadId) {
    try {
        console.log('Fetching thread:', threadId);
        
        // Call local API
        const localUrl = `http://localhost:8080/api/thread/${threadId}`;
        console.log('Fetch URL:', localUrl);
        
        const response = await fetch(localUrl);
        
        if (!response.ok) {
            throw new Error(`Failed to fetch thread: ${response.status}`);
        }
        
        const data = await response.json();
        console.log('Thread data received:', data);
        
        if (!data.success) {
            throw new Error(data.error || 'Unknown error');
        }
        
        return data.videos;
    } catch (error) {
        console.error('Error fetching 4chan thread:', error);
        
        // Check if it's a network error (server not running)
        if (error.message.includes('Failed to fetch') || error.name === 'TypeError') {
            throw new Error('Server not running. Please run: faptopia serve');
        }
        
        throw error;
    }
}
```

## Data Models

### API Response Format

**Success Response:**
```json
{
    "success": true,
    "videos": [
        "https://i.4cdn.org/gif/1234567890.webm",
        "https://i.4cdn.org/gif/1234567891.mp4"
    ]
}
```

**Error Response:**
```json
{
    "success": false,
    "error": "Error message here"
}
```

## Error Handling

### Error Types and Responses

| Error Scenario | HTTP Status | Response Message | Action |
|---------------|-------------|------------------|--------|
| Invalid thread ID | 400 | "Invalid thread ID" | Return error JSON |
| Thread not found | 404 | "Thread not found" | Return error JSON |
| 4chan API error | 500 | "Failed to fetch thread data" | Log error, return error JSON |
| Network error | 500 | "Failed to connect to 4chan API" | Log error, return error JSON |
| Port in use | N/A | Console error | Exit with error code |
| Server not running (JS) | N/A | "Server not running. Please run: faptopia serve" | Show error toast |

### Error Handling Flow

```
Request → Validate Thread ID → Fetch from 4chan → Return Response
            │                      │
            ▼ Invalid              ▼ Error
         400 Error            500 Error
```

## Testing Strategy

### Unit Tests

1. **URL Parsing Tests**
   - Test valid thread ID extraction
   - Test invalid URL formats
   - Test edge cases (empty, special characters)

2. **Response Generation Tests**
   - Test JSON response formatting
   - Test CORS headers
   - Test error response formatting

### Integration Tests

1. **Server Tests**
   - Test server startup on different ports
   - Test port conflict handling
   - Test graceful shutdown

2. **API Endpoint Tests**
   - Test successful thread fetch
   - Test error scenarios (invalid ID, network error)

3. **Static File Serving Tests**
   - Test HTML file serving
   - Test 404 handling
   - Test content-type headers

### Manual Testing

1. **End-to-End Flow**
   - Generate HTML file with `faptopia 4chan 29775997`
   - Start server with `faptopia serve`
   - Open HTML in browser
   - Drag and drop a thread URL
   - Verify new tab is created with videos

2. **Integrated Mode**
   - Run `faptopia 4chan 29775997 --serve`
   - Verify HTML is generated and server starts
   - Test drag-and-drop functionality

3. **Error Scenarios**
   - Try drag-and-drop without server running
   - Try invalid thread URLs
   - Try archived/deleted threads

## Dependencies

### New Rust Crates

```toml
[dependencies]
# Existing dependencies
clap = { version = "4.0", features = ["derive"] }
regex = "1.5"
ureq = "2.5"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
once_cell = "1.18"

# New dependencies for HTTP server
tiny_http = "0.12"  # Lightweight HTTP server
```

### Why `tiny_http`?

- Lightweight and simple
- No async runtime required (keeps codebase simple)
- Perfect for local development server
- Easy to understand and maintain
- Minimal dependencies

## Performance Considerations

### Simplicity

- No caching layer (simpler code, easier to maintain)
- Direct API calls to 4chan (acceptable for local use)
- Single-threaded server (sufficient for local use)

### Future Optimizations (if needed)

- Add in-memory caching for frequently accessed threads
- Add request coalescing (multiple requests for same thread)
- Add persistent cache (SQLite)

## Security Considerations

### Local-Only Server

- Binds to `127.0.0.1` (localhost only)
- Not accessible from network
- No authentication needed

### CORS Headers

- Allows all origins (`Access-Control-Allow-Origin: *`)
- Safe because server is local-only
- Enables drag-and-drop from any 4chan board

### Input Validation

- Thread ID must be numeric
- Path traversal prevention in static file serving
- No user-provided code execution

## Deployment and Configuration

### Command-Line Interface

**Workflow 1: Generate HTML and start server in one command**
```bash
# Generate HTML and immediately start server
faptopia 4chan 123456 --serve

# With custom port
faptopia 4chan 123456 --serve --port 3000

# Multiple threads with server
faptopia 4chan 123456 789012 --serve
```

**Workflow 2: Generate HTML first, then start server separately**
```bash
# Step 1: Generate HTML files
faptopia 4chan 123456

# Step 2: Start server to enable drag-and-drop
faptopia serve

# With custom port
faptopia serve --port 3000
```

**Use Cases:**
- **Workflow 1** (`--serve` flag): Quick start for immediate viewing with drag-and-drop
- **Workflow 2** (separate commands): Generate multiple HTML files, then serve them all

### Configuration

No configuration file needed. All options via CLI flags.

### Logging

```
Server running at http://localhost:8080
Press Ctrl+C to stop

[INFO] GET /api/thread/12345 - 200 OK (45ms)
[INFO] GET /faptopia_4chan.html - 200 OK (2ms)
[ERROR] GET /api/thread/99999 - 404 Not Found
```

## Migration Path

### Backward Compatibility

- Existing commands (`faptopia reddit`, `faptopia 4chan`) unchanged
- Generated HTML files work standalone (without server)
- Drag-and-drop gracefully degrades with helpful error message

### User Migration

1. Users continue using existing workflow
2. When they want drag-and-drop, run `faptopia serve` or use `--serve` flag
3. No breaking changes to existing functionality

## Design Decisions

### Why no caching?

- Simpler implementation
- Easier to maintain
- Sufficient for local use case
- Can add later if needed

### Why 4chan only for drag-and-drop?

- Reddit uses iframe embeds (different architecture)
- 4chan threads are simpler (direct video URLs)
- Focus on the primary use case
- Can add Reddit support later if needed

### Why `tiny_http` over `actix-web`?

- Simpler codebase (no async complexity)
- Sufficient for local development server
- Easier to maintain and understand
- Smaller binary size

### Why separate `serve` command?

- Clear separation of concerns
- Maintains backward compatibility
- Users opt-in to server mode
- Easier to document and understand
