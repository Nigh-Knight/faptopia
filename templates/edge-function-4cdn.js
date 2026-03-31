export default async (request) => {
    const url = new URL(request.url);
    const videoPath = url.pathname.replace('/4cdn/', '');

    const resp = await fetch(`https://i.4cdn.org/${videoPath}`, {
        headers: {
            'Referer': 'https://boards.4chan.org/',
            'User-Agent': 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36',
        },
    });

    return new Response(resp.body, {
        status: resp.status,
        headers: {
            'Content-Type': resp.headers.get('Content-Type') || 'video/mp4',
            'Access-Control-Allow-Origin': '*',
            'Cache-Control': 'public, max-age=86400',
        },
    });
};

export const config = { path: "/4cdn/*" };
