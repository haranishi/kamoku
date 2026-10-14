#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""取込の試験に使う、合成データのCSVを作る。

中身はすべて架空です（店名に「サンプル」「デモ」を含める）。実在の口座・店・金額は使いません。
Shift_JIS（Windows-31J）とBOMつきのファイルは手で作りにくいので、このスクリプトで作ります。

    python3 tools/make-fixtures.py          # crates/kamoku-import/tests/fixtures/synthetic/ に書く
    python3 tools/make-fixtures.py --check  # 書かずに、今のファイルと同じかだけ確かめる（違えば終了コード1）

標準ライブラリだけを使う。
"""

from __future__ import annotations

import sys
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "crates" / "kamoku-import" / "tests" / "fixtures" / "synthetic"
BOM = b"\xef\xbb\xbf"

# (ファイル名, 文字コード, 先頭のBOM, 改行, 行の並び)
FIXTURES = [
    ("utf8.csv", "utf-8", b"", "\n", [
        "日付,摘要,出金,入金,残高",
        '2026/10/01,サンプルストア,"1,650",,"98,350"',
        '2026/10/02,デモ鉄道,420,,"97,930"',
        '2026/10/05,振込 サンプル商事,,"55,000","152,930"',
        '2026/10/05,振込 サンプル商事,,"55,000","207,930"',
    ]),
    ("utf8_bom.csv", "utf-8", BOM, "\r\n", [
        "取引日,お取引内容,お引出金額,お預入金額",
        '2026-10-01,ＡＴＭ　サンプル,"10,000",',
        "2026-10-03,利息 デモ,,1",
    ]),
    # Windows-31J の拡張文字（①）を含める。
    ("sjis.csv", "cp932", b"", "\r\n", [
        "年月日,摘要,お支払金額,お預り金額,残高",
        'R8.10.1,デモ商店①,"1,650",,98350',
        '令和8年10月2日,サンプル電力,"5,400",,92950',
        '2026年10月3日,デモ給与,,"300,000",392950',
    ]),
    ("split_columns.csv", "utf-8", b"", "\n", [
        "日付\t内容\t出金\t入金",
        "20261001\tサンプルカフェ\t880\t",
        "20261002\tデモ書店\t1980\t",
        "20261003\t返金 デモ書店\t\t1980",
    ]),
    ("signed.csv", "utf-8", b"", "\n", [
        "日付,摘要,金額",
        '2026/10/01,サンプル給与,"300,000"',
        '2026/10/02,カード引落 デモ,"△45,600"',
        "2026/10/03,手数料 デモ,▲220",
        "2026/10/04,返品 デモ,(1650)",
        "2026/10/05,調整 デモ,1650-",
        "2026/10/06,サンプル払戻,−500",
    ]),
    ("card.csv", "utf-8", b"", "\r\n", [
        "サンプルカード ご利用明細（デモ）",
        "ご利用日,ご利用先,ご利用金額,支払区分,今回支払額",
        '2026/10/01,サンプルマート,"¥1,650",1回,"¥1,650"',
        '2026/10/02,デモオンライン,"¥3,300",1回,"¥3,300"',
        '2026/10/05,デモオンライン 返品,"-¥3,300",1回,"-¥3,300"',
    ]),
    ("quoted.csv", "utf-8", b"", "\r\n", [
        "日付,摘要,出金,入金",
        '2026/10/01,"サンプル商店, 本店",500,',
        '2026/10/02,"デモ""引用""店",600,',
        '2026/10/03,"二行の\r\n摘要",700,',
        "2026/10/04,ふつうの行,800,",
    ]),
    ("broken_quote.csv", "utf-8", b"", "\n", [
        "日付,摘要,出金,入金",
        "2026/10/01,サンプル,100,",
        '2026/10/02,"閉じていない,200,',
        "2026/10/03,デモ,300,",
    ]),
]


def render(encoding: str, bom: bytes, newline: str, rows: list[str]) -> bytes:
    return bom + (newline.join(rows) + newline).encode(encoding)


def main(argv: list[str]) -> int:
    check = "--check" in argv
    OUT.mkdir(parents=True, exist_ok=True)
    differs = []
    for name, encoding, bom, newline, rows in FIXTURES:
        data = render(encoding, bom, newline, rows)
        path = OUT / name
        if check:
            if not path.exists() or path.read_bytes() != data:
                differs.append(name)
        else:
            path.write_bytes(data)
            print(f"書いた: {path.relative_to(OUT.parents[4])}（{len(data)}バイト）")
    if differs:
        print("今のファイルと違う: " + ", ".join(differs))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
