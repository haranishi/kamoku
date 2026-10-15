// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 借用する現在状態を、索引を使わない全履歴の参照計算と比べる。

mod common;
use common::{entry, id};
use kamoku_ledger::{EntryState, EventKind, Ledger, LedgerError};
use proptest::prelude::*;

const EMPTY: Ledger = Ledger::new();

fn reference(ledger: &Ledger) -> Vec<EntryState<'_>> {
    ledger
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Posted)
        .map(|posted| {
            let history: Vec<_> = ledger
                .events()
                .iter()
                .filter(|event| event.entry_id == posted.entry_id)
                .collect();
            EntryState {
                latest: history.last().unwrap(),
                entry: history
                    .iter()
                    .rev()
                    .find_map(|event| event.entry.as_ref())
                    .unwrap(),
            }
        })
        .collect()
}

fn assert_reference(ledger: &Ledger) {
    let expected = reference(ledger);
    assert_eq!(ledger.entry_states().collect::<Vec<_>>(), expected);
    for state in &expected {
        assert_eq!(ledger.entry_state(&state.latest.entry_id), Some(*state));
    }
    assert_eq!(
        ledger.current(),
        expected
            .iter()
            .filter(|state| state.latest.kind != EventKind::Voided)
            .map(|state| state.entry)
            .collect::<Vec<_>>()
    );
    ledger.verify().unwrap();
}

fn interleaved() -> Ledger {
    let mut ledger = Ledger::new();
    ledger.post(entry("A", 10), 1).unwrap();
    ledger.post(entry("B", 20), 2).unwrap();
    ledger.revise(entry("A", 11), "Aの訂正", 3).unwrap();
    ledger.post(entry("C", 30), 4).unwrap();
    ledger.revise(entry("B", 21), "Bの訂正", 5).unwrap();
    ledger.void(&id("A"), "Aの取消", 6).unwrap();
    ledger.post(entry("D", 40), 7).unwrap();
    ledger.revise(entry("C", 31), "Cの訂正", 8).unwrap();
    ledger
}

#[test]
fn interleaved_states_preserve_posting_order_and_last_valid_entry() {
    let ledger = interleaved();
    assert_reference(&ledger);
    let states: Vec<_> = ledger.entry_states().collect();
    assert_eq!(
        states
            .iter()
            .map(|state| state.entry.id().as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "C", "D"]
    );
    assert_eq!(states[0].latest.kind, EventKind::Voided);
    assert_eq!(states[0].latest.revision, 3);
    assert_eq!(states[0].entry.total().get(), 11);
    assert_eq!(states[1].latest.revision, 2);
    assert_eq!(ledger.entry_state(&id("missing")), None);
}

#[test]
fn restored_and_cloned_states_support_independent_later_changes() {
    let original = interleaved();
    let mut restored = Ledger::from_events(original.events().to_vec()).unwrap();
    let mut cloned = original.clone();
    assert_eq!(
        restored.entry_states().collect::<Vec<_>>(),
        reference(&original)
    );
    assert_eq!(
        cloned.entry_states().collect::<Vec<_>>(),
        reference(&original)
    );
    restored.revise(entry("B", 22), "復元後", 9).unwrap();
    restored.post(entry("E", 50), 10).unwrap();
    cloned.void(&id("C"), "複製後", 9).unwrap();
    cloned.post(entry("F", 60), 10).unwrap();
    for ledger in [&original, &restored, &cloned] {
        assert_reference(ledger);
    }
    assert_eq!(original.entry_states().count(), 4);
    assert_eq!(restored.entry_states().count(), 5);
    assert_eq!(cloned.entry_states().count(), 5);
    assert_eq!(EMPTY.entry_states().count(), 0);
    let mut empty = Ledger::from_events(vec![]).unwrap();
    assert_eq!(empty.entry_states().count(), 0);
    empty.post(entry("first", 1), 1).unwrap();
    assert_reference(&empty);
}

#[test]
fn failed_changes_leave_the_ordered_states_unchanged() {
    let mut ledger = interleaved();
    let before = ledger.events().to_vec();
    assert!(matches!(
        ledger.post(entry("B", 99), 9),
        Err(LedgerError::DuplicateId(_))
    ));
    assert!(matches!(
        ledger.revise(entry("missing", 99), "訂正", 9),
        Err(LedgerError::UnknownId(_))
    ));
    assert!(matches!(
        ledger.revise(entry("A", 99), "訂正", 9),
        Err(LedgerError::AlreadyVoided(_))
    ));
    assert!(matches!(
        ledger.void(&id("A"), "取消", 9),
        Err(LedgerError::AlreadyVoided(_))
    ));
    assert_eq!(
        ledger.revise(entry("B", 99), "  ", 9),
        Err(LedgerError::EmptyReason)
    );
    assert_eq!(
        ledger.void(&id("C"), "\n", 9),
        Err(LedgerError::EmptyReason)
    );
    assert_eq!(ledger.events(), before);
    assert_reference(&ledger);
    ledger.post(entry("E", 50), 10).unwrap();
    assert_eq!(ledger.entry_states().last().unwrap().entry.id(), &id("E"));
    assert_reference(&ledger);
}

proptest! {
    #[test]
    fn mixed_successful_and_failed_changes_match_full_history(
        changes in prop::collection::vec((0u8..3, 0u8..16, 1i64..10_000), 0..80)
    ) {
        let mut ledger = Ledger::new();
        for (operation, key, amount) in changes {
            let name = format!("E{key}");
            let _ = match operation {
                0 => ledger.post(entry(&name, amount), 1),
                1 => ledger.revise(entry(&name, amount), "訂正", 1),
                _ => ledger.void(&id(&name), "取消", 1),
            };
            assert_reference(&ledger);
        }
        assert_reference(&ledger.clone());
        assert_reference(&Ledger::from_events(ledger.events().to_vec()).unwrap());
    }
}
