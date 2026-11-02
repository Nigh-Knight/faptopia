import http.server
import urllib.request
import json
import sys
import os
import socket

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 6969
DIRECTORY = sys.argv[2] if len(sys.argv) > 2 else '.'

PROXIES = {
    '/proxy/4cdn/': ('https://i.4cdn.org/', {'Referer': 'https://boards.4chan.org/'}),
}

# Cached RedGifs auth token
_redgifs_token = None

def get_redgifs_token():
    global _redgifs_token
    if _redgifs_token:
        return _redgifs_token
    req = urllib.request.Request('https://api.redgifs.com/v2/auth/temporary')
    req.add_header('User-Agent', 'rust:faptopia:v1.0')
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())
    _redgifs_token = data['token']
    return _redgifs_token

def resolve_redgifs_video(gif_id):
    global _redgifs_token
    token = get_redgifs_token()
    req = urllib.request.Request(f'https://api.redgifs.com/v2/gifs/{gif_id.lower()}')
    req.add_header('User-Agent', 'rust:faptopia:v1.0')
    req.add_header('Authorization', f'Bearer {token}')
    try:
        resp = urllib.request.urlopen(req)
    except urllib.error.HTTPError:
        # Token might be expired, retry with fresh one
        _redgifs_token = None
        token = get_redgifs_token()
        req = urllib.request.Request(f'https://api.redgifs.com/v2/gifs/{gif_id.lower()}')
        req.add_header('User-Agent', 'rust:faptopia:v1.0')
        req.add_header('Authorization', f'Bearer {token}')
        resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())
    urls = data['gif']['urls']
    hd = urls.get('hd') or urls['sd']
    sd = urls.get('sd') or hd
    return hd, sd

class ProxyHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)

    def do_GET(self):
        # On-demand RedGifs video resolution
        if self.path.startswith('/video/redgifs/'):
            gif_id = self.path[len('/video/redgifs/'):]
            try:
                hd_url, sd_url = resolve_redgifs_video(gif_id)
                # Try HD first, fall back to SD if the CDN refuses it
                self._proxy_url([hd_url, sd_url], {
                    'Referer': 'https://www.redgifs.com/',
                    'Origin': 'https://www.redgifs.com',
                })
            except Exception as e:
                try:
                    self.send_error(502, str(e))
                except (BrokenPipeError, ConnectionResetError):
                    pass
            return

        # 4chan proxy
        for prefix, (upstream, headers) in PROXIES.items():
            if self.path.startswith(prefix):
                remote_path = self.path[len(prefix):]
                url = upstream + remote_path
                self._proxy_url(url, headers)
                return

        super().do_GET()

    def _proxy_url(self, urls, headers):
        if isinstance(urls, str):
            urls = [urls]
        last_error = None
        for url in urls:
            req = urllib.request.Request(url)
            req.add_header('User-Agent', 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36')
            for k, v in headers.items():
                req.add_header(k, v)
            range_header = self.headers.get('Range')
            if range_header:
                req.add_header('Range', range_header)
            try:
                resp = urllib.request.urlopen(req)
            except urllib.error.HTTPError as e:
                last_error = e
                continue
            except Exception as e:
                last_error = e
                continue
            try:
                self.send_response(resp.status)
                for h in ['Content-Length', 'Content-Range', 'Accept-Ranges']:
                    val = resp.headers.get(h)
                    if val:
                        self.send_header(h, val)
                # Set correct Content-Type based on extension (Brave/Chrome strict about MIME)
                if url.endswith('.webm'):
                    self.send_header('Content-Type', 'video/webm')
                elif url.endswith('.mp4'):
                    self.send_header('Content-Type', 'video/mp4')
                else:
                    ct = resp.headers.get('Content-Type')
                    if ct:
                        self.send_header('Content-Type', ct)
                self.send_header('Accept-Ranges', 'bytes')
                self.send_header('Access-Control-Allow-Origin', '*')
                self.end_headers()
                while True:
                    chunk = resp.read(65536)
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                return
            except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                return
            except Exception as e:
                last_error = e
                continue
        # All candidate URLs failed
        code = getattr(last_error, 'code', 502) if last_error else 502
        reason = getattr(last_error, 'reason', str(last_error)) if last_error else 'proxy error'
        try:
            self.send_error(code, str(reason))
        except (BrokenPipeError, ConnectionResetError):
            pass

    def handle_one_request(self):
        try:
            super().handle_one_request()
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            pass

    def log_message(self, format, *args):
        pass

print(f'Server running on port {PORT}')
http.server.HTTPServer(('0.0.0.0', PORT), ProxyHandler).serve_forever()
