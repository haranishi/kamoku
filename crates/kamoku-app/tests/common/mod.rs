// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 試験の道具。一時フォルダの帳簿でサーバーを `127.0.0.1:0` に起動し、合言葉からCookieを受け取り、
//! `std::net::TcpStream` で話す（HTTPクライアントの依存を足さないため。試験の道具だけが接続する）。

#![allow(dead_code)]

use kamoku_app::http::Limits;
use kamoku_app::server::{self, ServeOptions, Server};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 起動した試験用のサーバー。手放すと止める。
pub struct TestServer {
    pub dir: tempfile::TempDir,
    pub db: PathBuf,
    pub port: u16,
    pub token: String,
    server: Option<Server>,
}

impl TestServer {
    pub fn start() -> Self {
        Self::start_with(|_| {})
    }
    pub fn start_with(tweak: impl FnOnce(&mut ServeOptions)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("data").join("kamoku.db");
        Self::on(dir, db, tweak)
    }
    /// 決まった場所の帳簿で起動する（再起動や改ざんの試験）。
    pub fn on(dir: tempfile::TempDir, db: PathBuf, tweak: impl FnOnce(&mut ServeOptions)) -> Self {
        let mut options = ServeOptions::new(db.clone());
        options.port = 0;
        tweak(&mut options);
        let server = server::start(&options).unwrap();
        Self {
            port: server.port(),
            token: server.token().to_owned(),
            server: Some(server),
            dir,
            db,
        }
    }
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    pub fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
    /// 合言葉から入って、Cookieつきの道具を返す。
    pub fn login(&self) -> Client {
        let r = raw(
            self.port,
            format!(
                "GET /?t={} HTTP/1.1\r\nHost: {}\r\n\r\n",
                self.token,
                self.host()
            )
            .as_bytes(),
        );
        assert_eq!(r.status, 303, "合言葉で入れない: {}", r.text());
        let cookie = r
            .header("set-cookie")
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        Client {
            port: self.port,
            cookie: Some(cookie),
        }
    }
    /// Cookieの無い道具。
    pub fn anonymous(&self) -> Client {
        Client {
            port: self.port,
            cookie: None,
        }
    }
    /// 止めて、帳簿のフォルダを返す（同じ帳簿で起動し直すため）。
    pub fn stop(mut self) -> (tempfile::TempDir, PathBuf) {
        self.shutdown();
        let dir = std::mem::replace(&mut self.dir, tempfile::tempdir().unwrap());
        (dir, self.db.clone())
    }
    fn shutdown(&mut self) {
        if let Some(server) = self.server.take() {
            server.request_stop();
            // 受け付けの待ちを解くための接続（試験の道具だけがする）。
            let _ = TcpStream::connect(("127.0.0.1", self.port));
            server.join();
        }
    }
}
impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 試験用の短い時間切れ（受け口の頑丈さの試験で使う）。
pub fn quick_limits() -> Limits {
    Limits {
        read_timeout: Duration::from_millis(300),
        request_timeout: Duration::from_millis(900),
        write_timeout: Duration::from_secs(2),
        linger: Duration::from_millis(200),
        ..Limits::default()
    }
}

/// 応答。
#[derive(Debug, Clone)]
pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub raw: Vec<u8>,
}
impl Resp {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("JSONではない（{e}）: {}", self.text()))
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    /// 失敗の `code`。
    pub fn code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
    pub fn field(&self) -> String {
        self.json()["error"]["field"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

/// 応答のバイト列を読む（ステータス行・見出し・本文）。読めなければ status 0。
pub fn parse(raw: &[u8]) -> Resp {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n");
    let Some(split) = split else {
        return Resp {
            status: 0,
            headers: Vec::new(),
            body: Vec::new(),
            raw: raw.to_vec(),
        };
    };
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap_or_default()
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_owned(), v.trim().to_owned()))
        .collect();
    Resp {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
        raw: raw.to_vec(),
    }
}

/// バイト列をそのまま送り、相手が閉じるまで読む（サーバーは毎回閉じる）。
pub fn raw_bytes(port: u16, bytes: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let _ = stream.write_all(bytes);
    // 送り終えたことを知らせる（時間切れを待たせない）。止まった接続の試験は、別に自分で接続する。
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    out
}

pub fn raw(port: u16, bytes: &[u8]) -> Resp {
    parse(&raw_bytes(port, bytes))
}

/// Cookieつき（または無し）でAPIを呼ぶ道具。変更の要求には自分のオリジンを付ける。
pub struct Client {
    pub port: u16,
    pub cookie: Option<String>,
}
impl Client {
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    /// 見出しを自分で決めて送る。`Host` と `Cookie` は付けてある。
    pub fn request(&self, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Resp {
        let mut text = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n",
            self.port
        );
        if let Some(cookie) = &self.cookie {
            let _ = write!(text, "Cookie: {cookie}\r\n");
        }
        for (name, value) in headers {
            let _ = write!(text, "{name}: {value}\r\n");
        }
        if !body.is_empty() || method != "GET" {
            let _ = write!(text, "Content-Length: {}\r\n", body.len());
        }
        text.push_str("\r\n");
        let mut bytes = text.into_bytes();
        bytes.extend_from_slice(body);
        raw(self.port, &bytes)
    }
    pub fn get(&self, path: &str) -> Resp {
        self.request("GET", path, &[], b"")
    }
    pub fn api(&self, path: &str) -> Resp {
        self.get(&format!("/api/v1{path}"))
    }
    /// JSONの本文で、変更の要求を送る（Origin つき）。
    pub fn send(&self, method: &str, path: &str, body: &Value) -> Resp {
        let origin = self.origin();
        let bytes = serde_json::to_vec(body).unwrap();
        self.request(
            method,
            &format!("/api/v1{path}"),
            &[("Origin", &origin), ("Content-Type", "application/json")],
            &bytes,
        )
    }
    pub fn post(&self, path: &str, body: &Value) -> Resp {
        self.send("POST", path, body)
    }
    pub fn put(&self, path: &str, body: &Value) -> Resp {
        self.send("PUT", path, body)
    }
    pub fn patch(&self, path: &str, body: &Value) -> Resp {
        self.send("PATCH", path, body)
    }
    /// 本文の無い変更の要求（DELETE、ignore など）。
    pub fn bare(&self, method: &str, path: &str) -> Resp {
        let origin = self.origin();
        self.request(
            method,
            &format!("/api/v1{path}"),
            &[("Origin", &origin)],
            b"",
        )
    }
    /// CSVを送る。
    pub fn upload(&self, query: &str, bytes: &[u8]) -> Resp {
        let origin = self.origin();
        self.request(
            "POST",
            &format!("/api/v1/imports/upload{query}"),
            &[
                ("Origin", &origin),
                ("Content-Type", "application/octet-stream"),
            ],
            bytes,
        )
    }
}

/// 試験でよく使う帳簿の形。課税事業者、開業日2026-01-01、口座は現金（w01）・預金（w02）・カード（w03）。
pub fn books(client: &Client) {
    let r = client.put(
        "/settings",
        &json!({
            "business_name": "サンプル事務所", "owner_name": "", "tax_mode": "taxable",
            "invoice_number": null, "rounding": "floor", "opened_on": "2026-01-01",
            "locked_until": null, "setup_done": true
        }),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    for (name, kind) in [("サンプル銀行", "bank"), ("サンプルカード", "card")] {
        let r = client.post(
            "/accounts",
            &json!({ "name": name, "reading": "", "wallet": { "type": kind } }),
        );
        assert_eq!(r.status, 201, "{}", r.text());
    }
}

/// 支出の入力。
pub fn expense(date: &str, amount: i64, account: &str, wallet: &str) -> Value {
    json!({ "kind": "expense", "date": date, "amount": amount, "account": account, "wallet": wallet,
            "tax_code": "P10", "description": "デモの支出" })
}

/// 仕訳を作って、その表現を返す。
pub fn create(client: &Client, input: &Value) -> Value {
    let r = client.post("/entries", input);
    assert_eq!(r.status, 201, "{}", r.text());
    r.json()
}

/// 合成のCSV（kamoku-import の試験の素材）。
pub fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kamoku-import/tests/fixtures/synthetic")
        .join(name);
    std::fs::read(path).unwrap()
}
