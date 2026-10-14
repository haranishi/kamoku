// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 1万仕訳の台帳で、ホームのまとめ（`GET /api/v1/reports/dashboard`）の応答時間と、
//! CSV 1,000行の取込（送る・確かめる・取り込む）の時間を測る。
//! `nfr.md` の目標（50ミリ秒以内、1,000件を1秒以内）と比べる材料。合成データだけを使う。
//!
//! `cargo run --release -p kamoku-app --example bench`

use kamoku_app::server::{self, ServeOptions};
use kamoku_books::{
    ComposeSettings, Rounding, SimpleInput, SimpleKind, Source, TaxMode, compose_simple,
};
use kamoku_ledger::EntryId;
use kamoku_store::{NewAccount, OpenOptions, Settings, Store};
use std::error::Error;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use time::{Date, Month};

const ENTRIES: u32 = 10_000;

fn input(i: u32) -> Result<SimpleInput, Box<dyn Error>> {
    let month = Month::try_from(u8::try_from(i % 12 + 1)?)?;
    let date = Date::from_calendar_date(2026, month, u8::try_from(i % 28 + 1)?)?;
    let income = i.is_multiple_of(10);
    Ok(SimpleInput {
        kind: if income {
            SimpleKind::Income
        } else {
            SimpleKind::Expense
        },
        date,
        amount: i64::from(1_000 + i % 9_000),
        account: Some(
            if income {
                "sales"
            } else {
                ["exp_comm", "exp_supplies", "exp_travel", "exp_vehicle"][(i % 4) as usize]
            }
            .to_owned(),
        ),
        wallet: if i.is_multiple_of(3) { "w02" } else { "w01" }.to_owned(),
        to_wallet: None,
        tax_code: Some(if income { "S10" } else { "P10" }.to_owned()),
        partner: String::new(),
        description: format!("デモの取引{i}"),
        business_pct: if income {
            None
        } else {
            Some(if i.is_multiple_of(5) { 70 } else { 100 })
        },
        memo: String::new(),
    })
}

fn build(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let (mut store, _) = Store::open(
        path,
        OpenOptions {
            create_if_missing: true,
        },
    )?;
    store.save_settings(
        Settings {
            business_name: "サンプル事務所".into(),
            tax_mode: "taxable".into(),
            opened_on: "2026-01-01".into(),
            setup_done: true,
            ..Settings::default()
        },
        1,
    )?;
    for (name, kind) in [
        ("現金", kamoku_books::WalletType::Cash),
        ("サンプル銀行", kamoku_books::WalletType::Bank),
    ] {
        store.add_account(
            NewAccount {
                name: name.into(),
                reading: String::new(),
                kind: kind.kind(),
                parent: None,
                wallet: Some(kamoku_books::Wallet {
                    kind,
                    institution: String::new(),
                }),
                default_tax: None,
                default_pct: 100,
            },
            1,
        )?;
    }
    let settings = ComposeSettings {
        rounding: Rounding::Floor,
        tax_mode: TaxMode::Taxable,
        source: Source::Demo,
    };
    let started = Instant::now();
    for chunk in 0..ENTRIES / 500 {
        let mut batch = Vec::new();
        for i in chunk * 500..(chunk + 1) * 500 {
            let composed = compose_simple(&input(i)?, store.chart(), &settings)?;
            batch.push((
                composed.to_entry(EntryId::new(&format!("bench_{i}"))?)?,
                None,
            ));
        }
        store.post_many(batch, 1)?;
    }
    println!("{ENTRIES}仕訳を作成: {:?}", started.elapsed());
    Ok(())
}

fn request(port: u16, text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.write_all(text.as_bytes())?;
    let mut out = Vec::new();
    stream.read_to_end(&mut out)?;
    Ok(out)
}

fn post(
    port: u16,
    cookie: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let host = format!("127.0.0.1:{port}");
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    let mut out = Vec::new();
    stream.read_to_end(&mut out)?;
    Ok(out)
}

fn body_json(response: &[u8]) -> Result<serde_json::Value, Box<dyn Error>> {
    let text = std::str::from_utf8(response)?;
    let (head, body) = text.split_once("\r\n\r\n").ok_or("本文が無い")?;
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.1 201") {
        return Err(format!("応答が成功でない: {}", head.lines().next().unwrap_or("")).into());
    }
    Ok(serde_json::from_str(body)?)
}

/// 合成データのCSV（日付・摘要・出金）を送り、確かめて、取り込むまでの時間を測る。
fn bench_import(port: u16, cookie: &str, rows: u32) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from("日付,摘要,出金,入金\n");
    for i in 0..rows {
        writeln!(csv, "2026-02-{:02},サンプル店{i},{},", i % 28 + 1, 100 + i)?;
    }
    let started = Instant::now();
    let upload = body_json(&post(
        port,
        cookie,
        "/api/v1/imports/upload?name=bench.csv&encoding=utf-8",
        "application/octet-stream",
        csv.as_bytes(),
    )?)?;
    let sent = started.elapsed();
    let request = serde_json::json!({
        "upload_id": upload["upload_id"],
        "wallet": "w02",
        "mapping": upload["guess"],
    });
    let started_preview = Instant::now();
    let preview = body_json(&post(
        port,
        cookie,
        "/api/v1/imports/preview",
        "application/json",
        request.to_string().as_bytes(),
    )?)?;
    let checked = started_preview.elapsed();
    let started_commit = Instant::now();
    let commit = body_json(&post(
        port,
        cookie,
        "/api/v1/imports/commit",
        "application/json",
        request.to_string().as_bytes(),
    )?)?;
    let committed = started_commit.elapsed();
    println!(
        "CSV {rows}行の取込: 送る {sent:?}、確かめる {checked:?}、取り込む {committed:?}、合計 {:?}（目標1,000件を1秒以内）。確かめの新規 {}、取り込んだ {}",
        sent + checked + committed,
        preview["counts"]["new"],
        commit["created"]
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let db = dir.path().join("kamoku.db");
    build(&db)?;
    let mut options = ServeOptions::new(db);
    options.port = 0;
    let srv = server::start(&options)?;
    let (port, token) = (srv.port(), srv.token().to_owned());
    let host = format!("127.0.0.1:{port}");
    let login = String::from_utf8(request(
        port,
        &format!("GET /?t={token} HTTP/1.1\r\nHost: {host}\r\n\r\n"),
    )?)?;
    let cookie = login
        .lines()
        .find_map(|l| l.strip_prefix("Set-Cookie: "))
        .and_then(|c| c.split(';').next())
        .ok_or("Cookieが無い")?
        .to_owned();
    let ask = format!(
        "GET /api/v1/reports/dashboard?month=2026-10 HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\n\r\n"
    );
    let warm = request(port, &ask)?;
    if !warm.starts_with(b"HTTP/1.1 200") {
        return Err("ホームのまとめが200でない".into());
    }
    let mut times = Vec::new();
    for _ in 0..30 {
        let started = Instant::now();
        request(port, &ask)?;
        times.push(started.elapsed());
    }
    times.sort();
    let median = times.get(times.len() / 2).copied().unwrap_or_default();
    let p95 = times
        .get(times.len() * 95 / 100)
        .copied()
        .unwrap_or_default();
    println!(
        "ホームのまとめ（{ENTRIES}仕訳、30回、接続から応答の終わりまで）: 最小 {:?}、中央 {median:?}、95% {p95:?}、最大 {:?}（目標50ミリ秒以内）",
        times.first().copied().unwrap_or_default(),
        times.last().copied().unwrap_or_default()
    );
    bench_import(port, &cookie, 1_000)?;
    srv.request_stop();
    let _ = TcpStream::connect(("127.0.0.1", port));
    srv.join();
    Ok(())
}
