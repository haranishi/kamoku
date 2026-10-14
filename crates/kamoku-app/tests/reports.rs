// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ホームのまとめと帳票（`docs/spec/api-reports.md`）。計算は kamoku-books なので、ここでは
//! 通信の形、期間の扱い、帳票どうしの一致を確かめる。

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

// 2年分の帳簿（2026年と2027年）。
fn two_years(c: &Client) {
    books(c);
    c.put(
        "/opening",
        &json!({ "balances": [{ "account": "w02", "amount": 500000 }] }),
    );
    create(
        c,
        &json!({ "kind": "income", "date": "2026-05-10", "amount": 330000, "account": "sales", "wallet": "w02", "tax_code": "S10" }),
    );
    create(c, &expense("2026-06-01", 33000, "exp_outsource", "w02"));
    create(
        c,
        &json!({ "kind": "expense", "date": "2026-07-01", "amount": 100000, "account": "owner_draw", "wallet": "w02" }),
    );
    create(
        c,
        &json!({ "kind": "income", "date": "2026-08-01", "amount": 20000, "account": "owner_inv", "wallet": "w01" }),
    );
    create(
        c,
        &json!({ "kind": "income", "date": "2027-02-01", "amount": 110000, "account": "sales", "wallet": "w02", "tax_code": "S10" }),
    );
    create(c, &expense("2027-03-01", 11000, "exp_comm", "w03"));
    create(
        c,
        &json!({ "kind": "expense", "date": "2027-03-15", "amount": 10000, "account": "owner_draw", "wallet": "w02" }),
    );
}

// AC-07.2: 今月と今年の数字、口座、12か月の推移、経費の上位5科目を1回で返す。AC-07.3: 事業主貸・借は入れない。
// AC-07.5: 消費税の見込みは課税事業者のときだけ。
#[test]
fn dashboard_has_the_documented_shape() {
    let s = TestServer::start();
    let c = s.login();
    two_years(&c);
    let d = c.api("/reports/dashboard?month=2026-08").json();
    assert_eq!(
        keys(&d),
        [
            "fiscal_year",
            "ledger",
            "month",
            "month_summary",
            "next_actions",
            "series",
            "tax",
            "top_expenses",
            "wallets",
            "ytd"
        ]
    );
    assert_eq!(d["month"], "2026-08");
    assert_eq!(d["fiscal_year"], 2026);
    assert_eq!(
        keys(&d["next_actions"]),
        ["inbox_draft", "inbox_out_amount"]
    );
    assert_eq!(
        d["month_summary"],
        json!({ "revenue": 0, "expense": 0, "profit": 0, "prev": { "revenue": 0, "expense": 0, "profit": 0 } })
    );
    assert_eq!(
        d["ytd"],
        json!({ "revenue": 330000, "expense": 33000, "profit": 297000 }),
        "事業主貸・借は入らない"
    );
    let series = d["series"].as_array().unwrap();
    assert_eq!(series.len(), 12);
    assert_eq!(series[0]["month"], "2026-01");
    assert_eq!(
        series[4],
        json!({ "month": "2026-05", "revenue": 330000, "expense": 0, "profit": 330000 })
    );
    assert_eq!(keys(&d["wallets"]), ["cash_total", "items", "owed_total"]);
    assert_eq!(d["tax"]["applicable"], true);
    assert_eq!(
        keys(&d["tax"]),
        [
            "applicable",
            "as_of",
            "estimate",
            "purchase_tax",
            "sales_tax"
        ]
    );
    assert_eq!(d["tax"]["sales_tax"], 30000);
    assert_eq!(d["tax"]["purchase_tax"], 3000);
    assert_eq!(d["tax"]["as_of"], "2026-08-31");
    assert_eq!(keys(&d["ledger"]), ["events", "head_hash", "verified"]);
    let june = c.api("/reports/dashboard?month=2026-06").json();
    assert_eq!(
        june["top_expenses"],
        json!([{ "account": "exp_outsource", "name": "外注費", "amount": 33000 }])
    );
    assert_eq!(june["month_summary"]["prev"]["profit"], 330000);
    let jan = c.api("/reports/dashboard?month=2027-01").json();
    assert_eq!(
        jan["month_summary"]["prev"]["revenue"], 0,
        "前の月は前の年の12月"
    );
    let mut settings = c.api("/settings").json();
    settings["tax_mode"] = json!("exempt");
    c.put("/settings", &settings);
    assert_eq!(
        c.api("/reports/dashboard?month=2026-08").json()["tax"],
        json!({ "applicable": false })
    );
    assert_eq!(c.api("/reports/dashboard?month=2026-13").status, 400);
    assert_eq!(c.api("/reports/dashboard").status, 200);
}

// AC-08.1・AC-08.4: 試算表の動きと期末の残高が貸借で一致し、総勘定元帳の期首・期末と同じ値。
// AC-08.2: 年をまたいでも 資産＝負債＋資本（元入金＋当期純利益）。AC-08.8: 年をまたぐ期間は断る。
#[test]
fn reports_balance_across_years() {
    let s = TestServer::start();
    let c = s.login();
    two_years(&c);
    for period in ["year=2026", "year=2027", "from=2027-01-01&to=2027-03-31"] {
        let tb = c.api(&format!("/reports/trial-balance?{period}")).json();
        assert_eq!(tb["balanced"], true, "{period}: {tb}");
        assert_eq!(tb["total_debit"], tb["total_credit"]);
        for row in tb["rows"].as_array().unwrap() {
            let code = row["code"].as_str().unwrap();
            let ledger = c
                .api(&format!("/reports/ledger?account={code}&{period}"))
                .json();
            assert_eq!(ledger["opening"], row["opening"], "{period} {code}");
            assert_eq!(ledger["closing"], row["closing"], "{period} {code}");
        }
    }
    let tb27 = c.api("/reports/trial-balance?year=2027").json();
    let opening = |code: &str| {
        tb27["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == code)
            .map(|r| r["opening"].clone())
    };
    // 2026年の利益297,000と、事業主借20,000−事業主貸100,000は、2027年の期首に元入金へ入る。
    assert_eq!(
        opening("capital"),
        Some(json!(500000 + 297000 + 20000 - 100000))
    );
    assert_eq!(opening("owner_draw").unwrap_or(json!(0)), json!(0));
    let bs = c.api("/reports/balance-sheet?as_of=2027-03-31").json();
    assert_eq!(bs["balanced"], true, "{bs}");
    let equity = bs["equity"]["rows"].as_array().unwrap();
    assert_eq!(equity.last().unwrap()["code"], "net_income_current");
    assert_eq!(bs["net_income_current"], 110000 - 11000);
    let capital = equity.iter().find(|r| r["code"] == "capital").unwrap();
    assert_eq!(capital["amount"], 717000);
    let spans = c.api("/reports/trial-balance?from=2026-12-01&to=2027-01-31");
    assert_eq!(
        (spans.status, spans.code().as_str()),
        (422, "period_spans_years")
    );
    for path in ["income-statement", "ledger?account=w02&", "tax-summary"] {
        let sep = if path.ends_with('&') { "" } else { "?" };
        let r = c.api(&format!(
            "/reports/{path}{sep}from=2026-12-01&to=2027-01-31"
        ));
        assert_eq!(r.code(), "period_spans_years", "{path}");
    }
    assert_eq!(
        c.api("/reports/trial-balance?from=2026-05-01&to=2026-04-01")
            .status,
        400
    );
    assert_eq!(c.api("/reports/trial-balance?from=2026-05-01").status, 400);
    assert_eq!(c.api("/reports/ledger?year=2026").status, 400);
    assert_eq!(
        c.api("/reports/ledger?account=nothing&year=2026").code(),
        "unknown_account"
    );
}

// AC-08.3: 損益計算書は月別の列でも出せ、12か月の合計が年の合計と一致する。
#[test]
fn monthly_income_statement_adds_up() {
    let s = TestServer::start();
    let c = s.login();
    two_years(&c);
    let total = c.api("/reports/income-statement?year=2026").json();
    assert_eq!(total["net_income"], 297000);
    assert!(total.get("months").is_none());
    let monthly = c.api("/reports/income-statement?year=2026&by=month").json();
    let months = monthly["months"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    let sum: i64 = months
        .iter()
        .map(|m| m["net_income"].as_i64().unwrap())
        .sum();
    assert_eq!(sum, monthly["net_income"].as_i64().unwrap());
    for row in monthly["revenue"]["rows"].as_array().unwrap() {
        let s: i64 = row["months"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .sum();
        assert_eq!(s, row["amount"].as_i64().unwrap());
    }
    assert_eq!(
        c.api("/reports/income-statement?year=2026&by=week").status,
        400
    );
}

// AC-08.5: 消費税の集計は税区分ごとに課税標準と税額を出し、経過措置の割合は日付で決まる。
#[test]
fn tax_summary_uses_dates_for_transitional_rates() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    create(
        &c,
        &json!({ "kind": "income", "date": "2026-10-10", "amount": 110000, "account": "sales", "wallet": "w02", "tax_code": "S10" }),
    );
    let mut before = expense("2026-09-30", 11000, "exp_outsource", "w02");
    before["tax_code"] = json!("P10X");
    create(&c, &before);
    let mut after = expense("2026-10-01", 11000, "exp_outsource", "w02");
    after["tax_code"] = json!("P10X");
    create(&c, &after);
    let t = c.api("/reports/tax-summary?year=2026").json();
    assert_eq!(
        keys(&t),
        [
            "disclaimer",
            "estimate",
            "purchase_tax",
            "rows",
            "sales_tax",
            "transitional"
        ]
    );
    assert_eq!(t["sales_tax"], 10000);
    let transitional = t["transitional"].as_array().unwrap();
    let percents: Vec<i64> = transitional
        .iter()
        .map(|r| r["percent"].as_i64().unwrap())
        .collect();
    assert_eq!(percents, vec![70, 80]);
    assert_eq!(t["purchase_tax"], 1000 * 70 / 100 + 1000 * 80 / 100);
    assert!(t["disclaimer"].as_str().unwrap().contains("2026-10-14"));
    assert_eq!(t["rows"][0]["tax_code"], "S10");
}
