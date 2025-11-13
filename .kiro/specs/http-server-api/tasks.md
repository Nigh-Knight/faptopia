# Implementation Plan

- [x] 1. Add HTTP server dependency and create server module
  - Add `tiny_http` crate to Cargo.toml dependencies
  - Create new file `src/server.rs` for server implementation
  - Add module declaration in `src/main.rs`
  - _Requirements: 1, 1.1_

- [x] 2. Implement CLI command structure with --serve flag
  - [x] 2.1 Add `--serve` flag to `ThreadId` struct
    - Add `serve: bool` field with clap attribute
    - Add `port: u16` field with default value 8080
    - _Requirements: 1.1, 5_
  
  - [x] 2.2 Add `--serve` flag to `SubReddit` struct
    - Add `serve: bool` field with clap attribute
    - Add `port: u16` field with default value 8080
    - _Requirements: 1.1, 5_
  
  - [x] 2.3 Add standalone `Serve` command
    - Create `ServeArgs` struct with port field
    - Add `Serve(ServeArgs)` variant to `Commands` enum
    - _Requirements: 1, 5_

- [x] 3. Implement basic HTTP server structure
  - [x] 3.1 Create `AppServer` struct
    - Define struct with `port` field
    - Implement `new()` constructor
    - _Requirements: 1_
  
  - [x] 3.2 Implement server startup logic
    - Implement `start()` method that binds to localhost
    - Add error handling for port already in use
    - Add startup success message with URL
    - Add Ctrl+C instruction message
    - _Requirements: 1, 5, 6_
  
  - [x] 3.3 Implement request routing
    - Create `handle_request()` method
    - Route `/api/thread/:id` to thread handler
    - Route all other paths to static file handler
    - Return 404 for unsupported methods
    - _Requirements: 2_

- [x] 4. Implement thread API endpoint
  - [x] 4.1 Create thread ID extraction function
    - Implement `extract_thread_id()` to parse URL path
    - Handle invalid thread ID formats
    - Return `Option<u64>` for valid/invalid IDs
    - _Requirements: 2, 6_
  
  - [x] 4.2 Implement thread API handler
    - Create `handle_thread_api()` method
    - Extract thread ID from request URL
    - Call existing `fetch_video_links_4chan()` function
    - Handle success and error cases
    - _Requirements: 2, 6_
  
  - [x] 4.3 Create JSON response helpers
    - Implement `create_json_response()` for success responses
    - Implement `create_error_response()` for error responses
    - Add proper Content-Type headers
    - Add CORS headers (`Access-Control-Allow-Origin: *`)
    - _Requirements: 2, 6_

- [x] 5. Implement static file serving
  - [x] 5.1 Create static file handler
    - Implement `handle_static_file()` method
    - Read file from filesystem
    - Handle file not found errors
    - _Requirements: 1_
  
  - [x] 5.2 Add content-type detection
    - Implement `get_content_type()` function
    - Support .html, .css, .js file extensions
    - Default to application/octet-stream
    - _Requirements: 1_

- [x] 6. Integrate server with main CLI flow
  - [x] 6.1 Update 4chan command handler
    - After HTML generation, check if `--serve` flag is set
    - If true, create and start AppServer
    - Handle server errors gracefully
    - _Requirements: 1.1_
  
  - [x] 6.2 Update Reddit command handler
    - After HTML generation, check if `--serve` flag is set
    - If true, create and start AppServer
    - Handle server errors gracefully
    - _Requirements: 1.1_
  
  - [x] 6.3 Implement standalone serve command
    - Handle `Commands::Serve` variant
    - Create and start AppServer without HTML generation
    - Handle server errors gracefully
    - _Requirements: 1_

- [x] 7. Update JavaScript to use local API
  - [x] 7.1 Update fetch4chanThread function
    - Change fetch URL to `http://localhost:8080/api/thread/${threadId}`
    - Parse JSON response with `success` and `videos` fields
    - Handle network errors (server not running)
    - Update error messages to mention `faptopia serve`
    - _Requirements: 3_
  
  - [x] 7.2 Update addThreadTab function
    - Ensure it works with new API response format
    - Handle empty video arrays gracefully
    - _Requirements: 3_

- [x] 8. Update documentation
  - [x] 8.1 Update README with server commands
    - Document `faptopia serve` command
    - Document `--serve` flag usage
    - Document `--port` flag
    - Add examples for both workflows
    - _Requirements: 1, 1.1, 5_
  
  - [x] 8.2 Add server usage notes
    - Explain when server is needed (drag-and-drop)
    - Explain backward compatibility (standalone HTML still works)
    - Add troubleshooting section for common issues
    - _Requirements: 4_

- [-] 9. Build and test end-to-end
  - [x] 9.1 Test standalone server mode
    - Generate HTML with `faptopia 4chan [id]`
    - Start server with `faptopia serve`
    - Test drag-and-drop functionality
    - Verify new tabs are created correctly
    - _Requirements: 1, 2, 3_
  
  - [x] 9.2 Test integrated mode
    - Run `faptopia 4chan [id] --serve`
    - Verify HTML is generated
    - Verify server starts automatically
    - Test drag-and-drop functionality
    - _Requirements: 1.1, 2, 3_
  
  - [x] 9.3 Test error scenarios
    - Test invalid thread IDs
    - Test port conflicts
    - Test server not running (drag-and-drop should show helpful error)
    - Test archived/deleted threads
    - _Requirements: 6_
  
  - [x] 9.4 Test custom port
    - Run server with `--port 3000`
    - Verify server starts on correct port
    - Update JavaScript to use custom port (or document limitation)
    - _Requirements: 5_
