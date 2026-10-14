// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 受け口の頑丈さ（巨大な Content-Length、遅い接続、でたらめな見出し）。`tiny_http` 0.12 は、
//! 巨大な Content-Length の要求1つで異常終了したので、受け口を自作した。この試験が通る限り、同じ穴は無い。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn status_ok(s: &TestServer) {
    let r = s.login().api("/status");
    assert_eq!(r.status, 200, "直後の正常な要求に応えない: {}", r.text());
}

// (a) 巨大な Content-Length の要求が413で終わり、サーバーが生きていて次の要求に応える。
#[test]
fn huge_content_length_is_413_and_server_survives() {
    let s = TestServer::start();
    let c = s.login();
    let cookie = c.cookie.clone().unwrap();
    for _ in 0..3 {
        let r = raw(
            s.port,
            format!(
                "POST /api/v1/entries HTTP/1.1\r\nHost: {}\r\nCookie: {cookie}\r\nOrigin: {}\r\nContent-Type: application/json\r\nContent-Length: 4611686018427387904\r\n\r\nabc",
                s.host(),
                s.origin()
            )
            .as_bytes(),
        );
        assert_eq!(r.status, 413, "{}", String::from_utf8_lossy(&r.raw));
        assert_eq!(r.code(), "too_large");
    }
    // 合言葉の無い要求でも同じ（読む前に断るので、何も確保しない）。
    let anon = raw(s.port, format!("POST /api/v1/entries HTTP/1.1\r\nHost: {}\r\nContent-Length: 18446744073709551615\r\n\r\n", s.host()).as_bytes());
    assert_eq!(anon.status, 401);
    status_ok(&s);
}

// (b) 見出しを途中まで送って止めた接続は時間切れで閉じられ、その間もほかの要求に応える。
#[test]
fn stalled_headers_time_out_without_blocking_others() {
    let s = TestServer::start_with(|o| o.limits = quick_limits());
    let mut stalled = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    stalled
        .write_all(
            format!(
                "GET /api/v1/status HTTP/1.1\r\nHost: {}\r\nX-Slow: a",
                s.host()
            )
            .as_bytes(),
        )
        .unwrap();
    let started = Instant::now();
    for _ in 0..5 {
        status_ok(&s);
    }
    assert!(
        started.elapsed() < Duration::from_millis(800),
        "ほかの要求が待たされた: {:?}",
        started.elapsed()
    );
    stalled
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut out = Vec::new();
    let read = stalled.read_to_end(&mut out);
    assert!(
        read.is_ok() || started.elapsed() < Duration::from_secs(5),
        "時間切れで閉じられない"
    );
    let r = parse(&out);
    assert!(
        r.status == 408 || out.is_empty(),
        "{}",
        String::from_utf8_lossy(&out)
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

// (b) 処理のスレッドの数だけ止めても、時間切れでスレッドが空き、待っていた要求に応える。
#[test]
fn stalled_connections_free_their_workers() {
    let s = TestServer::start_with(|o| {
        o.limits = quick_limits();
        o.limits.workers = 2;
    });
    let stalls: Vec<TcpStream> = (0..2)
        .map(|_| {
            let mut t = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
            t.write_all(b"GET / HTTP/1.1\r\n").unwrap();
            t
        })
        .collect();
    std::thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    status_ok(&s);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    drop(stalls);
}

// 同時接続の上限を超えた接続には503を返して閉じる。接続ごとにスレッドを作らない。
#[test]
fn too_many_connections_get_503() {
    let s = TestServer::start_with(|o| {
        o.limits = quick_limits();
        o.limits.max_connections = 2;
        o.limits.workers = 1;
    });
    let holds: Vec<TcpStream> = (0..2)
        .map(|_| TcpStream::connect(("127.0.0.1", s.port)).unwrap())
        .collect();
    std::thread::sleep(Duration::from_millis(50));
    let r = raw(
        s.port,
        format!("GET /api/v1/status HTTP/1.1\r\nHost: {}\r\n\r\n", s.host()).as_bytes(),
    );
    assert_eq!(r.status, 503, "{}", String::from_utf8_lossy(&r.raw));
    assert_eq!(r.code(), "busy");
    drop(holds);
    std::thread::sleep(Duration::from_millis(1200));
    status_ok(&s);
}

fn ends_in_error_or_close(s: &TestServer, bytes: &[u8], label: &str) {
    let out = raw_bytes(s.port, bytes);
    let r = parse(&out);
    assert!(
        out.is_empty() || (400..600).contains(&r.status),
        "{label}: {}",
        String::from_utf8_lossy(&out)
    );
}

// (c) 壊れた要求のどれも、4xx/5xx か切断で終わり、直後の正常な要求に応える。
#[test]
fn malformed_requests_end_in_errors_and_server_keeps_serving() {
    let s = TestServer::start_with(|o| o.limits = quick_limits());
    let host = s.host();
    let cases: Vec<(&str, Vec<u8>, Option<u16>)> = vec![
        ("見出しが大きすぎる", format!("GET / HTTP/1.1\r\nHost: {host}\r\nX-Big: {}\r\n\r\n", "a".repeat(20_000)).into_bytes(), Some(431)),
        ("要求行が長すぎる", format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(9_000)).into_bytes(), Some(414)),
        ("見出しが多すぎる", format!("GET / HTTP/1.1\r\n{}\r\n", "X-A: 1\r\n".repeat(101)).into_bytes(), Some(431)),
        ("chunked", format!("POST /api/v1/entries HTTP/1.1\r\nHost: {host}\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n").into_bytes(), Some(501)),
        ("不正な要求行", b"GARBAGE\r\n\r\n".to_vec(), Some(400)),
        ("版が違う", b"GET / HTTP/2.0\r\n\r\n".to_vec(), Some(505)),
        ("知らないメソッド", b"OPTIONS / HTTP/1.1\r\n\r\n".to_vec(), Some(405)),
        ("NULを含む見出し", format!("GET / HTTP/1.1\r\nHost: {host}\r\nX-A: a\0b\r\n\r\n").into_bytes(), Some(400)),
        ("制御文字を含む見出し", format!("GET / HTTP/1.1\r\nHost: {host}\r\nX-A: a\x1bb\r\n\r\n").into_bytes(), Some(400)),
        ("CRだけの改行", format!("GET / HTTP/1.1\r\nHost: {host}\rX-A: b\r\n\r\n").into_bytes(), Some(400)),
        ("数でない長さ", format!("POST / HTTP/1.1\r\nHost: {host}\r\nContent-Length: ten\r\n\r\n").into_bytes(), Some(400)),
        ("負の長さ", format!("POST / HTTP/1.1\r\nHost: {host}\r\nContent-Length: -5\r\n\r\n").into_bytes(), Some(400)),
        ("桁あふれの長さ", format!("POST / HTTP/1.1\r\nHost: {host}\r\nContent-Length: 99999999999999999999999\r\n\r\n").into_bytes(), Some(400)),
        ("食い違う長さ", format!("POST / HTTP/1.1\r\nHost: {host}\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n").into_bytes(), Some(400)),
        ("見出しの途中で終わる", format!("GET / HTTP/1.1\r\nHost: {host}\r\n").into_bytes(), None),
        ("空", Vec::new(), None),
    ];
    for (label, bytes, expected) in cases {
        let out = raw_bytes(s.port, &bytes);
        let r = parse(&out);
        match expected {
            Some(status) => assert_eq!(
                r.status,
                status,
                "{label}: {}",
                String::from_utf8_lossy(&out)
            ),
            None => assert!(out.is_empty() || (400..600).contains(&r.status), "{label}"),
        }
        status_ok(&s);
    }
}

// (c) 本文を途中まで送って止める、Content-Length より短い本文で送信を閉じる。
#[test]
fn short_or_stalled_bodies_end_in_errors() {
    let s = TestServer::start_with(|o| o.limits = quick_limits());
    let c = s.login();
    let head = format!(
        "POST /api/v1/entries/preview HTTP/1.1\r\nHost: {}\r\nCookie: {}\r\nOrigin: {}\r\nContent-Type: application/json\r\nContent-Length: 500\r\n\r\n{{\"kind\":",
        s.host(),
        c.cookie.clone().unwrap(),
        s.origin()
    );
    // 途中で止める（接続は開いたまま）→ 時間切れの408。
    let mut stalled = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    stalled.write_all(head.as_bytes()).unwrap();
    stalled
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut out = Vec::new();
    let _ = stalled.read_to_end(&mut out);
    assert_eq!(parse(&out).status, 408, "{}", String::from_utf8_lossy(&out));
    // 短いまま送信を閉じる → 400。
    let mut short = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    short.write_all(head.as_bytes()).unwrap();
    short.shutdown(std::net::Shutdown::Write).unwrap();
    short
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut out = Vec::new();
    let _ = short.read_to_end(&mut out);
    assert_eq!(parse(&out).status, 400, "{}", String::from_utf8_lossy(&out));
    status_ok(&s);
}

// (c) ランダムなバイト列（数百回）でも、毎回4xx/5xxか切断で終わり、止まらない。
#[test]
fn random_bytes_never_break_the_server() {
    let s = TestServer::start_with(|o| o.limits = quick_limits());
    let mut seed: u64 = 0x2026_1014;
    let mut next = || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) as u32
    };
    let prefixes: [&[u8]; 4] = [
        b"",
        b"GET / HTTP/1.1\r\n",
        b"POST /api/v1/entries HTTP/1.1\r\nHost: x\r\n",
        b"G",
    ];
    for i in 0..300u32 {
        let len = (next() % 600) as usize;
        let mut bytes: Vec<u8> = prefixes[(i % 4) as usize].to_vec();
        bytes.extend((0..len).map(|_| (next() & 0xff) as u8));
        if i % 3 == 0 {
            bytes.extend_from_slice(b"\r\n\r\n");
        }
        ends_in_error_or_close(&s, &bytes, &format!("ランダム{i}"));
        if i % 50 == 0 {
            status_ok(&s);
        }
    }
    status_ok(&s);
}

// 範囲: 1つの接続で1つの要求だけ（後に続く要求は読まない）。応答は Connection: close。
#[test]
fn pipelined_requests_get_only_one_answer() {
    let s = TestServer::start();
    let c = s.login();
    let one = format!(
        "GET /api/v1/status HTTP/1.1\r\nHost: {}\r\nCookie: {}\r\n\r\n",
        s.host(),
        c.cookie.clone().unwrap()
    );
    let out = raw_bytes(s.port, format!("{one}{one}").as_bytes());
    let text = String::from_utf8_lossy(&out);
    assert_eq!(text.matches("HTTP/1.1 ").count(), 1, "{text}");
    assert_eq!(parse(&out).status, 200);
}

// 範囲: Expect: 100-continue は検査を通ったときだけ 100 を返す。HEAD は本文を返さない。HTTP/1.0 も読む。
#[test]
fn expect_continue_head_and_http10() {
    let s = TestServer::start();
    let c = s.login();
    let cookie = c.cookie.clone().unwrap();
    let body = br#"{"kind":"expense","date":"2026-10-05","amount":1100,"account":"exp_comm","wallet":"w01","tax_code":"P10"}"#;
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let head = format!(
        "POST /api/v1/entries/preview HTTP/1.1\r\nHost: {}\r\nCookie: {cookie}\r\nOrigin: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nExpect: 100-continue\r\n\r\n",
        s.host(),
        s.origin(),
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    let mut interim = [0u8; 25];
    stream.read_exact(&mut interim).unwrap();
    assert_eq!(&interim, b"HTTP/1.1 100 Continue\r\n\r\n");
    stream.write_all(body).unwrap();
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    assert_eq!(parse(&out).status, 200, "{}", String::from_utf8_lossy(&out));
    let big = raw(
        s.port,
        format!("POST /api/v1/entries/preview HTTP/1.1\r\nHost: {}\r\nCookie: {cookie}\r\nOrigin: {}\r\nContent-Type: application/json\r\nContent-Length: 2000000\r\nExpect: 100-continue\r\n\r\n", s.host(), s.origin()).as_bytes(),
    );
    assert_eq!(big.status, 413);
    assert!(!String::from_utf8_lossy(&big.raw).contains("100 Continue"));
    let head_only = c.request("HEAD", "/api/v1/status", &[], b"");
    assert_eq!(head_only.status, 200);
    assert_eq!(head_only.body, Vec::<u8>::new());
    assert!(
        head_only
            .header("Content-Length")
            .unwrap()
            .parse::<usize>()
            .unwrap()
            > 0
    );
    let old = raw(
        s.port,
        format!(
            "GET /api/v1/status HTTP/1.0\r\nHost: {}\r\nCookie: {cookie}\r\n\r\n",
            s.host()
        )
        .as_bytes(),
    );
    assert_eq!(old.status, 200);
}
