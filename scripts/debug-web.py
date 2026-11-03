#!/usr/bin/env python3
"""Run Trunk with a loopback-only sink for Forma's debug navigation log."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import subprocess
import sys
import threading
import time
from urllib.parse import parse_qs, urlparse
from urllib.request import urlopen
import webbrowser


ROOT = Path(__file__).resolve().parent.parent
LOG = ROOT / "forma-debug.log"
DEBUG_URL = "http://127.0.0.1:8080/?debug=1"


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path != "/log":
            self.send_error(404)
            return
        messages = parse_qs(parsed.query, keep_blank_values=True).get("message", [])
        if messages:
            with LOG.open("a", encoding="utf-8") as stream:
                for message in messages:
                    stream.write(message.replace("\r", "\\r").replace("\n", "\\n") + "\n")
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()

    def log_message(self, _format, *_args):
        pass


def open_debug_page_when_ready():
    for _ in range(240):
        try:
            with urlopen(DEBUG_URL, timeout=0.5):
                webbrowser.open(DEBUG_URL)
                return
        except OSError:
            time.sleep(0.25)


def main():
    LOG.write_text("", encoding="utf-8")
    server = ThreadingHTTPServer(("127.0.0.1", 8081), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    print(f"Forma navigation log: {LOG}", flush=True)
    trunk_args = sys.argv[1:]
    if "--open" in trunk_args:
        trunk_args = [argument for argument in trunk_args if argument != "--open"]
        threading.Thread(target=open_debug_page_when_ready, daemon=True).start()
    try:
        try:
            return subprocess.run(["trunk", "serve", *trunk_args], cwd=ROOT).returncode
        except KeyboardInterrupt:
            return 130
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    raise SystemExit(main())
