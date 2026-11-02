const SHELL_CACHE = 'faptopia-shell-v4';
const DB_NAME = 'faptopia-videos';
const DB_VERSION = 1;
const STORE = 'videos';

function openDB() {
    return new Promise((resolve, reject) => {
        const req = indexedDB.open(DB_NAME, DB_VERSION);
        req.onupgradeneeded = () => {
            const db = req.result;
            if (!db.objectStoreNames.contains(STORE)) {
                db.createObjectStore(STORE, { keyPath: 'id' });
            }
        };
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

function idbGet(id) {
    return openDB().then(db => new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, 'readonly');
        const req = tx.objectStore(STORE).get(id);
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    }));
}

function idbPut(record) {
    return openDB().then(db => new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, 'readwrite');
        tx.objectStore(STORE).put(record);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
    }));
}

function idbClear() {
    return openDB().then(db => new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, 'readwrite');
        tx.objectStore(STORE).clear();
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
    }));
}

function idbStats() {
    return openDB().then(db => new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, 'readonly');
        const store = tx.objectStore(STORE);
        let count = 0;
        let bytes = 0;
        store.openCursor().onsuccess = (e) => {
            const cur = e.target.result;
            if (cur) {
                count++;
                bytes += cur.value.size || 0;
                cur.continue();
            }
        };
        tx.oncomplete = () => resolve({ count, bytes });
        tx.onerror = () => reject(tx.error);
    }));
}

// Build a Response from a cached Blob, honoring Range requests so <video> works on iOS
function blobResponse(blob, contentType, rangeHeader) {
    const total = blob.size;
    const headersBase = {
        'Content-Type': contentType,
        'Accept-Ranges': 'bytes',
        'Cache-Control': 'public, max-age=31536000, immutable',
    };
    if (rangeHeader) {
        const m = /^bytes=(\d+)-(\d*)$/.exec(rangeHeader);
        if (m) {
            const start = parseInt(m[1], 10);
            const end = m[2] ? Math.min(parseInt(m[2], 10), total - 1) : total - 1;
            if (start >= total || end < start) {
                return new Response(null, {
                    status: 416,
                    headers: { 'Content-Range': `bytes */${total}` },
                });
            }
            const slice = blob.slice(start, end + 1, contentType);
            return new Response(slice, {
                status: 206,
                headers: {
                    ...headersBase,
                    'Content-Length': String(slice.size),
                    'Content-Range': `bytes ${start}-${end}/${total}`,
                },
            });
        }
    }
    return new Response(blob, {
        status: 200,
        headers: { ...headersBase, 'Content-Length': String(total) },
    });
}

// Inflight streams for concurrent requests of the same gif id
const inflight = new Map();
let prefetchChain = Promise.resolve();

function rangeStartsAtZero(range) {
    if (!range) return true;
    const m = /^bytes=(\d+)-/.exec(range);
    return !!m && parseInt(m[1], 10) === 0;
}

async function handleRedgifs(request, event) {
    const url = new URL(request.url);
    const id = url.pathname.replace('/video/redgifs/', '').toLowerCase();
    if (!id) return fetch(request);
    const range = request.headers.get('Range');

    // 1. Cache hit -> serve from blob with full Range support
    try {
        const hit = await idbGet(id);
        if (hit && hit.blob) {
            return blobResponse(hit.blob, hit.contentType || 'video/mp4', range);
        }
    } catch (_) {}

    // 2. A cache fill is already running -> wait for it, then serve from blob
    if (inflight.has(id)) {
        try {
            const { blob, contentType } = await inflight.get(id);
            return blobResponse(blob, contentType, range);
        } catch (_) {
            return fetch(request);
        }
    }

    // 3. Seek before the cache exists -> pass through upstream, don't start a
    //    cache fill here (another request from offset 0 will, or already has)
    if (!rangeStartsAtZero(range)) {
        return fetch(request);
    }

    // 4. Fresh miss from byte 0 -> tee upstream: stream one branch to the
    //    client so playback starts immediately, buffer the other into IDB
    let upstream;
    try {
        upstream = await fetch(`/video/redgifs/${id}`);
    } catch (_) {
        return fetch(request);
    }
    if (!upstream.ok || !upstream.body) return fetch(request);

    const contentType = upstream.headers.get('Content-Type') || 'video/mp4';
    const contentLengthHeader = upstream.headers.get('Content-Length');
    const total = contentLengthHeader ? parseInt(contentLengthHeader, 10) : null;

    const [clientBranch, cacheBranch] = upstream.body.tee();

    const cachePromise = (async () => {
        const reader = cacheBranch.getReader();
        const chunks = [];
        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                chunks.push(value);
            }
            const blob = new Blob(chunks, { type: contentType });
            try {
                await idbPut({ id, blob, contentType, size: blob.size, cachedAt: Date.now() });
            } catch (_) {}
            return { blob, contentType };
        } finally {
            inflight.delete(id);
        }
    })();
    inflight.set(id, cachePromise);
    if (event && event.waitUntil) event.waitUntil(cachePromise.catch(() => {}));

    const headers = {
        'Content-Type': contentType,
        'Accept-Ranges': 'bytes',
        'Cache-Control': 'public, max-age=31536000, immutable',
    };
    if (total != null) headers['Content-Length'] = String(total);

    if (range && total != null) {
        headers['Content-Range'] = `bytes 0-${total - 1}/${total}`;
        return new Response(clientBranch, { status: 206, headers });
    }
    return new Response(clientBranch, { status: 200, headers });
}

async function prefetchRedgifs(id) {
    const normalized = String(id || '').toLowerCase();
    if (!normalized) return;

    try {
        const hit = await idbGet(normalized);
        if (hit && hit.blob) return;
    } catch (_) {}

    if (inflight.has(normalized)) {
        try { await inflight.get(normalized); } catch (_) {}
        return;
    }

    let upstream;
    try {
        upstream = await fetch(`/video/redgifs/${normalized}`);
    } catch (_) {
        return;
    }
    if (!upstream.ok) return;

    const contentType = upstream.headers.get('Content-Type') || 'video/mp4';
    const blob = await upstream.blob();
    try {
        await idbPut({
            id: normalized,
            blob,
            contentType,
            size: blob.size,
            cachedAt: Date.now(),
        });
    } catch (_) {}
}

self.addEventListener('install', (e) => {
    e.waitUntil((async () => {
        try {
            const cache = await caches.open(SHELL_CACHE);
            // Add each file individually so one missing file (e.g. only 4chan
            // generated, not reddit) doesn't abort the whole install.
            await Promise.all(['/', '/faptopia_reddit.html', '/faptopia_4chan.html'].map(
                url => fetch(url).then(res => res.ok ? cache.put(url, res) : null).catch(() => {})
            ));
        } catch (_) {}
        if (self.registration && navigator.storage && navigator.storage.persist) {
            navigator.storage.persist().catch(() => {});
        }
    })());
    self.skipWaiting();
});

self.addEventListener('activate', (e) => {
    e.waitUntil((async () => {
        const keys = await caches.keys();
        await Promise.all(
            keys.filter(k => k.startsWith('faptopia-') && k !== SHELL_CACHE).map(k => caches.delete(k))
        );
        await self.clients.claim();
    })());
});

self.addEventListener('fetch', (e) => {
    const url = new URL(e.request.url);

    if (url.pathname.startsWith('/video/redgifs/')) {
        e.respondWith(handleRedgifs(e.request, e));
        return;
    }

    // 4chan: pass through (threads are ephemeral and the worker already sets cache headers)
    if (url.pathname.startsWith('/proxy/4cdn/')) return;

    if (e.request.method !== 'GET') return;

    // HTML navigations: network-first so we never get stuck on a broken cached
    // page; fall back to cache only when offline.
    const isNavigation = e.request.mode === 'navigate'
        || (e.request.headers.get('Accept') || '').includes('text/html');
    if (isNavigation) {
        e.respondWith((async () => {
            try {
                const res = await fetch(e.request);
                if (res && res.status === 200) {
                    const clone = res.clone();
                    caches.open(SHELL_CACHE).then(c => c.put(e.request, clone)).catch(() => {});
                }
                return res;
            } catch (_) {
                const cached = await caches.match(e.request);
                return cached || new Response('Offline', { status: 503 });
            }
        })());
        return;
    }

    // Other assets (css/js/icons): cache-first with background refresh
    e.respondWith(
        caches.match(e.request).then(hit => {
            const network = fetch(e.request).then(res => {
                if (res && res.status === 200) {
                    const clone = res.clone();
                    caches.open(SHELL_CACHE).then(c => c.put(e.request, clone)).catch(() => {});
                }
                return res;
            }).catch(() => hit);
            return hit || network;
        })
    );
});

self.addEventListener('message', async (e) => {
    const msg = e.data || {};
    const reply = (data) => e.source && e.source.postMessage(data);
    if (msg.type === 'video-cache-stats') {
        try {
            const stats = await idbStats();
            let quota = null;
            if (navigator.storage && navigator.storage.estimate) {
                try { quota = await navigator.storage.estimate(); } catch (_) {}
            }
            reply({ type: 'video-cache-stats', ...stats, quota });
        } catch (err) {
            reply({ type: 'video-cache-stats', count: 0, bytes: 0, error: String(err) });
        }
    } else if (msg.type === 'video-cache-clear') {
        try {
            await idbClear();
            reply({ type: 'video-cache-cleared' });
        } catch (err) {
            reply({ type: 'video-cache-cleared', error: String(err) });
        }
    } else if (msg.type === 'video-prefetch') {
        const ids = Array.isArray(msg.ids) ? msg.ids : [];
        prefetchChain = prefetchChain.then(async () => {
            for (const id of ids) {
                await prefetchRedgifs(id);
            }
        }).catch(() => {});
    }
});
