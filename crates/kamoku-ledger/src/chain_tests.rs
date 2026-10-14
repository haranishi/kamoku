// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    AccountCode, ChainError, Entry, EntryId, Event, EventKind, Ledger, Line, Side, Yen,
    encoding::event_hash,
};
use time::macros::date;
fn entry(name: &str) -> Entry {
    Entry::new(
        EntryId::new(name).unwrap(),
        date!(2026 - 01 - 01),
        "",
        vec![
            Line::new(AccountCode::new("A").unwrap(), Side::Debit, Yen::new(1)),
            Line::new(AccountCode::new("B").unwrap(), Side::Credit, Yen::new(1)),
        ],
    )
    .unwrap()
}
fn events() -> Vec<Event> {
    let mut ledger = Ledger::new();
    ledger.post(entry("E"), 0).unwrap();
    ledger.revise(entry("E"), "訂正", 1).unwrap();
    ledger.void(&EntryId::new("E").unwrap(), "取消", 2).unwrap();
    ledger.events().to_vec()
}
fn resign(events: &mut [Event]) {
    let mut prev = crate::Hash::GENESIS;
    for event in events {
        event.prev_hash = prev;
        event.hash = event_hash(event).unwrap();
        prev = event.hash;
    }
}
fn rule(expected_rule: &'static str, mut events: Vec<Event>, mutate: impl FnOnce(&mut Vec<Event>)) {
    mutate(&mut events);
    resign(&mut events);
    match Ledger::from_events(events).unwrap_err() {
        ChainError::RuleViolation { rule, .. } => assert_eq!(rule, expected_rule),
        other => panic!("expected {expected_rule}, got {other:?}"),
    }
}
#[test]
fn first_event_must_be_posted() {
    rule("初回は登録・版1", events(), |e| {
        // Revisedの条件を満たし、初回種別の規則だけに違反させる。
        e[0].kind = EventKind::Revised;
        e[0].reason = Some("訂正".to_owned());
    });
}
#[test]
fn first_revision_must_be_one() {
    rule("初回は登録・版1", events(), |e| {
        e.truncate(1);
        e[0].revision = 2;
    });
}
#[test]
fn duplicate_post_is_rejected() {
    rule("登録の重複", events(), |e| {
        e.truncate(2);
        e[1].kind = EventKind::Posted;
        e[1].reason = None;
    });
}
#[test]
fn revisions_must_be_contiguous() {
    rule("版の不連続", events(), |e| {
        e.truncate(2);
        e[1].revision = 3;
    });
}
#[test]
fn events_after_void_are_rejected() {
    rule("取消後のイベント", events(), |e| {
        let mut last = e[1].clone();
        last.seq = 4;
        last.revision = 4;
        e.push(last);
    });
}
#[test]
fn posted_entry_is_required() {
    rule("仕訳と識別子の不一致", events(), |e| {
        e[0].entry = None;
    });
}
#[test]
fn revised_entry_is_required() {
    rule("仕訳と識別子の不一致", events(), |e| {
        e[1].entry = None;
    });
}
#[test]
fn entry_id_must_match() {
    rule("仕訳と識別子の不一致", events(), |e| {
        e[0].entry = Some(entry("OTHER"));
    });
}
#[test]
fn void_must_not_have_entry() {
    rule("取消に仕訳が存在", events(), |e| {
        e[2].entry = Some(entry("E"));
    });
}
#[test]
fn post_must_not_have_reason() {
    rule("登録に理由が存在", events(), |e| {
        e[0].reason = Some("登録理由".to_owned());
    });
}
#[test]
fn revision_reason_is_required() {
    rule("訂正・取消の理由なし", events(), |e| {
        e[1].reason = None;
    });
}
#[test]
fn void_reason_must_not_be_blank() {
    rule("訂正・取消の理由なし", events(), |e| {
        e[2].reason = Some("　 \n".to_owned());
    });
}
#[test]
fn stored_reason_can_have_padding() {
    // 復元仕様はtrimした理由の非空を要求し、保存済みの前後空白は許容する。
    let mut padded = events();
    padded[1].reason = Some(" 訂正 ".to_owned());
    resign(&mut padded);
    assert!(Ledger::from_events(padded).is_ok());
}
#[test]
fn chain_error_order() {
    let mut seq = events();
    seq[0].seq = 2;
    assert_eq!(
        Ledger::from_events(seq).unwrap_err(),
        ChainError::WrongSequence {
            expected: 1,
            found: 2
        }
    );
    let mut link = events();
    link[1].prev_hash = crate::Hash::GENESIS;
    assert_eq!(
        Ledger::from_events(link).unwrap_err(),
        ChainError::BrokenLink { seq: 2 }
    );
    let mut hash = events();
    hash[0].recorded_at += 1;
    assert_eq!(
        Ledger::from_events(hash).unwrap_err(),
        ChainError::BadHash { seq: 1 }
    );
}
