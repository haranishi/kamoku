// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 上限と境界（コードレビューの指摘 A3・B5・B7）。大きすぎる入力は、処理を始める前に断る。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// CSVを取り込んで確定する（列の対応は推定のまま）。
fn commit(c: &Client, csv: &str) -> Value {
    let up = c.upload("", csv.as_bytes());
    assert_eq!(up.status, 200, "{}", up.text());
    let up = up.json();
    let job = json!({ "upload_id": up["upload_id"], "wallet": "w02", "mapping": up["guess"] });
    let done = c.post("/imports/commit", &job);
    assert_eq!(done.status, 200, "{}", done.text());
    done.json()
}

// A3（api-inbox.md）: 確定は一度に2,000件まで。15万件の ids は、処理を始める前に422で断り、
// 全体のロックを長く握らない（直後の /status がすぐ応える）。
#[test]
fn confirm_refuses_150000_ids_before_any_work() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let ids: Vec<u64> = (1..=150_000).collect();
    let started = Instant::now();
    let r = c.post("/drafts/confirm", &json!({ "ids": ids }));
    let took = started.elapsed();
    assert_eq!(
        (r.status, r.code().as_str()),
        (422, "bad_request"),
        "{}",
        r.text()
    );
    assert!(r.text().contains("2,000件"), "{}", r.text());
    assert!(took < Duration::from_secs(2), "断るまで {took:?}");
    let started = Instant::now();
    assert_eq!(c.api("/status").status, 200);
    assert!(started.elapsed() < Duration::from_secs(1));
}

// A3 の境界: ちょうど2,000件は受け付け（知らない番号は行ごとの not_found）、2,001件は422。
// 数えるのは送った要素の数（重なりをまとめる前）。重なった番号は1つにまとめ、失敗は送った順に並ぶ。
#[test]
fn confirm_takes_exactly_2000_ids_in_order() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mut ids: Vec<i64> = (1..=2000).rev().collect();
    let r = c.post("/drafts/confirm", &json!({ "ids": ids }));
    assert_eq!(r.status, 200, "{}", r.text());
    let failed: Vec<i64> = r.json()["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_i64().unwrap())
        .collect();
    assert_eq!(failed, ids);
    ids.push(5);
    let r = c.post("/drafts/confirm", &json!({ "ids": ids }));
    assert_eq!(
        (r.status, r.code().as_str()),
        (422, "bad_request"),
        "{}",
        r.text()
    );
    let mut repeated = vec![7, 3, 7, 3];
    repeated.extend([1; 10]);
    let r = c.post("/drafts/confirm", &json!({ "ids": repeated }));
    let failed: Vec<i64> = r.json()["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_i64().unwrap())
        .collect();
    assert_eq!(failed, [7, 3, 1]);
}

// B5（data-model.md の決まり16）: ホームのまとめの「未確認の出金の合計」は、ブラウザの数値で
// 正確に扱える 9,007,199,254,740,991 まで。ちょうどなら返し、1円でも超えれば 422 amount_range。
#[test]
fn inbox_out_amount_stops_at_the_safe_integer() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let max_row: i64 = 9_999_999_999_999;
    let rest = 9_007_199_254_740_991 - 900 * max_row;
    let mut csv = String::from("日付,摘要,出金,入金\r\n");
    for i in 0..900 {
        writeln!(csv, "2026/10/05,上限の行{i},{max_row},\r").unwrap();
    }
    writeln!(csv, "2026/10/05,残りの行,{rest},\r").unwrap();
    assert_eq!(commit(&c, &csv)["created"], 901);
    let d = c.api("/reports/dashboard?month=2026-10");
    assert_eq!(d.status, 200, "{}", d.text());
    assert_eq!(
        d.json()["next_actions"]["inbox_out_amount"],
        9_007_199_254_740_991_i64
    );
    commit(&c, "日付,摘要,出金,入金\r\n2026/10/06,1円の行,1,\r\n");
    let d = c.api("/reports/dashboard?month=2026-10");
    assert_eq!(
        (d.status, d.code().as_str()),
        (422, "amount_range"),
        "{}",
        d.text()
    );
    assert_eq!(
        d.json()["error"]["message"],
        "未確認の出金の合計が大きすぎます。"
    );
}

// B7（api-inbox.md）: 取込の履歴は offset と limit で区切る。total は全件数。
#[test]
fn import_batches_honour_offset_and_report_the_total() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    for i in 1..=3 {
        commit(
            &c,
            &format!("日付,摘要,出金,入金\r\n2026/10/0{i},履歴{i},,100\r\n"),
        );
    }
    let mut seen = Vec::new();
    for offset in 0..3 {
        let page = c
            .api(&format!("/imports/batches?limit=1&offset={offset}"))
            .json();
        assert_eq!(page["total"], 3, "{page}");
        let items = page["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{page}");
        seen.push(items[0]["batch_id"].as_i64().unwrap());
    }
    assert!(seen.windows(2).all(|w| w[0] > w[1]), "新しい順: {seen:?}");
    let past = c.api("/imports/batches?limit=1&offset=3").json();
    assert_eq!(past["items"], json!([]));
    assert_eq!(past["total"], 3);
}
