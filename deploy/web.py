#!/usr/bin/env python3
"""The container's command (ADR 0130): runs the server given as arguments, passes its output on, and serves
a page on WEB_PORT (7780) with that output and the newest client build, CLIENT_ZIP with version.txt beside it.
The server's 5 s stats line is kept apart as the page's status, so the log holds what happened."""
import collections, html, os, shutil, signal, subprocess, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG = collections.deque(maxlen=5000)
STATUS = [""]
ZIP = os.environ.get("CLIENT_ZIP", "/data/client/TechnoApe.zip")
# the launcher (deploy/join.bat) compares version.txt with its own and takes the zip when they differ
FILES = {"/TechnoApe.zip": ZIP, "/version.txt": os.path.join(os.path.dirname(ZIP), "version.txt")}
PAGE = """<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>TechnoApe server</title>
<style>body{background:#111;color:#ddd;font:15px system-ui;margin:16px}a{color:#7cf}
pre{background:#000;padding:8px;height:65vh;overflow:auto;white-space:pre-wrap}</style>
<h1>TechnoApe server</h1>
<p>%s</p>
<p>Unzip it and start <b>Join Tower.bat</b>: it joins the server, <b id=addr></b>, and from then on fetches a
newer build by itself. You need Tailscale running, Windows 10 or 11, a graphics card with ray tracing (RTX 20
series, RX 6000, Arc, or newer) on a current driver, and a CPU from 2013 or later.</p>
<p id=status></p><pre id=log aria-label="Server log"></pre>
<script>
addr.textContent = location.hostname + ':%s'
async function tick() {
  try {
    const t = await (await fetch('/log')).text(), i = t.indexOf('\\n')
    const end = log.scrollTop + log.clientHeight >= log.scrollHeight - 4
    status.textContent = t.slice(0, i); log.textContent = t.slice(i + 1)
    if (end) log.scrollTop = log.scrollHeight
  } catch (e) { status.textContent = 'server unreachable' }
}
tick(); setInterval(tick, 2000)
</script>"""


def build():
    if not os.path.isfile(ZIP):
        return "No client build uploaded yet."
    try:
        version = open(FILES["/version.txt"]).read().strip()
    except OSError:
        version = time.strftime("%Y-%m-%d %H:%M", time.localtime(os.path.getmtime(ZIP)))
    return '<a href="/TechnoApe.zip" download>Download the newest build</a> (%s, %.1f MB)' % (html.escape(version), os.path.getsize(ZIP) / 1e6)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/":
            self.reply("text/html; charset=utf-8", (PAGE % (build(), os.environ.get("PORT", "7777"))).encode())
        elif self.path == "/log":
            self.reply("text/plain; charset=utf-8", (STATUS[0].strip() + "\n" + "".join(LOG)).encode())
        elif self.path in FILES and os.path.isfile(FILES[self.path]):
            with open(FILES[self.path], "rb") as f:
                self.send_response(200)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(os.fstat(f.fileno()).st_size))
                self.end_headers()
                shutil.copyfileobj(f, self.wfile)
        else:
            self.send_error(404)

    def reply(self, kind, body):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *a):  # the page polls every 2 s: its requests are not the server's log
        pass


def main():
    child = subprocess.Popen(sys.argv[1:], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")

    def stop(*_):  # docker stop: the server saves and ends on its `stop` line
        try:
            child.stdin.write("stop\n")
            child.stdin.flush()
        except OSError:
            child.terminate()

    signal.signal(signal.SIGTERM, stop)
    web = ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("WEB_PORT", "7780"))), Handler)
    threading.Thread(target=web.serve_forever, daemon=True).start()
    for line in child.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        if line.startswith('{"tick"'):
            STATUS[0] = line
        else:
            LOG.append(time.strftime("%Y-%m-%d %H:%M:%S ") + line)
    sys.exit(child.wait())  # the server ended: so does the container, and Docker starts it again


if __name__ == "__main__":
    main()
