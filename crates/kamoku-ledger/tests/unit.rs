// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phase 0 の受け入れ試験。

mod common;
use common::{entry, id, line};
use kamoku_ledger::{
    AccountCode, Entry, EntryError, Hash, HashError, IdError, Ledger, LedgerError, Side, Yen,
};
use time::macros::date;

fn build(lines: Vec<kamoku_ledger::Line>) -> Result<Entry, EntryError> {
    Entry::new(id("E"), date!(2026 - 01 - 05), "", lines)
}
fn pair() -> Vec<kamoku_ledger::Line> {
    vec![line(Side::Debit, 100), line(Side::Credit, 100)]
}
#[test]
fn identifiers_and_money() {
    for (text, error) in [
        ("", IdError::Empty),
        (&"x".repeat(65), IdError::TooLong),
        ("売上", IdError::BadChar),
        ("a b", IdError::BadChar),
    ] {
        assert_eq!(kamoku_ledger::EntryId::new(text), Err(error));
    }
    assert!(kamoku_ledger::EntryId::new(&"x".repeat(64)).is_ok());
    assert_eq!(AccountCode::new(""), Err(IdError::Empty));
    assert_eq!(AccountCode::new(&"x".repeat(17)), Err(IdError::TooLong));
    assert_eq!(AccountCode::new("a/"), Err(IdError::BadChar));
    assert_eq!(AccountCode::new("A-_09").unwrap().to_string(), "A-_09");
    assert!(AccountCode::new(&"x".repeat(16)).is_ok());
    assert_eq!(id("E-1").to_string(), "E-1");
    assert_eq!(Yen::new(1234).to_string(), "1234円");
    assert_eq!(Yen::default(), Yen::ZERO);
    assert_eq!(Yen::new(i64::MAX).checked_add(Yen::new(1)), None);
    assert_eq!(Yen::new(i64::MIN).checked_sub(Yen::new(1)), None);
    assert_eq!(Yen::new(2).checked_sub(Yen::new(1)), Some(Yen::new(1)));
}
#[test]
fn entry_errors_and_boundaries() {
    assert_eq!(build(vec![]), Err(EntryError::TooFewLines));
    assert_eq!(
        build(vec![line(Side::Debit, 1); 201]),
        Err(EntryError::TooManyLines)
    );
    for amount in [0, -1, i64::MIN] {
        assert_eq!(
            build(vec![line(Side::Debit, amount), line(Side::Credit, 1)]),
            Err(EntryError::NonPositiveAmount { index: 0 })
        );
    }
    for tax in [-1, 101] {
        assert_eq!(
            build(vec![
                line(Side::Debit, 100).with_tax("TAX10", Yen::new(tax)),
                line(Side::Credit, 100)
            ]),
            Err(EntryError::BadTax { index: 0 })
        );
    }
    let mut without_code = line(Side::Debit, 100);
    without_code.tax_amount = Yen::new(1);
    assert_eq!(
        build(vec![without_code, line(Side::Credit, 100)]),
        Err(EntryError::BadTax { index: 0 })
    );
    for code in ["", "bad code", "全角", &"a".repeat(33)] {
        assert_eq!(
            build(vec![
                line(Side::Debit, 100).with_tax(code, Yen::ZERO),
                line(Side::Credit, 100)
            ]),
            Err(EntryError::BadTaxCode { index: 0 })
        );
    }
    assert_eq!(
        Entry::new(id("E"), date!(2026 - 01 - 05), &"あ".repeat(334), pair()),
        Err(EntryError::TextTooLong)
    );
    assert_eq!(
        build(vec![
            line(Side::Debit, 100).with_memo(&"x".repeat(1001)),
            line(Side::Credit, 100)
        ]),
        Err(EntryError::TextTooLong)
    );
    for side in [Side::Debit, Side::Credit] {
        assert_eq!(
            build(vec![line(side, 1), line(side, 1)]),
            Err(EntryError::MissingSide)
        );
    }
    for side in [Side::Debit, Side::Credit] {
        assert_eq!(
            build(vec![
                line(side, i64::MAX),
                line(side, 1),
                line(
                    if side == Side::Debit {
                        Side::Credit
                    } else {
                        Side::Debit
                    },
                    1
                )
            ]),
            Err(EntryError::Overflow)
        );
    }
    assert_eq!(
        build(vec![line(Side::Debit, 100), line(Side::Credit, 99)]),
        Err(EntryError::Unbalanced {
            debit: Yen::new(100),
            credit: Yen::new(99)
        })
    );
    let lines = vec![line(Side::Debit, 1); 100]
        .into_iter()
        .chain(vec![line(Side::Credit, 1); 100])
        .collect();
    assert!(build(lines).is_ok());
    assert!(
        Entry::new(
            id("E"),
            date!(2026 - 01 - 05),
            &"x".repeat(1000),
            vec![
                line(Side::Debit, 100)
                    .with_tax(&"a".repeat(32), Yen::new(100))
                    .with_memo(&"x".repeat(1000)),
                line(Side::Credit, 100)
            ]
        )
        .is_ok()
    );
    assert_eq!(entry("E", i64::MAX).total(), Yen::new(i64::MAX));
}
#[test]
fn ledger_errors_and_queries() {
    let mut ledger = Ledger::default();
    assert_eq!(ledger.head_hash(), Hash::GENESIS);
    assert_eq!(ledger.current(), [] as [&Entry; 0]);
    assert_eq!(ledger.history(&id("E")), [] as [&kamoku_ledger::Event; 0]);
    assert!(ledger.trial_balance().unwrap().is_empty());
    assert_eq!(
        ledger.revise(entry("E", 1), "理由", 0),
        Err(LedgerError::UnknownId(id("E")))
    );
    assert_eq!(
        ledger.void(&id("E"), "理由", 0),
        Err(LedgerError::UnknownId(id("E")))
    );
    ledger.post(entry("B", 100), 0).unwrap();
    ledger.post(entry("A", 50), 0).unwrap();
    assert_eq!(
        ledger.post(entry("B", 100), 0),
        Err(LedgerError::DuplicateId(id("B")))
    );
    for reason in ["", " \t\n", "　"] {
        assert_eq!(
            ledger.revise(entry("B", 20), reason, 0),
            Err(LedgerError::EmptyReason)
        );
        assert_eq!(
            ledger.void(&id("B"), reason, 0),
            Err(LedgerError::EmptyReason)
        );
    }
    assert_eq!(
        ledger
            .revise(entry("B", 120), "  訂正　", -10)
            .unwrap()
            .reason
            .as_deref(),
        Some("訂正")
    );
    assert_eq!(
        ledger
            .current()
            .iter()
            .map(|e| e.id().as_str())
            .collect::<Vec<_>>(),
        vec!["B", "A"]
    );
    assert_eq!(ledger.current().first().unwrap().total(), Yen::new(120));
    assert_eq!(
        ledger
            .history(&id("B"))
            .iter()
            .map(|e| e.seq)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let balance = ledger.trial_balance().unwrap();
    let account = balance.get(&AccountCode::new("101").unwrap()).unwrap();
    assert_eq!(account.debit, Yen::new(170));
    assert_eq!(account.credit, Yen::new(170));
    assert_eq!(
        ledger
            .void(&id("A"), " 取消 ", 0)
            .unwrap()
            .reason
            .as_deref(),
        Some("取消")
    );
    assert_eq!(ledger.current(), vec![&entry("B", 120)]);
    assert_eq!(
        ledger.revise(entry("A", 1), "理由", 0),
        Err(LedgerError::AlreadyVoided(id("A")))
    );
    assert_eq!(
        ledger.void(&id("A"), "理由", 0),
        Err(LedgerError::AlreadyVoided(id("A")))
    );
    assert_eq!(
        ledger.post(entry("A", 1), 0),
        Err(LedgerError::DuplicateId(id("A")))
    );
    assert_eq!(ledger.events().len(), 4);
    ledger.verify().unwrap();
}
#[test]
fn trial_balance_overflow() {
    let mut ledger = Ledger::new();
    ledger.post(entry("A", i64::MAX), 0).unwrap();
    ledger.post(entry("B", 1), 0).unwrap();
    assert_eq!(ledger.trial_balance(), Err(LedgerError::Overflow));
}
#[test]
fn hashes() {
    assert_eq!(Hash::from_hex("0"), Err(HashError::InvalidLength));
    assert_eq!(Hash::from_hex(&"g".repeat(64)), Err(HashError::InvalidHex));
    assert_eq!(
        Hash::from_hex(&"Ａ".repeat(21)),
        Err(HashError::InvalidLength)
    );
    let hash = Hash::from_hex(&"aB".repeat(32)).unwrap();
    assert_eq!(hash.to_hex(), "ab".repeat(32));
    assert_eq!(format!("{hash:?}"), hash.to_string());
    assert_eq!(Hash::GENESIS.as_bytes(), &[0; 32]);
}

#[test]
fn balances_across_accounts_and_revisions() {
    fn make(name: &str, amount: i64) -> Entry {
        Entry::new(
            id(name),
            date!(2026 - 01 - 01),
            "",
            vec![
                kamoku_ledger::Line::new(
                    AccountCode::new("101").unwrap(),
                    Side::Debit,
                    Yen::new(amount),
                ),
                kamoku_ledger::Line::new(
                    AccountCode::new("401").unwrap(),
                    Side::Credit,
                    Yen::new(amount),
                ),
            ],
        )
        .unwrap()
    }
    let mut ledger = Ledger::new();
    ledger.post(make("A", 100), 0).unwrap();
    ledger.post(make("B", 50), 1).unwrap();
    ledger.revise(make("A", 120), "訂正", 2).unwrap();
    ledger.void(&id("B"), "取消", 3).unwrap();
    let balance = ledger.trial_balance().unwrap();
    assert_eq!(balance.len(), 2);
    assert_eq!(
        balance[&AccountCode::new("101").unwrap()],
        kamoku_ledger::Balance {
            debit: Yen::new(120),
            credit: Yen::ZERO
        }
    );
    assert_eq!(
        balance[&AccountCode::new("401").unwrap()],
        kamoku_ledger::Balance {
            debit: Yen::ZERO,
            credit: Yen::new(120)
        }
    );
    // 貸方側の集計が先に桁あふれする構成も確認する。
    let mut credit_overflow = Ledger::new();
    credit_overflow.post(make("A", i64::MAX), 0).unwrap();
    let second = Entry::new(
        id("B"),
        date!(2026 - 01 - 01),
        "",
        vec![
            kamoku_ledger::Line::new(AccountCode::new("102").unwrap(), Side::Debit, Yen::new(1)),
            kamoku_ledger::Line::new(AccountCode::new("401").unwrap(), Side::Credit, Yen::new(1)),
        ],
    )
    .unwrap();
    credit_overflow.post(second, 0).unwrap();
    assert_eq!(credit_overflow.trial_balance(), Err(LedgerError::Overflow));
}
