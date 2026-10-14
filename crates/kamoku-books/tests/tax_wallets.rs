// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 消費税の集計と、口座の残高の固定値の試験。

// JSONの例は、通信の約束（docs/spec/）と同じ書き方で比べたいので、数字に区切りを入れない。
#![allow(clippy::unreadable_literal)]
mod common;
use common::{chart, entry, expense, floor, income, refs};
use kamoku_books::{
    JournalInput, JournalLineInput, SimpleInput, compose_journal, compose_simple, tax_summary,
    wallet_balances,
};
use kamoku_ledger::{Entry, Side};
use serde_json::json;
use time::macros::date;

fn tax_example() -> Vec<Entry> {
    let chart = chart();
    let taxed = |mut i: SimpleInput, code: &str| {
        i.tax_code = Some(code.to_owned());
        i
    };
    let inputs = [
        taxed(
            income(date!(2026 - 10 - 01), 110_000, "sales", "w03"),
            "S10",
        ),
        taxed(
            expense(date!(2026 - 10 - 03), 11_000, "exp_comm", "w02"),
            "P10",
        ),
        taxed(
            expense(date!(2026 - 09 - 30), 11_000, "exp_outsource", "w03"),
            "P10X",
        ),
        taxed(
            expense(date!(2026 - 10 - 04), 1001, "exp_outsource", "w03"),
            "P10X",
        ),
        taxed(
            expense(date!(2026 - 10 - 05), 1001, "exp_outsource", "w03"),
            "P10X",
        ),
        taxed(
            expense(date!(2026 - 10 - 06), 5000, "exp_insurance", "w03"),
            "NT",
        ),
    ];
    let mut out: Vec<Entry> = inputs
        .iter()
        .enumerate()
        .map(|(n, i)| {
            entry(
                &compose_simple(i, &chart, &floor()).unwrap(),
                &format!("T{n}"),
            )
        })
        .collect();
    // 売上の返品（借方の S10）は、課税標準と税額から差し引かれる。
    let refund = JournalInput {
        date: date!(2026 - 10 - 02),
        description: "返品 デモ".to_owned(),
        partner: String::new(),
        lines: vec![
            JournalLineInput {
                account: "sales".into(),
                side: Side::Debit,
                amount: 1100,
                tax_code: Some("S10".into()),
                tax_amount: None,
                memo: String::new(),
            },
            JournalLineInput {
                account: "w03".into(),
                side: Side::Credit,
                amount: 1100,
                tax_code: None,
                tax_amount: None,
                memo: String::new(),
            },
        ],
    };
    out.push(entry(
        &compose_journal(&refund, &chart, &floor()).unwrap(),
        "T9",
    ));
    out
}

// AC-08.5: 税区分ごとの課税標準と税額。経過措置は日付で割合を引き、明細ごとに切り捨てる（63.7円→63円を2回）。
#[test]
fn tax_summary_fixed_values() {
    let entries = tax_example();
    let summary = tax_summary(
        &refs(&entries),
        &chart(),
        date!(2026 - 09 - 01),
        date!(2026 - 10 - 31),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&summary).unwrap(),
        json!({ "rows": [
            { "tax_code": "S10", "label": "課税売上10%", "gross": 108900, "tax": 9900 },
            { "tax_code": "P10", "label": "課税仕入10%", "gross": 11000, "tax": 1000 },
            { "tax_code": "P10X", "label": "免税事業者等からの課税仕入れ10%（経過措置）", "gross": 13002, "tax": 1182 },
            { "tax_code": "NT", "label": "非課税", "gross": 5000, "tax": 0 } ],
          "sales_tax": 9900, "purchase_tax": 1926, "estimate": 7974,
          "transitional": [
            { "percent": 70, "gross": 2002, "tax": 182, "deductible": 126 },
            { "percent": 80, "gross": 11000, "tax": 1000, "deductible": 800 } ],
          "disclaimer": "参考値です。簡易課税と特例、経過措置の適用条件（請求書等と帳簿の保存、一の仕入先ごとの上限）は反映していません。課税の区分は現在の設定を使っています。経過措置の表の最終確認日は2026-10-14です。" })
    );
    assert!(
        summary
            .disclaimer
            .contains(kamoku_jp_tax::TRANSITIONAL_TABLE_CHECKED_ON)
    );
}

#[test]
fn wallet_balances_fixed_values() {
    let entries = tax_example();
    let w = wallet_balances(&refs(&entries), &chart(), date!(2026 - 10 - 31)).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        json!({ "items": [
            { "code": "w01", "name": "現金", "type": "cash", "balance": 0 },
            { "code": "w03", "name": "デモ銀行", "type": "bank", "balance": 90898 },
            { "code": "w02", "name": "サンプルカード", "type": "card", "balance": 11000 } ],
          "cash_total": 90898, "owed_total": 11000 })
    );
}
