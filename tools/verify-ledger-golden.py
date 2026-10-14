#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""台帳のハッシュの固定値を、Rustとは別の実装で計算し直して突き合わせる。

`crates/kamoku-ledger/src/encoding.rs` の先頭に書いた符号化の仕様だけを見て書いた。
Rustのコードは読まない。2つの実装が同じ値を出すことで、仕様の書き間違いと
実装の誤りの両方に気づけるようにするため。

    python3 tools/verify-ledger-golden.py          # 突き合わせる（食い違えば終了コード1）
    python3 tools/verify-ledger-golden.py --print  # 計算した値を表示するだけ

標準ライブラリだけを使う。
"""

from __future__ import annotations

import hashlib
import re
import struct
import sys
from pathlib import Path

DEBIT, CREDIT = 1, 2
POSTED, REVISED, VOIDED = 1, 2, 3


def lp(text: str) -> bytes:
    raw = text.encode("utf-8")
    return struct.pack(">I", len(raw)) + raw


def opt_lp(text: str | None) -> bytes:
    return b"\x00" if text is None else b"\x01" + lp(text)


def enc_line(line: tuple) -> bytes:
    account, side, amount, tax_code, tax_amount, memo = line
    return (
        lp(account)
        + bytes([side])
        + struct.pack(">q", amount)
        + opt_lp(tax_code)
        + struct.pack(">q", tax_amount)
        + lp(memo)
    )


def enc_meta(meta: dict[str, str]) -> bytes:
    if not meta:
        return b""
    out = b"\x01" + struct.pack(">I", len(meta))
    for key in sorted(meta, key=lambda k: k.encode("utf-8")):
        out += lp(key) + lp(meta[key])
    return out


def enc_entry(entry: dict) -> bytes:
    year, month, day = entry["date"]
    out = lp(entry["id"]) + struct.pack(">i", year) + bytes([month, day])
    out += lp(entry["description"]) + struct.pack(">I", len(entry["lines"]))
    for line in entry["lines"]:
        out += enc_line(line)
    return out + enc_meta(entry.get("meta", {}))


def event_hash(prev: bytes, seq: int, event: dict) -> bytes:
    data = lp("kamoku.ledger.event.v1") + prev
    data += struct.pack(">Q", seq) + bytes([event["kind"]])
    data += lp(event["entry_id"]) + struct.pack(">I", event["revision"])
    data += struct.pack(">q", event["recorded_at"]) + opt_lp(event.get("reason"))
    entry = event.get("entry")
    data += b"\x00" if entry is None else b"\x01" + enc_entry(entry)
    return hashlib.sha256(data).digest()


def chain(events: list[dict]) -> list[str]:
    prev = bytes(32)
    hashes = []
    for seq, event in enumerate(events, start=1):
        prev = event_hash(prev, seq, event)
        hashes.append(prev.hex())
    return hashes


# Phase 0 の固定値（`fixed_event_hashes`）と同じ入力。
SALE = ("101", DEBIT, 110_000, None, 0, "")
PHASE0 = [
    {"kind": POSTED, "entry_id": "E-0001", "revision": 1, "recorded_at": 1_767_571_200,
     "entry": {"id": "E-0001", "date": (2026, 1, 5), "description": "売上入金 テスト",
               "lines": [SALE, ("401", CREDIT, 110_000, "TAX10", 10_000, "テスト売上")]}},
    {"kind": REVISED, "entry_id": "E-0001", "revision": 2, "recorded_at": 1_767_657_600,
     "reason": "金額訂正",
     "entry": {"id": "E-0001", "date": (2026, 1, 5), "description": "売上入金 テスト(訂正)",
               "lines": [("101", DEBIT, 108_000, None, 0, ""),
                         ("401", CREDIT, 108_000, "TAX10", 9818, "テスト売上")]}},
    {"kind": POSTED, "entry_id": "E-0002", "revision": 1, "recorded_at": 1_768_003_200,
     "entry": {"id": "E-0002", "date": (2026, 1, 10), "description": "消耗品",
               "lines": [("510", DEBIT, 3300, "TAX10", 300, ""), ("101", CREDIT, 3300, None, 0, "")]}},
    {"kind": VOIDED, "entry_id": "E-0002", "revision": 2, "recorded_at": 1_768_089_600,
     "reason": "重複入力"},
]

# 付随情報つき（`meta_event_hashes`）。1組、3組（Rust側は鍵の降順に挿入）、日本語の値。
EXPENSE_LINES = [
    ("exp_comm", DEBIT, 7700, "P10", 700, ""),
    ("owner_draw", DEBIT, 3300, None, 0, "家事按分 30%"),
    ("w02", CREDIT, 11_000, None, 0, ""),
]
WITH_META = [
    {"kind": POSTED, "entry_id": "E-1001", "revision": 1, "recorded_at": 1_791_158_400,
     "entry": {"id": "E-1001", "date": (2026, 10, 5), "description": "10月分 通信費",
               "lines": EXPENSE_LINES, "meta": {"kind": "expense"}}},
    {"kind": POSTED, "entry_id": "E-1002", "revision": 1, "recorded_at": 1_791_244_800,
     "entry": {"id": "E-1002", "date": (2026, 10, 6), "description": "売上 デモ",
               "lines": [("w01", DEBIT, 55_000, None, 0, ""), ("sales", CREDIT, 55_000, "S10", 5000, "")],
               "meta": {"source": "import", "partner": "Sample Shop", "kind": "income"}}},
    {"kind": REVISED, "entry_id": "E-1001", "revision": 2, "recorded_at": 1_791_331_200,
     "reason": "取引先の入力漏れ",
     "entry": {"id": "E-1001", "date": (2026, 10, 5), "description": "10月分 通信費",
               "lines": EXPENSE_LINES, "meta": {"kind": "expense", "partner": "サンプル通信（デモ）"}}},
]

GROUPS = {"fixed_event_hashes": PHASE0, "meta_event_hashes": WITH_META}
BLOCK = re.compile(r"//\s*GOLDEN-BEGIN\s+(\w+)\s*\n(.*?)//\s*GOLDEN-END", re.S)
HEX = re.compile(r'"([0-9a-f]{64})"')


def main(argv: list[str]) -> int:
    computed = {name: chain(events) for name, events in GROUPS.items()}
    if "--print" in argv:
        for name, hashes in computed.items():
            print(name)
            for value in hashes:
                print(f'    "{value}",')
        return 0
    root = Path(__file__).resolve().parent.parent
    golden = root / "crates" / "kamoku-ledger" / "tests" / "golden.rs"
    found = {name: HEX.findall(body) for name, body in BLOCK.findall(golden.read_text("utf-8"))}
    failed = False
    for name in sorted(set(computed) | set(found)):
        expected, actual = computed.get(name), found.get(name)
        if expected == actual:
            print(f"一致: {name}（{len(actual)}件）")
            continue
        failed = True
        if actual is None:
            print(f"食い違い: {name} の GOLDEN-BEGIN/GOLDEN-END が golden.rs にありません")
        elif expected is None:
            print(f"食い違い: golden.rs の {name} を計算する定義が、このスクリプトにありません")
        else:
            print(f"食い違い: {name}")
            for index in range(max(len(expected), len(actual))):
                mine = expected[index] if index < len(expected) else "(なし)"
                theirs = actual[index] if index < len(actual) else "(なし)"
                mark = "  " if mine == theirs else "!!"
                print(f"  {mark} {index + 1}: 計算 {mine} / 固定値 {theirs}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
