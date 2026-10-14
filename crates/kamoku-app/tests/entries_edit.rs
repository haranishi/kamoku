// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳を直す・取り消す・探す（`docs/spec/api-entries.md`）。訂正・取消・締め・一覧の絞り込み。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::{Value, json};

fn revise_body(input: &Value, reason: Option<&str>, revision: Option<i64>) -> Value {
    let mut body = input.clone();
    if let Some(r) = reason {
        body["reason"] = json!(r);
    }
    if let Some(v) = revision {
        body["expected_revision"] = json!(v);
    }
    body
}

// AC-06.2: 訂正と取消には理由が要り、元の記録は履歴に残る。AC-06.6: 古い版からの訂正は stale_revision。
#[test]
fn revise_and_void_keep_history() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let input = expense("2026-10-05", 1100, "exp_comm", "w01");
    let saved = create(&c, &input);
    let path = format!("/entries/{}", saved["id"].as_str().unwrap());
    let mut fixed = input.clone();
    fixed["amount"] = json!(2200);
    let no_reason = c.put(&path, &revise_body(&fixed, None, Some(1)));
    assert_eq!(
        (no_reason.status, no_reason.code().as_str()),
        (422, "reason_required")
    );
    let blank = c.put(&path, &revise_body(&fixed, Some("   "), Some(1)));
    assert_eq!(blank.code(), "reason_required");
    let no_revision = c.put(&path, &revise_body(&fixed, Some("金額の入力ミス"), None));
    assert_eq!(no_revision.status, 400);
    let stale = c.put(&path, &revise_body(&fixed, Some("金額の入力ミス"), Some(7)));
    assert_eq!(
        (stale.status, stale.code().as_str()),
        (409, "stale_revision")
    );
    let revised = c.put(&path, &revise_body(&fixed, Some("金額の入力ミス"), Some(1)));
    assert_eq!(revised.status, 200, "{}", revised.text());
    assert_eq!(revised.json()["revision"], 2);
    assert_eq!(revised.json()["total"], 2200);
    let void = c.post(
        &format!("{path}/void"),
        &json!({ "reason": "二重に登録したため", "expected_revision": 2 }),
    );
    assert_eq!(void.status, 200);
    assert_eq!(void.json()["state"], "voided");
    assert_eq!(
        void.json()["total"],
        2200,
        "取消済みは最後に有効だった版の明細を見せる"
    );
    let twice = c.post(
        &format!("{path}/void"),
        &json!({ "reason": "もう一度", "expected_revision": 3 }),
    );
    assert_eq!(
        (twice.status, twice.code().as_str()),
        (409, "already_voided")
    );
    let after = c.put(&path, &revise_body(&fixed, Some("直す"), Some(3)));
    assert_eq!(after.status, 409);
    let detail = c.api(&path).json();
    let kinds: Vec<&str> = detail["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["posted", "revised", "voided"]);
    assert_eq!(detail["history"][1]["reason"], "金額の入力ミス");
    assert_eq!(detail["history"][0]["lines"][0]["amount"], 1100);
    assert!(detail["history"][2]["lines"].is_null());
    assert_eq!(detail["history"][0]["hash"].as_str().unwrap().len(), 64);
    assert_eq!(c.api("/entries/e_nothing").status, 404);
    assert_eq!(c.api("/entries/%20").status, 404);
}

// AC-06.3: 締めた日以前の日付への登録・訂正・取消は断る。訂正は動かす前後の両方を見る。
#[test]
fn locked_period_is_enforced() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let before = create(&c, &expense("2026-03-10", 1100, "exp_comm", "w01"));
    let later = create(&c, &expense("2026-04-10", 1100, "exp_comm", "w01"));
    let mut settings = c.api("/settings").json();
    settings["locked_until"] = json!("2026-03-31");
    assert_eq!(c.put("/settings", &settings).status, 200);
    let r = c.post("/entries", &expense("2026-03-15", 500, "exp_comm", "w01"));
    assert_eq!((r.status, r.code().as_str()), (422, "locked_period"));
    let into_lock = c.put(
        &format!("/entries/{}", later["id"].as_str().unwrap()),
        &revise_body(
            &expense("2026-03-01", 1100, "exp_comm", "w01"),
            Some("日付の訂正"),
            Some(1),
        ),
    );
    assert_eq!(into_lock.code(), "locked_period");
    let void = c.post(
        &format!("/entries/{}/void", before["id"].as_str().unwrap()),
        &json!({ "reason": "取消", "expected_revision": 1 }),
    );
    assert_eq!(void.code(), "locked_period");
    assert_eq!(
        c.post("/entries", &expense("2026-04-01", 500, "exp_comm", "w01"))
            .status,
        201
    );
}

// AC-06.1: 日付・科目・口座・金額・文字列で絞れる。AC-06.5: 取消済みは既定で隠れ、絞り込みで出せる。
#[test]
fn journal_filters_and_order() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mut phone = expense("2026-10-01", 11000, "exp_comm", "w03");
    phone["partner"] = json!("サンプル通信");
    phone["memo"] = json!("携帯の分");
    let phone = create(&c, &phone);
    let pens = create(&c, &expense("2026-10-01", 550, "exp_supplies", "w01"));
    let mut book = expense("2026-09-15", 3300, "exp_books", "w02");
    book["description"] = json!("技術書 デモ");
    let book = create(&c, &book);
    let sale = create(
        &c,
        &json!({ "kind": "income", "date": "2026-10-20", "amount": 220000, "account": "sales", "wallet": "w02", "tax_code": "S10" }),
    );
    let voided = create(&c, &expense("2026-10-02", 999, "exp_comm", "w01"));
    let vid = voided["id"].as_str().unwrap();
    assert_eq!(
        c.post(
            &format!("/entries/{vid}/void"),
            &json!({ "reason": "二重", "expected_revision": 1 })
        )
        .status,
        200
    );
    let ids = |path: &str| -> Vec<String> {
        let resp = c.api(path);
        assert_eq!(resp.status, 200, "{path}: {}", resp.text());
        resp.json()["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let id = |v: &Value| v["id"].as_str().unwrap().to_owned();
    // 日付の新しい順、同じ日は番号の大きい順。取消済みは既定で出ない。
    assert_eq!(
        ids("/entries"),
        vec![id(&sale), id(&pens), id(&phone), id(&book)]
    );
    assert_eq!(ids("/entries?state=voided"), vec![vid.to_owned()]);
    assert_eq!(ids("/entries?state=all").len(), 5);
    assert_eq!(
        ids("/entries?from=2026-10-01&to=2026-10-01"),
        vec![id(&pens), id(&phone)]
    );
    assert_eq!(ids("/entries?account=exp_books"), vec![id(&book)]);
    assert_eq!(ids("/entries?wallet=w03"), vec![id(&phone)]);
    assert_eq!(ids("/entries?kind=income"), vec![id(&sale)]);
    assert_eq!(ids("/entries?min=1000&max=5000"), vec![id(&book)]);
    assert_eq!(
        ids("/entries?q=%E3%82%B5%E3%83%B3%E3%83%97%E3%83%AB%E9%80%9A%E4%BF%A1"),
        vec![id(&phone)]
    );
    assert_eq!(ids("/entries?q=%E6%90%BA%E5%B8%AF"), vec![id(&phone)]);
    assert_eq!(
        ids("/entries?q=%E6%B6%88%E8%80%97%E5%93%81"),
        vec![id(&pens)]
    );
    assert_eq!(ids("/entries?q=11%2C000"), vec![id(&phone)]);
    assert_eq!(
        ids("/entries?q=%E6%8A%80%E8%A1%93%E6%9B%B8"),
        vec![id(&book)]
    );
    assert_eq!(ids("/entries?source=manual").len(), 4);
    let page = c.api("/entries?limit=2&offset=1").json();
    assert_eq!(page["total"], 4);
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["sum_amount"], 11000 + 550 + 3300 + 220_000);
    assert_eq!(c.api("/entries?limit=501").status, 400);
    assert_eq!(c.api("/entries?state=nothing").status, 400);
}

// api-entries: 取引先がマスタに無ければ同じトランザクションで足し、登録番号があれば invoice_no に入れる。
#[test]
fn partners_are_added_and_invoice_numbers_recorded() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let p = c.post(
        "/partners",
        &json!({ "name": "サンプル通信", "invoice_number": "T1234567890123" }),
    );
    assert_eq!(p.status, 201);
    let mut known = expense("2026-10-05", 1100, "exp_comm", "w01");
    known["partner"] = json!(" サンプル通信 ");
    let saved = create(&c, &known);
    assert_eq!(saved["meta"]["invoice_no"], "T1234567890123");
    assert_eq!(saved["partner"], "サンプル通信");
    let mut fresh = expense("2026-10-06", 1100, "exp_comm", "w01");
    fresh["partner"] = json!("デモ商店");
    let saved = create(&c, &fresh);
    assert!(saved["meta"].get("invoice_no").is_none());
    let names: Vec<String> = c.api("/partners").json()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"デモ商店".to_owned()));
    assert_eq!(names.iter().filter(|n| *n == "サンプル通信").count(), 1);
}

// api-entries: 仕訳の形の詳細の input は null。期首残高の仕訳は PUT と void を422で断る。
#[test]
fn journal_input_and_opening_entries() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let journal = create(
        &c,
        &json!({ "kind": "journal", "date": "2026-10-05", "description": "複合", "lines": [
        { "account": "exp_comm", "side": "debit", "amount": 1100, "tax_code": "P10" },
        { "account": "w01", "side": "credit", "amount": 1100 } ] }),
    );
    assert!(journal["input"].is_null());
    assert_eq!(journal["lines"][0]["tax_amount"], 100);
    let opening = c.put(
        "/opening",
        &json!({ "balances": [{ "account": "w01", "amount": 50000 }] }),
    );
    assert_eq!(opening.status, 200, "{}", opening.text());
    let oid = opening.json()["entry_id"].as_str().unwrap().to_owned();
    let r = c.put(
        &format!("/entries/{oid}"),
        &revise_body(
            &expense("2026-01-01", 1, "exp_comm", "w01"),
            Some("x"),
            Some(1),
        ),
    );
    assert_eq!((r.status, r.code().as_str()), (422, "bad_request"));
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("期首残高")
    );
    let r = c.post(
        &format!("/entries/{oid}/void"),
        &json!({ "reason": "x", "expected_revision": 1 }),
    );
    assert_eq!(r.status, 422);
}
