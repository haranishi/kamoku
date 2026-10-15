#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exercise embedded Rust/WASM assets against a synthetic, authenticated server."""

import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parent.parent


def binary():
    result = subprocess.run(
        ["cargo", "build", "-p", "kamoku-app", "--bin", "kamoku", "--locked",
         "--message-format=json-render-diagnostics"],
        cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True,
    )
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if message.get("target", {}).get("name") == "kamoku" and message.get("executable"):
            return message["executable"]
    raise RuntimeError("Cargo did not return the kamoku executable.")


def start(server):
    selector = selectors.DefaultSelector()
    selector.register(server.stdout, selectors.EVENT_READ)
    startup = bytearray()
    try:
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            if server.poll() is not None:
                break
            for _key, _event in selector.select(timeout=0.2):
                # Never print server startup output: it includes an auth URL.
                startup.extend(os.read(server.stdout.fileno(), 4096))
                match = re.search(rb"(http://127\.0\.0\.1:\d+)/\?t=([0-9a-f]{32})(?![0-9a-f])", startup)
                if match:
                    return tuple(part.decode("ascii") for part in match.groups())
    finally:
        selector.close()
    raise RuntimeError("Synthetic server failed to start; auth-bearing output was withheld.")


def main():
    for filename in ["kamoku_ui.js", "kamoku_ui_bg.wasm"]:
        if not (ROOT / "ui/rust/pkg" / filename).is_file():
            raise RuntimeError("Build the Rust preview first: bash tools/build-rust-ui.sh")
    command = ROOT / "e2e/node_modules/@playwright/test/cli.js"
    if not command.is_file():
        raise RuntimeError("Prepare the browser tools first: cd e2e && npm ci")
    server = subprocess.Popen(
        [binary(), "demo", "--no-open", "--port", "0"],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )
    try:
        origin, token = start(server)
        environment = {**os.environ, "KAMOKU_URL": origin, "KAMOKU_TOKEN": token}
        subprocess.run(
            ["node", str(command), "test", "--config", "rust-ui.config.mjs", "--project=flows"],
            cwd=ROOT / "e2e", env=environment, check=True, timeout=600,
        )
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)
        server.stdout.close()


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        # Subprocess arguments contain no tokens; server env/output stay private.
        print(f"Rust UI verification failed: {error}", file=sys.stderr)
        sys.exit(1)
