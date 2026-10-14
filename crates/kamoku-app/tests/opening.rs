// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! テンプレート・期首残高・口座の残高・台帳の検査（`docs/spec/api-masters.md`）。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::json;

// api-masters: テンプレートは日付を除いた単純な形。消すと200（本文つき）。
#[test]
fn templates_round_trip() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let body = json!({ "kind": "expense", "amount": 7700, "account": "exp_comm", "wallet": "w03", "tax_code": "P10", "partner": "サンプル通信", "description": "携帯電話の利用料", "business_pct": 70, "memo": "" });
    let t = c.post(
        "/templates",
        &json!({ "name": "携帯電話の利用料", "body": body }),
    );
    assert_eq!(t.status, 201, "{}", t.text());
    let t = t.json();
    assert!(t["body"].get("date").is_none());
    assert_eq!(t["body"]["amount"], 7700);
    let mut dated = body.clone();
    dated["date"] = json!("2026-10-01");
    assert_eq!(
        c.post("/templates", &json!({ "name": "x", "body": dated }))
            .status,
        422
    );
    let journal = json!({ "kind": "journal", "lines": [] });
    assert_eq!(
        c.post("/templates", &json!({ "name": "x", "body": journal }))
            .status,
        422
    );
    let broken = json!({ "kind": "expense", "amount": 0, "account": "exp_comm", "wallet": "w01" });
    let r = c.post("/templates", &json!({ "name": "x", "body": broken }));
    assert_eq!((r.status, r.field().as_str()), (422, "body.amount"));
    let list = c.api("/templates").json();
    assert_eq!(list["total"], 1);
    let id = t["id"].as_i64().unwrap();
    let del = c.bare("DELETE", &format!("/templates/{id}"));
    assert_eq!(del.status, 200);
    assert_eq!(del.json()["deleted"], true);
    assert_eq!(c.bare("DELETE", &format!("/templates/{id}")).status, 404);
    assert_eq!(c.api("/templates").json()["total"], 0);
}

// AC-01.3: 期首残高は1つの仕訳で元入金で合わせ、見直しは理由つきの訂正。債務超過なら元入金は借方。
// AC-01.5: すべて0なら仕訳を作らず、あれば「期首残高なし」で取り消す。
#[test]
fn opening_balances_follow_the_rules() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let empty = c.api("/opening").json();
    assert_eq!(
        empty,
        json!({ "date": "2026-01-01", "balances": [], "capital": 0, "entry_id": null })
    );
    let zero = c.put(
        "/opening",
        &json!({ "balances": [{ "account": "w01", "amount": 0 }] }),
    );
    assert_eq!(zero.status, 200);
    assert!(zero.json()["entry_id"].is_null());
    assert_eq!(c.api("/entries?state=all").json()["total"], 0);
    let first = c.put("/opening", &json!({ "date": "2026-01-01", "balances": [
        { "account": "w01", "amount": 50000 }, { "account": "w02", "amount": 300000 }, { "account": "w03", "amount": 20000 } ] }));
    assert_eq!(first.status, 200, "{}", first.text());
    let first = first.json();
    assert_eq!(first["capital"], 330_000);
    let id = first["entry_id"].as_str().unwrap().to_owned();
    let entry = c.api(&format!("/entries/{id}")).json();
    assert_eq!(entry["kind"], "opening");
    assert_eq!(entry["meta"]["source"], "opening");
    let same = c.put("/opening", &json!({ "balances": [
        { "account": "w01", "amount": 50000 }, { "account": "w02", "amount": 300000 }, { "account": "w03", "amount": 20000 } ] }));
    assert_eq!(same.json()["entry_id"], json!(id));
    assert_eq!(
        c.api(&format!("/entries/{id}")).json()["revision"],
        1,
        "同じ内容では版を増やさない"
    );
    let insolvent = c.put("/opening", &json!({ "balances": [{ "account": "w01", "amount": 10000 }, { "account": "w03", "amount": 60000 }] }));
    assert_eq!(insolvent.json()["capital"], -50_000);
    let detail = c.api(&format!("/entries/{id}")).json();
    assert_eq!(detail["revision"], 2);
    assert_eq!(detail["history"][1]["reason"], "期首残高の見直し");
    let capital_line = detail["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["account"] == "capital")
        .unwrap()
        .clone();
    assert_eq!(capital_line["side"], "debit");
    let none = c.put("/opening", &json!({ "balances": [] }));
    assert!(none.json()["entry_id"].is_null());
    let voided = c.api(&format!("/entries/{id}")).json();
    assert_eq!(voided["state"], "voided");
    assert_eq!(voided["last_reason"], "期首残高なし");
    for bad in [
        json!({ "balances": [{ "account": "owner_draw", "amount": 1 }] }),
        json!({ "balances": [{ "account": "exp_comm", "amount": 1 }] }),
        json!({ "balances": [{ "account": "w01", "amount": 1 }, { "account": "w01", "amount": 2 }] }),
        json!({ "balances": [{ "account": "w01", "amount": -1 }] }),
        json!({ "date": "2027-01-01", "balances": [{ "account": "w01", "amount": 1 }] }),
    ] {
        assert_eq!(c.put("/opening", &bad).status, 422, "{bad}");
    }
}

// api-masters: 口座の残高。カードは未払い額（正）。
#[test]
fn wallet_balances_include_cards_as_owed() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    c.put(
        "/opening",
        &json!({ "balances": [{ "account": "w02", "amount": 100000 }] }),
    );
    create(&c, &expense("2026-10-05", 5500, "exp_comm", "w03"));
    create(&c, &expense("2026-10-06", 1100, "exp_comm", "w02"));
    let w = c.api("/wallets?as_of=2026-10-31").json();
    let balance = |code: &str| {
        w["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["code"] == code)
            .unwrap()["balance"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(balance("w02"), 98_900);
    assert_eq!(balance("w03"), 5_500);
    assert_eq!(w["owed_total"], 5_500);
    assert_eq!(w["cash_total"], 98_900);
    assert_eq!(c.api("/wallets?as_of=2026-01-01").json()["owed_total"], 0);
}

// AC-09.1: 台帳の整合性の検査。AC-09.2の照合: 控えたハッシュが何番目かが分かる。
#[test]
fn verify_finds_noted_hashes() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    create(&c, &expense("2026-10-05", 1100, "exp_comm", "w01"));
    let old_head = c.api("/status").json()["ledger"]["head_hash"]
        .as_str()
        .unwrap()
        .to_owned();
    create(&c, &expense("2026-10-06", 1100, "exp_comm", "w01"));
    let head = c.api("/status").json()["ledger"]["head_hash"]
        .as_str()
        .unwrap()
        .to_owned();
    let plain = c.post("/verify", &json!({ "noted_hash": null })).json();
    assert_eq!(plain["ok"], true);
    assert_eq!(plain["events"], 2);
    assert_eq!(plain["head_hash"], json!(head));
    assert!(plain["noted"].is_null());
    let older = c
        .post("/verify", &json!({ "noted_hash": old_head.to_uppercase() }))
        .json();
    assert_eq!(
        older["noted"],
        json!({ "found": true, "seq": 1, "is_head": false })
    );
    let current = c.post("/verify", &json!({ "noted_hash": head })).json();
    assert_eq!(current["noted"]["is_head"], true);
    let unknown = c
        .post("/verify", &json!({ "noted_hash": "a".repeat(64) }))
        .json();
    assert_eq!(unknown["noted"]["found"], false);
    assert_eq!(
        c.post("/verify", &json!({ "noted_hash": "xyz" })).status,
        400
    );
}
