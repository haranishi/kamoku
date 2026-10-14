// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の組み立ての、手で計算した固定値の試験。

// JSONの例は、通信の約束（docs/spec/）と同じ書き方で比べたいので、数字に区切りを入れない。
#![allow(clippy::unreadable_literal)]
mod common;
use common::{chart, entry, expense, floor, income, settings, transfer};
use kamoku_books::{LineView, Rounding, TaxCode, compose_simple, tax_included};
use kamoku_ledger::{Line, Side, Yen};
use time::macros::date;

fn lines(c: &kamoku_books::Composed) -> Vec<(String, Side, i64, Option<String>, i64, String)> {
    c.lines
        .iter()
        .map(|l: &Line| {
            (
                l.account.as_str().to_owned(),
                l.side,
                l.amount.get(),
                l.tax_code.clone(),
                l.tax_amount.get(),
                l.memo.clone(),
            )
        })
        .collect()
}
fn row(
    a: &str,
    s: Side,
    amount: i64,
    tax: Option<(&str, i64)>,
    memo: &str,
) -> (String, Side, i64, Option<String>, i64, String) {
    (
        a.to_owned(),
        s,
        amount,
        tax.map(|t| t.0.to_owned()),
        tax.map_or(0, |t| t.1),
        memo.to_owned(),
    )
}
const D: Side = Side::Debit;
const C: Side = Side::Credit;

// AC-03.1 AC-03.2 AC-02.2: 11,000円・割合70%・P10・切捨て。事業分7,700円の税は700円、残り3,300円は事業主貸。
#[test]
fn eleven_thousand_at_seventy_percent() {
    let mut i = expense(date!(2026 - 10 - 05), 11_000, "exp_comm", "w02");
    i.tax_code = Some("P10".to_owned());
    i.business_pct = Some(70);
    i.partner = "サンプル通信".to_owned();
    i.description = "10月分 通信費".to_owned();
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(
        lines(&c),
        vec![
            row("exp_comm", D, 7700, Some(("P10", 700)), ""),
            row("owner_draw", D, 3300, None, "家事按分 30%"),
            row("w02", C, 11_000, None, ""),
        ]
    );
    assert_eq!(c.total(), Some(Yen::new(11_000)));
    let meta: Vec<(&str, &str)> = c
        .meta
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        meta,
        vec![
            ("business_pct", "70"),
            ("kind", "expense"),
            ("partner", "サンプル通信"),
            ("source", "manual"),
        ]
    );
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    // AC-02.2: 画面に見せる明細と、保存する仕訳の明細は同じ。
    let saved = entry(&c, "E1");
    assert_eq!(saved.lines(), c.lines.as_slice());
    assert_eq!(saved.meta(), &c.meta);
    let chart = chart();
    let shown = c.line_views(&chart);
    let stored: Vec<LineView> = saved
        .lines()
        .iter()
        .map(|l| LineView::new(l, &chart))
        .collect();
    assert_eq!(shown, stored);
    // 通信の約束の例（docs/spec/api-entries.md）と同じ形のJSON。口座の名前だけが合成の名前。
    assert_eq!(
        serde_json::to_value(&shown).unwrap(),
        serde_json::json!([
            { "account": "exp_comm", "account_name": "通信費", "side": "debit", "amount": 7700, "tax_code": "P10", "tax_amount": 700, "memo": "" },
            { "account": "owner_draw", "account_name": "事業主貸", "side": "debit", "amount": 3300, "tax_code": null, "tax_amount": 0, "memo": "家事按分 30%" },
            { "account": "w02", "account_name": "サンプルカード", "side": "credit", "amount": 11000, "tax_code": null, "tax_amount": 0, "memo": "" }
        ])
    );
}

// AC-03.2: 1円・割合50%は、0.5円を四捨五入して事業分1円、残り0円（事業主貸の行は作らない）。3円・50%は2円と1円。
#[test]
fn half_yen_rounds_up() {
    let mut i = expense(date!(2026 - 10 - 05), 1, "exp_supplies", "w01");
    i.business_pct = Some(50);
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(
        lines(&c),
        vec![
            row("exp_supplies", D, 1, None, ""),
            row("w01", C, 1, None, "")
        ]
    );
    i.amount = 3;
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(
        lines(&c),
        vec![
            row("exp_supplies", D, 2, None, ""),
            row("owner_draw", D, 1, None, "家事按分 50%"),
            row("w01", C, 3, None, ""),
        ]
    );
}

// AC-03.5: 事業分が0円（割合0、または端数）なら、科目の行を作らず、事業主貸と口座の2行にして警告を出す。
#[test]
fn zero_business_share_has_no_account_line() {
    for (amount, pct, memo) in [(1000, 0, "家事按分 100%"), (1, 40, "家事按分 60%")] {
        let mut i = expense(date!(2026 - 10 - 05), amount, "exp_comm", "w02");
        i.business_pct = Some(pct);
        i.tax_code = Some("P10".to_owned());
        let c = compose_simple(&i, &chart(), &floor()).unwrap();
        assert_eq!(
            lines(&c),
            vec![
                row("owner_draw", D, amount, None, memo),
                row("w02", C, amount, None, "")
            ]
        );
        assert!(
            c.warnings.iter().any(|w| w.contains("事業分が0円")),
            "{:?}",
            c.warnings
        );
        assert!(c.lines.iter().all(|l| l.amount > Yen::ZERO));
    }
    // 割合100なら事業主貸の行は無い。
    let i = expense(date!(2026 - 10 - 05), 1000, "exp_comm", "w02");
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(
        lines(&c),
        vec![
            row("exp_comm", D, 1000, None, ""),
            row("w02", C, 1000, None, "")
        ]
    );
}

// 端数処理の設定ごとの税額（1,000円の10%は90.9円）。軽減税率、非課税も。
#[test]
fn tax_by_rounding_and_rate() {
    for (rounding, tax) in [
        (Rounding::Floor, 90),
        (Rounding::Round, 91),
        (Rounding::Ceil, 91),
    ] {
        let mut i = expense(date!(2026 - 10 - 05), 1000, "exp_supplies", "w01");
        i.tax_code = Some("P10".to_owned());
        let c = compose_simple(&i, &chart(), &settings(rounding)).unwrap();
        assert_eq!(c.lines[0].tax_amount, Yen::new(tax));
    }
    assert_eq!(
        tax_included(TaxCode::P8, Yen::new(1080), Rounding::Floor).unwrap(),
        Yen::new(80)
    );
    assert_eq!(
        tax_included(TaxCode::NT, Yen::new(1080), Rounding::Ceil).unwrap(),
        Yen::ZERO
    );
    assert_eq!(
        tax_included(TaxCode::P10X, Yen::new(11_000), Rounding::Floor).unwrap(),
        Yen::new(1000)
    );
    let mut i = expense(date!(2026 - 10 - 05), 5000, "exp_insurance", "w03");
    i.tax_code = Some("NT".to_owned());
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(
        lines(&c)[0],
        row("exp_insurance", D, 5000, Some(("NT", 0)), "")
    );
}

// 収入と振替の形。メモは口座の明細に入る。
#[test]
fn income_and_transfer_lines() {
    let mut i = income(date!(2026 - 10 - 06), 55_000, "sales", "w03");
    i.tax_code = Some("S10".to_owned());
    i.memo = "請求書 デモ-01".to_owned();
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(
        lines(&c),
        vec![
            row("w03", D, 55_000, None, "請求書 デモ-01"),
            row("sales", C, 55_000, Some(("S10", 5000)), "")
        ]
    );
    assert_eq!(c.meta["kind"], "income");
    assert!(!c.meta.contains_key("business_pct"));
    let c = compose_simple(
        &transfer(date!(2026 - 10 - 07), 30_000, "w03", "w01"),
        &chart(),
        &floor(),
    )
    .unwrap();
    assert_eq!(
        lines(&c),
        vec![
            row("w01", D, 30_000, None, ""),
            row("w03", C, 30_000, None, "")
        ]
    );
    assert_eq!(c.meta["kind"], "transfer");
}

// AC-03.3: 割合を省くと、科目の既定の割合（車両費は30%）を使う。
#[test]
fn default_pct_comes_from_account() {
    let mut i = expense(date!(2026 - 10 - 05), 10_000, "exp_vehicle", "w02");
    i.business_pct = None;
    let c = compose_simple(&i, &chart(), &floor()).unwrap();
    assert_eq!(c.meta["business_pct"], "30");
    assert_eq!(c.lines[0].amount, Yen::new(3000));
    assert_eq!(c.lines[1].amount, Yen::new(7000));
}
