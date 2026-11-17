const SHELL_CACHE = 'faptopia-shell-v6-streaming';
const SHELL_URLS = ['/', '/faptopia_reddit.html', '/faptopia_4chan.html'];

// Video loading belongs to the browser and proxy. Never wait for a full-file
// cache fill, tee a media stream, or schedule a second background downloader.
function isMedia(url, request) {
    return request.destination === 'video' || request.destination === 'audio'
        || url.pathname.startsWith('/video/redgifs/')
        || url.pathname.startsWith('/proxy/4cdn/');
}

self.addEventListener('install', event => {
    event.waitUntil((async () => {
        const cache = await caches.open(SHELL_CACHE);
        await Promise.all(SHELL_URLS.map(async path => {
            try {
                const response = await fetch(path, { cache: 'reload' });
                if (response.ok) await cache.put(path, response);
            } catch (_) {}
        }));
        await self.skipWaiting();
    })());
});

self.addEventListener('activate', event => {
    event.waitUntil((async () => {
        const keys = await caches.keys();
        await Promise.all(keys.filter(key => key.startsWith('faptopia-') && key !== SHELL_CACHE)
            .map(key => caches.delete(key)));
        await self.clients.claim();
    })());
});

self.addEventListener('fetch', event => {
    const request = event.request;
    if (request.method !== 'GET') return;
    const url = new URL(request.url);
    if (isMedia(url, request)) {
        // Keep Range, query parameters and abort signal. Avoid old HD entries
        // in the HTTP cache; application-owned video blobs are no longer read.
        event.respondWith(fetch(request, { cache: 'no-store' }));
        return;
    }
    if (url.origin !== self.location.origin) return;
    const navigation = request.mode === 'navigate'
        || (request.headers.get('Accept') || '').includes('text/html');
    event.respondWith((async () => {
        const cache = await caches.open(SHELL_CACHE);
        const hit = await cache.match(request);
        if (!navigation && hit) return hit;
        try {
            const response = await fetch(request);
            if (response.status === 200 && response.type !== 'opaque') {
                event.waitUntil(cache.put(request, response.clone()).catch(() => {}));
            }
            return response;
        } catch (_) {
            return hit || new Response('Offline', { status: 503 });
        }
    })());
});

// Compatibility for existing cache controls. Legacy video blobs are never
// used for playback; they remain inspectable/removable without deleting data
// unexpectedly during activation.
function legacyVideos(clear) {
    return new Promise((resolve, reject) => {
        const open = indexedDB.open('faptopia-videos', 1);
        open.onupgradeneeded = () => {
            if (!open.result.objectStoreNames.contains('videos')) {
                open.result.createObjectStore('videos', { keyPath: 'id' });
            }
        };
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
            const db = open.result;
            let count = 0, bytes = 0;
            let transaction;
            try {
                transaction = db.transaction('videos', clear ? 'readwrite' : 'readonly');
                const store = transaction.objectStore('videos');
                if (clear) store.clear();
                else store.openCursor().onsuccess = e => {
                    const cursor = e.target.result;
                    if (!cursor) return;
                    count++;
                    bytes += cursor.value.size || 0;
                    cursor.continue();
                };
            } catch (error) { db.close(); reject(error); return; }
            transaction.oncomplete = () => { db.close(); resolve({ count, bytes }); };
            transaction.onerror = () => { db.close(); reject(transaction.error); };
            transaction.onabort = () => { db.close(); reject(transaction.error); };
        };
    });
}

self.addEventListener('message', event => {
    const type = (event.data || {}).type;
    // Deliberately ignore old startup/mobile prefetch messages.
    if (type === 'video-prefetch') return;
    if (type !== 'video-cache-clear' && type !== 'video-cache-stats') return;
    event.waitUntil((async () => {
        const reply = value => { if (event.source) event.source.postMessage(value); };
        try {
            const stats = await legacyVideos(type === 'video-cache-clear');
            if (type === 'video-cache-clear') reply({ type: 'video-cache-cleared' });
            else {
                let quota = null;
                try { if (navigator.storage && navigator.storage.estimate) quota = await navigator.storage.estimate(); } catch (_) {}
                reply({ type: 'video-cache-stats', ...stats, quota, videoCachingEnabled: false });
            }
        } catch (error) {
            reply({ type: type === 'video-cache-clear' ? 'video-cache-cleared' : 'video-cache-stats', error: String(error) });
        }
    })());
});
