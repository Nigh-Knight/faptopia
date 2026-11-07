// Behavioral harness for faptopia's script.js resilience layer.
// Evals the REAL template with DOM stubs, then drives the actual functions.
// Run with: node tests/script_resilience.test.js
const fs = require('fs');
const path = require('path');
const code = fs.readFileSync(path.join(__dirname, '..', 'templates', process.env.SC || 'script.js'), 'utf8');

// ---- timer capture (we drive retries/stalls/grace manually) ----
const timers = [];
globalThis.setTimeout = (fn, ms) => { timers.push({ fn, ms: ms || 0 }); return timers.length; };
globalThis.clearTimeout = () => {};
globalThis.clearInterval = () => {};

// ---- DOM stubs ----
const classListStub = { add() {}, remove() {}, toggle() {} };

let items = [];
let galleryStub = null;

function makeVideo(src, sdUrl) {
    const listeners = {};
    const video = {
        dataset: sdUrl ? { sdUrl, gifId: 'x' } : {},
        attributes: { preload: 'none', src },
        paused: true, readyState: 0, error: null, ended: false, muted: false,
        played: { length: 0 }, loadCount: 0,
        _listeners: listeners,
        addEventListener(ev, fn) { (listeners[ev] = listeners[ev] || []).push(fn); },
        removeEventListener() {},
        setAttribute(k, v) { this.attributes[k] = v; },
        getAttribute(k) { return this.attributes[k]; },
        load() { this.loadCount++; this.error = null; },
        play() { this.paused = false; return Promise.resolve(); },
        pause() { this.paused = true; },
        querySelector(sel) { return sel === 'source' ? this._source : null; },
        closest(sel) { return sel === '.gallery-item' ? this._item : (sel === '.gallery-em' ? galleryStub : null); },
    };
    video._source = {
        getAttribute: () => video.attributes.src,
        setAttribute: (k, v) => { video.attributes.src = v; },
    };
    return video;
}

function makeItem(video) {
    const item = {
        appendCalls: [],
        querySelector(sel) {
            if (sel === 'video') return video;
            if (sel === '.video-error') return null;
            return null;
        },
        appendChild(el) { this.appendCalls.push(el); },
    };
    video._item = item;
    return item;
}

function setGallery(videos) {
    items = videos.map(makeItem);
    galleryStub = { id: 'gallery-0', querySelectorAll: (sel) => sel === '.gallery-item' ? items : [], scrollTo() {} };
}

let toastShown = false;
let toastText = '';

globalThis.document = {
    hidden: false,
    querySelectorAll(sel) {
        if (sel === '.tab-button') return [{ classList: classListStub, querySelector: () => ({ textContent: 'r/test' }) }];
        if (sel === '.gallery-section') return [];
        if (sel === '#gallery-0 .gallery-item') return items;
        if (sel === '#gallery-0 video') return [];
        return [];
    },
    querySelector(sel) {
        if (sel === '.gallery-item video source') return null; // direct (non-proxied) mode by default
        if (sel === '[data-section="0"]') return { classList: classListStub, style: {} };
        return null;
    },
    getElementById(id) {
        if (id === 'gallery-0') return galleryStub;
        if (id === 'throttle-toast') return {
            classList: { add() { toastShown = true; }, remove() { toastShown = false; } },
            set textContent(v) { toastText = v; }, get textContent() { return toastText; },
        };
        return null;
    },
    addEventListener() {},
    createElement() {
        return {
            className: '', innerHTML: '', style: {}, classList: classListStub,
            appendChild() {}, querySelector() { return { addEventListener() {} }; },
        };
    },
};

globalThis.location = { origin: 'file://', protocol: 'file:', pathname: '/faptopia_reddit.html' };
globalThis.window = { innerWidth: 1400, matchMedia: () => ({ matches: false }), addEventListener() {} };
// Run rAF callbacks synchronously so feed-focus tests can drive them directly.
globalThis.requestAnimationFrame = (fn) => { fn(); return 0; };
globalThis.navigator = {};
globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };

// ---- eval the real script with tests appended ----
const testBody = `
// ================= TESTS =================
let pass = 0, fail = 0;
function assert(name, cond) { if (cond) { pass++; console.log('PASS', name); } else { fail++; console.log('FAIL', name); } }
function fire(v, ev) { (v._listeners[ev] || []).forEach(f => f({})); }
function st(v) { return videoState.get(v); }
function reset(v) { const s = getVideoState(v); s.attempts = 0; s.sdTried = false; s.playedOnce = false; s.overlayShown = false; s.originalSrc = null; }
function newTimersSince(n) { return timers.slice(n); }
function flushTimer(t) { t.fn(); }

// ---------- A: HD error retries, then SD swap, then overlay ----------
const va = makeVideo('https://media.redgifs.com/A.mp4', 'https://media.redgifs.com/A-mobile.mp4');
setGallery([va]);
attachVideoResilience(0);
assert('A1: resilience handlers attached', va._listeners.error && va._listeners.error.length > 0);

fire(va, 'error');
assert('A2: first failure schedules retry, no SD swap', st(va).attempts === 1 && va.attributes.src === 'https://media.redgifs.com/A.mp4');
fire(va, 'error');
assert('A3: second failure -> attempt 2 (HD limit)', st(va).attempts === 2 && !st(va).sdTried);
fire(va, 'error');
assert('A4: third failure -> SD swap + immediate reload', st(va).sdTried && va.attributes.src === 'https://media.redgifs.com/A-mobile.mp4' && va.loadCount >= 1 && st(va).attempts === 0);
fire(va, 'error');
assert('A5: SD failure -> retry on SD', st(va).attempts === 1 && va.attributes.src === 'https://media.redgifs.com/A-mobile.mp4');
fire(va, 'error'); fire(va, 'error'); fire(va, 'error'); // SD attempts 2,3,4
assert('A6: SD exhausted -> overlay shown', st(va).overlayShown && items[0].appendCalls.length > 0 && items[0].appendCalls[0].className === 'video-error');

// manual Retry from overlay restores HD
const beforeRetry = va.loadCount;
retryPlayback(va, items[0]);
assert('A7: manual retry restores HD + clears state', va.attributes.src === 'https://media.redgifs.com/A.mp4' && !st(va).sdTried && st(va).attempts === 0 && st(va).overlayShown === false && va.loadCount > beforeRetry);

// ---------- G: time-based HD grace -> SD downgrade ----------
const vg = makeVideo('https://media.redgifs.com/G.mp4', 'https://media.redgifs.com/G-mobile.mp4');
setGallery([vg]);
attachVideoResilience(0);
reset(vg);
burstFailures = []; throttleMode = false; // A-series tripped the breaker
const tBeforePlay = timers.length;
vg.paused = false; // emulate browser state after play()
fire(vg, 'play'); // playback requested, fetch hangs forever (throttle signature)
const playTimers = newTimersSince(tBeforePlay);
assert('G1: play arms grace (8s) + stall (12s) timers', playTimers.some(t => t.ms === HD_GRACE_MS) && playTimers.some(t => t.ms === STALL_TIMEOUT_MS));
assert('G2: still on HD before grace expires', vg.attributes.src === 'https://media.redgifs.com/G.mp4');
const graceTimer = playTimers.find(t => t.ms === HD_GRACE_MS);
flushTimer(graceTimer);
assert('G3: grace expiry downgrades to SD', st(vg).sdTried && vg.attributes.src === 'https://media.redgifs.com/G-mobile.mp4' && vg.loadCount >= 1);
// SD fetch now has its own stall watchdog armed (retryPlayback called play())
const tBeforeRearm = timers.length;
fire(vg, 'play'); // the swap's play() re-arms the watchdog
const rearmTimers = newTimersSince(tBeforeRearm);
assert('G4: SD fetch re-arms stall watchdog only (no second grace)', rearmTimers.some(t => t.ms === STALL_TIMEOUT_MS) && !rearmTimers.some(t => t.ms === HD_GRACE_MS));
const stallTimer = rearmTimers.find(t => t.ms === STALL_TIMEOUT_MS);
flushTimer(stallTimer);
assert('G5: SD stall after downgrade -> failure/retry path', st(vg).attempts === 1 && st(vg).sdTried === true);

// ---------- T: throttle mode starts never-played videos on SD ----------
const vt = makeVideo('https://media.redgifs.com/T.mp4', 'https://media.redgifs.com/T-mobile.mp4');
setGallery([vt]);
attachVideoResilience(0);
reset(vt);
burstFailures = [Date.now(), Date.now() - 1000, Date.now() - 2000]; // trip the breaker
throttleMode = true;
const tBeforeT = timers.length;
fire(vt, 'play');
const tTimers = newTimersSince(tBeforeT);
assert('T1: throttle mode skips HD for never-played videos', vt.attributes.src === 'https://media.redgifs.com/T-mobile.mp4' && st(vt).sdTried);
assert('T2: no grace timer armed after SD-first switch', !tTimers.some(t => t.ms === HD_GRACE_MS));
burstFailures = []; throttleMode = false;

// ---------- C: direction-aware neighbor warming ----------
const v0 = makeVideo('https://media.redgifs.com/0.mp4');
const v1 = makeVideo('https://media.redgifs.com/1.mp4');
const v2 = makeVideo('https://media.redgifs.com/2.mp4');
setGallery([v0, v1, v2]);
warmQueue.length = 0; warmInFlight = false; warmFocus = null; warmDepth = 0;
currentSection = 0; currentIndices = [1];
navDirection = 'forward';

warmNeighbors(0, 1);
assert('C1: forward -> NEXT warms first', v2.attributes.preload === 'auto' && v0.attributes.preload === 'none' && warmQueue.length === 1);
fire(v2, 'loadeddata');
const pump1 = timers[timers.length - 1];
pump1.fn();
assert('C2: then prev warms', v0.attributes.preload === 'auto' && warmQueue.length === 0);
fire(v0, 'loadeddata');
const pump2 = timers[timers.length - 1];
pump2.fn();
assert('C3: queue drains', warmInFlight === false);

// backwards navigation
const w0 = makeVideo('https://media.redgifs.com/w0.mp4');
const w1 = makeVideo('https://media.redgifs.com/w1.mp4');
const w2 = makeVideo('https://media.redgifs.com/w2.mp4');
setGallery([w0, w1, w2]);
warmQueue.length = 0; warmInFlight = false; warmFocus = null; warmDepth = 0;
navDirection = 'back';
warmNeighbors(0, 1);
assert('C4: backward -> PREV warms first', w0.attributes.preload === 'auto' && w2.attributes.preload === 'none');

// wrap-around: at index 0 going back, prev is the LAST video
const x0 = makeVideo('https://media.redgifs.com/x0.mp4');
const x1 = makeVideo('https://media.redgifs.com/x1.mp4');
const x2 = makeVideo('https://media.redgifs.com/x2.mp4');
setGallery([x0, x1, x2]);
warmQueue.length = 0; warmInFlight = false; warmFocus = null; warmDepth = 0;
navDirection = 'back';
warmNeighbors(0, 0);
assert('C5: wrap-around warms last video first for reverse nav', x2.attributes.preload === 'auto' && x1.attributes.preload === 'none');

// ---------- F: frontier extension while lingering ----------
const f0 = makeVideo('https://media.redgifs.com/f0.mp4');
const f1 = makeVideo('https://media.redgifs.com/f1.mp4');
const f2 = makeVideo('https://media.redgifs.com/f2.mp4');
const f3 = makeVideo('https://media.redgifs.com/f3.mp4');
const f4 = makeVideo('https://media.redgifs.com/f4.mp4');
setGallery([f0, f1, f2, f3, f4]);
warmQueue.length = 0; warmInFlight = false; warmFocus = null; warmDepth = 0;
currentIndices = [2];
navDirection = 'forward';

warmNeighbors(0, 2);            // f3 in flight, queue [f1]
fire(f3, 'loadeddata');
timers[timers.length - 1].fn(); // pump -> f1 in flight
fire(f1, 'loadeddata');
timers[timers.length - 1].fn(); // pump -> queue empty -> frontier depth 1 -> f3 already warm, skipped -> depth 2 -> f4 in flight
assert('F1: frontier extends 2 ahead while lingering', f4.attributes.preload === 'auto' && warmDepth === 2);
fire(f4, 'loadeddata');
timers[timers.length - 1].fn(); // pump -> frontier depth 3 -> f0 (wrap) in flight
assert('F2: frontier extends 3 ahead (wraps)', f0.attributes.preload === 'auto' && warmDepth === 3);
fire(f0, 'loadeddata');
timers[timers.length - 1].fn(); // pump -> depth limit reached, stops
assert('F3: frontier stops at depth limit', warmInFlight === false && warmDepth === 3);

// ---------- M: mobile feed focus (continuous vertical feed) ----------
isMobile = true;
// Videos at natural heights; gallery viewport 0..800 (center 400)
const mRects = [
    { top: 0, height: 300 },    // center 150, dist 250
    { top: 300, height: 300 },  // center 450, dist 50  <- focused
    { top: 600, height: 300 },  // center 750, dist 350
];
const mv = [0, 1, 2].map(i => {
    const v = makeVideo('https://media.redgifs.com/m' + i + '.mp4');
    v.getBoundingClientRect = () => mRects[i];
    return v;
});
items = mv.map(makeItem);
galleryStub.getBoundingClientRect = () => ({ top: 0, height: 800 });
galleryStub.querySelectorAll = (sel) => {
    if (sel === '.gallery-item') return items;
    if (sel === '.gallery-item video') return items.map(it => it.querySelector('video'));
    return [];
};
currentSection = 0; currentIndices = [0]; feedRaf = 0;
onFeedFocus();
assert('M1: feed picks the video nearest vertical center', currentIndices[0] === 1);
const m1Focused = currentIndices[0];
onFeedFocus(); // no scroll change -> no churn
assert('M2: repeated focus pass is a no-op', currentIndices[0] === m1Focused);

// Scroll: item 2 becomes centered -> focus moves
mRects[1] = { top: -300, height: 300 }; // scrolled up off the top
mRects[2] = { top: 250, height: 300 };  // center 400 = viewport middle -> focused
mRects[0] = { top: 550, height: 300 };  // below
onFeedFocus();
assert('M3: feed focus follows scroll (item 2 now focused)', currentIndices[0] === 2);

// Board restore + keyboard nav target the right items
let scrolled = -1;
items.forEach((it, i) => { it.scrollIntoView = () => { scrolled = i; }; });
scrollFeedItemTo(0, 2, 'instant');
assert('M4: board restore targets remembered feed item', scrolled === 2);
currentIndices = [1];
handleNavigation('forward');              // 1 -> 2
assert('M5: forward nav targets next feed item', scrolled === 2);
currentIndices = [0];
handleNavigation('back');                 // wrap 0 -> 2
assert('M5b: back nav wraps first -> last', scrolled === 2);
currentIndices = [2];
handleNavigation('forward');              // wrap 2 -> 0
assert('M6: forward nav wraps last -> first', scrolled === 0);
isMobile = false;

// ---------- D: proxied mode skips SD swap ----------
burstFailures = []; throttleMode = false;
document.querySelector = (sel) => sel === '.gallery-item video source'
    ? { src: 'http://localhost:6969/video/redgifs/a' }
    : (sel === '[data-section="0"]' ? { classList: classListStub, style: {} } : null);
location.origin = 'http://localhost:6969';
const vd = makeVideo('http://localhost:6969/video/redgifs/a', 'https://media.redgifs.com/a-mobile.mp4');
setGallery([vd]);
attachVideoResilience(0);
reset(vd);
fire(vd, 'error'); fire(vd, 'error'); fire(vd, 'error'); // HD_ATTEMPTS=2 exhausted -> proxy handles fallback
assert('D1: proxied -> overlay, no SD swap to direct URL', st(vd).overlayShown === true && vd.attributes.src === 'http://localhost:6969/video/redgifs/a');

console.log('-----');
console.log(pass + ' passed, ' + fail + ' failed');
if (fail > 0) process.exit(1);
`;

// The script file uses `const` at top level; eval everything in ONE scope.
eval(code + '\n' + testBody);
