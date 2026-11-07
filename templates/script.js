// Platform identifier injected at build time
const PLATFORM = '{{PLATFORM}}';
const PREFETCH_LIMIT = 8;

// Index of the currently visible board (section)
let currentSection = 0;
// Track the current number of sections dynamically
let totalSections = 0;
// Per-board remembered focused item index, so switching boards restores position
let currentIndices = [];
// Auto-scroll mode: when enabled, videos advance automatically on end
let autoScroll = false;

// Undo stack for tab close
let _undoStack = null;
let _undoTimer = null;

function showUndoToast(text) {
    const toast = document.getElementById('undo-toast');
    const toastText = document.getElementById('undo-toast-text');
    const toastBtn = document.getElementById('undo-toast-btn');
    if (!toast || !toastBtn) return;
    toastText.textContent = text || 'Tab closed';
    toastBtn.onclick = function() {
        toast.classList.remove('show');
        if (_undoStack && _undoStack.undo) {
            _undoStack.undo();
            _undoStack = null;
        }
    };
    toast.classList.add('show');
    if (_undoTimer) clearTimeout(_undoTimer);
    _undoTimer = setTimeout(() => {
        toast.classList.remove('show');
        _undoStack = null;
    }, 4000);
}

// closeTab: removes a tab and its associated section from the DOM, with undo
function closeTab(event, sectionIndex) {
    // Prevent event bubbling
    event.stopPropagation();

    // Board clones sit at both ends of the track and would shift every index
    const cloneBoards = isMobile;
    if (cloneBoards) teardownBoardWrap();

    const allTabs = document.querySelectorAll('.tab-button');
    const allSections = document.querySelectorAll('.gallery-section');
    
    if (allTabs.length <= 1) return;
    
    // Save undo data
    const closedTabHTML = allTabs[sectionIndex].outerHTML;
    const closedSectionHTML = allSections[sectionIndex].outerHTML;
    const closedLabel = allTabs[sectionIndex].querySelector('.tab-label')?.textContent || 'Tab';
    const closedIndices = [...currentIndices];
    const closedCurrentSection = currentSection;
    const closedTotalSections = totalSections;
    
    // Mute and pause videos in the section being closed
    document.querySelectorAll(`#gallery-${sectionIndex} video`).forEach(v => {
        v.muted = true;
        v.pause();
    });
    
    // Remove the tab and section from the DOM
    allTabs[sectionIndex].remove();
    allSections[sectionIndex].remove();
    
    // Update state
    if (currentSection === sectionIndex) {
        currentSection = sectionIndex > 0 ? sectionIndex - 1 : 0;
    } else if (currentSection > sectionIndex) {
        currentSection--;
    }
    currentIndices.splice(sectionIndex, 1);
    totalSections--;
    
    // Re-index remaining tabs
    reindexTabsAndSections();
    showSection(currentSection);
    rebuildAfterTabChange();
    
    // Register undo for the toast
    _undoStack = {
        undo: function() {
            if (cloneBoards) teardownBoardWrap();
            const tabsContainer = document.querySelector('.tabs');
            const sectionsContainer = document.querySelector('.gallery-section')?.parentElement;
            if (!tabsContainer || !sectionsContainer) return;
            
            const tmp = document.createElement('div');
            tmp.innerHTML = closedTabHTML;
            const restoredTab = tmp.firstElementChild;
            const existingTabs = tabsContainer.querySelectorAll('.tab-button, .gallery-toggle');
            let insertBefore = null;
            let countBefore = 0;
            for (const el of existingTabs) {
                if (el.classList.contains('gallery-toggle')) { insertBefore = el; break; }
                countBefore++;
            }
            tabsContainer.insertBefore(restoredTab, (countBefore <= sectionIndex && insertBefore) ? insertBefore : existingTabs[sectionIndex] || null);
            
            tmp.innerHTML = closedSectionHTML;
            const restoredSection = tmp.firstElementChild;
            sectionsContainer.insertBefore(restoredSection, sectionsContainer.querySelectorAll('.gallery-section')[sectionIndex] || null);
            
            currentIndices = closedIndices;
            currentSection = closedCurrentSection;
            totalSections = closedTotalSections;
            
            reindexTabsAndSections();
            rebuildAfterTabChange();
            showSection(currentSection);
            updateProgress();
        }
    };
    
    showUndoToast('Closed ' + closedLabel);
}

function reindexTabsAndSections() {
    const tabs = document.querySelectorAll('.tab-button');
    const sections = document.querySelectorAll('.gallery-section');
    tabs.forEach((tab, i) => {
        tab.onclick = () => showSection(i);
        const cb = tab.querySelector('.tab-close');
        if (cb) cb.onclick = (e) => closeTab(e, i);
        if (sections[i]) {
            sections[i].setAttribute('data-section', i);
            const g = sections[i].querySelector('.gallery-em');
            if (g) g.id = `gallery-${i}`;
        }
    });
    if (tabs.length === 1) tabs[0].classList.add('last-tab');
    else tabs.forEach(t => t.classList.remove('last-tab'));
}

// Removed: preloadBoardVideo, preloadVideoOffsets — eager preloading overwhelmed
// the single HTTP/2 connection to the CDN, causing 60s stalls and protocol errors.
// Videos load lazily (preload="none") except for paced one-at-a-time neighbour
// warming (warmNeighbors) and auto-retry on CDN throttling.

function collectWarmVideoIds(limit = PREFETCH_LIMIT) {
    const ids = [];
    const seen = new Set();
    const pushId = (id) => {
        if (!id || seen.has(id)) return;
        seen.add(id);
        ids.push(id);
    };

    const currentGallery = galleryOf(currentSection);
    if (currentGallery) {
        const items = Array.from(currentGallery.querySelectorAll('.gallery-item'))
            .filter(el => !el.classList.contains('is-clone'));
        const total = items.length;
        const center = currentIndices[currentSection] || 0;
        [0, 1, -1, 2, -2, 3, -3].forEach((offset) => {
            if (!total) return;
            const item = items[(center + offset + total) % total];
            const video = item && item.querySelector('video[data-gif-id]');
            pushId(video && video.dataset.gifId);
        });
    }

    for (let offset = 1; offset < totalSections && ids.length < limit; offset++) {
        const gallery = galleryOf((currentSection + offset) % totalSections);
        if (!gallery) continue;
        const video = gallery.querySelector('.gallery-item video[data-gif-id]');
        pushId(video && video.dataset.gifId);
    }

    return ids.slice(0, limit);
}

function warmPwaVideos() {
    const ids = collectWarmVideoIds();
    if (!ids.length) return;

    navigator.serviceWorker.ready
        .then(reg => {
            const worker = navigator.serviceWorker.controller || reg.active;
            if (!worker) return;
            worker.postMessage({ type: 'video-prefetch', ids });
        })
        .catch(() => {});
}

// ===== BOARD AXIS (mobile) =====
// The board track is a native horizontal scroll-snap container: one board per
// screen with mandatory snapping, so a swipe always settles exactly on a board
// edge. It used to be driven by Embla's transforms, which stopped a few pixels
// short of the snap and left a sliver of the neighbouring board on screen.
let boardSettleTimer = 0;
// Where the viewport is being sent programmatically and which board that is
// (null target = the user is dragging). Keeps the settle handler from fighting
// a smooth scroll and lets galleryOf() resolve the arriving copy.
let boardScrollLeft = null;
let boardScrollTarget = null;
let boardScrollTries = 0;

function boardViewport() { return document.getElementById('board-viewport'); }

function boardWidth() {
    const vp = boardViewport();
    return (vp && vp.clientWidth) || window.innerWidth;
}

// ===== BOARD WRAP =====
// The first and last board are cloned onto the opposite ends of the track, so
// swiping off either end continues into the other end instead of dead-stopping.
// Board indices stay real: a clone carries data-real, and every lookup that
// cares about what is on screen resolves through galleryOf().
function boardSections() { return Array.from(document.querySelectorAll('.gallery-section')); }
function realSections() { return boardSections().filter(s => !s.classList.contains('is-clone')); }

function realBoard(el, slot) {
    const r = el && el.dataset ? el.dataset.real : null;
    return r == null ? slot : parseInt(r, 10);
}

// galleryOf: the feed element for board `index` — the on-screen copy when the
// board is cloned, otherwise the canonical one.
function galleryOf(index) {
    const matches = boardSections().filter((s, i) => realBoard(s, i) === index);
    if (!matches.length) return document.getElementById(`gallery-${index}`);
    if (matches.length === 1) return matches[0].querySelector('.gallery-em');
    const vp = boardViewport();
    if (!vp) return matches[0].querySelector('.gallery-em');
    const mid = (boardScrollLeft !== null ? boardScrollLeft : vp.scrollLeft)
        + vp.clientWidth / 2;
    let best = matches[0], bestDist = Infinity;
    for (const s of matches) {
        const r = s.getBoundingClientRect();
        const d = Math.abs(r.left + r.width / 2 - mid);
        if (d < bestDist) { bestDist = d; best = s; }
    }
    return best.querySelector('.gallery-em');
}

function boardOffset(el) {
    const vp = boardViewport();
    if (!vp) return 0;
    return el.getBoundingClientRect().left - vp.getBoundingClientRect().left + vp.scrollLeft;
}

function setupBoardWrap() {
    const track = document.getElementById('board-track');
    const secs = boardSections();
    if (!isMobile || !track || secs.length < 2 || track.querySelector('.gallery-section.is-clone')) return;
    secs.forEach((el, i) => { el.dataset.real = i; });
    const clone = (el) => {
        const c = el.cloneNode(true);
        c.classList.add('is-clone');
        c.dataset.real = el.dataset.real;
        const g = c.querySelector('.gallery-em');
        if (g) g.id = `gallery-${el.dataset.real}-c`;
        return c;
    };
    track.insertBefore(clone(secs[secs.length - 1]), secs[0]);
    track.appendChild(clone(secs[0]));
}

function teardownBoardWrap() {
    boardSections().filter(s => s.classList.contains('is-clone')).forEach(s => s.remove());
    boardSections().forEach(s => { delete s.dataset.real; });
}

// reanchorBoard: the user settled on a cloned board. Carry the feed position
// over to the real board and jump the viewport there in one instant step.
function reanchorBoard(realIndex, cloneSection) {
    const vp = boardViewport();
    const target = realSections()[realIndex];
    if (!vp || !target) return false;
    const from = cloneSection.querySelector('.gallery-em');
    const to = target.querySelector('.gallery-em');
    if (from && to) to.scrollTop = from.scrollTop;
    vp.scrollLeft = boardOffset(target);
    finishSectionSwitch(realIndex);
    return true;
}

// onBoardScroll: the viewport has no scrollend everywhere, so settle by timer.
// A drag settles as soon as events stop; a programmatic scroll waits until the
// viewport has actually arrived before the board is committed.
function onBoardScroll() {
    if (!isMobile) return;
    const vp = boardViewport();
    if (!vp) return;
    clearTimeout(boardSettleTimer);
    boardSettleTimer = setTimeout(() => {
        const w = boardWidth();
        if (boardScrollTarget !== null) {
            const want = boardScrollLeft;
            if (Math.abs(vp.scrollLeft - want) > 2) {
                // Still travelling. scroll-snap-stop can stop a smooth scroll at
                // an intermediate board, so bail out to a jump if it never lands.
                if (++boardScrollTries > 20) vp.scrollLeft = want;
                else { onBoardScroll(); return; }
            }
            const idx = boardScrollTarget;
            boardScrollTarget = null;
            boardScrollLeft = null;
            boardScrollTries = 0;
            if (idx !== currentSection) finishSectionSwitch(idx);
            return;
        }
        const secs = boardSections();
        const slot = Math.max(0, Math.min(secs.length - 1, Math.round(vp.scrollLeft / w)));
        const sec = secs[slot];
        if (!sec) return;
        if (sec.classList.contains('is-clone')) {
            reanchorBoard(realBoard(sec, slot), sec);
            return;
        }
        const idx = realBoard(sec, slot);
        if (idx !== currentSection) finishSectionSwitch(idx);
    }, 90);
}

function initBoardAxis() {
    const vp = boardViewport();
    if (!vp) return;
    vp.addEventListener('scroll', onBoardScroll, { passive: true });
}

// showSection: reveals the board at `index`. On mobile it drives the board
// viewport; on desktop it toggles .hidden on sections.
function showSection(index, opts) {
    if (isMobile) {
        const vp = boardViewport();
        const target = realSections()[index];
        if (vp && target) {
            const off = boardOffset(target);
            if (Math.abs(vp.scrollLeft - off) > 2) {
                boardScrollTarget = index;
                boardScrollLeft = off;
                boardScrollTries = 0;
                vp.scrollTo({
                    left: off,
                    behavior: (opts && opts.instant) ? 'instant' : 'smooth',
                });
            } else {
                boardScrollTarget = null;
                boardScrollLeft = null;
            }
        }
        finishSectionSwitch(index);
        return;
    }
    // Desktop: instant switch to .hidden, restore per-board horizontal position
    document.querySelectorAll('.gallery-section').forEach(s => s.classList.add('hidden'));
    const sec = document.querySelector(`[data-section="${index}"]`);
    if (sec) sec.classList.remove('hidden');
    const gallery = galleryOf(index);
    if (gallery) gallery.scrollTo({ left: (currentIndices[index] || 0) * window.innerWidth, behavior: 'instant' });
    finishSectionSwitch(index);
}

// finishSectionSwitch: single source of truth for "board X is now active".
// Pauses every video, focuses the remembered one in X, and syncs all chrome
// (tabs, pill, dropdown, progress). On mobile the board's native feed scroller
// is jumped to the remembered item (native scrollTop persists across switches).
function finishSectionSwitch(index) {
    currentSection = index;
    Stats.switchTab(index);
    clearWarmQueue();

    // Pause everything; updateVideoFocus below re-engages the focused video
    document.querySelectorAll('video').forEach(v => { v.muted = true; v.pause(); });

    // Tab bar (desktop chrome — hidden on mobile)
    document.querySelectorAll('.tab-button').forEach(tab => tab.classList.remove('active'));
    const tb = document.querySelectorAll('.tab-button')[index];
    if (tb) tb.classList.add('active');

    // Mobile feed: jump this board's native scroller to the remembered item.
    if (isMobile) {
        scrollFeedItemTo(index, currentIndices[index] || 0, 'instant');
    }
    updateProgress();
    updateSwitcherPill();
    updateMenuSelection();
    updateVideoFocus(index, currentIndices[index]);
}

function rebuildAfterTabChange() {
    if (!isMobile) return;
    buildSwitcherMenu();
    // The tab set changed: re-establish the wraps and their handlers, then
    // re-anchor — a removed board shifts the track and the browser clamps
    // scrollLeft to a position that may no longer match currentSection.
    for (let i = 0; i < totalSections; i++) {
        const g = document.getElementById(`gallery-${i}`);
        setupFeedWrap(g);
        attachVideoEndedListeners(g, i);
        attachVideoResilience(g, i);
    }
    setupBoardWrap();
    boardSections()
        .filter(s => s.classList.contains('is-clone'))
        .forEach(s => {
            const g = s.querySelector('.gallery-em');
            const idx = realBoard(s, boardSections().indexOf(s));
            attachVideoEndedListeners(g, idx);
            attachVideoResilience(g, idx);
        });
    showSection(currentSection, { instant: true });
}

// ===== FEED ITEM LOOKUP =====
// Board feeds wrap around: a page of items is cloned onto both ends of every
// feed so scrolling past the first item continues into the last (and the other
// way round) with no seam. Clones carry .is-clone and the data-real index of
// the item they mirror, so every real index keeps exactly one canonical
// element and one nearest element.
const WRAP_ITEMS = 6;

function itemsOf(gallery) { return gallery.querySelectorAll('.gallery-item'); }

function realCount(gallery) {
    if (!gallery) return 0;
    const clones = gallery.querySelectorAll('.gallery-item.is-clone');
    return itemsOf(gallery).length - clones.length;
}

function realIndex(el, slot) {
    const r = el && el.dataset ? el.dataset.real : null;
    return r == null ? slot : parseInt(r, 10);
}

// measure: an item's box, falling back to its video (heights can live on either).
function measure(el) {
    const own = el.getBoundingClientRect ? el.getBoundingClientRect() : null;
    if (own && own.height) return own;
    const v = el.querySelector ? el.querySelector('video') : null;
    return (v && v.getBoundingClientRect) ? v.getBoundingClientRect() : null;
}

// itemByReal: the canonical (non-clone) element for a real index.
function itemByReal(gallery, realIndex_) {
    if (!gallery) return null;
    const sel = '.gallery-item[data-real="' + realIndex_ + '"]';
    const hit = gallery.querySelector ? gallery.querySelector(sel + ':not(.is-clone)') : null;
    if (hit) return hit;
    const any = gallery.querySelector ? gallery.querySelector(sel) : null;
    return any || itemsOf(gallery)[realIndex_] || null;
}

// nearestItemByReal: the copy of a real index closest to the current scroll
// position, so wrap navigation lands one item away instead of jumping the board.
function nearestItemByReal(gallery, realIndex_) {
    if (!gallery) return null;
    const matches = gallery.querySelector
        ? Array.from(gallery.querySelectorAll('.gallery-item[data-real="' + realIndex_ + '"]'))
        : [];
    if (!matches.length) return itemsOf(gallery)[realIndex_] || null;
    const at = gallery.scrollTop || 0;
    let best = matches[0], bestDist = Infinity;
    for (const m of matches) {
        const d = Math.abs((m.offsetTop || 0) - at);
        if (d < bestDist) { bestDist = d; best = m; }
    }
    return best;
}

// setupFeedWrap: clone the first and last page of a feed onto the opposite ends.
function setupFeedWrap(gallery) {
    if (!isMobile || !gallery || gallery.querySelector('.is-clone')) return;
    const items = Array.from(itemsOf(gallery));
    if (items.length < 2) return;
    const wrap = Math.min(WRAP_ITEMS, items.length);
    items.forEach((el, i) => { el.dataset.real = i; });

    const track = gallery.querySelector('.gallery-em__track') || gallery;
    const clone = (el) => {
        const c = el.cloneNode(true);
        c.classList.add('is-clone');
        c.dataset.real = el.dataset.real;
        c.removeAttribute('data-index');
        c.querySelectorAll('video').forEach(v => {
            v.removeAttribute('autoplay');
            v.setAttribute('preload', 'none');
        });
        return c;
    };
    // Head clones are the tail items, in order, before the first real item
    items.slice(-wrap).forEach(el => track.insertBefore(clone(el), items[0]));
    // Tail clones are the head items, in order, after the last real item
    items.slice(0, wrap).forEach(el => track.appendChild(clone(el)));
}

// reanchorFeed: the user settled on a clone, so snap the identical real item to
// the same screen position in a single instant jump. Nothing is animated, so it
// reads as one continuous feed rather than a wrap.
function reanchorFeed(sectionIndex) {
    const gallery = galleryOf(sectionIndex);
    if (!gallery) return false;
    const items = Array.from(itemsOf(gallery));
    if (!items.length) return false;
    const rect = gallery.getBoundingClientRect();
    const middle = rect.top + rect.height / 2;
    // Find the clone actually on screen and nearest the viewport middle
    let clone = null, bestDist = Infinity;
    for (const el of items) {
        if (!el.classList.contains('is-clone')) continue;
        const r = el.getBoundingClientRect();
        if (r.bottom <= rect.top || r.top >= rect.bottom) continue;
        const d = Math.abs((r.top + r.height / 2) - middle);
        if (d < bestDist) { bestDist = d; clone = el; }
    }
    if (!clone) return false;
    const real = itemByReal(gallery, clone.dataset.real);
    if (!real) return false;
    const delta = (real.offsetTop || 0) - (clone.offsetTop || 0);
    if (!delta) return false;
    gallery.scrollTop += delta;
    updateVideoFocus(sectionIndex, currentIndices[sectionIndex]);
    return true;
}

// updateVideoFocus: play/unmute only the focused video in the section,
// pause and mute other videos to avoid multiple audio streams.
function updateVideoFocus(sectionIndex, itemIndex) {
    const gallery = galleryOf(sectionIndex);
    const target = gallery ? (itemByReal(gallery, itemIndex) || {}) : {};
    const targetVideo = target.querySelector ? target.querySelector('video') : null;
    const videos = document.querySelectorAll(`#gallery-${sectionIndex} video`);
    videos.forEach(video => {
        if (video === targetVideo) {
            // Play muted first so autoplay policies allow it; unmute only once
            // playback is actually running (avoids the "autoplay prevented" error
            // on iOS/Android where unmuted programmatic play is blocked).
            video.muted = true;
            video.play().then(() => {
                video.muted = false;
            }).catch(() => {
                // Still try to unmute for the next user interaction
                video.muted = false;
            });
        } else {
            // If the other video has ever played, pause and mute it.
            if (!video.paused || video.played.length > 0) {
                video.muted = true;
                try { video.pause(); } catch (e) { /* ignore */ }
            }
        }
    });
}

// toggleAutoScroll: enable/disable auto-advance on video end, update nav hint and show alert.
function toggleAutoScroll() {
    autoScroll = !autoScroll;
    const navHint = document.getElementById('nav-hint');
    const alert = document.getElementById('autoscroll-alert');
    if (navHint) navHint.classList.toggle('auto-scroll-active', autoScroll);
    if (alert) {
        alert.textContent = autoScroll ? 'Auto-scroll activated' : 'Auto-scroll deactivated';
        alert.classList.add('show');
        clearTimeout(alert._hideTimer);
        alert._hideTimer = setTimeout(() => alert.classList.remove('show'), 1500);
    }
}

// ===== VIDEO RESILIENCE =====
// RedGifs' CDN throttles per-IP (fake 404s + infinite stalls). Auto-retry with
// exponential backoff, TIME-BASED downgrade to SD (throttled fetches hang
// silently, so waiting out N retries wastes ~50s — instead we give HD a short
// grace period, then drop to the much smaller SD file), and burst
// circuit-breaking so a heavy session can't dead-end the gallery.
const STALL_TIMEOUT_MS = 12000;
const HD_GRACE_MS = 8000;      // no data within this -> downgrade HD to SD
const RETRY_BASE_MS = 2000;
const RETRY_MAX_MS = 30000;
const HD_ATTEMPTS = 2;         // hard-error retries on HD before downgrade
const SD_ATTEMPTS = 3;
const THROTTLE_WINDOW_MS = 30000;
const THROTTLE_TRIGGER = 3;
const THROTTLE_BACKOFF_MULT = 3;
const WARM_FRONTIER = 3;       // keep warming this many videos ahead while lingering

const videoState = new WeakMap();
let burstFailures = [];
let throttleMode = false;

function getVideoState(video) {
    let s = videoState.get(video);
    if (!s) {
        s = { attempts: 0, sdTried: false, playedOnce: false, overlayShown: false, originalSrc: null, graceTimer: null };
        videoState.set(video, s);
    }
    return s;
}

function clearStallTimers(video) {
    clearTimeout(video._stallTimer);
    const st = videoState.get(video);
    if (st) clearTimeout(st.graceTimer);
}

function recordFailure() {
    const now = Date.now();
    burstFailures = burstFailures.filter(t => now - t < THROTTLE_WINDOW_MS);
    burstFailures.push(now);
    throttleMode = burstFailures.length >= THROTTLE_TRIGGER;
    if (throttleMode) showThrottleToast();
    return throttleMode;
}

function recordSuccess() {
    const now = Date.now();
    burstFailures = burstFailures.filter(t => now - t < THROTTLE_WINDOW_MS);
    if (burstFailures.length === 0 && throttleMode) {
        throttleMode = false;
        hideThrottleToast();
    }
}

function backoffDelay(attempt) {
    let delay = RETRY_BASE_MS * Math.pow(2, attempt - 1);
    if (throttleMode) delay *= THROTTLE_BACKOFF_MULT;
    const jitter = 0.7 + Math.random() * 0.6;
    return Math.min(Math.round(delay * jitter), RETRY_MAX_MS);
}

function isProxied() {
    // --serve / deployed galleries resolve videos through a local/remote proxy
    const src = document.querySelector('.gallery-item video source');
    return !!src && src.src.startsWith(location.origin + '/video/');
}

function showThrottleToast() {
    const el = document.getElementById('throttle-toast');
    if (!el) return;
    el.textContent = 'RedGifs is throttling this network - retrying slower';
    el.classList.add('show');
}

function hideThrottleToast() {
    const el = document.getElementById('throttle-toast');
    if (el) el.classList.remove('show');
}

function handleVideoFailure(video, item) {
    if (!video) return;
    clearStallTimers(video);
    const state = getVideoState(video);
    if (!state.originalSrc) {
        const s = video.querySelector('source');
        state.originalSrc = s ? s.getAttribute('src') : null;
    }
    recordFailure();

    const maxAttempts = state.sdTried ? SD_ATTEMPTS : HD_ATTEMPTS;
    if (state.attempts < maxAttempts) {
        state.attempts++;
        setTimeout(() => retryPlayback(video, item), backoffDelay(state.attempts));
        return;
    }

    // Current URL exhausted. Drop to SD once. Skipped in proxied mode:
    // the proxy already falls back to SD server-side.
    if (switchToSd(video, item)) return;

    showVideoOverlay(video, item);
}

// Downgrade a video from HD to its baked-in SD URL and restart playback.
// No-op if there is no SD URL or the downgrade already happened.
function switchToSd(video, item) {
    const state = getVideoState(video);
    const sd = video.dataset.sdUrl;
    const source = video.querySelector('source');
    if (!sd || !source || state.sdTried || isProxied()) return false;
    if (!state.originalSrc) state.originalSrc = source.getAttribute('src');
    state.sdTried = true;
    state.attempts = 0;
    source.setAttribute('src', sd);
    retryPlayback(video, item);
    return true;
}

function retryPlayback(video, item) {
    const state = getVideoState(video);
    if (state.overlayShown) {
        const fb = item && item.querySelector('.video-error');
        if (fb) fb.remove();
        state.overlayShown = false;
        // Manual retry: restore the original HD source for a clean slate;
        // the auto-downgrade re-engages if the network is still bad.
        if (state.sdTried && state.originalSrc) {
            const source = video.querySelector('source');
            if (source) source.setAttribute('src', state.originalSrc);
            state.sdTried = false;
            state.attempts = 0;
        }
    }
    try {
        video.load();
        video.muted = true;
        video.play().then(() => {
            // Only unmute if this video is still the focused one, so retries
            // firing after the user navigated away stay silent.
            const gi = video.closest('.gallery-item');
            const gallery = video.closest('.gallery-em');
            if (gi && gallery) {
                const sectionIndex = parseInt(gallery.id.replace('gallery-', ''));
        const items = gallery.querySelectorAll('.gallery-item');
        const slot = Array.from(items).indexOf(gi);
                if (sectionIndex === currentSection && realIndex(gi, slot) === currentIndices[currentSection]) {
                    video.muted = false;
                }
            }
        }).catch(() => {});
    } catch (e) { /* ignore */ }
}

function showVideoOverlay(video, item) {
    const state = getVideoState(video);
    if (state.overlayShown || !item || item.querySelector('.video-error')) return;
    state.overlayShown = true;
    const fb = document.createElement('div');
    fb.className = 'video-error';
    fb.innerHTML = '<strong>Video unavailable</strong><button class="video-retry-btn">Retry</button>';
    item.appendChild(fb);
    fb.querySelector('.video-retry-btn').addEventListener('click', function(e) {
        e.stopPropagation();
        retryPlayback(video, item);
    });
}

// attachVideoResilience: error auto-retry, cold-start stall watchdog, and
// neighbor-warming hooks for every video in a section.
function attachVideoResilience(galleryOrIndex, sectionIndex) {
    const secIndex = typeof galleryOrIndex === 'number' ? galleryOrIndex : sectionIndex;
    const gallery = (galleryOrIndex && galleryOrIndex.querySelectorAll)
        ? galleryOrIndex
        : document.getElementById(`gallery-${secIndex}`);
    if (!gallery) return;
    const items = gallery.querySelectorAll('.gallery-item');
    items.forEach((item, i) => {
        const video = item.querySelector('video');
        if (!video) return;

        // Hard failures (404 etc.)
        video.removeEventListener('error', video._errorHandler);
        video._errorHandler = function() { handleVideoFailure(video, item); };
        video.addEventListener('error', video._errorHandler);

        // Playback watchdog: two tiers. A throttled fetch can hang forever
        // without firing 'error', so the GRACE tier drops a stuck HD video to
        // SD as soon as it's clearly not coming (much faster than grinding
        // through retries); the STALL tier handles everything else.
        video.removeEventListener('play', video._stallStart);
        video._stallStart = function() {
            clearStallTimers(video);
            const st = getVideoState(video);
            // Throttle mode: skip HD entirely for videos that have never
            // played — SD is far more likely to slip through the pipe.
            if (throttleMode && !st.sdTried && !st.playedOnce && video.dataset.sdUrl && !isProxied()) {
                switchToSd(video, item);
                return;
            }
            // HD grace: no data within HD_GRACE_MS -> downgrade to SD
            if (!st.sdTried && video.dataset.sdUrl && !isProxied()) {
                st.graceTimer = setTimeout(() => {
                    if (!video.paused && video.readyState < 2 && !video.error && !video.ended) {
                        switchToSd(video, item);
                    }
                }, HD_GRACE_MS);
            }
            // Full stall watchdog: never delivered (and can't downgrade)
            video._stallTimer = setTimeout(() => {
                if (!video.paused && video.readyState < 2 && !video.error && !video.ended) {
                    handleVideoFailure(video, item);
                }
            }, STALL_TIMEOUT_MS);
        };
        video.addEventListener('play', video._stallStart);

        video.removeEventListener('playing', video._playingHandler);
        video._playingHandler = function() {
            clearStallTimers(video);
            const st = getVideoState(video);
            // A successful HD play restores full HD behavior; a successful SD
            // play stays on SD (fast) until the user manually retries HD.
            const src = video.querySelector('source');
            const onSd = !!(st.sdTried && src && video.dataset.sdUrl
                && src.getAttribute('src') === video.dataset.sdUrl);
            if (!onSd) st.sdTried = false;
            st.attempts = 0;
            st.playedOnce = true;
            recordSuccess();
            if (currentSection === secIndex && currentIndices[secIndex] === i) {
                warmNeighbors(secIndex, i);
            }
        };
        video.addEventListener('playing', video._playingHandler);

        video.removeEventListener('pause', video._pauseHandler);
        video._pauseHandler = () => clearStallTimers(video);
        video.addEventListener('pause', video._pauseHandler);
    });
}

// ===== PACED NEIGHBOR PRELOADING =====
// Warm the videos beside the current one (both directions, wrapping around so
// reverse navigation — first -> last — is just as instant as forward).
// Direction-aware: the likely-next video warms first. While the user lingers
// on a video, the frontier extends a few videos ahead.
// Strictly ONE fetch at a time: the old eager preloader saturated the single
// HTTP/2 connection to the CDN and caused 60s stalls.
const warmQueue = [];
let warmInFlight = false;
let navDirection = 'forward';
let warmFocus = null;
let warmDepth = 0;

function getVideoAt(sectionIndex, itemIndex) {
    const gallery = galleryOf(sectionIndex);
    if (!gallery) return null;
    const item = itemByReal(gallery, itemIndex);
    return item ? item.querySelector('video') : null;
}

function warmNeighbors(sectionIndex, itemIndex) {
    if (throttleMode || document.hidden) return;
    const gallery = galleryOf(sectionIndex);
    if (!gallery) return;
    const total = realCount(gallery);
    if (total < 2) return;
    const key = sectionIndex + '-' + itemIndex;
    if (warmFocus !== key) { warmFocus = key; warmDepth = 0; }
    const prev = (itemIndex - 1 + total) % total;
    const next = (itemIndex + 1) % total;
    // Warm the video the user is most likely to go to first
    if (navDirection === 'back') {
        enqueueWarm(sectionIndex, prev);
        enqueueWarm(sectionIndex, next);
    } else {
        enqueueWarm(sectionIndex, next);
        enqueueWarm(sectionIndex, prev);
    }
    pumpWarmQueue();
}

// While the user stays on a video, keep extending the warm frontier ahead so
// deep-gallery navigation finds videos already buffered.
function extendWarmFrontier() {
    if (warmInFlight || throttleMode || document.hidden) return;
    if (warmQueue.length > 0) return;
    if (!warmFocus) return;
    const parts = warmFocus.split('-');
    const sectionIndex = parseInt(parts[0], 10);
    const itemIndex = parseInt(parts[1], 10);
    // Focus moved (navigated or switched board): stop extending
    if (currentSection !== sectionIndex || currentIndices[sectionIndex] !== itemIndex) {
        warmFocus = null;
        return;
    }
    if (warmDepth >= WARM_FRONTIER) return;
    const gallery = galleryOf(sectionIndex);
    if (!gallery) return;
    const total = realCount(gallery);
    if (total < 2) return;
    warmDepth++;
    const step = navDirection === 'back' ? -warmDepth : warmDepth;
    enqueueWarm(sectionIndex, (itemIndex + step + total) % total);
    pumpWarmQueue();
}

function enqueueWarm(sectionIndex, itemIndex) {
    const key = sectionIndex + '-' + itemIndex;
    if (warmQueue.some(j => j.key === key)) return;
    // Never warm the video that is currently focused (frontier wrap-around)
    if (sectionIndex === currentSection && itemIndex === currentIndices[sectionIndex]) return;
    const video = getVideoAt(sectionIndex, itemIndex);
    if (!video || video.getAttribute('preload') === 'auto') return;
    const st = videoState.get(video);
    if (st && (st.overlayShown || st.attempts > 0)) return;
    warmQueue.push({ key, section: sectionIndex, index: itemIndex });
    if (warmQueue.length > 6) warmQueue.shift();
}

function clearWarmQueue() {
    warmQueue.length = 0;
    warmFocus = null;
}

function pumpWarmQueue() {
    if (warmInFlight || throttleMode || document.hidden) return;
    const job = warmQueue.shift();
    if (!job) { extendWarmFrontier(); return; }
    const video = getVideoAt(job.section, job.index);
    if (!video || video.getAttribute('preload') === 'auto') {
        pumpWarmQueue();
        return;
    }
    warmInFlight = true;
    let finished = false;
    const done = () => {
        if (finished) return;
        finished = true;
        warmInFlight = false;
        setTimeout(pumpWarmQueue, 500);
    };
    video.addEventListener('loadeddata', done, { once: true });
    video.addEventListener('error', done, { once: true });
    video.addEventListener('abort', done, { once: true });
    setTimeout(done, 20000); // safety net if no event fires
    video.setAttribute('preload', 'auto');
}

// attachVideoEndedListeners: wire up 'ended' events on all videos in a section.
function attachVideoEndedListeners(galleryOrIndex, sectionIndex) {
    const secIndex = typeof galleryOrIndex === 'number' ? galleryOrIndex : sectionIndex;
    const gallery = (galleryOrIndex && galleryOrIndex.querySelectorAll)
        ? galleryOrIndex
        : document.getElementById(`gallery-${secIndex}`);
    if (!gallery) return;
    const videos = gallery.querySelectorAll('video');
    videos.forEach((video, i) => {
        // Remove any previous listener to avoid duplicates
        video.removeEventListener('ended', video._autoScrollHandler);
        video._autoScrollHandler = () => {
            const el = video.closest ? video.closest('.gallery-item') : null;
            const idx = realIndex(el, i);
            if (autoScroll && currentSection === secIndex && currentIndices[secIndex] === idx) {
                handleNavigation('forward');
            }
        };
        video.addEventListener('ended', video._autoScrollHandler);
    });
}

// scrollToIndex: scrolls the given section to item `index` and updates remembered index + focus.
function scrollToIndex(sectionIndex, index) {
    const gallery = galleryOf(sectionIndex);
    if (!gallery) return;
    gallery.scrollTo({
        left: index * window.innerWidth,
        behavior: 'smooth'
    });
    currentIndices[sectionIndex] = index;
    updateVideoFocus(sectionIndex, index);
    updateProgress();
    // Neighbour warming happens via the paced preloader (warmNeighbors):
    // when the focused video starts playing, its two neighbours are warmed
    // one at a time so forward AND reverse navigation are instant.
}

// handleNavigation: move forward/back within the current board, wrapping around.
// ===== MOBILE FEED NAVIGATION =====
// Mobile boards are a vertical feed (no inner snap carousel). "Forward"/"back"
// scroll the currently focused item to the viewport center; the scroll handler
// (onFeedScroll) keeps the focused index / progress / playback in sync.
function scrollFeedItemTo(sectionIndex, itemIndex, behavior) {
    const gallery = galleryOf(sectionIndex);
    if (!gallery) return;
    const item = nearestItemByReal(gallery, itemIndex);
    if (!item) return;
    // Keep the focus hint on the copy we are travelling to, not a stale slot.
    focusSlots[sectionIndex] = Array.prototype.indexOf.call(itemsOf(gallery), item);
    if (typeof item.scrollIntoView !== 'function') return;
    const smooth = behavior !== 'instant';
    try {
        item.scrollIntoView({ block: 'center', behavior: smooth ? 'smooth' : 'instant' });
    } catch (e) {
        item.scrollIntoView({ block: 'center' });
    }
}

// pickFocusedItem: the video nearest the feed's vertical center is the one that
// autoplays with sound (single-stream, like desktop); everything else stays muted.
// Scans a small window around `hint` (the last focused DOM slot) so fast
// scrolling stays O(window) instead of O(total) per frame. Returns a DOM slot.
function pickFocusedItem(gallery, hint) {
    const rect = gallery.getBoundingClientRect();
    if (!rect || !rect.height) return null;
    const middle = rect.top + rect.height / 2;
    const items = itemsOf(gallery);
    const total = items.length;
    if (!total) return null;

    const center = (hint != null && hint >= 0 && hint < total) ? hint : 0;
    let best = null, bestDist = Infinity;
    const WINDOW = 3;
    for (let d = 0; d <= WINDOW; d++) {
        const candidates = d === 0 ? [center] : [center - d, center + d];
        for (const i of candidates) {
            if (i < 0 || i >= total) continue;
            const r = measure(items[i]);
            if (!r || r.bottom <= rect.top || r.top >= rect.bottom) continue; // off-screen
            const c = r.top + r.height / 2;
            const dist = Math.abs(c - middle);
            if (dist < bestDist) { bestDist = dist; best = i; }
        }
    }
    return best;
}

// Focus is tracked by DOM slot (clones make slots non-1:1 with real indices);
// currentIndices stays in real indices.
let focusSlots = [];

let feedRaf = 0;
// onFeedFocus: capture-phase scroll listener drives one-at-a-time playback as
// the user scrolls the active board's feed (rAF-throttled to one pass/frame).
function onFeedFocus() {
    if (!isMobile || feedRaf) return;
    feedRaf = requestAnimationFrame(() => {
        feedRaf = 0;
        const gallery = galleryOf(currentSection);
        if (!gallery) return;
        const slot = pickFocusedItem(gallery, focusSlots[currentSection]);
        if (slot === null) return;
        focusSlots[currentSection] = slot;
        const idx = realIndex(itemsOf(gallery)[slot], slot);
        if (idx === currentIndices[currentSection]) return;
        currentIndices[currentSection] = idx;
        updateVideoFocus(currentSection, idx);
        updateProgress();
        // Neighbor warming is triggered by the focused video's 'playing' handler.
    });
}

// onFeedSettle: once the feed stops moving, hop off a clone band and onto the
// identical real item so the scroll can never run out of feed.
let feedSettleTimer = 0;
function onFeedSettle() {
    if (!isMobile) return;
    clearTimeout(feedSettleTimer);
    feedSettleTimer = setTimeout(() => {
        if (reanchorFeed(currentSection)) {
            const gallery = galleryOf(currentSection);
            const slot = pickFocusedItem(gallery, focusSlots[currentSection]);
            if (slot !== null) focusSlots[currentSection] = slot;
        }
    }, 110);
}

document.addEventListener('scroll', onFeedFocus, { capture: true, passive: true });
document.addEventListener('scroll', onFeedSettle, { capture: true, passive: true });

function handleNavigation(direction) {
    Stats.recordNavigation();
    navDirection = direction;
    if (isMobile) {
        const gallery = galleryOf(currentSection);
        if (!gallery) return;
        const items = gallery.querySelectorAll('.gallery-item');
        const totalItems = realCount(gallery);
        if (totalItems === 0) return;
        const cur = currentIndices[currentSection] || 0;
        const newIndex = direction === 'forward'
            ? (cur + 1) % totalItems
            : (cur - 1 + totalItems) % totalItems;
        scrollFeedItemTo(currentSection, newIndex, 'smooth');
        return;
    }
    const gallery = galleryOf(currentSection);
    if (!gallery) return;
    const items = gallery.querySelectorAll('.gallery-item');
    const totalItems = items.length;
    if (totalItems === 0) return;
    const newIndex = direction === 'forward'
        ? (currentIndices[currentSection] + 1) % totalItems
        : (currentIndices[currentSection] - 1 + totalItems) % totalItems;
    
    scrollToIndex(currentSection, newIndex);
}

// switchBoard: move to a neighboring board (wraps around). Uses the remembered
// per-board index. dir 'back' moves to the previous board (reverse navigation).
function switchBoard(dir) {
    const delta = dir === 'back' ? -1 : 1;
    const next = (currentSection + delta + totalSections) % totalSections;
    showSection(next);
}

// saveCurrentVideo: downloads the currently focused video to disk
function saveCurrentVideo() {
    Stats.recordSave();
    const gallery = galleryOf(currentSection);
    if (!gallery) return;
    const item = itemByReal(gallery, currentIndices[currentSection]);
    if (!item) return;
    const source = item.querySelector('video source');
    if (!source || !source.src) return;

    const url = source.src;
    const filename = url.split('/').pop().split('?')[0] || 'video.mp4';

    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
}

// Keyboard shortcuts:
// X => next item, Z => previous item, C => next board, Ctrl+S => save current video
document.addEventListener('keydown', (e) => {
    // Ignore events when focused on inputs (safety)
    const active = document.activeElement;
    if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || active.isContentEditable)) return;

    if (e.ctrlKey && (e.key === 's' || e.key === 'S')) {
        e.preventDefault();
        saveCurrentVideo();
        return;
    }

    if (e.key === 'x' || e.key === 'X') handleNavigation('forward');
    if (e.key === 'z' || e.key === 'Z') handleNavigation('back');
    // Board switching: C / ArrowRight = next board, ArrowLeft = previous (reverse).
    // preventDefault stops the browser's native arrow-scroll on the tile strip.
    if (e.key === 'c' || e.key === 'C' || e.key === 'ArrowRight' || e.key === 'Right') { e.preventDefault(); switchBoard('next'); }
    if (e.key === 'ArrowLeft' || e.key === 'Left') { e.preventDefault(); switchBoard('back'); }
    if (e.key === 'p' || e.key === 'P') toggleAutoScroll();
});

// Initialize totalSections based on initial section count
totalSections = document.querySelectorAll('.gallery-section').length;
currentIndices = new Array(totalSections).fill(0);
focusSlots = new Array(totalSections).fill(0);

// ===== MOBILE SUPPORT =====
let isMobile = window.innerWidth <= 768 || window.matchMedia('(pointer: coarse)').matches;

// Attach auto-scroll ended listeners and resilience handlers for all sections
for (let i = 0; i < totalSections; i++) {
    attachVideoEndedListeners(document.getElementById(`gallery-${i}`), i);
    attachVideoResilience(document.getElementById(`gallery-${i}`), i);
}

// Wrap each feed around on mobile, then clone the end boards onto the opposite
// ends of the track. Done after the handlers are bound so the cloneNode copies
// carry them (the clone boards get their own attachment below).
if (isMobile) {
    for (let i = 0; i < totalSections; i++) setupFeedWrap(document.getElementById(`gallery-${i}`));
    setupBoardWrap();
    boardSections()
        .filter(s => s.classList.contains('is-clone'))
        .forEach(s => {
            const g = s.querySelector('.gallery-em');
            const idx = realBoard(s, boardSections().indexOf(s));
            attachVideoEndedListeners(g, idx);
            attachVideoResilience(g, idx);
        });
}


// Visibility change: pause all videos when tab hidden / phone locked
document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
        document.querySelectorAll('video').forEach(v => {
            v.pause();
            v.muted = true;
        });
    }
});

// ===== MERGED BOARD SWITCHER (mobile pill + dropdown) =====
let switcherOpen = false;

function escapeHtml(s) {
    const d = document.createElement('div');
    d.textContent = s == null ? '' : String(s);
    return d.innerHTML;
}

function boardLabels() {
    return Array.from(document.querySelectorAll('.tab-button .tab-label')).map(el => el.textContent);
}

// buildSwitcherMenu: one dropdown that both picks the active board and
// switches platform (all boards in a gallery share the current platform;
// the footer row links to the other platform's gallery file).
function buildSwitcherMenu() {
    const menu = document.getElementById('board-switcher-menu');
    if (!menu) return;
    const labels = boardLabels();
    const isReddit = PLATFORM === 'reddit';
    const accent = isReddit ? 'var(--accent-reddit)' : 'var(--accent-4chan)';
    const otherName = isReddit ? '4chan' : 'reddit';
    const otherHref = isReddit ? 'faptopia_4chan.html' : 'faptopia_reddit.html';
    const otherAccent = isReddit ? 'var(--accent-4chan)' : 'var(--accent-reddit)';

    let html = '<div class="board-switcher__group-label">' + (isReddit ? 'Reddit' : '4chan') + '</div>';
    labels.forEach((label, i) => {
        html += '<button type="button" class="board-switcher__option' + (i === currentSection ? ' active' : '') +
            '" role="option" data-index="' + i + '" aria-selected="' + (i === currentSection ? 'true' : 'false') + '">' +
            '<span class="board-switcher__dot-mini" style="background:' + accent + '"></span>' +
            '<span>' + escapeHtml(label) + '</span></button>';
    });
    html += '<a class="board-switcher__switch" href="' + otherHref + '" style="color:' + otherAccent + '">' +
        otherName.toUpperCase() + ' gallery</a>';
    menu.innerHTML = html;

    menu.querySelectorAll('.board-switcher__option').forEach(btn => {
        btn.addEventListener('click', () => {
            const idx = parseInt(btn.dataset.index, 10);
            closeSwitcher();
            if (!isNaN(idx)) showSection(idx, { instant: true });
        });
    });
}

function updateSwitcherPill() {
    const label = document.getElementById('board-switcher-label');
    if (!label) return;
    const tabs = document.querySelectorAll('.tab-button .tab-label');
    label.textContent = tabs[currentSection] ? tabs[currentSection].textContent : '';
    const dot = document.getElementById('board-switcher-dot');
    if (dot) {
        const accent = PLATFORM === 'reddit' ? 'var(--accent-reddit)' : 'var(--accent-4chan)';
        dot.style.background = accent;
        dot.style.boxShadow = '0 0 8px ' + accent;
    }
}

function updateMenuSelection() {
    document.querySelectorAll('.board-switcher__option').forEach((el, i) => {
        const on = i === currentSection;
        el.classList.toggle('active', on);
        el.setAttribute('aria-selected', on ? 'true' : 'false');
    });
}

function openSwitcher() {
    const sw = document.getElementById('board-switcher');
    const trigger = document.getElementById('board-switcher-trigger');
    if (sw) sw.classList.add('open');
    if (trigger) trigger.setAttribute('aria-expanded', 'true');
    switcherOpen = true;
    updateMenuSelection();
}

function closeSwitcher() {
    const sw = document.getElementById('board-switcher');
    const trigger = document.getElementById('board-switcher-trigger');
    if (sw) sw.classList.remove('open');
    if (trigger) trigger.setAttribute('aria-expanded', 'false');
    switcherOpen = false;
}

function toggleSwitcher() {
    switcherOpen ? closeSwitcher() : openSwitcher();
}

function wireSwitcher() {
    const trigger = document.getElementById('board-switcher-trigger');
    if (!trigger) return;
    trigger.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleSwitcher();
    });
    document.addEventListener('click', (e) => {
        if (switcherOpen && !(e.target.closest && e.target.closest('#board-switcher'))) closeSwitcher();
    });
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && switcherOpen) closeSwitcher();
    });
}

// Mobile-specific behavior: native board axis + merged board switcher
if (isMobile) {
    // Platform accent drives the pill dot + dropdown accents
    document.documentElement.style.setProperty(
        '--accent-active',
        PLATFORM === 'reddit' ? '#ff4500' : '#2ea44f'
    );
    buildSwitcherMenu();
    wireSwitcher();
    initBoardAxis();
}

// updateProgress: show current position as "N / Total" in the progress counter
function updateProgress() {
    const el = document.getElementById('progress-counter');
    if (!el) return;
    const gallery = galleryOf(currentSection);
    if (!gallery) return;
    const total = realCount(gallery);
    if (!total) return;
    const current = (currentIndices[currentSection] || 0) + 1;
    el.textContent = current + ' / ' + total;
}

// ===== USAGE TRACKING =====
const Stats = (() => {
    const KEY = 'faptopia_stats';
    const MAX_SESSIONS = 365;

    let sessionStart = Date.now();
    let tabStart = Date.now();
    let tabName = null;
    let sessionSources = {};
    let sessionVideos = 0;
    let sessionSaved = 0;
    let ended = false;

    function load() {
        try {
            const raw = localStorage.getItem(KEY);
            return raw ? JSON.parse(raw) : { sessions: [] };
        } catch(e) { return { sessions: [] }; }
    }

    function persist(data) {
        try { localStorage.setItem(KEY, JSON.stringify(data)); } catch(e) {}
    }

    function getTabLabel(index) {
        const labels = document.querySelectorAll('.tab-button .tab-label');
        return labels[index] ? labels[index].textContent.trim() : ('Tab ' + index);
    }

    function flushTab() {
        if (!tabName) return;
        const elapsed = (Date.now() - tabStart) / 1000;
        sessionSources[tabName] = (sessionSources[tabName] || 0) + elapsed;
        tabStart = Date.now();
    }

    function switchTab(newIndex) {
        flushTab();
        tabName = getTabLabel(newIndex);
    }

    function recordNavigation() { sessionVideos++; }
    function recordSave() { sessionSaved++; }

    function endSession() {
        if (ended) return;
        ended = true;
        flushTab();
        const duration = Math.round((Date.now() - sessionStart) / 1000);
        if (duration < 3) return;
        const data = load();
        const rounded = {};
        for (const [k, v] of Object.entries(sessionSources)) rounded[k] = Math.round(v);
        data.sessions.push({
            start: sessionStart,
            end: Date.now(),
            duration,
            platform: PLATFORM,
            sources: rounded,
            videosSeen: sessionVideos,
            saved: sessionSaved,
            date: new Date(sessionStart).toISOString().slice(0, 10)
        });
        if (data.sessions.length > MAX_SESSIONS) {
            data.sessions = data.sessions.slice(-MAX_SESSIONS);
        }
        persist(data);
    }

    function getStats() {
        const data = load();
        const sessions = data.sessions || [];

        // Live current-session source times (flush without saving)
        const liveSources = Object.assign({}, sessionSources);
        if (tabName) {
            const live = (Date.now() - tabStart) / 1000;
            liveSources[tabName] = (liveSources[tabName] || 0) + live;
        }
        const liveRounded = {};
        for (const [k, v] of Object.entries(liveSources)) liveRounded[k] = Math.round(v);

        // Lifetime aggregates
        let totalSecs = 0, totalVideos = 0, totalSaved = 0;
        const allSources = {};
        const byDate = {};
        for (const s of sessions) {
            totalSecs += s.duration || 0;
            totalVideos += s.videosSeen || 0;
            totalSaved += s.saved || 0;
            for (const [src, secs] of Object.entries(s.sources || {})) {
                allSources[src] = (allSources[src] || 0) + secs;
            }
            if (s.date) byDate[s.date] = (byDate[s.date] || 0) + 1;
        }

        const topSources = Object.entries(allSources).sort((a, b) => b[1] - a[1]).slice(0, 8);
        const maxSecs = topSources.length ? topSources[0][1] : 1;

        // Consecutive day streak (ending today or yesterday counts)
        let streak = 0;
        const d = new Date();
        // If today has no sessions yet, start check from yesterday
        if (!byDate[d.toISOString().slice(0, 10)]) d.setDate(d.getDate() - 1);
        while (byDate[d.toISOString().slice(0, 10)]) {
            streak++;
            d.setDate(d.getDate() - 1);
        }

        // Last 7 days activity
        const last7 = [];
        for (let i = 6; i >= 0; i--) {
            const day = new Date();
            day.setDate(day.getDate() - i);
            const key = day.toISOString().slice(0, 10);
            last7.push({ date: key, count: byDate[key] || 0 });
        }

        return {
            current: {
                duration: Math.round((Date.now() - sessionStart) / 1000),
                sources: liveRounded,
                videosSeen: sessionVideos,
                saved: sessionSaved,
                platform: PLATFORM
            },
            lifetime: {
                sessions: sessions.length,
                totalSecs,
                totalVideos,
                totalSaved,
                topSources,
                maxSecs,
                avgSecs: sessions.length ? Math.round(totalSecs / sessions.length) : 0,
                streak,
                last7
            }
        };
    }

    function clearData() {
        try { localStorage.removeItem(KEY); } catch(e) {}
        sessionSources = {};
        sessionVideos = 0;
        sessionSaved = 0;
    }

    window.addEventListener('beforeunload', endSession);
    window.addEventListener('pagehide', endSession);
    // Init first tab label after DOM ready
    setTimeout(() => { if (!tabName) tabName = getTabLabel(0); }, 0);

    return { switchTab, recordNavigation, recordSave, getStats, clearData };
})();

// Initialize the first section on load
showSection(0);
updateProgress();

// Register service worker for PWA (only works over HTTPS, e.g. Cloudflare Pages)
if ('serviceWorker' in navigator && location.protocol === 'https:') {
    navigator.serviceWorker.register('sw.js')
        .then(() => {
            // Give the shell a moment to settle, then warm the first likely videos
            // into the SW cache so the installed PWA opens faster on mobile.
            setTimeout(warmPwaVideos, 1200);
        })
        .catch(() => {});
} else if ('serviceWorker' in navigator) {
    // Clean up stale workers on HTTP
    navigator.serviceWorker.getRegistrations().then(regs => regs.forEach(r => r.unregister()));
}
