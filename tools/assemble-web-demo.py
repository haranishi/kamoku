#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""ブラウザのお試し版の静的ファイルを1つのディレクトリにまとめる。

標準ライブラリだけを使う。出力先は置き換える。日時は build-info.json に入れない。
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LICENSE_NAMES = (
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "COPYING",
    "COPYING.md",
    "UNLICENSE",
    "LICENCE",
    "LICENCE.md",
    "LICENCE.txt",
)
# 案内の帯の「ソース（GitHub）」の行き先。画面のソースには外のURLを書かない決まりなので、ここで渡す。
SOURCE_URL = "https://github.com/haranishi/kamoku"
WASM_ROOTS = ("kamoku-web", "kamoku-ui")
# 画面の WASM の出力（ui/rust/pkg）のうち、配るもの。ほかがあれば、配り漏れになるので止める。
UI_PKG_FILES = {"kamoku_ui.js", "kamoku_ui_bg.wasm"}
# 前回の出力と分かる印。3つそろい、直下が出力の名前だけのときに限り、出力先を消して作り直す。
OUTPUT_MARKERS = ("build-info.json", "web/shim.js", "THIRD_PARTY_LICENSES.txt")
# 出力の直下に置く名前。ほかの物がある場所（取り込み先のサイトのフォルダなど）は消さない。.DS_Store は Finder が作る。
OUTPUT_ENTRIES = {
    "index.html", "favicon.svg", "css", "rust", "web", "build-info.json", "THIRD_PARTY_LICENSES.txt", ".DS_Store",
}
MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE")
KEEP_EXACT = {
    "favicon.svg",
    "rust/pkg/kamoku_ui.js",
    "rust/pkg/kamoku_ui_bg.wasm",
}


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    sys.exit(1)


def allowed(rel: str) -> bool:
    """ui_rules の配信範囲から、boot.js とプレビュー用のファイルを除く。"""
    if rel in KEEP_EXACT:
        return True
    parts = rel.split("/")
    if any(part.startswith(".") or not part for part in parts):
        return False
    return len(parts) == 2 and parts[0] == "css" and parts[1].endswith(".css")


def normal_dep(dep: dict) -> bool:
    kinds = dep.get("dep_kinds")
    if kinds is None:
        return dep.get("kind") in (None, "normal")
    return any(item.get("kind") in (None, "normal") for item in kinds)


def license_paths(pkg: dict) -> list[Path]:
    start = Path(pkg["manifest_path"]).parent
    names = list(LICENSE_NAMES)
    if pkg.get("license_file"):
        names.append(pkg["license_file"])
    found: list[Path] = []

    def add(directory: Path) -> None:
        candidates = [directory / name for name in names]
        # LICENSE-MIT.md のような名前もある。名前が LICENSE などで始まるファイルを拾う。
        if directory.is_dir():
            candidates += sorted(
                path
                for path in directory.iterdir()
                if path.name.upper().startswith(LICENSE_PREFIXES)
            )
        for path in candidates:
            candidate = path.resolve()
            if candidate.is_file() and candidate not in found:
                found.append(candidate)

    add(start)
    if start == ROOT or ROOT in start.parents:
        current = start.parent
        while True:
            add(current)
            if current == ROOT or current.parent == current:
                break
            current = current.parent
    return found


def wasm_packages() -> list[dict]:
    proc = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            "wasm32-unknown-unknown",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        fail(proc.stderr.strip() or "cargo metadata failed")
    data = json.loads(proc.stdout)
    by_id = {pkg["id"]: pkg for pkg in data["packages"]}
    nodes = {node["id"]: node for node in data["resolve"]["nodes"]}
    # 配る WASM は2つ。帳簿（kamoku-web）と画面（kamoku-ui）の両方の依存を載せる。
    roots = []
    for name in WASM_ROOTS:
        found = [pkg["id"] for pkg in data["packages"] if pkg["name"] == name]
        if len(found) != 1:
            fail(f"cargo metadata に {name} が1つ見つかりません")
        roots.extend(found)
    reached: list[dict] = []
    seen: set[str] = set()
    stack = list(roots)
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        pkg = by_id.get(current)
        if pkg is None:
            fail(f"依存の解決結果にパッケージがありません: {current}")
        reached.append(pkg)
        node = nodes.get(current, {})
        for dep in node.get("deps", []):
            if normal_dep(dep):
                stack.append(dep["pkg"])
    reached.sort(key=lambda pkg: (pkg["name"], pkg.get("version", "")))
    return reached


def license_text(packages: list[dict]) -> str:
    lines = [
        "kamoku ブラウザのお試し版に入る依存クレート。",
        "cargo metadata --format-version 1 --locked --filter-platform wasm32-unknown-unknown",
        "の解決結果から、kamoku-web（帳簿）と kamoku-ui（画面）の通常依存をたどって作った。日時は入れない。",
        "",
        "どちらの WASM にも、Rust の標準ライブラリ（MIT OR Apache-2.0）の一部が入る。",
        "",
    ]
    printed: dict[str, str] = {}
    for pkg in packages:
        license_name = pkg.get("license") or "（license 欄が無い・未確認）"
        lines.append(f"## {pkg['name']} {pkg.get('version', '')}".rstrip())
        lines.append(f"License: {license_name}")
        lines.append("")
        files = license_paths(pkg)
        if not files:
            lines.extend(missing_license(pkg, packages))
            continue
        for path in files:
            key = str(path)
            if key in printed:
                lines.append(f"（本文は {printed[key]} と同じ）")
                lines.append("")
                continue
            printed[key] = f"{pkg['name']} の {path.name}"
            body = path.read_text(encoding="utf-8", errors="replace").strip()
            lines.append(f"----- {path.name} -----")
            lines.append(body)
            lines.append("")
    lines.extend(sqlite_note(packages))
    return "\n".join(lines).rstrip() + "\n"


def sqlite_note(packages: list[dict]) -> list[str]:
    lines = ["## SQLite", ""]
    wasm = next((pkg for pkg in packages if pkg["name"] == "sqlite-wasm-rs"), None)
    if wasm is None:
        lines.append("依存グラフに sqlite-wasm-rs が無い（未確認）。")
    else:
        base = Path(wasm["manifest_path"]).parent
        headers = sorted(base.rglob("sqlite3.h"))
        if not headers:
            lines.append("sqlite-wasm-rs の sqlite3.h は見つかりませんでした（未確認）。")
        else:
            head = "\n".join(headers[0].read_text(encoding="utf-8", errors="replace").splitlines()[:12])
            if "disclaims copyright" in head:
                lines.append(
                    f"sqlite-wasm-rs {wasm.get('version', '')} が同梱する sqlite3.h の冒頭。"
                    "著者はこのソースの著作権を放棄すると宣言している。"
                )
                lines.append("")
                lines.append(head)
            else:
                lines.append("sqlite3.h はあったが、著作権放棄の文は確認できなかった（未確認）。")
        lines.append("")
        lines.append("ラッパー sqlite-wasm-rs の LICENSE は、上のそのクレートの節。")
    lines.append("")
    return lines


def missing_license(pkg: dict, packages: list[dict]) -> list[str]:
    """本文のファイルが無いクレート。同じリポジトリの本文か、MIT の定型文を、断り書きつきで載せる。"""
    repo = pkg.get("repository")
    sibling = next(
        (
            other
            for other in packages
            if other is not pkg and repo and other.get("repository") == repo and license_paths(other)
        ),
        None,
    )
    if sibling is not None:
        path = license_paths(sibling)[0]
        body = path.read_text(encoding="utf-8", errors="replace").strip()
        return [
            "このクレートにはライセンス本文のファイルが無い。Cargo.toml の repository が同じ "
            f"{sibling['name']} {sibling.get('version', '')} に同梱の {path.name} を、"
            "同じリポジトリの本文として載せる。",
            f"----- {path.name}（{sibling['name']} より） -----",
            body,
            "",
        ]
    lines = ["このクレートにはライセンス本文のファイルが無く、本文と著作権表示は未確認。"]
    authors = ", ".join(pkg.get("authors") or []) or "（Cargo.toml に authors 欄が無い）"
    lines.append(f"Cargo.toml の authors: {authors}")
    if pkg.get("license") == "MIT":
        lines.extend(["license 欄が MIT なので、MIT の定型文を載せる。", "", MIT_TEXT])
    lines.append("")
    return lines


def index_html() -> str:
    html = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")
    charset = '  <meta charset="utf-8">\n'
    metas = (
        charset
        + '  <meta name="kamoku-edition" content="web-demo">\n'
        + '  <meta name="kamoku-demo-today" content="2026-09-30">\n'
        + f'  <meta name="kamoku-source" content="{SOURCE_URL}">\n'
    )
    script = '  <script type="module" src="boot.js"></script>\n'
    replacement = (
        '  <link rel="stylesheet" href="web/demo.css">\n'
        '  <script type="module" src="web/shim.js"></script>\n'
    )
    boot = "        <noscript>\n"
    status = '        <p id="boot-status">読み込み中です。しばらくお待ちください。</p>\n' + boot
    for old, new, label in (
        (charset, metas, "charset"),
        (script, replacement, "boot.js"),
        (boot, status, "noscript"),
    ):
        if html.count(old) != 1:
            fail(f"ui/index.html の {label} を1か所見つけられません")
        html = html.replace(old, new, 1)
    scripts = [line.strip() for line in html.splitlines() if "<script" in line]
    if scripts != ['<script type="module" src="web/shim.js"></script>']:
        fail("index.html に shim 以外の script が残っています")
    if "style=" in html:
        fail("index.html に inline style があります")
    return html


def guarded_output(raw: str) -> Path:
    out = Path(raw).resolve()
    if out == ROOT or out in ROOT.parents:
        fail("出力先がリポジトリそのもの、またはその親です")
    protected = [ROOT / name for name in ("ui", "web", "crates", "docs", "e2e", "tools", ".git")]
    if any(out == path or out in path.parents for path in protected):
        fail("出力先がソースの中です")
    if (out / "Cargo.toml").exists() or (out / ".git").exists():
        fail("出力先が別のプロジェクトに見えます")
    return out


def clear_output(out: Path) -> None:
    """出力先が空か、前回の出力と分かるときだけ消す。ほかのファイルがある場所は、何も消さずに止める。"""
    if not out.exists():
        return
    if not out.is_dir():
        fail(f"出力先 {out} はフォルダではありません。何も消さずに止めました")
    if not any(out.iterdir()):
        out.rmdir()
        return
    names = {path.name for path in out.iterdir()}
    if names <= OUTPUT_ENTRIES and all((out / marker).is_file() for marker in OUTPUT_MARKERS):
        shutil.rmtree(out)
        return
    fail(
        f"出力先 {out} には、お試し版の前回の出力ではないファイルがあります。"
        "消してよいか分からないので、何も消さずに止めました。空のフォルダか、新しい場所を指定してください"
    )


def check_ui_pkg(pkg: Path) -> None:
    """ui/rust/pkg に、配る2ファイル以外（snippets のフォルダなど）があれば止める。"""
    if not pkg.is_dir():
        fail("画面の WASM（ui/rust/pkg）がありません。先に tools/build-rust-ui.sh を通してください")
    extra = sorted(path.name for path in pkg.iterdir() if path.name not in UI_PKG_FILES)
    if extra:
        fail(
            "ui/rust/pkg に、お試し版が配らないものがあります: "
            + ", ".join(extra)
            + "。画面が使うなら配り漏れになるので、組み立てを止めました"
        )


def copy_ui(out: Path) -> None:
    ui = ROOT / "ui"
    check_ui_pkg(ui / "rust" / "pkg")
    copied = False
    for path in ui.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(ui).as_posix()
        if not allowed(rel):
            continue
        dest = out / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, dest)
        copied = True
    required = [
        out / "favicon.svg",
        out / "css",
        out / "rust" / "pkg" / "kamoku_ui.js",
        out / "rust" / "pkg" / "kamoku_ui_bg.wasm",
    ]
    if not copied or any(not path.exists() for path in required):
        fail("画面のファイルが足りません。先に tools/build-rust-ui.sh を通してください")


def copy_web(out: Path, pkg: Path) -> None:
    web = out / "web"
    web.mkdir(parents=True)
    for name in ("shim.js", "demo.css"):
        source = ROOT / "web" / name
        if not source.is_file():
            fail(f"missing {source}")
        shutil.copy2(source, web / name)
    dest = web / "pkg"
    dest.mkdir()
    files = [path for path in pkg.iterdir() if path.is_file()]
    if not files:
        fail("wasm-bindgen の出力が空です")
    for path in files:
        shutil.copy2(path, dest / path.name)
    for name in ("kamoku_web.js", "kamoku_web_bg.wasm"):
        if not (dest / name).is_file():
            fail(f"wasm-bindgen の出力に {name} がありません")


def modified() -> bool:
    """作業フォルダにコミットしていない変更があるか。"""
    proc = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        fail(proc.stderr.strip() or "git status に失敗しました")
    return bool(proc.stdout.strip())


def commit() -> str:
    proc = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    value = proc.stdout.strip()
    if proc.returncode != 0 or not value:
        fail(proc.stderr.strip() or "git rev-parse HEAD に失敗しました")
    return value


def build_info(revision: str, dirty: bool) -> dict:
    info: dict = {"commit": revision}
    if dirty:
        # 元のコミットに無い変更から作った出力だと分かるようにする。
        info["modified"] = True
    return info


def main() -> None:
    if len(sys.argv) != 3:
        fail("usage: assemble-web-demo.py OUTPUT PKG_DIR")
    out = guarded_output(sys.argv[1])
    pkg = Path(sys.argv[2]).resolve()
    if not pkg.is_dir():
        fail("PKG_DIR がありません")
    packages = wasm_packages()
    licenses = license_text(packages)
    revision = commit()
    html = index_html()
    check_ui_pkg(ROOT / "ui" / "rust" / "pkg")
    clear_output(out)
    out.mkdir(parents=True)
    copy_ui(out)
    copy_web(out, pkg)
    (out / "index.html").write_text(html, encoding="utf-8")
    (out / "build-info.json").write_text(
        json.dumps(build_info(revision, modified()), ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    (out / "THIRD_PARTY_LICENSES.txt").write_text(licenses, encoding="utf-8")
    if (out / "boot.js").exists() or (out / "rust" / "index.html").exists():
        fail("配らないファイルが出力に入りました")


if __name__ == "__main__":
    main()
