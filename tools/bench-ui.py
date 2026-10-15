#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare two embedded production UIs using fresh synthetic demo servers.

Build both native binaries with the same toolchain/release flags first. Browser
dependencies must already be installed in e2e/. This tool installs nothing.
Authentication travels through anonymous server output and the Node stdin pipe;
only aggregate metrics and binary hashes reach stdout/the requested JSON file.
"""

import argparse
from contextlib import ExitStack, contextmanager
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import tempfile
import time


MARKER = "Synthetic UI benchmark"
ROWS = 200


class BenchmarkError(Exception):
    """A fixed diagnostic containing no HTTP/server output."""


class Client:
    def __init__(self, port, timeout):
        self.origin = f"http://127.0.0.1:{port}"
        self.port, self.timeout, self.cookie = port, timeout, ""

    def request(self, method, path, body=None, content_type="application/json"):
        headers = {"Origin": self.origin}
        if self.cookie:
            headers["Cookie"] = self.cookie
        if body is not None:
            headers["Content-Type"] = content_type
            if not isinstance(body, bytes):
                body = json.dumps(body).encode("utf-8")
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=self.timeout)
        try:
            conn.request(method, path, body, headers)
            response = conn.getresponse()
            return response.status, dict(response.getheaders()), response.read()
        finally:
            conn.close()

    def json(self, method, path, body=None, content_type="application/json"):
        status, _, data = self.request(method, "/api/v1" + path, body, content_type)
        if status not in (200, 201):
            raise BenchmarkError("Fixture API request failed")
        value = json.loads(data)
        if not isinstance(value, dict):
            raise BenchmarkError("Fixture API response was not an object")
        return value


@contextmanager
def demo_server(binary, temporary, timeout):
    temporary.mkdir(mode=0o700)
    with tempfile.TemporaryFile(dir=temporary) as private_output:
        proc = subprocess.Popen(
            [str(binary), "demo", "--port", "0", "--no-open"],
            stdin=subprocess.DEVNULL, stdout=private_output, stderr=private_output,
            env={**os.environ, "TMPDIR": str(temporary)},
        )
        try:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if hasattr(os, "pread"):
                    output = os.pread(private_output.fileno(), 65536, 0)
                else:
                    private_output.seek(0)
                    output = private_output.read(65536)
                match = re.search(rb"http://127\.0\.0\.1:(\d+)/\?t=([0-9a-f]+)\r?\n", output)
                if match:
                    client = Client(int(match[1]), timeout)
                    status, headers, _ = client.request("GET", "/?t=" + match[2].decode("ascii"))
                    cookie = next((v for k, v in headers.items() if k.lower() == "set-cookie"), "")
                    if status != 303 or "=" not in cookie:
                        raise BenchmarkError("Demo authentication failed")
                    client.cookie = cookie.split(";", 1)[0]
                    yield client
                    return
                if proc.poll() is not None:
                    raise BenchmarkError("Demo exited during startup")
                time.sleep(0.01)
            raise BenchmarkError("Demo startup timed out")
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=5)


def prepare(client):
    before = client.json("GET", "/status")
    if (before.get("mode") != "demo" or before.get("setup_done") is not True
            or before.get("ledger", {}).get("verified") is not True):
        raise BenchmarkError("Unverified synthetic demo fixture")
    accounts = client.json("GET", "/accounts")["items"]
    wallet = next((a["code"] for a in accounts if a.get("enabled")
                   and (a.get("wallet") or {}).get("type") == "card"), None)
    if wallet is None:
        raise BenchmarkError("Synthetic demo has no card wallet")
    lines = ["日付,摘要,出金,入金"] + [
        f"2026-02-{i % 28 + 1:02},{MARKER} {i + 1:03},{300 + i * 10},"
        for i in range(ROWS)
    ]
    uploaded = client.json("POST", "/imports/upload?name=synthetic-ui.csv&encoding=utf-8",
                           "\r\n".join(lines).encode("utf-8"), "application/octet-stream")
    committed = client.json("POST", "/imports/commit", {
        "upload_id": uploaded["upload_id"], "wallet": wallet,
        "mapping": uploaded["guess"], "save_profile": False,
    })
    if committed.get("created") != ROWS or committed.get("invalid") != 0:
        raise BenchmarkError("Synthetic import did not create exactly 200 valid rows")
    drafts = client.json("GET", "/drafts?q=Synthetic%20UI%20benchmark&limit=500")
    entries = client.json("GET", "/entries?state=all&limit=50")
    if drafts.get("total") != ROWS or len(drafts.get("items", [])) != ROWS or not entries.get("items"):
        raise BenchmarkError("Synthetic UI fixture counts were incorrect")
    return {
        "private_ledger_hash": before["ledger"]["head_hash"],
        "entries": before["ledger"]["entries"], "events": before["ledger"]["events"],
        "initial_drafts": before["inbox"]["draft"], "benchmark_drafts": ROWS,
        "journal_ids": [item["id"] for item in entries["items"]],
    }


def browser_run(runner, config, timeout, temporary):
    proc = subprocess.Popen(
        ["node", str(runner)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=os.name == "posix",
        env={**os.environ, "TMPDIR": str(temporary)},
    )
    try:
        stdout, _ = proc.communicate(json.dumps(config).encode("utf-8"), timeout=timeout)
        report = json.loads(stdout)
        if proc.returncode != 0 or report.get("complete") is not True:
            stage = report.get("failed_stage", "unknown")
            allowed = r"(?:browser_launch|(?:baseline|head)_(?:cold_home|cold_journal|home_to_journal|journal_to_home|inbox_setup|inbox_input))"
            stage = stage if isinstance(stage, str) and re.fullmatch(allowed, stage) else "unknown"
            raise BenchmarkError("Browser measurement failed during " + stage)
        return report
    finally:
        if proc.poll() is None:
            if os.name == "posix":
                os.killpg(proc.pid, signal.SIGTERM)
            else:
                proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                if os.name == "posix":
                    os.killpg(proc.pid, signal.SIGKILL)
                else:
                    proc.kill()
                proc.wait(timeout=5)


def binary_info(path, revision):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return {"sha256": digest.hexdigest(), "bytes": path.stat().st_size, "revision": revision}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--head", type=Path, required=True)
    parser.add_argument("--baseline-ref", default="unrecorded")
    parser.add_argument("--head-ref", default="uncommitted")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--input-samples", type=int, default=20)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--total-timeout", type=float, default=600)
    parser.add_argument("--channel", choices=["chrome", "chromium"], default="chromium")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not (1 <= args.samples <= 100 and 1 <= args.input_samples <= 100):
        parser.error("sample counts must be in 1..100")
    if any(not math.isfinite(v) or v <= 0 for v in [args.timeout, args.total_timeout]):
        parser.error("timeouts must be positive finite numbers")
    if any(not re.fullmatch(r"(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64}|uncommitted|unrecorded)", ref)
           for ref in [args.baseline_ref, args.head_ref]):
        parser.error("revision labels must be a commit hash, uncommitted, or unrecorded")
    report = {"schema": 1, "complete": False}
    stage = "preparation"
    try:
        binaries = {"baseline": args.baseline.resolve(), "head": args.head.resolve()}
        if any(not path.is_file() or not os.access(path, os.X_OK) for path in binaries.values()):
            raise BenchmarkError("A supplied binary was unavailable")
        runner = Path(__file__).resolve().parents[1] / "e2e/ui-performance.mjs"
        with tempfile.TemporaryDirectory(prefix="ui-bench-private-", dir=args.work_dir) as temporary:
            with ExitStack() as stack:
                stage = "startup"
                clients = {name: stack.enter_context(demo_server(binary, Path(temporary) / name, args.timeout))
                           for name, binary in binaries.items()}
                stage = "fixture"
                fixtures = {name: prepare(client) for name, client in clients.items()}
                if fixtures["baseline"] != fixtures["head"]:
                    raise BenchmarkError("Demo fixtures differed between builds")
                config = {"samples": args.samples, "inputSamples": args.input_samples,
                          "timeoutMs": args.timeout * 1000, "channel": args.channel,
                          "journalIds": fixtures["baseline"]["journal_ids"], "marker": MARKER,
                          "variants": {name: {"origin": c.origin, "cookie": c.cookie}
                                       for name, c in clients.items()}}
                stage = "browser"
                report = browser_run(runner, config, args.total_timeout, temporary)
                report["fixture"] = {k: v for k, v in fixtures["baseline"].items()
                                     if k not in ["private_ledger_hash", "journal_ids"]}
                report["provenance"] = {
                    "baseline": binary_info(binaries["baseline"], args.baseline_ref),
                    "head": binary_info(binaries["head"], args.head_ref),
                    "platform": platform.platform(), "python": platform.python_version(),
                    "ui_source": "embedded production assets; no ui-dir override",
                    "harness": {
                        "python_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                        "browser_sha256": hashlib.sha256(runner.read_bytes()).hexdigest(),
                    },
                }
                text = json.dumps(report, ensure_ascii=False)
                if "?t=" in text or "kamoku_session=" in text or any(c.cookie in text for c in clients.values()):
                    raise BenchmarkError("Output secret check failed")
    except (Exception, KeyboardInterrupt) as error:
        report = {"schema": 1, "complete": False, "failed_stage": stage,
                  "error": str(error) if isinstance(error, BenchmarkError)
                  else "UI benchmark failed; authentication and private diagnostics withheld"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))
    return 0 if report.get("complete") else 1


if __name__ == "__main__":
    sys.exit(main())
