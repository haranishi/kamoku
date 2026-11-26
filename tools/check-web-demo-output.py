#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""組み立てたお試し版を、公開してよい形か調べる。標準ライブラリだけを使う。

- 全ファイルをバイト単位で調べ、手元の絶対パス（/Users/・/home/・/Volumes/・HOME の値）があれば失敗する。
  WASM に panic の場所や C の __FILE__ として残りやすいため。
- テキストの行が、秘密の値を書いたように見える形に当たれば失敗する（配信先の漏洩検査と同じ式）。
- 1ファイルが25MiBを超えたら失敗する（配信先の上限）。
成功したら、各ファイルの大きさを表示する。
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

MAX_BYTES = 25 * 1024 * 1024
PATH_MARKERS = (b"/Users/", b"/home/", b"/Volumes/")
SECRET_LINE = re.compile(
    r"""(api[_-]?key|secret|token|password)\s*[:=]\s*["'][^"']{8,}["']""",
    re.IGNORECASE,
)
TEXT_SUFFIXES = {".html", ".js", ".mjs", ".css", ".json", ".txt", ".svg", ".md"}


def markers(home: str | None) -> list[bytes]:
    found = list(PATH_MARKERS)
    # HOME が「/」だけのような短い値では、どのファイルにも当たってしまうので使わない。
    if home and len(home.strip("/")) >= 2:
        found.append(home.rstrip("/").encode())
    return found


def problems(root: Path, home: str | None) -> list[str]:
    """見つかった問題（日本語の1行ずつ）。空なら公開してよい。"""
    found: list[str] = []
    needles = markers(home)
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        rel = path.relative_to(root).as_posix()
        data = path.read_bytes()
        if len(data) > MAX_BYTES:
            found.append(f"{rel}: {len(data)} バイトで、25MiB の上限を超えています")
        for needle in needles:
            if needle in data:
                found.append(f"{rel}: 手元の絶対パス（{needle.decode(errors='replace')}）が入っています")
        if path.suffix.lower() in TEXT_SUFFIXES:
            text = data.decode("utf-8", errors="replace")
            for number, line in enumerate(text.splitlines(), start=1):
                if SECRET_LINE.search(line):
                    found.append(f"{rel}:{number}: 秘密の値を書いたように見える行があります")
    return found


def sizes(root: Path) -> list[tuple[str, int]]:
    return [
        (p.relative_to(root).as_posix(), p.stat().st_size)
        for p in sorted(root.rglob("*"))
        if p.is_file()
    ]


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: check-web-demo-output.py OUTPUT", file=sys.stderr)
        sys.exit(2)
    root = Path(sys.argv[1]).resolve()
    if not root.is_dir():
        print(f"出力がありません: {root}", file=sys.stderr)
        sys.exit(2)
    found = problems(root, os.environ.get("HOME"))
    if found:
        print("公開できないものが出力に入っています:", file=sys.stderr)
        for line in found:
            print(f"  {line}", file=sys.stderr)
        sys.exit(1)
    for rel, size in sizes(root):
        print(f"{size:>10}  {rel}")


if __name__ == "__main__":
    main()
