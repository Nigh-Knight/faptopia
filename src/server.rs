// HTTP server module for serving static files and providing API endpoints

use tiny_http::{Server, Response, Request, Method, StatusCode, Header};
use serde_json;

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
        
        // Fetch from 4chan API
        match crate::fetch_video_links_4chan(&[thread_id]) {
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
    
    fn handle_static_file(&self, request: Request) {
        let url = request.url();
        
        // Determine file path - default to index.html for root
        let file_path = if url == "/" {
            "index.html"
        } else {
            // Remove leading slash
            &url[1..]
        };
        
        // Read file from filesystem
        match std::fs::read(file_path) {
            Ok(content) => {
                let content_type = get_content_type(file_path);
                let response = Response::from_data(content)
                    .with_header(
                        Header::from_bytes(
                            &b"Content-Type"[..], 
                            content_type.as_bytes()
                        ).unwrap()
                    );
                let _ = request.respond(response);
            }
            Err(_) => {
                // File not found
                let response = Response::from_string("File not found")
                    .with_status_code(StatusCode(404));
                let _ = request.respond(response);
            }
        }
    }
}

// Helper function to extract thread ID from URL path
fn extract_thread_id(url: &str) -> Option<u64> {
    // Extract ID from "/api/thread/12345"
    url.strip_prefix("/api/thread/")
        .and_then(|id_str| id_str.parse::<u64>().ok())
}

// Helper function to create JSON success response
fn create_json_response(status: u16, video_urls: &[String]) -> Response<std::io::Cursor<Vec<u8>>> {
    let json = serde_json::json!({
        "success": true,
        "videos": video_urls
    });
    
    Response::from_string(json.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
        )
        .with_header(
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
        )
}

// Helper function to create JSON error response
fn create_error_response(status: u16, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let json = serde_json::json!({
        "success": false,
        "error": message
    });
    
    Response::from_string(json.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
        )
        .with_header(
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
        )
}

// Helper function to determine content type based on file extension
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
