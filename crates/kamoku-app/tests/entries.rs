// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳を作る・直す・取り消す・探す（`docs/spec/api-entries.md`）。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::json;

// AC-02.2: プレビューで見える明細と、保存される明細は同じ。AC-03.1/03.2: 按分の合計が支払額と一致し、税は事業分だけ。
#[test]
fn preview_matches_saved_entry_and_split_adds_up() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let input = json!({ "kind": "expense", "date": "2026-10-05", "amount": 11000, "account": "exp_comm",
        "wallet": "w03", "tax_code": "P10", "partner": "サンプル通信", "description": "10月分 通信費",
        "business_pct": 70, "memo": "" });
    let preview = c.post("/entries/preview", &input);
    assert_eq!(preview.status, 200, "{}", preview.text());
    let preview = preview.json();
    let saved = create(&c, &input);
    assert_eq!(saved["lines"], preview["lines"]);
    assert_eq!(saved["total"], preview["total"]);
    let lines = saved["lines"].as_array().unwrap();
    let amounts: Vec<i64> = lines
        .iter()
        .map(|l| l["amount"].as_i64().unwrap())
        .collect();
    assert_eq!(amounts, vec![7700, 3300, 11000]);
    assert_eq!(lines[0]["tax_amount"], 700);
    assert_eq!(lines[1]["account"], "owner_draw");
    assert_eq!(lines[1]["tax_amount"], 0);
    assert_eq!(lines[1]["memo"], "家事按分 30%");
    assert_eq!(saved["state"], "active");
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["meta"]["business_pct"], "70");
    assert_eq!(saved["input"]["business_pct"], 70);
    let detail = c
        .api(&format!("/entries/{}", saved["id"].as_str().unwrap()))
        .json();
    assert_eq!(detail["lines"], preview["lines"]);
}

// AC-03.5: 事業分が0円になる按分は、0円の明細を作らず、事業主貸と口座の2行にして、警告を出す。
#[test]
fn zero_business_share_has_no_zero_line() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    for (amount, pct) in [(5000, 0), (1, 40)] {
        let mut input = expense("2026-10-05", amount, "exp_vehicle", "w01");
        input["business_pct"] = json!(pct);
        let p = c.post("/entries/preview", &input).json();
        assert_eq!(p["lines"].as_array().unwrap().len(), 2, "{p}");
        assert_ne!(
            p["warnings"].as_array().unwrap().len(),
            0,
            "事業分が0円の警告が無い"
        );
        let saved = create(&c, &input);
        assert_eq!(saved["lines"], p["lines"]);
    }
}

// AC-02.6: 貸借が合わない、金額が0以下、存在しない科目などは保存せず、原因と直し方を返す。
#[test]
fn invalid_inputs_are_refused_with_codes() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let unbalanced = json!({ "kind": "journal", "date": "2026-10-05", "lines": [
        { "account": "exp_comm", "side": "debit", "amount": 11000 },
        { "account": "w01", "side": "credit", "amount": 10000 } ] });
    let r = c.post("/entries", &unbalanced);
    assert_eq!(r.status, 422);
    assert_eq!(r.code(), "unbalanced");
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("1,000円")
    );
    let cases = [
        (
            expense("2026-10-05", 0, "exp_comm", "w01"),
            422,
            "amount_range",
        ),
        (
            expense("2026-10-05", -5, "exp_comm", "w01"),
            422,
            "amount_range",
        ),
        (
            expense("2026-10-05", 10_000_000_000_000, "exp_comm", "w01"),
            422,
            "amount_range",
        ),
        (
            expense("2026-10-05", 100, "nothing", "w01"),
            422,
            "unknown_account",
        ),
        (
            expense("2026-10-05", 100, "", "w01"),
            422,
            "missing_account",
        ),
        (
            expense("2026-02-30", 100, "exp_comm", "w01"),
            400,
            "bad_request",
        ),
        (
            json!({ "kind": "expense", "date": "2026-10-05", "amount": 1.5, "account": "exp_comm", "wallet": "w01" }),
            400,
            "bad_request",
        ),
        (
            json!({ "kind": "expense", "date": "2026-10-05", "amount": 100, "account": "exp_comm", "wallet": "w01", "tax_code": "X9" }),
            422,
            "bad_tax_code",
        ),
        (
            json!({ "kind": "expense", "date": "2026-10-05", "amount": 100, "account": "exp_comm", "wallet": "w01", "business_pct": 101 }),
            422,
            "bad_request",
        ),
        (json!({ "kind": "nothing" }), 400, "bad_request"),
    ];
    for (input, status, code) in cases {
        let r = c.post("/entries", &input);
        assert_eq!(
            (r.status, r.code().as_str()),
            (status, code),
            "{input}: {}",
            r.text()
        );
    }
    let disabled = c.patch("/accounts/exp_books", &json!({ "enabled": false }));
    assert_eq!(disabled.status, 200);
    let r = c.post("/entries", &expense("2026-10-05", 100, "exp_books", "w01"));
    assert_eq!((r.status, r.code().as_str()), (422, "disabled_account"));
    assert_eq!(c.api("/entries?state=all").json()["total"], 0);
}

// AC-02.8: 相手科目が口座のときは、税区分と割合を断る。同じ口座どうしの振替も断る。
#[test]
fn wallet_counterparts_refuse_tax_and_ratio() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mut card_payment = expense("2026-10-27", 5000, "w03", "w02");
    let r = c.post("/entries", &card_payment);
    assert_eq!((r.status, r.code().as_str()), (422, "bad_request"));
    card_payment["tax_code"] = json!(null);
    card_payment["business_pct"] = json!(50);
    assert_eq!(c.post("/entries", &card_payment).status, 422);
    card_payment["business_pct"] = json!(null);
    assert_eq!(c.post("/entries", &card_payment).status, 201);
    let same = json!({ "kind": "transfer", "date": "2026-10-01", "amount": 100, "wallet": "w02", "to_wallet": "w02" });
    assert_eq!(c.post("/entries", &same).status, 422);
}

// AC-02.7: 同じ client_id の再送は、二重に登録せず最初の結果を200で返す。本文が違えば409。
#[test]
fn client_id_resends_return_the_first_result() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mut input = expense("2026-10-05", 1100, "exp_comm", "w01");
    input["client_id"] = json!("c_7f3a0001");
    let first = c.post("/entries", &input);
    assert_eq!(first.status, 201);
    let again = c.post("/entries", &input);
    assert_eq!(again.status, 200);
    assert_eq!(again.json()["id"], first.json()["id"]);
    input["amount"] = json!(2200);
    let changed = c.post("/entries", &input);
    assert_eq!(
        (changed.status, changed.code().as_str()),
        (409, "client_id_reused")
    );
    assert_eq!(c.api("/entries").json()["total"], 1);
    // 失敗した送信は覚えないので、同じ印で直して送り直せる。
    let mut bad = expense("2026-10-05", 0, "exp_comm", "w01");
    bad["client_id"] = json!("c_7f3a0002");
    assert_eq!(c.post("/entries", &bad).status, 422);
    bad["amount"] = json!(500);
    assert_eq!(c.post("/entries", &bad).status, 201);
}
