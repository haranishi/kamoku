// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の形と期首残高の組み立て、訂正の初期値（組み立て直し）の試験。

// JSONの例は、通信の約束（docs/spec/）と同じ書き方で比べたいので、数字に区切りを入れない。
#![allow(clippy::unreadable_literal)]
mod common;
use common::{chart, entry, expense, floor, income, settings, transfer};
use kamoku_books::{
    ComposeError, JournalInput, JournalLineInput, Rounding, compose_journal, compose_opening,
    compose_simple, reconstruct_simple,
};
use kamoku_ledger::{AccountCode, Side, Yen};
use time::macros::date;

fn jl(account: &str, side: Side, amount: i64) -> JournalLineInput {
    JournalLineInput {
        account: account.to_owned(),
        side,
        amount,
        tax_code: None,
        tax_amount: None,
        memo: String::new(),
    }
}
fn journal(lines: Vec<JournalLineInput>) -> JournalInput {
    JournalInput {
        date: date!(2026 - 10 - 10),
        description: "仕訳の試験".to_owned(),
        partner: String::new(),
        lines,
    }
}

// AC-02.6: 貸借が合わなければ、差額つきの日本語で断る（docs/spec/api-common.md の例と同じ文）。
#[test]
fn unbalanced_journal_message() {
    let e = compose_journal(
        &journal(vec![
            jl("exp_comm", Side::Debit, 11_000),
            jl("w01", Side::Credit, 10_000),
        ]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!(e.code(), "unbalanced");
    assert_eq!(e.field(), Some("lines"));
    assert_eq!(
        e.message(),
        "貸借が一致しません。借方 11,000円、貸方 10,000円です。差の1,000円を、どちらかの行に足してください。"
    );
}

// 税額を省いた行は金額から計算する。NTの税額は0、税区分の無い行に税額は入れられない。
#[test]
fn journal_tax_rules() {
    let mut a = jl("exp_supplies", Side::Debit, 1000);
    a.tax_code = Some("P10".to_owned());
    let mut b = jl("exp_supplies", Side::Debit, 1000);
    b.tax_code = Some("P10".to_owned());
    b.tax_amount = Some(95);
    let c = compose_journal(
        &journal(vec![a.clone(), b.clone(), jl("w01", Side::Credit, 2000)]),
        &chart(),
        &settings(Rounding::Ceil),
    )
    .unwrap();
    assert_eq!(c.lines[0].tax_amount, Yen::new(91));
    assert_eq!(c.lines[1].tax_amount, Yen::new(95));
    assert_eq!(c.meta["kind"], "journal");
    let mut nt = jl("exp_insurance", Side::Debit, 1000);
    nt.tax_code = Some("NT".to_owned());
    nt.tax_amount = Some(1);
    let e = compose_journal(
        &journal(vec![nt, jl("w01", Side::Credit, 1000)]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!(
        (e.code(), e.field()),
        ("bad_tax_code", Some("lines[0].tax_amount"))
    );
    let mut no_code = jl("exp_comm", Side::Debit, 1000);
    no_code.tax_amount = Some(10);
    let e = compose_journal(
        &journal(vec![no_code, jl("w01", Side::Credit, 1000)]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!(e.code(), "bad_tax_code");
    b.tax_amount = Some(1001);
    let e = compose_journal(
        &journal(vec![b, jl("w01", Side::Credit, 1000)]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!(
        (e.code(), e.field()),
        ("bad_request", Some("lines[0].tax_amount"))
    );
    let e = compose_journal(
        &journal(vec![jl("w01", Side::Credit, 1000)]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!((e.code(), e.field()), ("bad_request", Some("lines")));
    let e = compose_journal(
        &journal(vec![jl("nope", Side::Debit, 1), jl("w01", Side::Credit, 1)]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!(
        (e.code(), e.field()),
        ("unknown_account", Some("lines[0].account"))
    );
    let e = compose_journal(
        &journal(vec![
            jl("exp_comm", Side::Debit, 0),
            jl("w01", Side::Credit, 0),
        ]),
        &chart(),
        &floor(),
    )
    .unwrap_err();
    assert_eq!(
        (e.code(), e.field()),
        ("amount_range", Some("lines[0].amount"))
    );
}

fn code(s: &str) -> AccountCode {
    AccountCode::new(s).unwrap()
}

// AC-01.3: 期首残高は、資産を借方、負債を貸方に入れ、差額を元入金で合わせる。債務超過なら元入金は借方。
#[test]
fn opening_balances() {
    let c = compose_opening(
        date!(2026 - 01 - 01),
        &[
            (code("w03"), Yen::new(500_000)),
            (code("w01"), Yen::new(30_000)),
            (code("loan_pay"), Yen::new(200_000)),
        ],
        &chart(),
    )
    .unwrap()
    .unwrap();
    let got: Vec<(&str, Side, i64)> = c
        .lines
        .iter()
        .map(|l| (l.account.as_str(), l.side, l.amount.get()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("w03", Side::Debit, 500_000),
            ("w01", Side::Debit, 30_000),
            ("loan_pay", Side::Credit, 200_000),
            ("capital", Side::Credit, 330_000),
        ]
    );
    assert_eq!(
        (c.meta["kind"].as_str(), c.meta["source"].as_str()),
        ("opening", "opening")
    );
    let over = compose_opening(
        date!(2026 - 01 - 01),
        &[
            (code("w01"), Yen::new(100_000)),
            (code("w02"), Yen::new(300_000)),
        ],
        &chart(),
    )
    .unwrap()
    .unwrap();
    let last = over.lines.last().unwrap();
    assert_eq!(
        (last.account.as_str(), last.side, last.amount),
        ("capital", Side::Debit, Yen::new(200_000))
    );
    // 資産と負債が同じなら、元入金の行は作らない（0円の明細を作らない）。
    let even = compose_opening(
        date!(2026 - 01 - 01),
        &[(code("w01"), Yen::new(100)), (code("w02"), Yen::new(100))],
        &chart(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(even.lines.len(), 2);
}

// AC-01.5: 期首残高がすべて0なら仕訳を作らない。0の行は入れない。
#[test]
fn opening_zero_rows_are_dropped() {
    assert_eq!(
        compose_opening(
            date!(2026 - 01 - 01),
            &[(code("w01"), Yen::ZERO), (code("w03"), Yen::ZERO)],
            &chart()
        )
        .unwrap(),
        None
    );
    assert_eq!(
        compose_opening(date!(2026 - 01 - 01), &[], &chart()).unwrap(),
        None
    );
    let c = compose_opening(
        date!(2026 - 01 - 01),
        &[(code("w01"), Yen::ZERO), (code("w03"), Yen::new(1))],
        &chart(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(c.lines.len(), 2);
    assert!(c.lines.iter().all(|l| l.amount > Yen::ZERO));
}

#[test]
fn opening_errors() {
    let bad = |balances: &[(AccountCode, Yen)]| -> ComposeError {
        compose_opening(date!(2026 - 01 - 01), balances, &chart()).unwrap_err()
    };
    assert_eq!(bad(&[(code("sales"), Yen::new(1))]).code(), "bad_request");
    assert_eq!(bad(&[(code("capital"), Yen::new(1))]).code(), "bad_request");
    assert_eq!(
        bad(&[(code("owner_draw"), Yen::new(1))]).code(),
        "bad_request"
    );
    assert_eq!(
        bad(&[(code("w01"), Yen::new(1)), (code("w01"), Yen::new(2))]).code(),
        "bad_request"
    );
    assert_eq!(bad(&[(code("w01"), Yen::new(-1))]).code(), "amount_range");
    assert_eq!(bad(&[(code("w99"), Yen::new(1))]).code(), "unknown_account");
    assert_eq!(
        bad(&[(code("w04"), Yen::new(1))]).code(),
        "disabled_account"
    );
}

// AC-06.4 の土台: 単純な形で作った仕訳は、明細から元の入力に戻せる。戻せない形は None。
#[test]
fn reconstruct_round_trips() {
    let chart = chart();
    let mut inputs = [
        expense(date!(2026 - 10 - 05), 11_000, "exp_comm", "w02"),
        income(date!(2026 - 10 - 06), 55_000, "sales", "w03"),
        transfer(date!(2026 - 10 - 07), 30_000, "w03", "w01"),
        expense(date!(2026 - 10 - 08), 50_000, "w02", "w03"),
    ];
    inputs[0].business_pct = Some(70);
    inputs[0].tax_code = Some("P10".to_owned());
    inputs[0].partner = "サンプル通信".to_owned();
    inputs[0].memo = "メモ".to_owned();
    inputs[1].tax_code = Some("S10".to_owned());
    inputs[1].description = "売上 デモ".to_owned();
    inputs[2].memo = "引き出し".to_owned();
    for (n, i) in inputs.iter().enumerate() {
        for rounding in [Rounding::Floor, Rounding::Round, Rounding::Ceil] {
            let c = compose_simple(i, &chart, &settings(rounding)).unwrap();
            let e = entry(&c, &format!("E{n}"));
            assert_eq!(reconstruct_simple(&e, &chart).as_ref(), Some(i), "{i:?}");
        }
    }
    // 事業分が0円の支出は、科目が分からないので None。
    let mut zero = expense(date!(2026 - 10 - 05), 1000, "exp_comm", "w02");
    zero.business_pct = Some(0);
    let e = entry(&compose_simple(&zero, &chart, &floor()).unwrap(), "Z");
    assert_eq!(reconstruct_simple(&e, &chart), None);
    // 仕訳の形と期首残高は None。
    let j = compose_journal(
        &journal(vec![
            jl("exp_comm", Side::Debit, 1),
            jl("w01", Side::Credit, 1),
        ]),
        &chart,
        &floor(),
    )
    .unwrap();
    assert_eq!(reconstruct_simple(&entry(&j, "J"), &chart), None);
    let o = compose_opening(date!(2026 - 01 - 01), &[(code("w01"), Yen::new(1))], &chart)
        .unwrap()
        .unwrap();
    assert_eq!(reconstruct_simple(&entry(&o, "O"), &chart), None);
    // 付随情報を書き換えた仕訳（割合が明細と合わない）は None。
    let c = compose_simple(&inputs[0], &chart, &floor()).unwrap();
    let forged = entry(&c, "F").with_meta("business_pct", "60").unwrap();
    assert_eq!(reconstruct_simple(&forged, &chart), None);
    // 呼び出し側が足す付随情報（取り込み元など）は、組み立て直しの邪魔をしない。
    let tagged = entry(&c, "T")
        .with_meta("draft", "31")
        .unwrap()
        .with_meta("invoice_no", "T1234567890123")
        .unwrap();
    assert_eq!(
        reconstruct_simple(&tagged, &chart).as_ref(),
        Some(&inputs[0])
    );
}
