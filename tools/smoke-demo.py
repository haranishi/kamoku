#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""できあがった実行ファイルで `kamoku demo` を起動し、HTTPで一通り確かめるスモーク試験。

使い方:  python3 tools/smoke-demo.py [--bin target/debug/kamoku]

Rustの試験は、部品とサーバーを別々に確かめる。この試験は、実際のバイナリを起動して、
入口(合言葉・Cookie・Host・Origin)、画面が最初に読む数字、帳票の貸借の一致、書き出し、
巨大な本文の長さへの耐性までを、外から1本の流れで確かめる。標準ライブラリだけで動く。
失敗した項目があれば、一覧を出して終了コード1。サーバーは必ず止める。
"""
import argparse
import http.client
import json
import re
import socket
import subprocess
import sys
import time

failures: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'  ok' if ok else 'FAIL'}  {name}{('  ' + detail) if detail and not ok else ''}")
    if not ok:
        failures.append(name)


class Client:
    def __init__(self, port: int) -> None:
        self.port = port
        self.cookie = ""

    def request(self, method: str, path: str, body: bytes | None = None, headers: dict | None = None,
                host: str | None = None, origin: bool = False, cookie: bool = True):
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        h = {"Host": host or f"127.0.0.1:{self.port}"}
        if origin:
            h["Origin"] = f"http://127.0.0.1:{self.port}"
        if cookie and self.cookie:
            h["Cookie"] = self.cookie
        if body is not None:
            h.setdefault("Content-Type", "application/json")
        h.update(headers or {})
        conn.putrequest(method, path, skip_host=True, skip_accept_encoding=True)
        for k, v in h.items():
            conn.putheader(k, v)
        conn.putheader("Content-Length", str(len(body) if body is not None else 0))
        conn.endheaders(body)
        res = conn.getresponse()
        data = res.read()
        out = (res.status, dict((k.lower(), v) for k, v in res.getheaders()), data)
        conn.close()
        return out

    def json(self, path: str, **kw):
        status, headers, data = self.request("GET", path, **kw)
        return status, (json.loads(data) if data else None)


def start(binary: str):
    proc = subprocess.Popen([binary, "demo", "--no-open", "--port", "0"], stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True)
    deadline = time.time() + 60
    url = None
    while time.time() < deadline and proc.poll() is None:
        line = proc.stdout.readline()
        m = re.search(r"http://127\.0\.0\.1:(\d+)/\?t=([0-9a-f]+)", line)
        if m:
            url = (int(m.group(1)), m.group(2))
            break
    if not url:
        proc.kill()
        sys.exit("デモの起動で、アドレスが出ませんでした。")
    return proc, url


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", default="target/debug/kamoku")
    args = parser.parse_args()
    proc, (port, token) = start(args.bin)
    c = Client(port)
    try:
        # --- 入口と認証
        status, _, _ = c.request("GET", "/api/v1/status", cookie=False)
        check("合言葉なしは401", status == 401, str(status))
        status, _, _ = c.request("GET", "/api/v1/status", host="evil.example:80", cookie=False)
        check("違うHostは421", status == 421, str(status))
        status, headers, _ = c.request("GET", f"/?t={'0' * 32}", cookie=False)
        check("違う合言葉は断る", status in (401, 403), str(status))
        status, headers, _ = c.request("GET", f"/?t={token}", cookie=False)
        cookie_line = headers.get("set-cookie", "")
        check("合言葉で303とCookie", status == 303 and "HttpOnly" in cookie_line and "SameSite=Strict" in cookie_line, cookie_line)
        c.cookie = cookie_line.split(";")[0]
        check("応答にCSPが付く", "default-src 'self'" in headers.get("content-security-policy", ""))
        status, _, _ = c.request("GET", "/api/v1/status", cookie=False, headers={"Cookie": "kamoku_session=" + "0" * 32})
        check("でたらめなCookieは401", status == 401, str(status))
        # --- 状態と画面
        status, body = c.json("/api/v1/status")
        ledger = (body or {}).get("ledger", {})
        check("状態: デモで、台帳の検証が通っている", status == 200 and body.get("mode") == "demo" and ledger.get("verified") is True)
        check("状態: events_loaded と events_in_db が出る", "events_loaded" in ledger and "events_in_db" in ledger)
        status, _, html = c.request("GET", "/")
        check("画面のHTMLが返る", status == 200 and b"<html" in html.lower())
        status, _, _ = c.request("GET", "/dev/mock-server.mjs")
        check("開発用のファイルは配らない", status == 404, str(status))
        status, _, _ = c.request("GET", "/%2e%2e/%2e%2e/etc/passwd")
        check("パスの抜けは404", status in (400, 404), str(status))
        # --- ホームと帳票
        status, d = c.json("/api/v1/reports/dashboard?month=2026-09")
        keys = {"next_actions", "month_summary", "ytd", "wallets", "series", "top_expenses", "tax", "ledger"}
        check("ホームのまとめの項目", status == 200 and keys <= set(d or {}), str(sorted(set(d or {}))))
        check("ホーム: 12か月の系列", len((d or {}).get("series", [])) == 12)
        status, tb = c.json("/api/v1/reports/trial-balance?from=2026-01-01&to=2026-12-31")
        check("試算表: 貸借が一致", status == 200 and tb["balanced"] is True and tb["total_debit"] == tb["total_credit"])
        status, bs = c.json("/api/v1/reports/balance-sheet?as_of=2026-09-30")
        check("貸借対照表: 等式が成り立つ", status == 200 and bs["balanced"] is True)
        status, pl = c.json("/api/v1/reports/income-statement?from=2026-01-01&to=2026-12-31")
        check("損益計算書: 利益=収益-費用", status == 200 and pl["net_income"] == pl["revenue"]["total"] - pl["expense"]["total"])
        status, span = c.json("/api/v1/reports/trial-balance?from=2026-12-01&to=2027-01-31")
        check("年をまたぐ期間は422", status == 422 and span["error"]["code"] == "period_spans_years", str(status))
        # --- 仕訳のプレビューと、書き込みの守り
        preview = json.dumps({"kind": "expense", "date": "2026-09-30", "amount": 11000, "account": "exp_comm",
                              "wallet": "w02", "tax_code": "P10", "partner": "サンプル通信",
                              "description": "通信費", "business_pct": 70}).encode()
        status, _, data = c.request("POST", "/api/v1/entries/preview", body=preview, origin=True)
        lines = json.loads(data).get("lines", []) if status == 200 else []
        debit = sum(x["amount"] for x in lines if x["side"] == "debit")
        credit = sum(x["amount"] for x in lines if x["side"] == "credit")
        check("プレビュー: 按分と貸借", status == 200 and debit == credit == 11000 and len(lines) == 3, str(status))
        status, _, _ = c.request("POST", "/api/v1/entries/preview", body=preview)
        check("Originなしの書き込みは403", status == 403, str(status))
        status, _, _ = c.request("POST", "/api/v1/entries/preview", body=preview, origin=True,
                                 headers={"Content-Type": "text/plain"})
        check("Content-Typeが違うと400", status == 400, str(status))
        status, _, _ = c.request("PUT", "/api/v1/settings", body=b"{}", origin=True,
                                 headers={"Origin": "http://evil.example"})
        check("違うOriginは403", status == 403, str(status))
        # --- 検査と書き出し
        status, v = c.request("POST", "/api/v1/verify", body=b'{"noted_hash":null}', origin=True)[0::2]
        check("台帳の検査", status == 200 and json.loads(v).get("ok") is True)
        status, headers, data = c.request("GET", "/api/v1/export/journal.csv?from=2026-01-01&to=2026-12-31")
        check("仕訳帳CSV: BOMつき", status == 200 and data.startswith(b"\xef\xbb\xbf"), str(data[:8]))
        status, headers, data = c.request("GET", "/api/v1/export/journal.csv?from=2026-01-01&to=2026-12-31&encoding=sjis")
        check("仕訳帳CSV: Shift_JISで読める", status == 200 and data.decode("shift_jis", "replace").count("�") == 0)
        status, headers, data = c.request("GET", "/api/v1/export/backup.db")
        check("バックアップはSQLite", status == 200 and data.startswith(b"SQLite format 3"))
        # --- 受け口の頑丈さ
        s = socket.create_connection(("127.0.0.1", port), timeout=5)
        s.sendall((f"POST /api/v1/entries HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 4611686018427387904\r\n"
                   f"Content-Type: application/json\r\nOrigin: http://127.0.0.1:{port}\r\nCookie: {c.cookie}\r\n\r\n").encode())
        first = s.recv(64).split(b"\r\n")[0]
        s.close()
        check("巨大なContent-Lengthは413", b"413" in first, first.decode("latin1"))
        status, _ = c.json("/api/v1/status")
        check("巨大な要求のあとも、サーバーは生きている", status == 200)
        s = socket.create_connection(("127.0.0.1", port), timeout=5)
        s.sendall(b"\x00\xff\xfe garbage\r\n\r\n")
        s.settimeout(5)
        try:
            junk = s.recv(64)
        except OSError:
            junk = b""
        s.close()
        check("でたらめなバイト列は4xxか切断", junk == b"" or junk.startswith(b"HTTP/1.1 4"), junk[:20].decode("latin1"))
        status, _ = c.json("/api/v1/status")
        check("でたらめな入力のあとも、サーバーは生きている", status == 200)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
    print()
    if failures:
        print(f"失敗 {len(failures)}件: " + " / ".join(failures))
        return 1
    print("スモーク試験: すべて通過しました。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
