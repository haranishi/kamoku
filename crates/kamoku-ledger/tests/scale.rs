// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 索引を使う大きな台帳の整合性と復元の試験。

mod common;
use common::{entry, id};
use kamoku_ledger::{Entry, Ledger, LedgerError};
use std::time::Instant;

#[test]
fn twenty_thousand_entries_with_revisions_and_voids() {
    let start = Instant::now();
    let mut ledger = Ledger::new();
    for n in 0..20_000 {
        ledger.post(entry(&format!("E{n}"), 100), n).unwrap();
    }
    // 登録とは逆順に訂正してもcurrentの順序は登録順を維持する。
    for n in (0..5_000).rev() {
        ledger
            .revise(entry(&format!("E{n}"), 200), "訂正", n)
            .unwrap();
    }
    for n in 0..2_000 {
        ledger.void(&id(&format!("E{n}")), "取消", n).unwrap();
    }
    ledger.verify().unwrap();
    assert_eq!(ledger.events().len(), 27_000);
    let current = ledger.current();
    assert_eq!(current.len(), 18_000);
    for (n, entry) in (2_000..20_000).zip(&current) {
        assert_eq!(entry.id(), &id(&format!("E{n}")));
        assert_eq!(entry.total().get(), if n < 5_000 { 200 } else { 100 });
    }
    let balance = ledger.trial_balance().unwrap();
    let debit: i128 = balance.values().map(|b| i128::from(b.debit.get())).sum();
    let credit: i128 = balance.values().map(|b| i128::from(b.credit.get())).sum();
    assert_eq!(debit, credit);
    assert_eq!(debit, 2_100_000);
    let events = ledger.events().to_vec();
    let mut restored = Ledger::from_events(events.clone()).unwrap();
    assert_eq!(restored.events(), events);
    assert_eq!(restored.head_hash(), ledger.head_hash());
    assert_eq!(restored.current(), current);
    for n in 0..20_000 {
        let key = id(&format!("E{n}"));
        let expected_len = if n < 2_000 {
            3
        } else if n < 5_000 {
            2
        } else {
            1
        };
        let history = restored.history(&key);
        assert_eq!(history.len(), expected_len);
        assert!(history.windows(2).all(|pair| pair[0].seq < pair[1].seq));
        assert!(history.iter().all(|event| event.entry_id == key));
        assert_eq!(history, ledger.history(&key));
    }
    // 復元した索引への追記、Clone、空からの復元も確認する。
    restored
        .revise(entry("E19999", 300), "復元後の訂正", 0)
        .unwrap();
    restored.void(&id("E19998"), "復元後の取消", 0).unwrap();
    restored.post(entry("NEW", 1), 0).unwrap();
    restored.verify().unwrap();
    let mut cloned = restored.clone();
    cloned.revise(entry("NEW", 2), "複製後の訂正", 0).unwrap();
    assert_eq!(restored.history(&id("NEW")).len(), 1);
    assert_eq!(cloned.history(&id("NEW")).len(), 2);
    assert_eq!(
        restored.post(entry("E0", 1), 0),
        Err(LedgerError::DuplicateId(id("E0")))
    );
    let mut empty = Ledger::from_events(vec![]).unwrap();
    assert_eq!(
        empty.history(&id("UNKNOWN")),
        Vec::<&kamoku_ledger::Event>::new()
    );
    assert_eq!(empty.current(), Vec::<&Entry>::new());
    empty.post(entry("E", 1), 0).unwrap();
    assert_eq!(empty.current().len(), 1);
    eprintln!("scale test (debug): {:?}", start.elapsed());
}

// const constructorの公開契約をコンパイル時にも確認する。
const EMPTY: Ledger = Ledger::new();
#[test]
fn const_constructor_is_empty() {
    assert_eq!(EMPTY.current(), Vec::<&Entry>::new());
}
