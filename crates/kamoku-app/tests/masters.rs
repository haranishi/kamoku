// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 状態・設定・科目・口座・取引先・テンプレート・期首残高・検査（`docs/spec/api-masters.md`）。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::{Value, json};

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

// api-masters: /status の項目。ledger は常に events_loaded と events_in_db を返す。
#[test]
fn status_has_the_documented_shape() {
    let s = TestServer::start();
    let c = s.login();
    let st = c.api("/status").json();
    assert_eq!(
        keys(&st),
        [
            "app",
            "inbox",
            "ledger",
            "mode",
            "schema",
            "setup_done",
            "version"
        ]
    );
    assert_eq!(
        keys(&st["ledger"]),
        [
            "entries",
            "error",
            "events",
            "events_in_db",
            "events_loaded",
            "head_hash",
            "head_seq",
            "last_recorded_at",
            "verified"
        ]
    );
    assert_eq!(st["app"], "kamoku");
    assert_eq!(st["mode"], "normal");
    assert_eq!(st["schema"], 1);
    assert_eq!(st["setup_done"], false);
    assert_eq!(st["ledger"]["head_hash"], "0".repeat(64));
    assert_eq!(st["ledger"]["verified"], true);
    assert!(st["ledger"]["error"].is_null());
    assert_eq!(keys(&st["inbox"]), ["draft", "ignored"]);
    books(&c);
    create(&c, &expense("2026-10-05", 1100, "exp_comm", "w01"));
    let st = c.api("/status").json();
    assert_eq!(st["ledger"]["events"], 1);
    assert_eq!(st["ledger"]["events_in_db"], 1);
    assert_eq!(st["ledger"]["entries"], 1);
    assert_eq!(st["ledger"]["head_seq"], 1);
    assert!(st["ledger"]["last_recorded_at"].as_i64().unwrap() > 1_700_000_000);
}

// SC-00: 初回は現金の口座が1つ用意されている。起動し直しても増えない。
#[test]
fn first_start_has_one_cash_wallet() {
    let s = TestServer::start();
    let wallets = |c: &Client| -> Vec<Value> {
        c.api("/accounts?wallet=1").json()["items"]
            .as_array()
            .unwrap()
            .clone()
    };
    let first = wallets(&s.login());
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["name"], "現金");
    assert_eq!(first[0]["wallet"]["type"], "cash");
    let (dir, db) = s.stop();
    let again = TestServer::on(dir, db, |_| {});
    assert_eq!(wallets(&again.login()).len(), 1);
}

// api-masters: 設定の置き換えと検査（登録番号は課税のときだけ、形はT+13桁）。
#[test]
fn settings_are_replaced_and_checked() {
    let s = TestServer::start();
    let c = s.login();
    let base = json!({ "business_name": "サンプル事務所", "owner_name": "", "tax_mode": "exempt",
        "invoice_number": null, "rounding": "floor", "opened_on": "2026-01-01", "locked_until": null, "setup_done": true });
    let r = c.put("/settings", &base);
    assert_eq!(r.status, 200);
    assert_eq!(r.json(), base);
    assert_eq!(c.api("/settings").json(), base);
    let mut bad = base.clone();
    bad["invoice_number"] = json!("T1234567890123");
    assert_eq!(c.put("/settings", &bad).code(), "bad_invoice_number");
    bad["tax_mode"] = json!("taxable");
    assert_eq!(c.put("/settings", &bad).status, 200);
    bad["invoice_number"] = json!("T123");
    let r = c.put("/settings", &bad);
    assert_eq!(
        (r.status, r.code().as_str(), r.field().as_str()),
        (422, "bad_invoice_number", "invoice_number")
    );
    let mut wrong = base.clone();
    wrong["tax_mode"] = json!("free");
    assert_eq!(
        (
            c.put("/settings", &wrong).status,
            c.put("/settings", &wrong).field()
        ),
        (400, "tax_mode".to_owned())
    );
    let mut no_date = base.clone();
    no_date["opened_on"] = json!("");
    assert_eq!(c.put("/settings", &no_date).status, 422);
    let mut missing = base.as_object().unwrap().clone();
    missing.remove("setup_done");
    assert_eq!(c.put("/settings", &Value::Object(missing)).status, 400);
}

// AC-01.2・api-masters: 口座と科目の追加、変えられる項目と変えられない項目。
#[test]
fn accounts_add_patch_and_protect() {
    let s = TestServer::start();
    let c = s.login();
    let card = c.post("/accounts", &json!({ "name": "デモカード", "reading": "でもかーど", "wallet": { "type": "card", "institution": "" } }));
    assert_eq!(card.status, 201, "{}", card.text());
    let card = card.json();
    assert_eq!(card["code"], "w02");
    assert_eq!(card["kind"], "liability");
    assert_eq!(card["parent"], "accrued");
    assert_eq!(card["normal_side"], "credit");
    let own = c.post(
        "/accounts",
        &json!({ "name": "デモ費", "kind": "expense", "default_tax": "P10", "default_pct": 30 }),
    );
    assert_eq!(own.json()["code"], "x01");
    let mismatch = c.post(
        "/accounts",
        &json!({ "name": "変な口座", "kind": "asset", "wallet": { "type": "card" } }),
    );
    assert_eq!(mismatch.status, 422);
    assert_eq!(
        c.post("/accounts", &json!({ "name": "  ", "kind": "expense" }))
            .status,
        422
    );
    assert_eq!(
        c.post(
            "/accounts",
            &json!({ "name": "x", "kind": "expense", "default_tax": "Z1" })
        )
        .code(),
        "bad_tax_code"
    );
    let renamed = c.patch(
        "/accounts/w02",
        &json!({ "name": "サンプルカード", "default_pct": 100 }),
    );
    assert_eq!(renamed.status, 200);
    assert_eq!(renamed.json()["name"], "サンプルカード");
    let type_change = c.patch(
        "/accounts/w02",
        &json!({ "wallet": { "type": "bank", "institution": "" } }),
    );
    assert_eq!(
        (type_change.status, type_change.code().as_str()),
        (409, "protected_account")
    );
    assert_eq!(
        c.patch("/accounts/exp_comm", &json!({ "kind": "revenue" }))
            .status,
        409
    );
    assert_eq!(
        c.patch("/accounts/owner_draw", &json!({ "name": "私用" }))
            .status,
        409
    );
    assert_eq!(
        c.patch("/accounts/cash", &json!({ "enabled": false }))
            .status,
        409
    );
    assert_eq!(
        c.patch("/accounts/nothing", &json!({ "name": "x" })).status,
        404
    );
    let off = c.patch(
        "/accounts/exp_books",
        &json!({ "enabled": false, "default_tax": null }),
    );
    assert_eq!(off.json()["enabled"], false);
    assert!(off.json()["default_tax"].is_null());
    let list = |q: &str| c.api(&format!("/accounts{q}")).json();
    assert!(list("").as_object().unwrap().contains_key("total"));
    assert!(
        list("?kind=expense")["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["kind"] == "expense")
    );
    assert!(
        list("?wallet=1")["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["wallet"].is_object())
    );
    let enabled = list("")["total"].as_u64().unwrap();
    let all = list("?include_disabled=1")["total"].as_u64().unwrap();
    assert!(all > enabled);
    assert_eq!(c.api("/accounts?kind=money").status, 400);
}

// api-masters: 取引先の名前は重複できない。登録番号は形を検査する。
#[test]
fn partners_are_unique_and_checked() {
    let s = TestServer::start();
    let c = s.login();
    let p = c.post("/partners", &json!({ "name": "サンプル商事", "reading": "さんぷるしょうじ", "invoice_number": "T1234567890123", "note": "" }));
    assert_eq!(p.status, 201);
    let p = p.json();
    assert_eq!(
        keys(&p),
        ["enabled", "id", "invoice_number", "name", "note", "reading"]
    );
    let dup = c.post("/partners", &json!({ "name": "サンプル商事" }));
    assert_eq!((dup.status, dup.code().as_str()), (409, "duplicate_name"));
    let bad = c.post(
        "/partners",
        &json!({ "name": "デモ商店", "invoice_number": "T12" }),
    );
    assert_eq!(
        (bad.status, bad.code().as_str()),
        (422, "bad_invoice_number")
    );
    let id = p["id"].as_i64().unwrap();
    let patched = c.patch(
        &format!("/partners/{id}"),
        &json!({ "invoice_number": null, "note": "メモ" }),
    );
    assert_eq!(patched.status, 200);
    assert!(patched.json()["invoice_number"].is_null());
    c.post(
        "/partners",
        &json!({ "name": "デモ商店", "reading": "でもしょうてん" }),
    );
    let found = c.api("/partners?q=%E3%81%A7%E3%82%82").json();
    assert_eq!(found["total"], 1);
    assert_eq!(
        c.api("/partners?limit=1").json()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        c.patch("/partners/999", &json!({ "name": "x" })).status,
        404
    );
}
