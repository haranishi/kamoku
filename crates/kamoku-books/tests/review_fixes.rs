// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! AC-08.1 / AC-08.2 / AC-08.5: 繰越と返品の固定値。
mod common;
use common::*;
use kamoku_books::*;
use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use time::macros::date;
fn journal(id: &str, date: time::Date, debit: &str, credit: &str, amount: i64) -> Entry {
    Entry::new(
        EntryId::new(id).unwrap(),
        date,
        "デモ",
        vec![
            Line::new(
                AccountCode::new(debit).unwrap(),
                Side::Debit,
                Yen::new(amount),
            ),
            Line::new(
                AccountCode::new(credit).unwrap(),
                Side::Credit,
                Yen::new(amount),
            ),
        ],
    )
    .unwrap()
}
#[test]
fn b1_owner_descendants_reset_and_carry_to_capital() {
    let mut accounts = accounts();
    for (code, parent) in [("x01", "owner_draw"), ("x02", "x01"), ("x03", "owner_inv")] {
        let mut a = accounts
            .iter()
            .find(|a| a.code.as_str() == parent)
            .unwrap()
            .clone();
        a.code = AccountCode::new(code).unwrap();
        a.parent = Some(AccountCode::new(parent).unwrap());
        a.name = "デモ子科目".into();
        a.system = false;
        accounts.push(a);
    }
    let chart = Chart::new(accounts).unwrap();
    let entries = vec![
        journal("a", date!(2025 - 01 - 01), "cash", "capital", 1000),
        journal("b", date!(2025 - 10 - 01), "x01", "cash", 100),
        journal("c", date!(2025 - 10 - 02), "x02", "cash", 30),
        journal("d", date!(2025 - 10 - 03), "cash", "x03", 50),
        journal("e", date!(2026 - 01 - 01), "x01", "cash", 20),
    ];
    let trial = trial_balance(
        &refs(&entries),
        &chart,
        date!(2026 - 01 - 01),
        date!(2026 - 01 - 01),
    )
    .unwrap();
    assert_eq!(
        trial.rows.iter().find(|r| r.code == "x01").unwrap().opening,
        Yen::ZERO
    );
    assert_eq!(
        trial
            .rows
            .iter()
            .find(|r| r.code == "capital")
            .unwrap()
            .opening,
        Yen::new(920)
    );
    assert!(
        !trial
            .rows
            .iter()
            .any(|r| r.code == "x02" || r.code == "x03")
    );
    assert!(trial.balanced);
    for (as_of, assets, capital) in [
        (date!(2025 - 12 - 31), 1050, 1000),
        (date!(2026 - 12 - 31), 920, 920),
        (date!(2027 - 12 - 31), 900, 900),
    ] {
        let bs = balance_sheet(&refs(&entries), &chart, as_of).unwrap();
        assert_eq!(bs.assets.total, Yen::new(assets));
        assert_eq!(bs.equity.total, Yen::new(capital));
        assert!(bs.balanced);
    }
}
#[test]
fn b2_purchase_and_full_refund_cancel_transitional_deduction() {
    for tax in [1, 91] {
        let purchase = journal("p", date!(2026 - 10 - 01), "exp_comm", "cash", tax * 11);
        let refund = journal("r", date!(2026 - 10 - 02), "cash", "exp_comm", tax * 11);
        let taxed = |e: Entry, index: usize| {
            let mut lines = e.lines().to_vec();
            lines[index].tax_code = Some("P10X".into());
            lines[index].tax_amount = Yen::new(tax);
            Entry::new(e.id().clone(), e.date(), "デモ", lines).unwrap()
        };
        let entries = vec![taxed(purchase, 0), taxed(refund, 1)];
        let summary = tax_summary(
            &refs(&entries),
            &chart(),
            date!(2026 - 10 - 01),
            date!(2026 - 10 - 31),
        )
        .unwrap();
        assert_eq!(summary.transitional[0].percent, 70);
        assert_eq!(summary.transitional[0].gross, Yen::ZERO);
        assert_eq!(summary.transitional[0].tax, Yen::ZERO);
        assert_eq!(summary.transitional[0].deductible, Yen::ZERO);
        assert_eq!(summary.purchase_tax, Yen::ZERO);
    }
}
