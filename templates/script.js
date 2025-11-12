// Index of the currently visible board (section)
let currentSection = 0;
// Track the current number of sections dynamically
let totalSections = 0;
// Per-board remembered focused item index, so switching boards restores position
let currentIndices = [];

// closeTab: removes a tab and its associated section from the DOM
function closeTab(event, sectionIndex) {
    // Prevent event bubbling to the tab button's onclick handler
    event.stopPropagation();
    
    // Get all tabs and sections
    const allTabs = document.querySelectorAll('.tab-button');
    const allSections = document.querySelectorAll('.gallery-section');
    
    // Don't allow closing the last tab
    if (allTabs.length <= 1) {
        return;
    }
    
    // Mute and pause all videos in the section being closed
    const videosToClose = document.querySelectorAll(`#gallery-${sectionIndex} video`);
    videosToClose.forEach(video => {
        video.muted = true;
        video.pause();
    });
    
    // Remove the tab button and gallery section from the DOM
    allTabs[sectionIndex].remove();
    allSections[sectionIndex].remove();
    
    // Update currentSection if the closed tab was active
    if (currentSection === sectionIndex) {
        // Switch to the nearest tab (prefer previous, fallback to next)
        const newSection = sectionIndex > 0 ? sectionIndex - 1 : 0;
        currentSection = newSection;
    } else if (currentSection > sectionIndex) {
        // Adjust currentSection index if a tab before it was closed
        currentSection--;
    }
    
    // Rebuild the currentIndices array to match the new section count
    currentIndices.splice(sectionIndex, 1);
    
    // Update totalSections to reflect the removed tab
    totalSections--;
    
    // Update remaining tab buttons' onclick handlers with corrected indices
    const updatedTabs = document.querySelectorAll('.tab-button');
    updatedTabs.forEach((tab, newIndex) => {
        // Update the main tab onclick
        tab.onclick = () => showSection(newIndex);
        
        // Update the close button onclick
        const closeButton = tab.querySelector('.tab-close');
        if (closeButton) {
            closeButton.onclick = (e) => closeTab(e, newIndex);
        }
        
        // Update the section data attribute
        const sections = document.querySelectorAll('.gallery-section');
        if (sections[newIndex]) {
            sections[newIndex].setAttribute('data-section', newIndex);
        }
        
        // Update gallery id
        const gallery = sections[newIndex]?.querySelector('.gallery-container');
        if (gallery) {
            gallery.id = `gallery-${newIndex}`;
        }
    });
    
    // Add/remove last-tab class based on remaining tab count
    if (updatedTabs.length === 1) {
        updatedTabs[0].classList.add('last-tab');
    } else {
        updatedTabs.forEach(tab => tab.classList.remove('last-tab'));
    }
    
    // Show the new current section
    showSection(currentSection);
}

// showSection: reveals the board at `index`, hides others, updates active tab,
// and ensures the correct video in that board has focus/play state.
function showSection(index) {
    // Track the previous section index before switching
    const previousSection = currentSection;
    
    // Mute and pause all videos in the previous section when switching tabs
    if (previousSection !== index) {
        const previousVideos = document.querySelectorAll(`#gallery-${previousSection} video`);
        previousVideos.forEach(video => {
            video.muted = true;
            video.pause();
        });
    }
    
    // Hide all sections
    document.querySelectorAll('.gallery-section').forEach(section => 
        section.classList.add('hidden'));
    // Remove active class from all tabs
    document.querySelectorAll('.tab-button').forEach(tab => 
        tab.classList.remove('active'));
    
    // Show the requested section and mark the corresponding tab active
    document.querySelector(`[data-section="${index}"]`).classList.remove('hidden');
    document.querySelectorAll('.tab-button')[index].classList.add('active');
    currentSection = index;

    // Restore the remembered index inside that section and update video playback
    updateVideoFocus(currentSection, currentIndices[currentSection]);
    // Also ensure the section container is scrolled to the remembered item
    const gallery = document.getElementById(`gallery-${currentSection}`);
    gallery.scrollTo({ left: currentIndices[currentSection] * window.innerWidth, behavior: 'instant' });
}

// updateVideoFocus: play/unmute only the focused video in the section,
// pause and mute other videos to avoid multiple audio streams.
function updateVideoFocus(sectionIndex, itemIndex) {
    const videos = document.querySelectorAll(`#gallery-${sectionIndex} video`);
    videos.forEach((video, i) => {
        if (i === itemIndex) {
            video.muted = false;
            // Attempt to play; may be blocked by autoplay policies, so catch errors.
            video.play().catch(e => console.log('Autoplay prevented:', e));
        } else {
            // If the other video has ever played, pause and mute it.
            if (!video.paused || video.played.length > 0) {
                video.muted = true;
                try { video.pause(); } catch (e) { /* ignore */ }
            }
        }
    });
}

// scrollToIndex: scrolls the given section to item `index` and updates remembered index + focus.
function scrollToIndex(sectionIndex, index) {
    const gallery = document.getElementById(`gallery-${sectionIndex}`);
    if (!gallery) return;
    gallery.scrollTo({
        left: index * window.innerWidth,
        behavior: 'smooth'
    });
    currentIndices[sectionIndex] = index;
    updateVideoFocus(sectionIndex, index);
}

// handleNavigation: move forward/back within the current board, wrapping around.
function handleNavigation(direction) {
    const gallery = document.getElementById(`gallery-${currentSection}`);
    if (!gallery) return;
    const items = gallery.querySelectorAll('.gallery-item');
    const totalItems = items.length;
    if (totalItems === 0) return;

    const newIndex = direction === 'forward'
        ? (currentIndices[currentSection] + 1) % totalItems
        : (currentIndices[currentSection] - 1 + totalItems) % totalItems;
    
    scrollToIndex(currentSection, newIndex);
}

// switchBoard: cycle to the next board (wraps). Uses the remembered per-board index.
function switchBoard() {
    const next = (currentSection + 1) % totalSections;
    showSection(next);
}

// Keyboard shortcuts:
// X => next item, Z => previous item, C => next board
document.addEventListener('keydown', (e) => {
    // Ignore events when focused on inputs (safety)
    const active = document.activeElement;
    if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || active.isContentEditable)) return;

    if (e.key === 'x' || e.key === 'X') handleNavigation('forward');
    if (e.key === 'z' || e.key === 'Z') handleNavigation('back');
    if (e.key === 'c' || e.key === 'C') switchBoard();
});

// parseThreadUrl: extracts thread ID from 4chan URL
function parseThreadUrl(text) {
    const regex = /boards\.4chan\.org\/gif\/thread\/(\d+)/;
    const match = text.match(regex);
    return match ? match[1] : null;
}

// fetch4chanThread: fetches thread data from 4chan API and extracts video URLs
async function fetch4chanThread(threadId) {
    try {
        const response = await fetch(`https://a.4cdn.org/gif/thread/${threadId}.json`);
        
        if (!response.ok) {
            throw new Error(`Failed to fetch thread: ${response.status}`);
        }
        
        const data = await response.json();
        
        // Extract video URLs from posts
        const videoUrls = [];
        if (data.posts) {
            for (const post of data.posts) {
                if (post.ext && (post.ext === '.webm' || post.ext === '.mp4') && post.tim) {
                    const videoUrl = `https://i.4cdn.org/gif/${post.tim}${post.ext}`;
                    videoUrls.push(videoUrl);
                }
            }
        }
        
        return videoUrls;
    } catch (error) {
        console.error('Error fetching 4chan thread:', error);
        throw new Error('Failed to fetch thread data. Please check the URL and try again.');
    }
}

// showLoading: displays loading overlay
function showLoading() {
    let overlay = document.getElementById('loading-overlay');
    if (!overlay) {
        overlay = document.createElement('div');
        overlay.id = 'loading-overlay';
        overlay.className = 'loading-overlay';
        overlay.innerHTML = '<div class="loading-spinner"></div>';
        document.body.appendChild(overlay);
    }
    overlay.style.display = 'flex';
}

// hideLoading: hides loading overlay
function hideLoading() {
    const overlay = document.getElementById('loading-overlay');
    if (overlay) {
        overlay.style.display = 'none';
    }
}

// showError: displays error toast notification
function showError(message) {
    const toast = document.createElement('div');
    toast.className = 'error-toast';
    toast.textContent = message;
    document.body.appendChild(toast);
    
    setTimeout(() => {
        toast.remove();
    }, 3000);
}

// addThreadTab: orchestrates adding a new 4chan thread tab
async function addThreadTab(threadId) {
    showLoading();
    
    try {
        // Fetch video URLs from 4chan
        const videoUrls = await fetch4chanThread(threadId);
        
        if (videoUrls.length === 0) {
            throw new Error('No videos found in this thread.');
        }
        
        // Get current tab and section counts
        const allTabs = document.querySelectorAll('.tab-button');
        const allSections = document.querySelectorAll('.gallery-section');
        const newIndex = allTabs.length;
        
        // Create new tab button
        const tabButton = document.createElement('button');
        tabButton.className = 'tab-button';
        tabButton.onclick = () => showSection(newIndex);
        tabButton.innerHTML = `
            <span class="tab-label">Thread ${threadId}</span>
            <span class="tab-close" onclick="closeTab(event, ${newIndex})">×</span>
        `;
        
        // Remove last-tab class from existing tabs
        allTabs.forEach(tab => tab.classList.remove('last-tab'));
        
        // Append tab to tab bar
        document.querySelector('.tabs').appendChild(tabButton);
        
        // Create new gallery section
        const section = document.createElement('div');
        section.className = 'gallery-section hidden';
        section.setAttribute('data-section', newIndex);
        
        const galleryContainer = document.createElement('div');
        galleryContainer.className = 'gallery-container';
        galleryContainer.id = `gallery-${newIndex}`;
        
        // Create video elements
        videoUrls.forEach(url => {
            const item = document.createElement('div');
            item.className = 'gallery-item';
            
            const video = document.createElement('video');
            video.src = url;
            video.loop = true;
            video.muted = true;
            video.preload = 'none';
            
            item.appendChild(video);
            galleryContainer.appendChild(item);
        });
        
        section.appendChild(galleryContainer);
        
        // Append section to body
        document.body.appendChild(section);
        
        // Update currentIndices array
        currentIndices.push(0);
        
        // Update totalSections to reflect the added tab
        totalSections++;
        
        // Switch to the new tab
        showSection(newIndex);
        
        hideLoading();
    } catch (error) {
        hideLoading();
        showError(error.message);
    }
}

// Drag-and-drop event handlers for adding 4chan threads
document.addEventListener('dragover', handleDragOver);
document.addEventListener('dragleave', handleDragLeave);
document.addEventListener('drop', handleDrop);

function handleDragOver(event) {
    event.preventDefault();
    document.body.classList.add('drag-active');
}

function handleDragLeave(event) {
    // Only remove if leaving the window entirely
    if (event.clientX === 0 && event.clientY === 0) {
        document.body.classList.remove('drag-active');
    }
}

function handleDrop(event) {
    event.preventDefault();
    document.body.classList.remove('drag-active');
    
    // Extract text from dataTransfer
    const text = event.dataTransfer.getData('text');
    
    // Parse 4chan thread URL
    const threadId = parseThreadUrl(text);
    
    if (threadId) {
        addThreadTab(threadId);
    } else {
        showError('Invalid 4chan thread URL. Please drop a valid /gif/ thread URL.');
    }
}

// Initialize totalSections based on initial section count
totalSections = document.querySelectorAll('.gallery-section').length;
currentIndices = new Array(totalSections).fill(0);

// Initialize the first section on load
showSection(0);
