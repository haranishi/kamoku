// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 帳票の、手で計算した固定値と、JSONの形（docs/spec/api-reports.md）の試験。

// JSONの例は、通信の約束（docs/spec/）と同じ書き方で比べたいので、数字に区切りを入れない。
#![allow(clippy::unreadable_literal)]
mod common;
use common::{chart, entry, expense, floor, income, numbered, refs};
use kamoku_books::{
    ReportError, SimpleInput, account_ledger, balance_sheet, compose_opening, compose_simple,
    income_statement, monthly_series, tax_summary, trial_balance, wallet_balances,
};
use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use serde_json::json;
use time::macros::date;

fn described(mut i: SimpleInput, text: &str) -> SimpleInput {
    text.clone_into(&mut i.description);
    i
}
/// 貸借対照表の例（docs/spec/api-reports.md の節末）と同じ数字の帳簿。
/// 2026年: 元入金500,000、利益300,000（売上400,000・経費100,000）、事業主貸100,000、事業主借20,000。
/// 2027年3月31日まで: 利益50,000（売上80,000・経費30,000）、事業主貸10,000。
fn spec_example() -> Vec<Entry> {
    let chart = chart();
    let opening = compose_opening(
        date!(2026 - 01 - 01),
        &[(AccountCode::new("w03").unwrap(), Yen::new(500_000))],
        &chart,
    )
    .unwrap()
    .unwrap();
    let mut out = vec![entry(&opening, "E0")];
    let inputs = [
        described(
            income(date!(2026 - 03 - 31), 400_000, "sales", "w03"),
            "売上 2026",
        ),
        described(
            expense(date!(2026 - 04 - 30), 100_000, "exp_supplies", "w03"),
            "消耗品 2026",
        ),
        described(
            expense(date!(2026 - 06 - 30), 100_000, "owner_draw", "w03"),
            "生活費 2026",
        ),
        described(
            income(date!(2026 - 07 - 31), 20_000, "owner_inv", "w03"),
            "個人から 2026",
        ),
        described(
            income(date!(2027 - 02 - 28), 80_000, "sales", "w03"),
            "売上 デモ",
        ),
        described(
            expense(date!(2027 - 03 - 15), 30_000, "exp_comm", "w03"),
            "通信費 デモ",
        ),
        described(
            expense(date!(2027 - 03 - 20), 10_000, "owner_draw", "w03"),
            "生活費",
        ),
        described(
            income(date!(2027 - 04 - 01), 999, "sales", "w03"),
            "期間の外",
        ),
    ];
    for (n, i) in inputs.iter().enumerate() {
        out.push(entry(
            &compose_simple(i, &chart, &floor()).unwrap(),
            &format!("E{}", n + 1),
        ));
    }
    out
}

// AC-08.2: 例の数字のとおり。元入金の行は720,000円、当期純利益は50,000円、資産に事業主貸10,000円。
#[test]
fn balance_sheet_spec_example() {
    let entries = spec_example();
    let bs = balance_sheet(&refs(&entries), &chart(), date!(2027 - 03 - 31)).unwrap();
    assert_eq!(
        serde_json::to_value(&bs).unwrap(),
        json!({ "as_of": "2027-03-31",
            "assets": { "rows": [
                { "code": "dep_other", "name": "普通預金・その他の預金", "amount": 760000, "own": 0, "children": [
                    { "code": "w03", "name": "デモ銀行", "amount": 760000, "own": 760000 } ] },
                { "code": "owner_draw", "name": "事業主貸", "amount": 10000, "own": 10000 } ],
              "total": 770000 },
            "liabilities": { "rows": [], "total": 0 },
            "equity": { "rows": [
                { "code": "capital", "name": "元入金", "amount": 720000, "own": 720000 },
                { "code": "net_income_current", "name": "当期純利益", "amount": 50000, "own": 50000 } ],
              "total": 770000 },
            "net_income_current": 50000, "balanced": true })
    );
    // 2026年の末では、まだ繰り越していない。
    let end = balance_sheet(&refs(&entries), &chart(), date!(2026 - 12 - 31)).unwrap();
    assert_eq!(end.net_income_current, Yen::new(300_000));
    assert_eq!(end.equity.rows[0].amount, Yen::new(500_000));
    assert!(end.balanced);
}

// AC-08.1: 試算表の期首・動き・期末。期首の元入金は仮想の繰越を含む。
#[test]
fn trial_balance_spec_example() {
    let entries = spec_example();
    let tb = trial_balance(
        &refs(&entries),
        &chart(),
        date!(2027 - 01 - 01),
        date!(2027 - 03 - 31),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&tb).unwrap(),
        json!({ "rows": [
            { "code": "owner_draw", "name": "事業主貸", "kind": "asset", "opening": 0, "debit": 10000, "credit": 0, "closing": 10000 },
            { "code": "w03", "name": "デモ銀行", "kind": "asset", "opening": 720000, "debit": 80000, "credit": 40000, "closing": 760000 },
            { "code": "capital", "name": "元入金", "kind": "equity", "opening": 720000, "debit": 0, "credit": 0, "closing": 720000 },
            { "code": "sales", "name": "売上高", "kind": "revenue", "opening": 0, "debit": 0, "credit": 80000, "closing": 80000 },
            { "code": "exp_comm", "name": "通信費", "kind": "expense", "opening": 0, "debit": 30000, "credit": 0, "closing": 30000 } ],
          "total_debit": 120000, "total_credit": 120000, "balanced": true })
    );
    // 年の途中から始めると、収益と費用の期首は、その年の1月からの累計。
    let mid = trial_balance(
        &refs(&entries),
        &chart(),
        date!(2027 - 03 - 01),
        date!(2027 - 03 - 31),
    )
    .unwrap();
    let sales = mid.rows.iter().find(|r| r.code == "sales").unwrap();
    assert_eq!(
        (sales.opening, sales.credit, sales.closing),
        (Yen::new(80_000), Yen::ZERO, Yen::new(80_000))
    );
}

// AC-08.4: 元帳の期首は試算表と同じ値で、残高の推移の末尾は試算表の期末と一致する。相手科目と諸口。
#[test]
fn ledger_spec_example() {
    let entries = spec_example();
    let ledger = account_ledger(
        &numbered(&entries),
        &chart(),
        "w03",
        date!(2027 - 01 - 01),
        date!(2027 - 03 - 31),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&ledger).unwrap(),
        json!({ "account": "w03", "opening": 720000, "rows": [
            { "date": "2027-02-28", "no": 6, "entry_id": "E5", "description": "売上 デモ", "counter": "売上高", "debit": 80000, "credit": 0, "balance": 800000 },
            { "date": "2027-03-15", "no": 7, "entry_id": "E6", "description": "通信費 デモ", "counter": "通信費", "debit": 0, "credit": 30000, "balance": 770000 },
            { "date": "2027-03-20", "no": 8, "entry_id": "E7", "description": "生活費", "counter": "事業主貸", "debit": 0, "credit": 10000, "balance": 760000 } ],
          "closing": 760000 })
    );
    let capital = account_ledger(
        &numbered(&entries),
        &chart(),
        "capital",
        date!(2027 - 01 - 01),
        date!(2027 - 12 - 31),
    )
    .unwrap();
    assert_eq!(
        (capital.opening, capital.closing),
        (Yen::new(720_000), Yen::new(720_000))
    );
    // 反対側に科目が2つ以上ある明細の相手科目は「諸口」。
    let mut split = expense(date!(2026 - 05 - 01), 10_000, "exp_comm", "w02");
    split.business_pct = Some(70);
    let e = entry(&compose_simple(&split, &chart(), &floor()).unwrap(), "S");
    let card = account_ledger(
        &[(1, &e)],
        &chart(),
        "w02",
        date!(2026 - 01 - 01),
        date!(2026 - 12 - 31),
    )
    .unwrap();
    assert_eq!(card.rows[0].counter, "諸口");
    assert_eq!(card.closing, Yen::new(10_000));
    assert_eq!(
        account_ledger(
            &[(1, &e)],
            &chart(),
            "nope",
            date!(2026 - 01 - 01),
            date!(2026 - 12 - 31)
        )
        .unwrap_err(),
        ReportError::UnknownAccount("nope".to_owned())
    );
}

// AC-08.3 AC-07.3: 月別の損益。事業主貸・事業主借は入らない。総計の形には months が無い。
#[test]
fn income_statement_and_series() {
    let entries = spec_example();
    let is = income_statement(
        &refs(&entries),
        &chart(),
        date!(2027 - 01 - 01),
        date!(2027 - 03 - 31),
        true,
    )
    .unwrap();
    let z = 0;
    assert_eq!(
        serde_json::to_value(&is).unwrap(),
        json!({ "revenue": { "rows": [ { "code": "sales", "name": "売上高", "amount": 80000, "months": [z, 80000, z, z, z, z, z, z, z, z, z, z] } ], "total": 80000 },
            "expense": { "rows": [ { "code": "exp_comm", "name": "通信費", "amount": 30000, "months": [z, z, 30000, z, z, z, z, z, z, z, z, z] } ], "total": 30000 },
            "net_income": 50000,
            "months": [
                { "month": "2027-01", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-02", "revenue": 80000, "expense": 0, "net_income": 80000 },
                { "month": "2027-03", "revenue": 0, "expense": 30000, "net_income": -30000 },
                { "month": "2027-04", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-05", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-06", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-07", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-08", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-09", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-10", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-11", "revenue": 0, "expense": 0, "net_income": 0 },
                { "month": "2027-12", "revenue": 0, "expense": 0, "net_income": 0 } ] })
    );
    let total = income_statement(
        &refs(&entries),
        &chart(),
        date!(2026 - 01 - 01),
        date!(2026 - 12 - 31),
        false,
    )
    .unwrap();
    let value = serde_json::to_value(&total).unwrap();
    assert!(value.get("months").is_none() && value["revenue"]["rows"][0].get("months").is_none());
    assert_eq!(
        (total.revenue.total, total.expense.total, total.net_income),
        (Yen::new(400_000), Yen::new(100_000), Yen::new(300_000))
    );
    let series = monthly_series(&refs(&entries), &chart(), 2026).unwrap();
    assert_eq!(series.len(), 12);
    assert_eq!(
        serde_json::to_value(&series[2]).unwrap(),
        json!({ "month": "2026-03", "revenue": 400000, "expense": 0, "profit": 400000 })
    );
    // 6月の事業主貸、7月の事業主借は、売上にも経費にも入らない。
    assert_eq!(
        (series[5].expense, series[6].revenue),
        (Yen::ZERO, Yen::ZERO)
    );
}

// AC-08.8: 帳票は1つの暦年に収まる期間だけ。期間の誤り、知らない科目・税区分、集計の上限も断る。
#[test]
fn report_errors() {
    let entries = spec_example();
    let all = refs(&entries);
    let c = chart();
    let spans =
        |e: Result<(), ReportError>| assert_eq!(e.unwrap_err(), ReportError::PeriodSpansYears);
    spans(trial_balance(&all, &c, date!(2026 - 12 - 01), date!(2027 - 01 - 31)).map(|_| ()));
    spans(
        income_statement(
            &all,
            &c,
            date!(2026 - 12 - 01),
            date!(2027 - 01 - 31),
            false,
        )
        .map(|_| ()),
    );
    spans(tax_summary(&all, &c, date!(2026 - 12 - 01), date!(2027 - 01 - 31)).map(|_| ()));
    spans(
        account_ledger(
            &numbered(&entries),
            &c,
            "w03",
            date!(2026 - 12 - 01),
            date!(2027 - 01 - 31),
        )
        .map(|_| ()),
    );
    let e = trial_balance(&all, &c, date!(2026 - 12 - 31), date!(2026 - 12 - 01)).unwrap_err();
    assert_eq!(
        (e, ReportError::PeriodSpansYears.code()),
        (ReportError::BadPeriod, "period_spans_years")
    );
    let line = |code: &str, side, amount| {
        Line::new(AccountCode::new(code).unwrap(), side, Yen::new(amount))
    };
    let stray = Entry::new(
        EntryId::new("X").unwrap(),
        date!(2026 - 05 - 01),
        "",
        vec![line("zzz", Side::Debit, 1), line("w01", Side::Credit, 1)],
    )
    .unwrap();
    assert_eq!(
        balance_sheet(&[&stray], &c, date!(2026 - 12 - 31)).unwrap_err(),
        ReportError::UnknownAccount("zzz".to_owned())
    );
    let odd_tax = Entry::new(
        EntryId::new("Y").unwrap(),
        date!(2026 - 05 - 01),
        "",
        vec![
            line("exp_comm", Side::Debit, 1).with_tax("TAX10", Yen::ZERO),
            line("w01", Side::Credit, 1),
        ],
    )
    .unwrap();
    assert_eq!(
        tax_summary(
            &[&odd_tax],
            &c,
            date!(2026 - 01 - 01),
            date!(2026 - 12 - 31)
        )
        .unwrap_err(),
        ReportError::UnknownTaxCode("TAX10".to_owned())
    );
    // 1明細ずつは台帳に入っても、合計がブラウザの数値の範囲を超えたら返さない（docs/spec/data-model.md 決まり16）。
    let big = |id: &str| {
        Entry::new(
            EntryId::new(id).unwrap(),
            date!(2026 - 05 - 01),
            "",
            vec![
                line("w01", Side::Debit, 5_000_000_000_000_000),
                line("sales", Side::Credit, 5_000_000_000_000_000),
            ],
        )
        .unwrap()
    };
    let (a, b) = (big("A"), big("B"));
    assert_eq!(
        wallet_balances(&[&a, &b], &c, date!(2026 - 12 - 31)).unwrap_err(),
        ReportError::AmountRange
    );
    assert!(wallet_balances(&[&a], &c, date!(2026 - 12 - 31)).is_ok());
    for e in [
        ReportError::BadPeriod,
        ReportError::AmountRange,
        ReportError::UnknownAccount("x".into()),
    ] {
        assert!(e.message().ends_with('。'));
    }
}
