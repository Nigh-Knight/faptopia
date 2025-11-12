# Design Document

## Overview

This design adds two interactive features to the Faptopia gallery: tab closing functionality and drag-and-drop support for adding 4chan threads. The implementation is entirely client-side JavaScript, requiring no backend changes. The design maintains the existing architecture while extending the tab management system and adding event handlers for drag-and-drop operations.

## Architecture

### Current Architecture
- **Backend**: Rust application that fetches media from Reddit/4chan and generates static HTML
- **Frontend**: Single-page application with embedded CSS/JS, no external dependencies
- **State Management**: JavaScript maintains current section index and per-board item indices

### Changes
- Add client-side tab manipulation (add/remove tabs and sections dynamically)
- Add drag-and-drop event handlers to capture and parse 4chan URLs
- Add API proxy or CORS-enabled fetch to retrieve 4chan thread data from the browser
- Extend state management to handle dynamic tab additions and removals
- Enhance tab switching to mute all videos in the previous tab before switching

## Components and Interfaces

### 1. Tab Close Button Component

**HTML Structure:**
```html
<button class="tab-button" onclick="showSection(0)">
    <span class="tab-label">Thread 12345</span>
    <span class="tab-close" onclick="closeTab(event, 0)">X</span>
</button>
```

**CSS Additions:**
```css
.tab-button {
    display: flex;
    align-items: center;
    gap: 8px;
}

.tab-close {
    font-size: 1.2rem;
    font-weight: bold;
    opacity: 0.7;
    transition: opacity 0.2s;
}

.tab-close:hover {
    opacity: 1;
}

.tab-button.last-tab .tab-close {
    display: none;
}
```

**JavaScript Interface:**
```javascript
function closeTab(event, sectionIndex)
// Prevents event bubbling to tab click handler
// Mutes and pauses all videos in the section being closed
// Removes tab button and gallery section from DOM
// Updates currentSection if closing active tab
// Rebuilds currentIndices array
// Updates remaining tabs' onclick handlers with new indices
```

### 2. Drag-and-Drop Handler

**Event Handlers:**
```javascript
// Prevent default drag behavior on document
document.addEventListener('dragover', handleDragOver);
document.addEventListener('drop', handleDrop);
document.addEventListener('dragleave', handleDragLeave);

function handleDragOver(event)
// Prevents default to allow drop
// Adds visual indicator class to body

function handleDragLeave(event)
// Removes visual indicator when drag leaves window

function handleDrop(event)
// Prevents default navigation
// Extracts text from dataTransfer
// Parses 4chan thread URL
// Calls addThreadTab if valid
```

**URL Parsing:**
```javascript
function parseThreadUrl(text)
// Regex: /boards\.4chan\.org\/gif\/thread\/(\d+)/
// Returns thread ID or null
```

### 3. Dynamic Tab Addition

**JavaScript Interface:**
```javascript
async function addThreadTab(threadId)
// Shows loading indicator
// Fetches thread JSON from 4chan API
// Parses video URLs from response
// Creates new tab button with close handler
// Creates new gallery section with video elements
// Appends to DOM
// Updates currentIndices array
// Calls showSection for new tab
// Hides loading indicator
```

**4chan API Integration:**
```javascript
async function fetch4chanThread(threadId)
// Fetches: https://a.4cdn.org/gif/thread/{threadId}.json
// Returns array of video URLs
// Handles CORS (may require proxy or CORS-anywhere)
```

### 4. Enhanced Tab Switching

**Modified showSection Function:**
```javascript
function showSection(index)
// Store previous section index
// Mute and pause ALL videos in previous section
// Hide all sections and deactivate all tabs
// Show requested section and activate corresponding tab
// Update currentSection
// Restore remembered index and update video focus for new section
// Scroll to remembered position
```

**Video Muting on Tab Switch:**
```javascript
function muteSection(sectionIndex)
// Select all videos in the specified section
// Pause each video
// Mute each video
// Ensures no audio continues from hidden tabs
```

## Data Models

### Tab State
```javascript
// Global state variables (existing + new)
let currentSection = 0;
let currentIndices = [];  // Dynamic array, grows/shrinks with tabs
let totalSections = 0;    // Track current number of sections
```

### 4chan Thread Response
```javascript
{
  "posts": [
    {
      "ext": ".webm",
      "tim": 1234567890123
    }
  ]
}
```

### Media Item Structure (for dynamic creation)
```javascript
{
  url: "https://i.4cdn.org/gif/1234567890123.webm",
  type: "video"
}
```

## Error Handling

### Tab Closing Errors
1. **Last Tab Protection**: Hide close button when only one tab remains
2. **Invalid Index**: Validate sectionIndex before removal
3. **DOM Sync**: Ensure tab buttons and sections stay synchronized

### Drag-and-Drop Errors
1. **Invalid URL**: Show user-friendly error message if URL doesn't match pattern
2. **Network Failure**: Display error if 4chan API fetch fails
3. **Empty Thread**: Handle threads with no video content gracefully
4. **CORS Issues**: Implement fallback or proxy if direct fetch is blocked

**Error Display:**
```javascript
function showError(message)
// Creates temporary toast notification
// Auto-dismisses after 3 seconds
```

**CSS for Error Toast:**
```css
.error-toast {
    position: fixed;
    top: 70px;
    right: 20px;
    background: #d32f2f;
    color: white;
    padding: 15px 20px;
    border-radius: 5px;
    z-index: 2000;
    animation: slideIn 0.3s ease-out;
}
```

## Visual Feedback

### Drag-and-Drop Indicator
```css
body.drag-active {
    outline: 3px dashed #4CAF50;
    outline-offset: -10px;
}

body.drag-active::after {
    content: "Drop 4chan thread URL here";
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: rgba(76, 175, 80, 0.9);
    color: white;
    padding: 30px 50px;
    border-radius: 10px;
    font-size: 1.5rem;
    z-index: 3000;
    pointer-events: none;
}
```

### Loading Indicator
```css
.loading-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 4000;
}

.loading-spinner {
    border: 4px solid rgba(255, 255, 255, 0.3);
    border-top: 4px solid white;
    border-radius: 50%;
    width: 50px;
    height: 50px;
    animation: spin 1s linear infinite;
}
```

## Testing Strategy

### Manual Testing Scenarios

**Tab Closing:**
1. Close middle tab → verify navigation switches to adjacent tab
2. Close active tab → verify auto-switch to nearest tab
3. Close all but one tab → verify close button disappears
4. Close tab and navigate → verify indices remain correct
5. Close tab with playing video → verify video stops

**Drag-and-Drop:**
1. Drag valid 4chan URL from address bar → verify new tab created
2. Drag text containing thread URL → verify URL extracted and tab created
3. Drag invalid URL → verify error message displayed
4. Drag URL while loading → verify loading indicator appears
5. Drag URL for thread with no videos → verify graceful error handling
6. Drop multiple URLs in sequence → verify all tabs created correctly

**Tab Switching with Muting:**
1. Play video in tab A, switch to tab B → verify tab A video is muted and paused
2. Switch between multiple tabs rapidly → verify no audio overlap
3. Switch tabs using keyboard shortcut (C) → verify previous tab mutes
4. Switch tabs using mouse click → verify previous tab mutes
5. Play multiple videos in a tab, switch away → verify all videos in that tab are muted

**Integration:**
1. Add tab via drag-drop, then close it → verify state consistency
2. Add multiple tabs, close several, navigate → verify no index errors
3. Close tabs while videos are playing → verify no audio leaks
4. Keyboard navigation after dynamic tab changes → verify shortcuts work
5. Switch tabs while videos are playing → verify previous tab audio stops

### Edge Cases
- Empty thread (no videos)
- Thread with only images (no .webm/.mp4)
- Malformed JSON response
- Network timeout
- CORS blocking
- Dragging non-text content
- Rapid successive drops
- Closing tabs during video playback

## Implementation Notes

### CORS Considerations
4chan API supports CORS, so direct fetch should work. If issues arise, options include:
1. Use a CORS proxy service
2. Add a simple Rust endpoint to proxy requests
3. Use browser extension to bypass CORS (development only)

### State Synchronization
When tabs are added/removed:
1. Rebuild `currentIndices` array to match new section count
2. Update all tab button `onclick` handlers with correct indices
3. Recalculate `currentSection` if it exceeds new bounds
4. Update `totalSections` counter

### Video Lifecycle
When closing a tab:
1. Pause all videos in that section
2. Mute all videos in that section
3. Remove event listeners
4. Set video src to empty to free memory
5. Remove DOM elements

When switching tabs:
1. Mute and pause all videos in the previous section
2. Only unmute and play the focused video in the new section
3. Keep other videos in the new section muted until navigated to

### Performance
- Lazy load videos in new tabs (preload="none")
- Only autoplay first video in newly added tab
- Limit maximum number of tabs (optional: 10 tabs)
- Clean up closed tab resources immediately
