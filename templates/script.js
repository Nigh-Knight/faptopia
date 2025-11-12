// Index of the currently visible board (section)
let currentSection = 0;
// Per-board remembered focused item index, so switching boards restores position
let currentIndices = new Array({{TOTAL_SECTIONS}}).fill(0);

// showSection: reveals the board at `index`, hides others, updates active tab,
// and ensures the correct video in that board has focus/play state.
function showSection(index) {
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
    const next = (currentSection + 1) % {{TOTAL_SECTIONS}};
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

// Initialize the first section on load
showSection(0);
