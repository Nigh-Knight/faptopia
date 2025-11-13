# Requirements Document

## Introduction

This feature adds an optional HTTP server mode to the faptopia application that enables dynamic drag-and-drop functionality for adding 4chan threads to generated galleries. The server acts as a backend API that handles 4chan API requests on behalf of the browser-based JavaScript, eliminating CORS restrictions while maintaining the existing standalone HTML generation capability.

## Glossary

- **Faptopia Application**: The Rust-based CLI tool that generates media gallery HTML files from Reddit and 4chan sources
- **HTTP Server**: A local web server running on the user's machine that serves HTML files and provides API endpoints
- **API Endpoint**: A URL path that accepts HTTP requests and returns data (e.g., `/api/thread/:id`)
- **CORS**: Cross-Origin Resource Sharing, a browser security mechanism that restricts cross-origin HTTP requests
- **Thread ID**: A numeric identifier for a 4chan thread (e.g., 29757154)
- **Standalone HTML**: Generated HTML files that can be opened directly in a browser without requiring a server
- **Drag-and-Drop**: Browser feature allowing users to drag URLs and drop them onto the page to trigger actions

## Requirements

### Requirement 1: Server Command (Standalone Mode)

**User Story:** As a user, I want to start a local HTTP server separately so that I can serve multiple generated HTML files and use drag-and-drop functionality

#### Acceptance Criteria

1. WHEN the user executes the command `faptopia serve`, THE Faptopia Application SHALL start an HTTP server on localhost port 8080
2. WHILE the HTTP server is running, THE Faptopia Application SHALL serve static HTML files from the current directory
3. WHEN the HTTP server starts successfully, THE Faptopia Application SHALL display a message indicating the server is running and the URL to access it
4. WHEN the user presses Ctrl+C, THE Faptopia Application SHALL gracefully shut down the HTTP server

### Requirement 1.1: Integrated Server Mode

**User Story:** As a user, I want to generate HTML and start the server in one command so that I can quickly view my gallery with drag-and-drop enabled

#### Acceptance Criteria

1. WHEN the user executes `faptopia 4chan [id] --serve`, THE Faptopia Application SHALL generate the HTML file and then start the HTTP server
2. WHEN the HTML generation fails, THE Faptopia Application SHALL not start the server and SHALL display an error message
3. WHEN the server starts after HTML generation, THE Faptopia Application SHALL display the same server status messages as standalone mode

### Requirement 2: Thread API Endpoint

**User Story:** As a developer, I want an API endpoint that fetches 4chan thread data so that the JavaScript frontend can request thread information without CORS issues

#### Acceptance Criteria

1. WHEN the HTTP server receives a GET request to `/api/thread/:id`, THE Faptopia Application SHALL extract the thread ID from the URL path
2. WHEN a valid thread ID is provided, THE Faptopia Application SHALL fetch thread data from the 4chan API using the existing `fetch_video_links_4chan` function
3. WHEN the thread data is successfully fetched, THE Faptopia Application SHALL return a JSON response containing an array of video URLs with HTTP status 200
4. IF the thread does not exist or the 4chan API returns an error, THEN THE Faptopia Application SHALL return a JSON error response with an appropriate HTTP status code (404 or 500)
5. WHEN the API endpoint returns a response, THE Faptopia Application SHALL include CORS headers to allow requests from any origin

### Requirement 3: JavaScript Integration

**User Story:** As a user, I want the drag-and-drop feature to work seamlessly when the server is running so that I can add threads without CORS errors

#### Acceptance Criteria

1. WHEN the JavaScript detects a dropped 4chan URL, THE Faptopia Application SHALL send a request to the local API endpoint at `http://localhost:8080/api/thread/:id`
2. WHEN the API request is successful, THE Faptopia Application SHALL create a new tab with the returned video URLs
3. IF the server is not running, THEN THE Faptopia Application SHALL display an error message instructing the user to run `faptopia serve`
4. WHEN the API request fails, THE Faptopia Application SHALL display a user-friendly error message

### Requirement 4: Backward Compatibility

**User Story:** As a user, I want to continue generating standalone HTML files that work without a server so that I can share files easily

#### Acceptance Criteria

1. WHEN the user generates HTML files using existing commands (e.g., `faptopia 4chan 123456`), THE Faptopia Application SHALL create standalone HTML files that can be opened directly in a browser
2. WHEN a standalone HTML file is opened without the server running, THE Faptopia Application SHALL display all initially generated content correctly
3. WHERE the server is not running, THE Faptopia Application SHALL display a helpful message when drag-and-drop is attempted, indicating that the server is required for this feature

### Requirement 5: Port Configuration

**User Story:** As a user, I want to specify a custom port for the server so that I can avoid port conflicts

#### Acceptance Criteria

1. WHERE the user provides a `--port` flag, THE Faptopia Application SHALL start the HTTP server on the specified port
2. IF the specified port is already in use, THEN THE Faptopia Application SHALL display an error message and exit gracefully
3. WHEN no port is specified, THE Faptopia Application SHALL default to port 8080


### Requirement 6: Error Handling

**User Story:** As a user, I want clear error messages when something goes wrong so that I can troubleshoot issues

#### Acceptance Criteria

1. IF the 4chan API is unreachable, THEN THE Faptopia Application SHALL return a JSON error response with message "Failed to connect to 4chan API"
2. IF the thread ID is invalid or malformed, THEN THE Faptopia Application SHALL return a JSON error response with HTTP status 400 and message "Invalid thread ID"
3. IF no videos are found in the thread, THEN THE Faptopia Application SHALL return a JSON response with an empty array and HTTP status 200
4. WHEN any error occurs, THE Faptopia Application SHALL log the error details to the console for debugging
