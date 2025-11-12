# Requirements Document

## Introduction

This document specifies requirements for enhancing the Faptopia media gallery application with two key features: the ability to close tabs via a close button, and the ability to add new 4chan /gif/ board tabs by dragging and dropping thread URLs directly onto the page.

## Glossary

- **Faptopia**: The media gallery web application that displays content from Reddit and 4chan in a tabbed interface
- **Tab**: A clickable button in the tab bar that switches between different content boards/sections
- **Gallery Section**: The content area associated with a tab, containing media items from a specific source
- **4chan Thread URL**: A URL in the format `https://boards.4chan.org/gif/thread/[thread_id]`
- **Thread ID**: The numeric identifier for a 4chan thread (e.g., 29776032)
- **Tab Bar**: The horizontal container at the top of the page displaying all tabs
- **Close Button**: An "X" icon on each tab that removes the tab and its associated content

## Requirements

### Requirement 1

**User Story:** As a user, I want to close tabs I'm no longer interested in, so that I can keep my workspace clean and focused on relevant content

#### Acceptance Criteria

1. THE Faptopia SHALL display a close button ("X" icon) on each tab in the tab bar
2. WHEN the user clicks the close button on a tab, THE Faptopia SHALL remove that tab from the tab bar
3. WHEN the user clicks the close button on a tab, THE Faptopia SHALL remove the associated gallery section from the DOM
4. WHEN the user closes the currently active tab, THE Faptopia SHALL automatically switch to the nearest remaining tab
5. IF only one tab remains, THEN THE Faptopia SHALL hide the close button on that tab to prevent closing the last tab
6. WHEN a tab is closed, THE Faptopia SHALL update the internal section indices to maintain correct navigation state

### Requirement 2

**User Story:** As a user, I want videos in background tabs to be muted when I switch tabs, so that I only hear audio from the tab I'm currently viewing

#### Acceptance Criteria

1. WHEN the user switches to a different tab, THE Faptopia SHALL mute all videos in the previously active tab
2. WHEN the user switches to a different tab, THE Faptopia SHALL pause all videos in the previously active tab
3. WHEN the user switches to a new tab, THE Faptopia SHALL only unmute and play the currently focused video in that tab
4. THE Faptopia SHALL ensure no audio plays from videos in hidden tabs at any time

### Requirement 3

**User Story:** As a user, I want to add new 4chan /gif/ boards by dragging thread URLs onto the page, so that I can quickly expand my gallery without using command-line tools

#### Acceptance Criteria

1. WHEN the user drags content over the Faptopia page, THE Faptopia SHALL display a visual indicator showing the drop zone is active
2. WHEN the user drops a 4chan thread URL onto the page, THE Faptopia SHALL extract the thread ID from the URL
3. IF the dropped content contains a valid 4chan /gif/ thread URL pattern, THEN THE Faptopia SHALL fetch media content from that thread
4. WHEN media content is successfully fetched, THE Faptopia SHALL create a new tab with the label "Thread [thread_id]"
5. WHEN a new tab is created via drag-and-drop, THE Faptopia SHALL automatically switch to display that new tab
6. IF the dropped content does not contain a valid 4chan thread URL, THEN THE Faptopia SHALL display an error message to the user
7. WHILE fetching thread content, THE Faptopia SHALL display a loading indicator to inform the user of the operation in progress
8. THE Faptopia SHALL support drag-and-drop from browser address bars, text selections, and links
