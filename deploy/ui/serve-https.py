#!/usr/bin/env python3
# Copyright 2026 Zyvor
# SPDX-License-Identifier: Apache-2.0
"""Built-in HTTPS static file server + optional /api reverse proxy (no nginx).

Usage:
  python3 serve-https.py --port 27173 [--dir .]
  GUESTKIT_API_UPSTREAM=http://127.0.0.1:8080 python3 serve-https.py --port 27173

If --api-upstream / GUESTKIT_API_UPSTREAM is set, /api/* is proxied to that
zyvor-api base (scheme://host:port, without /api/v1).
"""

from __future__ import annotations

import argparse
import http.client
import http.server
import os
import ssl
import subprocess
import sys
import urllib.parse
from pathlib import Path


def ensure_certs(cert: Path, key: Path) -> None:
    if cert.is_file() and key.is_file():
        return
    cert.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(key),
            "-out",
            str(cert),
            "-days",
            "825",
            "-subj",
            "/CN=guestkit-ui/O=Zyvor",
        ],
        check=True,
        capture_output=True,
    )


class Handler(http.server.SimpleHTTPRequestHandler):
    api_upstream: str | None = None

    def do_GET(self):  # noqa: N802
        if self._maybe_proxy():
            return
        super().do_GET()

    def do_HEAD(self):  # noqa: N802
        if self._maybe_proxy():
            return
        super().do_HEAD()

    def do_POST(self):  # noqa: N802
        if self._maybe_proxy():
            return
        self.send_error(405, "Method Not Allowed")

    def do_PUT(self):  # noqa: N802
        if self._maybe_proxy():
            return
        self.send_error(405, "Method Not Allowed")

    def do_DELETE(self):  # noqa: N802
        if self._maybe_proxy():
            return
        self.send_error(405, "Method Not Allowed")

    def _maybe_proxy(self) -> bool:
        if not self.api_upstream:
            return False
        parsed = urllib.parse.urlparse(self.path)
        if not parsed.path.startswith("/api/"):
            return False
        up = urllib.parse.urlparse(self.api_upstream)
        host = up.hostname or "127.0.0.1"
        port = up.port or (443 if up.scheme == "https" else 80)
        path = parsed.path + (f"?{parsed.query}" if parsed.query else "")
        length = int(self.headers.get("Content-Length", "0") or 0)
        body = self.rfile.read(length) if length else None
        try:
            if up.scheme == "https":
                conn = http.client.HTTPSConnection(host, port, timeout=120)
            else:
                conn = http.client.HTTPConnection(host, port, timeout=120)
            headers = {
                k: v
                for k, v in self.headers.items()
                if k.lower() not in {"host", "content-length", "transfer-encoding", "connection"}
            }
            headers["Host"] = host if not up.port else f"{host}:{port}"
            conn.request(self.command, path, body=body, headers=headers)
            resp = conn.getresponse()
            data = resp.read()
            self.send_response(resp.status)
            for k, v in resp.getheaders():
                if k.lower() in {"transfer-encoding", "connection", "content-encoding"}:
                    continue
                self.send_header(k, v)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(data)
            conn.close()
        except Exception as e:  # noqa: BLE001
            msg = f"API proxy error: {e}".encode()
            self.send_response(502)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(msg)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(msg)
        return True

    def log_message(self, fmt: str, *args) -> None:
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))


def main() -> int:
    ap = argparse.ArgumentParser(description="GuestKit UI HTTPS static server")
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--dir", default=".", help="Document root")
    ap.add_argument("--cert", default="tls/cert.pem")
    ap.add_argument("--key", default="tls/key.pem")
    ap.add_argument("--bind", default="0.0.0.0")
    ap.add_argument(
        "--api-upstream",
        default=os.environ.get("GUESTKIT_API_UPSTREAM", ""),
        help="Proxy /api/* to this host (e.g. http://127.0.0.1:8080)",
    )
    args = ap.parse_args()

    root = Path(args.dir).resolve()
    os.chdir(root)
    cert = Path(args.cert)
    key = Path(args.key)
    if not cert.is_absolute():
        cert = root / cert
    if not key.is_absolute():
        key = root / key
    ensure_certs(cert, key)

    Handler.api_upstream = (args.api_upstream or "").rstrip("/") or None
    httpd = http.server.ThreadingHTTPServer((args.bind, args.port), Handler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile=str(cert), keyfile=str(key))
    httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
    proxy = Handler.api_upstream or "(none)"
    print(
        f"guestkit-ui https://{args.bind}:{args.port}/ root={root} api_proxy={proxy}",
        flush=True,
    )
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nstopped", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
