// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 受け口の続き（コードレビューの指摘 B8・B9）。処理のスレッドを待った時間と、HEAD への断り方。

mod common;
use common::*;
use kamoku_app::http::Limits;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn after_head(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).into_owned();
    text.split_once("\r\n\r\n").map_or_else(
        || panic!("見出しの終わりが無い: {text}"),
        |(_, rest)| rest.to_owned(),
    )
}

// B9（RFC 9110 の HEAD）: 受け口が自分で断るとき（413・431）も、HEAD への応答には本文を付けない。
#[test]
fn head_rejections_carry_no_body() {
    let s = TestServer::start();
    let cookie = s.login().cookie.unwrap();
    let big_body = format!(
        "HEAD / HTTP/1.1\r\nHost: {}\r\nCookie: {cookie}\r\nContent-Length: 2000000\r\n\r\n",
        s.host()
    );
    let bytes = raw_bytes(s.port, big_body.as_bytes());
    assert!(
        bytes.starts_with(b"HTTP/1.1 413"),
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(after_head(&bytes), "");
    let big_head = format!(
        "HEAD / HTTP/1.1\r\nHost: {}\r\nX-Big: {}\r\n\r\n",
        s.host(),
        "a".repeat(20_000)
    );
    let bytes = raw_bytes(s.port, big_head.as_bytes());
    assert!(
        bytes.starts_with(b"HTTP/1.1 431"),
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(after_head(&bytes), "");
    // 比べるため: GET への同じ断りには本文がある。
    let bytes = raw_bytes(s.port, big_body.replace("HEAD", "GET").as_bytes());
    assert!(bytes.starts_with(b"HTTP/1.1 413"));
    assert!(after_head(&bytes).contains("too_large"));
}

// B8: 処理のスレッドが空くのを待った時間も、要求の期限（request_timeout）に含める。処理のスレッドの数より
// 多い低速な接続を並べても、どれも「受け付けてから期限まで」に閉じる。期限を過ぎて順番が来た接続は、
// 読まずに408で閉じる。
#[test]
fn queued_connections_close_within_their_own_deadline() {
    let limits = Limits {
        workers: 1,
        read_timeout: Duration::from_millis(400),
        request_timeout: Duration::from_millis(600),
        linger: Duration::from_millis(50),
        ..quick_limits()
    };
    let s = TestServer::start_with(|o| o.limits = limits);
    let started = Instant::now();
    let slow: Vec<_> = (0..6)
        .map(|_| {
            let (port, host) = (s.port, s.host());
            std::thread::spawn(move || {
                let mut conn = TcpStream::connect(("127.0.0.1", port)).unwrap();
                conn.write_all(
                    format!("GET /api/v1/status HTTP/1.1\r\nHost: {host}\r\nX-Slow: a").as_bytes(),
                )
                .unwrap();
                conn.set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut out = Vec::new();
                let _ = conn.read_to_end(&mut out);
                (started.elapsed(), out)
            })
        })
        .collect();
    for handle in slow {
        let (closed_at, out) = handle.join().unwrap();
        let text = String::from_utf8_lossy(&out);
        assert!(text.starts_with("HTTP/1.1 408"), "{text}");
        assert!(
            closed_at < Duration::from_millis(1500),
            "閉じたのは {closed_at:?}"
        );
    }
    assert_eq!(s.login().api("/status").status, 200);
}
