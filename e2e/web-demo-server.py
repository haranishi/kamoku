#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""お試し版を /kamoku-demo/ だけで配る。標準ライブラリだけを使う。

配信先と同じ応答の見出し（CSP など）を、すべての応答に付ける。
"""

from __future__ import annotations

import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit


# 配信先の見出しと同じ。変えるときは配信先と合わせる。
SECURITY_HEADERS = (
    (
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; "
        "style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self'; "
        "font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; "
        "form-action 'self'; object-src 'none'",
    ),
    ("X-Frame-Options", "DENY"),
    ("X-Content-Type-Options", "nosniff"),
    ("Referrer-Policy", "strict-origin-when-cross-origin"),
    ("Cross-Origin-Opener-Policy", "same-origin"),
)


class Handler(SimpleHTTPRequestHandler):
    """サブパス以外は存在しないパスとして扱う。"""

    def end_headers(self) -> None:
        for name, value in SECURITY_HEADERS:
            self.send_header(name, value)
        # 組み立て直した出力を、古い写しで試さないため。
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002 — 基底クラスの名前
        pass

    def do_GET(self) -> None:  # noqa: N802 — 基底クラスの名前
        path = urlsplit(self.path).path
        if path == "/kamoku-demo":
            self.send_response(302)
            self.send_header("Location", "/kamoku-demo/")
            self.end_headers()
            return
        super().do_GET()

    def translate_path(self, path: str) -> str:
        url_path = urlsplit(path).path
        if url_path == "/kamoku-demo" or url_path.startswith("/kamoku-demo/"):
            relative = url_path.removeprefix("/kamoku-demo") or "/"
            return super().translate_path(relative)
        return super().translate_path("/__no_such_kamoku_demo__")

    def guess_type(self, path: str) -> str:
        if str(path).endswith(".wasm"):
            return "application/wasm"
        return super().guess_type(path)


def main() -> None:
    if len(sys.argv) != 3:
        print("usage: web-demo-server.py ROOT PORT", file=sys.stderr)
        sys.exit(2)
    root = Path(sys.argv[1]).resolve()
    if not (root / "index.html").is_file():
        print(f"missing {root / 'index.html'}", file=sys.stderr)
        sys.exit(1)
    port = int(sys.argv[2])
    handler = partial(Handler, directory=str(root))
    server = ThreadingHTTPServer(("127.0.0.1", port), handler)
    print(f"listening http://127.0.0.1:{port}/kamoku-demo/", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
