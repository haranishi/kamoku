// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 期首残高の仕訳（`meta.kind` が `opening`）が、期間の初日と同じ日付なら、試算表と元帳の
//! 「期首」の列に入る（開業の年の試算表で、期首が0に見えないように）。AC-08.1 AC-08.4。

mod common;
use common::{chart, entry, floor, income, numbered, refs};
use kamoku_books::{TrialBalance, account_ledger, compose_opening, compose_simple, trial_balance};
use kamoku_ledger::{AccountCode, Entry, Yen};
use time::Date;
use time::macros::date;

/// 期首残高（デモ銀行に100万円。元入金で合わせる）と、3月10日の売上5万円。
fn book(opening_date: Date) -> Vec<Entry> {
    let c = chart();
    let opening = compose_opening(
        opening_date,
        &[(AccountCode::new("w03").unwrap(), Yen::new(1_000_000))],
        &c,
    )
    .unwrap()
    .unwrap();
    let sale = compose_simple(
        &income(date!(2026 - 03 - 10), 50_000, "sales", "w03"),
        &c,
        &floor(),
    )
    .unwrap();
    vec![entry(&opening, "O1"), entry(&sale, "S1")]
}

/// 科目の（期首, 借方の動き, 貸方の動き, 期末）。
fn numbers(tb: &TrialBalance, code: &str) -> (i64, i64, i64, i64) {
    let r = tb.rows.iter().find(|r| r.code == code).unwrap();
    (
        r.opening.get(),
        r.debit.get(),
        r.credit.get(),
        r.closing.get(),
    )
}

// 期首残高の仕訳が1月1日で、期間も1月1日から。期首の列に出て、動きには入らない。
#[test]
fn opening_on_the_first_day_is_the_opening_column() {
    let entries = book(date!(2026 - 01 - 01));
    let tb = trial_balance(
        &refs(&entries),
        &chart(),
        date!(2026 - 01 - 01),
        date!(2026 - 12 - 31),
    )
    .unwrap();
    assert_eq!(numbers(&tb, "w03"), (1_000_000, 50_000, 0, 1_050_000));
    assert_eq!(numbers(&tb, "capital"), (1_000_000, 0, 0, 1_000_000));
    assert_eq!(numbers(&tb, "sales"), (0, 0, 50_000, 50_000));
    assert_eq!(
        (tb.total_debit.get(), tb.total_credit.get(), tb.balanced),
        (50_000, 50_000, true)
    );
}

// 期間の初日が、期首残高の日付より後なら、これまでどおり、日付で期首に入る。
#[test]
fn opening_before_the_period_is_the_opening_column() {
    let entries = book(date!(2026 - 01 - 01));
    let tb = trial_balance(
        &refs(&entries),
        &chart(),
        date!(2026 - 02 - 01),
        date!(2026 - 12 - 31),
    )
    .unwrap();
    assert_eq!(numbers(&tb, "w03"), (1_000_000, 50_000, 0, 1_050_000));
    assert_eq!(numbers(&tb, "capital"), (1_000_000, 0, 0, 1_000_000));
}

// 期首残高の日付が期間の途中（開業が4月1日など）なら、その日の動きとして扱う（期首に入れない）。
#[test]
fn opening_inside_the_period_is_a_movement() {
    let entries = book(date!(2026 - 04 - 01));
    let tb = trial_balance(
        &refs(&entries),
        &chart(),
        date!(2026 - 01 - 01),
        date!(2026 - 12 - 31),
    )
    .unwrap();
    assert_eq!(numbers(&tb, "w03"), (0, 1_050_000, 0, 1_050_000));
    assert_eq!(numbers(&tb, "capital"), (0, 0, 1_000_000, 1_000_000));
}

// 総勘定元帳の期首も、試算表と同じ値。期首残高の仕訳は、行にならない。
#[test]
fn ledger_opening_matches_the_trial_balance() {
    let entries = book(date!(2026 - 01 - 01));
    let all = numbered(&entries);
    let ledger = account_ledger(
        &all,
        &chart(),
        "w03",
        date!(2026 - 01 - 01),
        date!(2026 - 12 - 31),
    )
    .unwrap();
    assert_eq!(ledger.opening, Yen::new(1_000_000));
    assert_eq!(ledger.rows.len(), 1);
    assert_eq!(ledger.rows[0].balance, Yen::new(1_050_000));
    assert_eq!(ledger.closing, Yen::new(1_050_000));
    let capital = account_ledger(
        &all,
        &chart(),
        "capital",
        date!(2026 - 01 - 01),
        date!(2026 - 12 - 31),
    )
    .unwrap();
    assert_eq!(capital.opening, Yen::new(1_000_000));
    assert_eq!(capital.rows.len(), 0);
    assert_eq!(capital.closing, Yen::new(1_000_000));
}

// 期首残高の仕訳が期間の初日のときも、貸借対照表の数字は変わらない（累計だから）。
#[test]
fn balance_sheet_is_unaffected() {
    let entries = book(date!(2026 - 01 - 01));
    let bs = kamoku_books::balance_sheet(&refs(&entries), &chart(), date!(2026 - 12 - 31)).unwrap();
    assert_eq!(bs.assets.total, Yen::new(1_050_000));
    assert!(bs.balanced);
}
