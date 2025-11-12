# Implementation Plan

- [x] 1. Enhance tab switching to mute previous tab videos
  - Modify the `showSection()` function to track the previous section index before switching
  - Add logic to mute and pause all videos in the previous section when switching tabs
  - Ensure the existing video focus logic only unmutes the current video in the new section
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [x] 2. Implement tab close button functionality
  - [x] 2.1 Update HTML generation in Rust to include close button structure
    - Modify `generate_gallery()` function in `src/main.rs` to wrap tab label in a span
    - Add close button span with "×" character to each tab button
    - _Requirements: 1.1_

  - [x] 2.2 Add CSS styling for close buttons
    - Add flexbox layout styles to `.tab-button` for label and close button alignment
    - Style `.tab-close` with appropriate sizing, opacity, and hover effects
    - Add `.tab-button.last-tab .tab-close` rule to hide close button on last remaining tab
    - _Requirements: 1.1_

  - [x] 2.3 Implement closeTab JavaScript function
    - Create `closeTab(event, sectionIndex)` function that prevents event bubbling
    - Mute and pause all videos in the section being closed
    - Remove the tab button and gallery section from the DOM
    - Update `currentSection` if the closed tab was active (switch to nearest tab)
    - Rebuild the `currentIndices` array to match the new section count
    - Update remaining tab buttons' onclick handlers with corrected indices
    - Add/remove `last-tab` class based on remaining tab count
    - _Requirements: 1.2, 1.3, 1.4, 1.5, 1.6_

- [ ] 3. Implement drag-and-drop functionality for adding threads
  - [x] 3.1 Add drag-and-drop event handlers
    - Add `dragover` event listener to document to prevent default and show drop indicator
    - Add `dragleave` event listener to remove drop indicator when drag leaves window
    - Add `drop` event listener to handle dropped content
    - _Requirements: 3.1_

  - [x] 3.2 Create URL parsing and validation
    - Implement `parseThreadUrl(text)` function with regex to extract thread ID from 4chan URLs
    - Return thread ID if valid, null otherwise
    - _Requirements: 3.2, 3.3_

  - [x] 3.3 Implement 4chan thread fetching
    - Create `fetch4chanThread(threadId)` async function to call 4chan API
    - Parse JSON response to extract video URLs (filter for .webm and .mp4 extensions)
    - Handle network errors and return appropriate error messages
    - _Requirements: 3.3, 3.6_

  - [x] 3.4 Implement dynamic tab creation
    - Create `addThreadTab(threadId)` async function to orchestrate tab addition
    - Show loading indicator while fetching
    - Call `fetch4chanThread()` to get video URLs
    - Generate new tab button HTML with close handler
    - Generate new gallery section HTML with video elements
    - Append new elements to DOM
    - Update `currentIndices` array with new entry
    - Call `showSection()` to switch to the new tab
    - Hide loading indicator
    - Display error toast if fetch fails or thread is empty
    - _Requirements: 3.4, 3.5, 3.6, 3.7_

- [x] 4. Add visual feedback and error handling
  - [x] 4.1 Implement CSS for drag-and-drop indicator
    - Add `.drag-active` class styles to body with dashed outline
    - Add `::after` pseudo-element with "Drop 4chan thread URL here" message
    - _Requirements: 3.1_

  - [x] 4.2 Implement loading indicator
    - Add CSS for `.loading-overlay` and `.loading-spinner` with animation
    - Create `showLoading()` and `hideLoading()` helper functions
    - _Requirements: 3.7_

  - [x] 4.3 Implement error toast notifications
    - Add CSS for `.error-toast` with slide-in animation
    - Create `showError(message)` function that displays toast and auto-dismisses after 3 seconds
    - _Requirements: 3.6_

- [x] 5. Update state management for dynamic tabs
  - [x] 5.1 Modify initialization to support dynamic section count
    - Replace hardcoded `{{TOTAL_SECTIONS}}` with a dynamic `totalSections` variable
    - Initialize `totalSections` based on initial section count
    - Update `totalSections` when tabs are added or removed
    - _Requirements: 1.6, 3.4_

  - [x] 5.2 Update keyboard navigation to use dynamic section count
    - Modify `switchBoard()` function to use `totalSections` instead of hardcoded value
    - Ensure modulo arithmetic works correctly with dynamic count
    - _Requirements: 1.6_
