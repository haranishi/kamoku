// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phase 0 の受け入れ試験。

mod common;
use common::{entry, id};
use kamoku_ledger::{AccountCode, ChainError, Entry, EventKind, Hash, Ledger, Line, Side, Yen};
use proptest::prelude::*;
use std::fmt::Write;
use time::macros::date;

fn operated(ops: &[(u8, u8, i64)]) -> Ledger {
    let mut ledger = Ledger::new();
    for &(slot, action, amount) in ops {
        let name = format!("E{slot}");
        let key = id(&name);
        let history = ledger.history(&key);
        match history.last().map(|e| e.kind) {
            None => {
                ledger.post(entry(&name, amount), amount).unwrap();
            }
            Some(EventKind::Voided) => {}
            Some(_) if action % 2 == 0 => {
                ledger
                    .revise(entry(&name, amount), " 訂正 ", amount)
                    .unwrap();
            }
            Some(_) => {
                ledger.void(&key, " 取消 ", amount).unwrap();
            }
        }
    }
    ledger
}
fn flipped(hash: Hash) -> Hash {
    let mut bytes = *hash.as_bytes();
    bytes[0] ^= 1;
    let mut encoded = String::new();
    for b in bytes {
        write!(&mut encoded, "{b:02x}").unwrap();
    }
    Hash::from_hex(&encoded).unwrap()
}
// IDと検証可能性を維持して、本文の指定要素だけを変える。
fn changed_entry(original: &Entry, field: u8) -> Entry {
    let mut lines = original.lines().to_vec();
    let mut day = original.date();
    let mut description = original.description().to_owned();
    match field {
        0 => {
            // 貸借一致を保つため両側の金額を同額だけ変える。
            for line in &mut lines {
                line.amount = line.amount.checked_add(Yen::new(1)).unwrap();
            }
        }
        1 => description.push('改'),
        2 => day = day.next_day().unwrap(),
        3 => lines[0].account = AccountCode::new("CHANGED").unwrap(),
        4 => lines[0].tax_code = Some("CHANGED".to_owned()),
        5 => lines[0].tax_amount = Yen::new(1),
        6 => lines[0].memo.push('改'),
        _ => unreachable!(),
    }
    Entry::new(original.id().clone(), day, &description, lines).unwrap()
}

proptest! {
    #[test]
    fn valid_operations(ops in prop::collection::vec((0u8..16, any::<u8>(), 1i64..1_000_000), 0..100)) {
        let ledger = operated(&ops);
        prop_assert!(ledger.verify().is_ok());
        let restored = Ledger::from_events(ledger.events().to_vec()).unwrap();
        prop_assert_eq!(restored.head_hash(), ledger.head_hash());
        let balance = restored.trial_balance().unwrap();
        let debit: i128 = balance.values().map(|b| i128::from(b.debit.get())).sum();
        let credit: i128 = balance.values().map(|b| i128::from(b.credit.get())).sum();
        prop_assert_eq!(debit, credit);
    }
    #[test]
    fn single_field_tampering(ops in prop::collection::vec((0u8..16, any::<u8>(), 1i64..1_000_000), 1..80), selection in any::<usize>(), field in 0u8..9) {
        let ledger = operated(&ops);
        let mut events = ledger.events().to_vec();
        let index = selection % events.len();
        let original = events[index].clone();
        let event = &mut events[index];
        match field {
            0 => event.seq = event.seq.wrapping_add(1),
            1 => event.kind = if event.kind == EventKind::Posted { EventKind::Revised } else { EventKind::Posted },
            2 => event.entry_id = id("CHANGED"),
            3 => event.revision = event.revision.wrapping_add(1),
            4 => event.recorded_at = event.recorded_at.wrapping_add(1),
            5 => event.reason = Some("別の理由".to_owned()),
            6 => {
                if let Some(original_entry) = &event.entry {
                    event.entry = Some(changed_entry(original_entry, 1));
                }
            },
            7 => event.prev_hash = flipped(event.prev_hash),
            _ => event.hash = flipped(event.hash),
        }
        prop_assume!(*event != original);
        prop_assert!(Ledger::from_events(events).is_err());
    }
    #[test]
    fn entry_body_tampering_is_bad_hash(amount in 1i64..1_000_000, revised in any::<bool>()) {
        let base = entry("E", amount);
        let lines = base.lines().iter().cloned()
            .map(|line| line.with_tax("TAX10", Yen::ZERO)).collect();
        let rich = Entry::new(base.id().clone(), base.date(), base.description(), lines).unwrap();
        let mut ledger = Ledger::new();
        ledger.post(rich.clone(), 0).unwrap();
        if revised { ledger.revise(rich, "訂正", 1).unwrap(); }
        let index = ledger.events().len() - 1;
        let original = ledger.events()[index].entry.as_ref().unwrap();
        // 毎回7要素を全て検査し、乱数の偏りによる未検査を避ける。
        for field in 0..7 {
            let replacement = changed_entry(original, field);
            prop_assert_eq!(replacement.id(), original.id());
            prop_assert_ne!(&replacement, original);
            let mut events = ledger.events().to_vec();
            let seq = events[index].seq;
            events[index].entry = Some(replacement);
            prop_assert_eq!(Ledger::from_events(events).unwrap_err(), ChainError::BadHash { seq });
        }
    }
    #[test]
    fn deletion_and_reordering(count in 3usize..30, pick in any::<usize>()) {
        let mut ledger = Ledger::new();
        for n in 0..count { ledger.post(entry(&format!("E{n}"), 1), 0).unwrap(); }
        let index = 1 + pick % (count - 2);
        let mut deleted = ledger.events().to_vec(); deleted.remove(index);
        prop_assert!(Ledger::from_events(deleted).is_err());
        let mut swapped = ledger.events().to_vec(); swapped.swap(index, index - 1);
        prop_assert!(Ledger::from_events(swapped).is_err());
    }
    #[test]
    fn entry_balance(a in 1i64..1_000_000, b in 1i64..1_000_000) {
        let lines = vec![common::line(Side::Debit, a), common::line(Side::Credit, b)];
        let result = Entry::new(id("E"), date!(2026-01-01), "", lines);
        if a != b { prop_assert!(result.is_err()); }
        if let Ok(entry) = result {
            let debit: i128 = entry.lines().iter().filter(|l| l.side == Side::Debit).map(|l| i128::from(l.amount.get())).sum();
            let credit: i128 = entry.lines().iter().filter(|l| l.side == Side::Credit).map(|l| i128::from(l.amount.get())).sum();
            prop_assert_eq!(debit, credit);
        }
    }
    #[test]
    fn arbitrary_line_sets(values in prop::collection::vec((any::<bool>(), any::<i64>()), 0..205)) {
        let lines: Vec<Line> = values.iter().map(|&(debit, value)| common::line(if debit { Side::Debit } else { Side::Credit }, value)).collect();
        let debit: i128 = lines.iter().filter(|l| l.side == Side::Debit).map(|l| i128::from(l.amount.get())).sum();
        let credit: i128 = lines.iter().filter(|l| l.side == Side::Credit).map(|l| i128::from(l.amount.get())).sum();
        let result = Entry::new(id("E"), date!(2026-01-01), "", lines);
        if debit != credit { prop_assert!(result.is_err()); }
        if let Ok(e) = result { prop_assert_eq!(debit, credit); prop_assert_eq!(i128::from(e.total().get()), debit); }
    }
    #[test]
    fn extreme_amounts(extra in 1i64..=i64::MAX) {
        let overflow = vec![common::line(Side::Debit, i64::MAX), common::line(Side::Debit, extra), common::line(Side::Credit, i64::MAX)];
        prop_assert!(Entry::new(id("E"), date!(2026-01-01), "", overflow).is_err());
        let negative = vec![common::line(Side::Debit, i64::MIN), common::line(Side::Credit, extra)];
        prop_assert!(Entry::new(id("E"), date!(2026-01-01), "", negative).is_err());
    }
}
#[test]
fn trailing_deletion_is_undetectable() {
    let mut ledger = Ledger::new();
    ledger.post(entry("E", 1), 0).unwrap();
    ledger.void(&id("E"), "取消", 1).unwrap();
    let head = ledger.head_hash();
    let mut events = ledger.events().to_vec();
    events.pop();
    // 末尾の削除は連鎖だけでは検知できない。外部に控えたheadとの比較が必要。
    let truncated = Ledger::from_events(events).unwrap();
    assert_ne!(truncated.head_hash(), head);
    assert_eq!(truncated.current().len(), 1);
    assert!(Ledger::from_events(vec![]).is_ok());
}
