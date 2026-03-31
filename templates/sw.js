const CACHE_NAME = 'faptopia-v1';

self.addEventListener('install', (e) => {
    e.waitUntil(
        caches.open(CACHE_NAME).then(cache =>
            cache.addAll(['/', '/faptopia_reddit.html', '/faptopia_4chan.html'])
                .catch(() => {}) // Don't fail if files don't exist yet
        )
    );
    self.skipWaiting();
});

self.addEventListener('activate', (e) => {
    e.waitUntil(
        caches.keys().then(keys =>
            Promise.all(keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k)))
        )
    );
    self.clients.claim();
});

self.addEventListener('fetch', (e) => {
    // Always fetch videos from network (too large to cache)
    if (e.request.url.includes('redgifs') || e.request.url.includes('4cdn')) return;

    e.respondWith(
        fetch(e.request)
            .then(res => {
                const clone = res.clone();
                caches.open(CACHE_NAME).then(cache => cache.put(e.request, clone));
                return res;
            })
            .catch(() => caches.match(e.request))
    );
});
