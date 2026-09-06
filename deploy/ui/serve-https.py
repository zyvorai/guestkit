#!/usr/bin/env python3
# Copyright 2026 Zyvor
# SPDX-License-Identifier: Apache-2.0
"""Built-in HTTPS static file server for GuestKit UI (no nginx).

Usage:
  python3 serve-https.py --port 27173 [--dir .] [--cert tls/cert.pem --key tls/key.pem]

If cert/key are missing, a self-signed cert is created under ./tls/ (lab use).
"""

from __future__ import annotations

import argparse
import http.server
import os
import ssl
import subprocess
import sys
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


def main() -> int:
    ap = argparse.ArgumentParser(description="GuestKit UI HTTPS static server")
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--dir", default=".", help="Document root")
    ap.add_argument("--cert", default="tls/cert.pem")
    ap.add_argument("--key", default="tls/key.pem")
    ap.add_argument("--bind", default="0.0.0.0")
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

    handler = http.server.SimpleHTTPRequestHandler
    httpd = http.server.ThreadingHTTPServer((args.bind, args.port), handler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile=str(cert), keyfile=str(key))
    httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
    print(f"guestkit-ui https://{args.bind}:{args.port}/ root={root}", flush=True)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nstopped", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
