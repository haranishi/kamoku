#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""cargo build --message-format=jsonの出力からkamokuの実行ファイルを選ぶ。"""

import json
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print("使い方: cargo-executable.py <cargoのJSON出力>", file=sys.stderr)
        return 2
    try:
        messages = [json.loads(line) for line in Path(argv[0]).read_text("utf-8").splitlines() if line.strip()]
        executables = {
            message["executable"]
            for message in messages
            if message.get("reason") == "compiler-artifact"
            and message.get("target", {}).get("name") == "kamoku"
            and "bin" in message.get("target", {}).get("kind", [])
            and isinstance(message.get("executable"), str)
            and message["executable"]
        }
        if len(executables) != 1:
            raise ValueError("実行ファイルが一意ではありません")
    except (OSError, ValueError, TypeError, AttributeError):
        print("Cargoの出力からkamokuの実行ファイルを一意に選べませんでした。", file=sys.stderr)
        return 1
    print(executables.pop())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
