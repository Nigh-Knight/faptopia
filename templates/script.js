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

// Initialize totalSections based on initial section count
totalSections = document.querySelectorAll('.gallery-section').length;
currentIndices = new Array(totalSections).fill(0);

// Initialize the first section on load
showSection(0);
