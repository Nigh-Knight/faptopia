const GALLERY_PATH = /^\/faptopia_(reddit|4chan)(?:\.html)?\/?$/;

async function isRefreshShell(response) {
    // Inspect only the beginning of the HTML; leave the original body untouched.
    const reader = response.clone().body?.getReader();
    if (!reader) return false;
    const decoder = new TextDecoder();
    let remaining = 1024;
    let prefix = '';
    try {
        while (remaining > 0) {
            const { value, done } = await reader.read();
            if (done) break;
            const chunk = value.subarray(0, remaining);
            prefix += decoder.decode(chunk, { stream: true });
            remaining -= chunk.length;
            if (/<\/head\s*>/i.test(prefix)) break;
        }
        return /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?refresh\b/i.test(prefix);
    } finally {
        // A tee branch's cancellation can wait for the original body to finish.
        void reader.cancel().catch(() => {});
    }
}

function galleryUnavailable() {
    return new Response(`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"><meta name="theme-color" content="#111"><link rel="manifest" href="/manifest.json"><title>Faptopia — Gallery unavailable</title><style>body{background:#111;color:#fff;font:1rem system-ui,sans-serif;padding:2rem}a{color:#9ecbff}nav{display:flex;gap:1.25rem}</style><main><h1>Gallery unavailable</h1><p>This deployment did not serve this gallery. Try the other feed or retry after a working build is uploaded.</p><nav aria-label="Galleries"><a href="/faptopia_reddit.html">Reddit</a><a href="/faptopia_4chan.html">4chan</a></nav></main></html>`, {
        status: 503,
        headers: { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' },
    });
}

export default {
    async fetch(request, env) {
        const url = new URL(request.url);
        const path = url.pathname;

        if (path.startsWith('/video/redgifs/')) {
            const gifId = path.replace('/video/redgifs/', '');
            try {
                const { hd, sd } = await resolveRedgifsVideo(gifId);
                const wantHd = url.searchParams.get('q') === 'hd';
                return proxyVideo(wantHd ? [hd, sd] : [sd, hd], {
                    'Referer': 'https://www.redgifs.com/',
                    'Origin': 'https://www.redgifs.com',
                }, request);
            } catch (e) {
                return new Response('Failed to resolve video', { status: 502 });
            }
        }

        if (path.startsWith('/proxy/4cdn/')) {
            const videoPath = path.replace('/proxy/4cdn/', '');
            return proxyVideo(`https://i.4cdn.org/${videoPath}${url.search}`, {
                'Referer': 'https://boards.4chan.org/',
            }, request);
        }

        const response = await env.ASSETS.fetch(request);
        if (request.method !== 'GET' || !GALLERY_PATH.test(path)) return response;
        if (response.status >= 300 && response.status < 400) return response;
        if (!response.ok || !(response.headers.get('Content-Type') || '').toLowerCase().includes('text/html') || await isRefreshShell(response)) {
            void response.body?.cancel().catch(() => {});
            return galleryUnavailable();
        }
        return response;
    }
};

let cachedToken = null;
let tokenExpiry = 0;
const RESOLVE_CACHE_TTL_SECS = 4 * 60 * 60;

async function getRedgifsToken() {
    if (cachedToken && Date.now() < tokenExpiry) return cachedToken;
    const resp = await fetch('https://api.redgifs.com/v2/auth/temporary', {
        headers: { 'User-Agent': 'rust:faptopia:v1.0' }
    });
    const data = await resp.json();
    cachedToken = data.token;
    tokenExpiry = Date.now() + 30 * 60 * 1000;
    return cachedToken;
}

async function resolveRedgifsVideo(gifId) {
    const normalized = gifId.toLowerCase();
    const cacheKey = new Request(`https://faptopia-resolve-cache.local/redgifs/${normalized}`);
    const cache = caches.default;
    const cached = await cache.match(cacheKey);
    if (cached) {
        const text = await cached.text();
        if (text) {
            try {
                const parsed = JSON.parse(text);
                if (parsed && parsed.hd) return { hd: parsed.hd, sd: parsed.sd || parsed.hd };
            } catch (_) {
                return { hd: text, sd: text };
            }
        }
    }
    const fetchResolve = async (token) => {
        const resp = await fetch(`https://api.redgifs.com/v2/gifs/${normalized}`, {
            headers: {
                'User-Agent': 'rust:faptopia:v1.0',
                'Authorization': `Bearer ${token}`
            }
        });
        if (!resp.ok) throw new Error(`RedGifs API error: ${resp.status}`);
        const data = await resp.json();
        const urls = data.gif.urls;
        const hd = urls.hd || urls.sd;
        const sd = urls.sd || hd;
        return { hd, sd };
    };
    let resolved;
    try {
        const token = await getRedgifsToken();
        resolved = await fetchResolve(token);
    } catch (_) {
        cachedToken = null;
        const newToken = await getRedgifsToken();
        resolved = await fetchResolve(newToken);
    }
    await cache.put(cacheKey, new Response(JSON.stringify(resolved), {
        headers: {
            'Cache-Control': `public, max-age=${RESOLVE_CACHE_TTL_SECS}`,
            'Content-Type': 'application/json',
        },
    }));
    return resolved;
}

async function proxyVideo(targetUrls, headers, originalRequest) {
    const urls = Array.isArray(targetUrls) ? targetUrls : [targetUrls];
    const reqHeaders = {
        'User-Agent': 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36',
        ...headers,
    };
    const range = originalRequest.headers.get('Range');
    if (range) reqHeaders['Range'] = range;
    let lastStatus = 502;
    for (const targetUrl of urls) {
        const resp = await fetch(targetUrl, { headers: reqHeaders });
        if (!resp.ok) {
            lastStatus = resp.status;
            continue;
        }
        const responseHeaders = new Headers();
        responseHeaders.set('Access-Control-Allow-Origin', '*');
        responseHeaders.set('Cache-Control', 'public, max-age=3600');
        responseHeaders.set('Accept-Ranges', 'bytes');
        for (const h of ['Content-Length', 'Content-Range', 'Accept-Ranges']) {
            const val = resp.headers.get(h);
            if (val) responseHeaders.set(h, val);
        }
        if (targetUrl.includes('.webm')) {
            responseHeaders.set('Content-Type', 'video/webm');
        } else if (targetUrl.includes('.mp4')) {
            responseHeaders.set('Content-Type', 'video/mp4');
        } else {
            const ct = resp.headers.get('Content-Type');
            if (ct) responseHeaders.set('Content-Type', ct);
        }
        return new Response(resp.body, { status: resp.status, headers: responseHeaders });
    }
    return new Response('Video unavailable', { status: lastStatus });
}
