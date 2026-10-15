#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare dashboard/one-draft confirmation using an already-built kamoku binary.

Run base and head release binaries on the same idle host with identical flags:
  python3 tools/bench-draft-queries.py --binary target/release/kamoku \
      --rows 10000 --samples 10 --timeout 5 --output result.json
Add --require-complete for the candidate. Timeouts are recorded, never counted as
successful latency samples. Invalid HTTP/JSON/accounting results always fail.
Only Python's standard library, loopback, and disposable synthetic DBs are used.
Server output contains authentication data; it is never printed or saved as an
artifact. A new server and copy of the seed DB isolate each measured operation.
"""

import argparse
from contextlib import closing, contextmanager
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import platform
import re
import socket
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time


AMOUNT = 110
WARMUPS = 1


class BenchmarkError(Exception):
    """A public, authentication-free benchmark failure."""


class RequestTimedOut(Exception):
    def __init__(self, elapsed_ms):
        self.elapsed_ms = elapsed_ms


class Client:
    def __init__(self, port, timeout):
        self.port = port
        self.timeout = timeout
        self.cookie = ""

    def request(self, method, path, body=None):
        headers = {"Origin": f"http://127.0.0.1:{self.port}"}
        if self.cookie:
            headers["Cookie"] = self.cookie
        if body is not None:
            headers["Content-Type"] = "application/json"
            body = json.dumps(body, ensure_ascii=False).encode("utf-8")
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=self.timeout)
        started = time.perf_counter()
        try:
            conn.request(method, path, body, headers)
            response = conn.getresponse()
            data = response.read()
            elapsed_ms = (time.perf_counter() - started) * 1000
            return response.status, dict(response.getheaders()), data, elapsed_ms
        except (socket.timeout, TimeoutError):
            raise RequestTimedOut((time.perf_counter() - started) * 1000) from None
        finally:
            conn.close()

    def json(self, method, path, body=None):
        status, _, data, elapsed_ms = self.request(method, "/api/v1" + path, body)
        if status != 200:
            raise BenchmarkError(f"API returned HTTP {status}")
        try:
            value = json.loads(data)
        except (UnicodeError, ValueError):
            raise BenchmarkError("API returned invalid JSON") from None
        if not isinstance(value, dict):
            raise BenchmarkError("API returned a non-object JSON value")
        return value, elapsed_ms


@contextmanager
def server(binary, data_dir, timeout):
    # Anonymous 0600 temporary file: no authentication URL enters the report or
    # a named log, even on startup failure or timeout. Polling has a real deadline.
    with tempfile.TemporaryFile(dir=data_dir.parent) as private_output:
        proc = subprocess.Popen(
            [str(binary), "serve", "--data", str(data_dir), "--port", "0", "--no-open"],
            stdin=subprocess.DEVNULL, stdout=private_output, stderr=private_output,
        )
        try:
            deadline = time.monotonic() + max(10, timeout)
            while time.monotonic() < deadline:
                # pread leaves the shared stdout file offset untouched while
                # the child is writing its startup message (Linux/macOS).
                if hasattr(os, "pread"):
                    startup_output = os.pread(private_output.fileno(), 65536, 0)
                else:
                    private_output.seek(0)
                    startup_output = private_output.read(65536)
                match = re.search(
                    rb"http://127\.0\.0\.1:(\d+)/\?t=([0-9a-zA-Z_-]+)\r?\n",
                    startup_output,
                )
                if match:
                    client = Client(int(match[1]), timeout)
                    status, headers, _, _ = client.request("GET", "/?t=" + match[2].decode("ascii"))
                    cookie = next((v for k, v in headers.items() if k.lower() == "set-cookie"), "")
                    if status != 303 or "=" not in cookie:
                        raise BenchmarkError("Server authentication failed")
                    client.cookie = cookie.split(";", 1)[0]
                    yield client
                    return
                if proc.poll() is not None:
                    raise BenchmarkError("Server exited before startup completed")
                time.sleep(0.01)
            raise BenchmarkError("Server startup timed out")
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=5)


def seed(binary, directory, rows, timeout):
    directory.mkdir()
    with server(binary, directory, timeout) as client:
        settings, _ = client.json("GET", "/settings")
        settings.update(business_name="Synthetic benchmark", tax_mode="taxable",
                        rounding="floor", opened_on="2026-01-01", setup_done=True)
        client.json("PUT", "/settings", settings)

    # Seed offline to avoid CSV/API row limits and exclude import time. The
    # binary creates its schema/masters first; all FK/check constraints stay on.
    with closing(sqlite3.connect(directory / "kamoku.db")) as conn:
        conn.execute("PRAGMA foreign_keys=ON")
        wallet = conn.execute("SELECT code FROM accounts WHERE wallet IS NOT NULL ORDER BY code LIMIT 1").fetchone()
        if wallet is None:
            raise BenchmarkError("The empty store has no wallet")
        conn.execute(
            "INSERT INTO import_batches(id,filename,wallet,recorded_at,created,duplicates,invalid) "
            "VALUES(1,'synthetic-benchmark.csv',?,1767225600,?,0,0)", (wallet[0], rows),
        )
        conn.executemany(
            "INSERT INTO drafts(id,batch_id,wallet,line,date,description,direction,amount,"
            "fingerprint,occurrence,account,tax_code,partner,memo,business_pct,status) "
            "VALUES(?,1,?,?,?,?,'out',?,?,1,'exp_comm','P10','','',100,'draft')",
            ((i, wallet[0], i, f"2026-{i % 12 + 1:02}-{i % 28 + 1:02}",
              f"Synthetic draft {i:06}", AMOUNT,
              hashlib.sha256(f"kamoku.draft.benchmark.v1:{i}".encode()).hexdigest())
             for i in range(1, rows + 1)),
        )
        conn.commit()
        if conn.execute("PRAGMA foreign_key_check").fetchone() is not None:
            raise BenchmarkError("Synthetic fixture violated foreign keys")


def copy_database(source, target):
    target.mkdir()
    with closing(sqlite3.connect(source / "kamoku.db")) as src:
        with closing(sqlite3.connect(target / "kamoku.db")) as dst:
            src.backup(dst)


def validate_status(client, drafts, posted):
    status, _ = client.json("GET", "/status")
    ledger = status.get("ledger", {})
    if (status.get("setup_done") is not True or ledger.get("verified") is not True
            or ledger.get("events") != posted or ledger.get("entries") != posted
            or status.get("inbox", {}).get("draft") != drafts):
        raise BenchmarkError("Unexpected fixture state or unverified ledger")


def measure(client, operation, rows, samples):
    times = []
    timeout_result = None
    completed_requests = 0
    validate_status(client, rows, 0)
    try:
        for index in range(WARMUPS + samples):
            if operation == "dashboard":
                value, elapsed = client.json("GET", "/reports/dashboard?month=2026-10")
                actions = value.get("next_actions", {})
                if (actions.get("inbox_draft") != rows
                        or actions.get("inbox_out_amount") != rows * AMOUNT):
                    raise BenchmarkError("Dashboard count or amount was incorrect")
            else:
                draft_id = index + 1
                value, elapsed = client.json("POST", "/drafts/confirm", {"ids": [draft_id]})
                confirmed = value.get("confirmed", [])
                if (value.get("failed") != [] or len(confirmed) != 1
                        or confirmed[0].get("id") != draft_id
                        or not confirmed[0].get("entry_id")):
                    raise BenchmarkError("One-draft confirmation was incorrect")
            completed_requests += 1
            if index >= WARMUPS:
                times.append(elapsed)
    except RequestTimedOut as exc:
        timeout_result = {"elapsed_ms": round(exc.elapsed_ms, 6),
                          "phase": "warmup" if completed_requests < WARMUPS else "sample",
                          "index": completed_requests}
    if timeout_result is None:
        posted = completed_requests if operation == "confirm_one" else 0
        validate_status(client, rows - posted, posted)
    ordered = sorted(times)
    return {
        "outcome": "timeout" if timeout_result else "complete",
        "sample_count": len(times), "completed_requests": completed_requests,
        "p50_ms": round(statistics.median(times), 6) if times else None,
        "p95_ms": round(ordered[math.ceil(len(times) * 0.95) - 1], 6) if times else None,
        "max_ms": round(max(times), 6) if times else None,
        "samples_ms": [round(value, 6) for value in times], "timeout": timeout_result,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--rows", type=int, choices=[10000, 100000], default=10000)
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--timeout", type=float, default=5)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.samples < args.rows or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("samples must be in [1, rows) and timeout must be a positive finite number")
    binary = args.binary.resolve()
    report = {
        "schema": 1,
        "fixture": {"version": 1, "draft_rows": args.rows, "posted_entries": 0,
                    "amount_per_row": AMOUNT, "total_out_amount": args.rows * AMOUNT,
                    "tax_mode": "taxable", "direction": "out", "account": "exp_comm"},
        "measurement": {"samples": args.samples, "warmups": WARMUPS,
                        "request_timeout_seconds": args.timeout, "units": "ms",
                        "percentile": "p50 median; p95 nearest rank",
                        "scope": "new TCP through complete response body; JSON parsing excluded",
                        "cache": "one warmup; OS and server warm; fresh DB/server per operation",
                        "python": platform.python_version(), "sqlite": sqlite3.sqlite_version,
                        "platform": platform.platform(), "machine": platform.machine()},
        "operations": {}, "complete": False, "errors": [],
    }
    try:
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise BenchmarkError("Binary does not exist or is not executable")
        with binary.open("rb") as stream:
            digest = hashlib.sha256()
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
            report["binary_sha256"] = digest.hexdigest()
        with tempfile.TemporaryDirectory(prefix="kamoku-bench-drafts-") as temporary:
            root = Path(temporary)
            seed_dir = root / "seed"
            seed(binary, seed_dir, args.rows, args.timeout)
            for operation in ("dashboard", "confirm_one"):
                data = root / operation
                copy_database(seed_dir, data)
                with server(binary, data, args.timeout) as client:
                    report["operations"][operation] = measure(client, operation, args.rows, args.samples)
        report["complete"] = all(value["outcome"] == "complete" for value in report["operations"].values())
    except BenchmarkError as exc:
        report["errors"].append(str(exc))
    except KeyboardInterrupt:
        report["errors"].append("Benchmark interrupted")
    except Exception as exc:
        # Exception messages/tracebacks can contain HTTP paths or headers. Keep
        # diagnostics categorical so no token, cookie, or server stdout escapes.
        report["errors"].append("Benchmark failed: " + type(exc).__name__)
    output = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(output, encoding="utf-8")
    print(output, end="")
    return int(bool(report["errors"]) or (args.require_complete and not report["complete"]))


if __name__ == "__main__":
    sys.exit(main())
