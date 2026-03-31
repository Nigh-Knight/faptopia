export default {
    async fetch(request, env) {
        const url = new URL(request.url);
        const path = url.pathname;

        // On-demand RedGifs video resolution: fetches a fresh URL each time
        if (path.startsWith('/video/redgifs/')) {
            const gifId = path.replace('/video/redgifs/', '');
            try {
                const videoUrl = await resolveRedgifsVideo(gifId);
                return proxyVideo(videoUrl, {
                    'Referer': 'https://www.redgifs.com/',
                    'Origin': 'https://www.redgifs.com',
                }, request);
            } catch (e) {
                return new Response('Failed to resolve video', { status: 502 });
            }
        }

        // Proxy 4chan videos (permanent CDN URLs, no resolution needed)
        if (path.startsWith('/proxy/4cdn/')) {
            const videoPath = path.replace('/proxy/4cdn/', '');
            return proxyVideo(`https://i.4cdn.org/${videoPath}${url.search}`, {
                'Referer': 'https://boards.4chan.org/',
            }, request);
        }

        // Serve static assets from Pages
        return env.ASSETS.fetch(request);
    }
};

// Cached RedGifs auth token (persists within worker isolate lifetime)
let cachedToken = null;
let tokenExpiry = 0;

async function getRedgifsToken() {
    if (cachedToken && Date.now() < tokenExpiry) {
        return cachedToken;
    }

    const resp = await fetch('https://api.redgifs.com/v2/auth/temporary', {
        headers: { 'User-Agent': 'rust:faptopia:v1.0' }
    });
    const data = await resp.json();
    cachedToken = data.token;
    tokenExpiry = Date.now() + 30 * 60 * 1000; // 30 minutes
    return cachedToken;
}

async function resolveRedgifsVideo(gifId) {
    const token = await getRedgifsToken();
    const resp = await fetch(`https://api.redgifs.com/v2/gifs/${gifId.toLowerCase()}`, {
        headers: {
            'User-Agent': 'rust:faptopia:v1.0',
            'Authorization': `Bearer ${token}`,
        }
    });

    if (!resp.ok) {
        // Token might be expired, retry with a fresh one
        cachedToken = null;
        const newToken = await getRedgifsToken();
        const retry = await fetch(`https://api.redgifs.com/v2/gifs/${gifId.toLowerCase()}`, {
            headers: {
                'User-Agent': 'rust:faptopia:v1.0',
                'Authorization': `Bearer ${newToken}`,
            }
        });
        if (!retry.ok) throw new Error(`RedGifs API error: ${retry.status}`);
        const data = await retry.json();
        return data.gif.urls.hd || data.gif.urls.sd;
    }

    const data = await resp.json();
    return data.gif.urls.hd || data.gif.urls.sd;
}

async function proxyVideo(targetUrl, headers, originalRequest) {
    const reqHeaders = {
        'User-Agent': 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36',
        ...headers,
    };

    // Forward range requests for video seeking
    const range = originalRequest.headers.get('Range');
    if (range) {
        reqHeaders['Range'] = range;
    }

    const resp = await fetch(targetUrl, { headers: reqHeaders });

    const responseHeaders = new Headers();
    responseHeaders.set('Access-Control-Allow-Origin', '*');
    responseHeaders.set('Cache-Control', 'public, max-age=3600');
    responseHeaders.set('Accept-Ranges', 'bytes');

    // Forward essential headers
    for (const h of ['Content-Length', 'Content-Range', 'Accept-Ranges']) {
        const val = resp.headers.get(h);
        if (val) responseHeaders.set(h, val);
    }

    // Set correct Content-Type based on file extension (Brave/Chrome are strict about MIME types)
    if (targetUrl.includes('.webm')) {
        responseHeaders.set('Content-Type', 'video/webm');
    } else if (targetUrl.includes('.mp4')) {
        responseHeaders.set('Content-Type', 'video/mp4');
    } else {
        const ct = resp.headers.get('Content-Type');
        if (ct) responseHeaders.set('Content-Type', ct);
    }

    return new Response(resp.body, {
        status: resp.status,
        headers: responseHeaders,
    });
}
