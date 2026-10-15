// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 安全の試験（`docs/spec/api-common.md` の入り口と認証、`nfr.md` の安全）。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::json;

const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self' 'wasm-unsafe-eval'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

fn assert_common_headers(r: &Resp) {
    assert_eq!(
        r.header("Content-Security-Policy"),
        Some(CSP),
        "{:?}",
        r.headers
    );
    assert_eq!(r.header("X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(r.header("Referrer-Policy"), Some("no-referrer"));
    assert_eq!(r.header("Connection"), Some("close"));
    assert!(r.header("Content-Length").is_some());
}

// nfr 安全: 合言葉もCookieも無い要求は401（APIはJSON、画面のページは日本語の短い案内）。
#[test]
fn no_cookie_is_401_json_for_api_and_html_for_pages() {
    let s = TestServer::start();
    let anon = s.anonymous();
    let r = anon.api("/status");
    assert_eq!(r.status, 401);
    assert_eq!(r.code(), "unauthorized");
    let page = anon.get("/");
    assert_eq!(page.status, 401);
    assert!(
        page.header("Content-Type")
            .unwrap()
            .starts_with("text/html")
    );
    assert!(page.text().contains("合言葉"));
    // 中身を調べずに通してはいけない: 任意の値は401。
    let forged = Client {
        port: s.port,
        cookie: Some("kamoku_session=00000000000000000000000000000000".to_owned()),
    };
    assert_eq!(forged.api("/status").status, 401);
    let empty = Client {
        port: s.port,
        cookie: Some("kamoku_session=".to_owned()),
    };
    assert_eq!(empty.api("/status").status, 401);
}

// api-common 入り口: 合言葉が合えば Cookie（HttpOnly・SameSite=Strict・Path=/）を付けて / へ303。
#[test]
fn token_entrance_sets_a_strict_cookie_and_redirects() {
    let s = TestServer::start();
    let wrong = raw(
        s.port,
        format!("GET /?t=deadbeef HTTP/1.1\r\nHost: {}\r\n\r\n", s.host()).as_bytes(),
    );
    assert_eq!(wrong.status, 401);
    assert!(wrong.header("Set-Cookie").is_none());
    let ok = raw(
        s.port,
        format!("GET /?t={} HTTP/1.1\r\nHost: {}\r\n\r\n", s.token, s.host()).as_bytes(),
    );
    assert_eq!(ok.status, 303);
    assert_eq!(ok.header("Location"), Some("/"));
    assert_eq!(ok.header("Cache-Control"), Some("no-store"));
    let cookie = ok.header("Set-Cookie").unwrap();
    assert!(cookie.starts_with("kamoku_session="));
    assert!(
        cookie.contains("HttpOnly")
            && cookie.contains("SameSite=Strict")
            && cookie.contains("Path=/")
    );
    assert_common_headers(&ok);
    // 合言葉は同じ起動の間は何度でも使え、そのたびに別のセッション値になる。
    let again = s.login();
    assert_ne!(
        again.cookie.as_deref(),
        Some(cookie.split(';').next().unwrap())
    );
    assert_eq!(again.api("/status").status, 200);
    assert_eq!(s.login().get("/").status, 200);
}

// api-common: 再起動すると、合言葉もセッション値も全部失効する。
#[test]
fn restart_invalidates_sessions_and_token() {
    let s = TestServer::start();
    let old_token = s.token.clone();
    let client = s.login();
    assert_eq!(client.api("/status").status, 200);
    let (dir, db) = s.stop();
    let s2 = TestServer::on(dir, db, |_| {});
    let stale = Client {
        port: s2.port,
        cookie: client.cookie.clone(),
    };
    assert_eq!(stale.api("/status").status, 401);
    let old = raw(
        s2.port,
        format!(
            "GET /?t={old_token} HTTP/1.1\r\nHost: {}\r\n\r\n",
            s2.host()
        )
        .as_bytes(),
    );
    assert_eq!(old.status, 401);
}

// api-common: Host が 127.0.0.1:<port> か localhost:<port> でなければ421。/?t= を含む全経路。
#[test]
fn wrong_host_is_421_everywhere() {
    let s = TestServer::start();
    let client = s.login();
    let cookie = client.cookie.clone().unwrap();
    for (path, host) in [
        ("/api/v1/status", "evil.example:1234".to_owned()),
        ("/api/v1/status", format!("evil.example:{}", s.port)),
        ("/api/v1/status", format!("127.0.0.1:{}", s.port + 1)),
        ("/", format!("example.com:{}", s.port)),
        (
            &format!("/?t={}", s.token),
            format!("rebind.example:{}", s.port),
        ),
    ] {
        let r = raw(
            s.port,
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\n\r\n").as_bytes(),
        );
        assert_eq!(r.status, 421, "{path} {host}");
        assert!(r.header("Set-Cookie").is_none());
        assert_common_headers(&r);
    }
    let missing = raw(
        s.port,
        format!("GET /api/v1/status HTTP/1.1\r\nCookie: {cookie}\r\n\r\n").as_bytes(),
    );
    assert_eq!(missing.status, 421);
    assert_eq!(parse(&missing.raw).code(), "bad_host");
    let local = raw(
        s.port,
        format!(
            "GET /api/v1/status HTTP/1.1\r\nHost: localhost:{}\r\nCookie: {cookie}\r\n\r\n",
            s.port
        )
        .as_bytes(),
    );
    assert_eq!(local.status, 200);
}

// api-common: 変更の要求は Origin が必須で完全一致。欠落・null・違うオリジンは403。Sec-Fetch-Site も見る。
#[test]
fn state_changes_need_our_exact_origin() {
    let s = TestServer::start();
    let c = s.login();
    let body = r#"{"name":"サンプル商事"}"#.as_bytes();
    let ours = c.origin();
    let other_port = format!("http://127.0.0.1:{}", s.port + 1);
    for headers in [
        vec![],
        vec![("Origin", "null")],
        vec![("Origin", "http://evil.example")],
        vec![("Origin", other_port.as_str())],
        vec![("Origin", "https://127.0.0.1")],
        vec![("Origin", ours.as_str()), ("Sec-Fetch-Site", "cross-site")],
        vec![("Origin", ours.as_str()), ("Sec-Fetch-Site", "same-site")],
        vec![("Origin", ours.as_str()), ("Origin", "http://evil.example")],
    ] {
        let mut all = headers.clone();
        all.push(("Content-Type", "application/json"));
        let r = c.request("POST", "/api/v1/partners", &all, body);
        assert_eq!(r.status, 403, "{headers:?}");
        assert_eq!(r.code(), "forbidden_origin");
    }
    for method in ["PUT", "PATCH", "DELETE"] {
        let r = c.request(
            method,
            "/api/v1/templates/1",
            &[("Content-Type", "application/json")],
            b"{}",
        );
        assert_eq!(r.status, 403, "{method}");
    }
    let ok = c.request(
        "POST",
        "/api/v1/partners",
        &[
            ("Origin", &ours),
            ("Sec-Fetch-Site", "same-origin"),
            ("Content-Type", "application/json"),
        ],
        body,
    );
    assert_eq!(ok.status, 201, "{}", ok.text());
    let localhost = format!("http://localhost:{}", s.port);
    let ok2 = c.request(
        "POST",
        "/api/v1/partners",
        &[("Origin", &localhost), ("Content-Type", "application/json")],
        r#"{"name":"デモ企画"}"#.as_bytes(),
    );
    assert_eq!(ok2.status, 201);
}

// api-common: CSPほかの見出しは、すべての応答に付ける。Cache-Control は API と入り口が no-store、画面のファイルは no-cache。
#[test]
fn security_headers_on_every_response() {
    let s = TestServer::start();
    let c = s.login();
    let cases = [
        c.api("/status"),
        c.api("/nothing"),
        s.anonymous().api("/status"),
        c.request(
            "POST",
            "/api/v1/partners",
            &[("Content-Type", "application/json")],
            b"{}",
        ),
        c.get("/js/main.js"),
        c.get("/no-such-file.js"),
        raw(s.port, b"BROKEN\r\n\r\n"),
        raw(
            s.port,
            format!(
                "POST /api/v1/entries HTTP/1.1\r\nHost: {}\r\nContent-Length: 99999999999\r\n\r\n",
                s.host()
            )
            .as_bytes(),
        ),
    ];
    for r in &cases {
        assert_common_headers(r);
        let policy = r.header("Content-Security-Policy").unwrap();
        assert!(policy.contains("'wasm-unsafe-eval'"));
        assert!(
            !policy
                .split_whitespace()
                .any(|word| word == "'unsafe-eval'")
        );
    }
    assert_eq!(cases[0].header("Cache-Control"), Some("no-store"));
    assert_eq!(cases[4].status, 200);
    assert_eq!(cases[4].header("Cache-Control"), Some("no-cache"));
    assert!(
        cases[4]
            .header("Content-Type")
            .unwrap()
            .starts_with("text/javascript")
    );
}

// 静的配信: `..` やパーセント符号化で外へ抜けるパス、配らないファイルは404。
#[test]
fn static_paths_cannot_escape_ui() {
    let s = TestServer::start();
    let c = s.login();
    assert_eq!(c.get("/").status, 200);
    assert!(c.get("/").text().contains("<html"));
    for path in [
        "/../Cargo.toml",
        "/..%2fCargo.toml",
        "/%2e%2e/Cargo.toml",
        "/css/../../Cargo.toml",
        "/dev/mock-server.mjs",
        "/tests/index.js",
        "/package.json",
        "/README.md",
        "/js/%00",
    ] {
        let r = c.get(path);
        assert_eq!(r.status, 404, "{path}");
        assert!(!r.text().contains("[workspace]"));
    }
}

// api-common: 本文の上限（JSON 1MB、CSV 10MB）を超えると413。読む前に断る。
#[test]
fn oversized_bodies_are_413() {
    let s = TestServer::start();
    let c = s.login();
    let origin = c.origin();
    let big = vec![b' '; 1024 * 1024 + 1];
    let r = c.request(
        "POST",
        "/api/v1/entries",
        &[("Origin", &origin), ("Content-Type", "application/json")],
        &big,
    );
    assert_eq!(r.status, 413);
    assert_eq!(r.code(), "too_large");
    let csv = vec![b'a'; 10 * 1024 * 1024 + 1];
    let r = c.upload("", &csv);
    assert_eq!(r.status, 413);
    // CSVは1MBを超えても、10MBまでは受け取る（中身が壊れていれば400）。
    let mut mid = b"\xE6\x97\xA5\xE4\xBB\x98,a\r\n".to_vec();
    mid.extend(std::iter::repeat_n(b'x', 2 * 1024 * 1024));
    assert_ne!(c.upload("", &mid).status, 413);
}

// api-common: 壊れたJSONは400、Content-Type の違いも400、メソッド違いは405、無い経路は404。
#[test]
fn malformed_requests_get_the_documented_codes() {
    let s = TestServer::start();
    let c = s.login();
    let origin = c.origin();
    let broken = c.request(
        "POST",
        "/api/v1/entries",
        &[("Origin", &origin), ("Content-Type", "application/json")],
        b"{",
    );
    assert_eq!(broken.status, 400);
    assert_eq!(broken.code(), "bad_request");
    let plain = c.request(
        "POST",
        "/api/v1/entries",
        &[("Origin", &origin), ("Content-Type", "text/plain")],
        b"{}",
    );
    assert_eq!(plain.status, 400);
    let not_object = c.post("/entries/preview", &json!([1, 2]));
    assert_eq!(not_object.status, 400);
    let wrong = c.bare("DELETE", "/status");
    assert_eq!(wrong.status, 405);
    assert!(wrong.header("Allow").unwrap().contains("GET"));
    let static_post = c.request("POST", "/index.html", &[("Origin", &origin)], b"");
    assert_eq!(static_post.status, 405);
    let missing = c.api("/no/such/route");
    assert_eq!(missing.status, 404);
    assert_eq!(missing.code(), "not_found");
    let bad_query = c.api("/entries?from=%zz");
    assert_eq!(bad_query.status, 400);
}

// AC-09.4: DBファイルは0600、フォルダは0700（Unix）。-wal と -shm も同じ。
#[cfg(unix)]
#[test]
fn database_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&s.db), 0o600);
    assert_eq!(mode(s.db.parent().unwrap()), 0o700);
    for suffix in ["-wal", "-shm"] {
        let side = std::path::PathBuf::from(format!("{}{suffix}", s.db.display()));
        if side.exists() {
            assert_eq!(mode(&side), 0o600, "{suffix}");
        }
    }
}
