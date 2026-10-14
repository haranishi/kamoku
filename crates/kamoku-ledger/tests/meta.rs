// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の付随情報（`meta`）の試験。AC-02.2（保存と表示の一致の土台）と、
//! docs/spec/data-model.md の「仕訳の付随情報」の決まり。

mod common;
use common::{entry, id};
use kamoku_ledger::{ChainError, Entry, EntryError, Ledger};
use proptest::prelude::*;
use std::collections::BTreeMap;

fn with_pairs(base: Entry, pairs: &[(String, String)]) -> Entry {
    pairs
        .iter()
        .fold(base, |e, (k, v)| e.with_meta(k, v).unwrap())
}

#[test]
fn meta_key_rules() {
    let thirty_two = "k".repeat(32);
    for key in [
        "a",
        "0",
        "kind",
        "business_pct",
        "x-y.z_0",
        thirty_two.as_str(),
    ] {
        assert!(entry("E", 1).with_meta(key, "v").is_ok(), "{key}");
    }
    let thirty_three = "k".repeat(33);
    for key in [
        "",
        thirty_three.as_str(),
        "Kind",
        "a b",
        "a/b",
        "ｋｉｎｄ",
        "取引先",
        "kind\n",
        "a+b",
    ] {
        assert_eq!(
            entry("E", 1).with_meta(key, "v"),
            Err(EntryError::BadMetaKey),
            "{key:?}"
        );
    }
}

#[test]
fn meta_value_and_count_limits() {
    assert!(entry("E", 1).with_meta("k", &"x".repeat(1000)).is_ok());
    assert!(entry("E", 1).with_meta("k", "").is_ok());
    assert_eq!(
        entry("E", 1).with_meta("k", &"x".repeat(1001)),
        Err(EntryError::MetaValueTooLong)
    );
    // 上限はバイト数。「あ」は3バイト。
    assert!(entry("E", 1).with_meta("k", &"あ".repeat(333)).is_ok());
    assert_eq!(
        entry("E", 1).with_meta("k", &"あ".repeat(334)),
        Err(EntryError::MetaValueTooLong)
    );
    let full = (0..16).fold(entry("E", 1), |e, n| {
        e.with_meta(&format!("k{n:02}"), "v").unwrap()
    });
    assert_eq!(full.meta().len(), 16);
    assert_eq!(
        full.clone().with_meta("k16", "v"),
        Err(EntryError::TooManyMeta)
    );
    // 既にある鍵の上書きは、組の数を増やさない。
    let replaced = full.with_meta("k00", "新しい値").unwrap();
    assert_eq!(replaced.meta().len(), 16);
    assert_eq!(replaced.meta()["k00"], "新しい値");
}

#[test]
fn meta_is_part_of_identity_and_hash() {
    let plain = entry("E", 100);
    assert!(plain.meta().is_empty());
    let tagged = plain.clone().with_meta("kind", "expense").unwrap();
    assert_ne!(plain, tagged);
    let mut a = Ledger::new();
    a.post(plain, 0).unwrap();
    let mut b = Ledger::new();
    b.post(tagged.clone(), 0).unwrap();
    assert_ne!(a.head_hash(), b.head_hash());
    // 訂正のイベントにも付き、復元した台帳で読める。
    b.revise(
        tagged.with_meta("partner", "サンプル商店").unwrap(),
        "取引先を足す",
        1,
    )
    .unwrap();
    let restored = Ledger::from_events(b.events().to_vec()).unwrap();
    let current = restored.current();
    assert_eq!(current[0].meta()["partner"], "サンプル商店");
    assert_eq!(current[0].meta()["kind"], "expense");
}

#[test]
fn tampered_meta_is_detected() {
    let mut ledger = Ledger::new();
    ledger
        .post(entry("E", 100).with_meta("partner", "サンプル").unwrap(), 0)
        .unwrap();
    let mut events = ledger.events().to_vec();
    let changed = entry("E", 100).with_meta("partner", "デモ").unwrap();
    events[0].entry = Some(changed);
    assert_eq!(
        Ledger::from_events(events).unwrap_err(),
        ChainError::BadHash { seq: 1 }
    );
}

fn pairs_strategy() -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::btree_map("[a-z0-9_.-]{1,32}", ".{0,40}", 1..=16)
        .prop_map(|map: BTreeMap<String, String>| map.into_iter().collect())
}

type Pairs = Vec<(String, String)>;
fn shuffled_pairs() -> impl Strategy<Value = (Pairs, Pairs)> {
    pairs_strategy().prop_flat_map(|pairs| (Just(pairs.clone()), Just(pairs).prop_shuffle()))
}

proptest! {
    // 鍵の検査は、許す文字と長さだけで決まる。正しい鍵と、惜しい鍵と、任意の文字列を混ぜる。
    #[test]
    fn key_validation_matches_rule(key in prop_oneof!["[a-z0-9_.-]{0,34}", "[a-zA-Z0-9_.+/ -]{1,33}", ".{0,40}"]) {
        let valid = (1..=32).contains(&key.len())
            && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.'));
        prop_assert_eq!(entry("E", 1).with_meta(&key, "v").is_ok(), valid);
    }
    // 挿入の順は、仕訳とハッシュに影響しない。
    #[test]
    fn insertion_order_does_not_matter((pairs, shuffled) in shuffled_pairs()) {
        let a = with_pairs(entry("E", 1), &pairs);
        let b = with_pairs(entry("E", 1), &shuffled);
        prop_assert_eq!(&a, &b);
        let mut la = Ledger::new();
        la.post(a, 0).unwrap();
        let mut lb = Ledger::new();
        lb.post(b, 0).unwrap();
        prop_assert_eq!(la.head_hash(), lb.head_hash());
    }
    // 付随情報を1つでも変えると（足す・値を変える・消す）、ハッシュが変わる。
    #[test]
    fn any_meta_change_changes_hash(pairs in pairs_strategy(), pick in any::<usize>(), extra in "[a-z]{1,8}") {
        let base = with_pairs(entry("E", 1), &pairs);
        let hash_of = |e: Entry| { let mut l = Ledger::new(); l.post(e, 0).unwrap(); l.head_hash() };
        let original = hash_of(base.clone());
        let index = pick % pairs.len();
        let mut changed_value = pairs.clone();
        changed_value[index].1.push('改');
        prop_assert_ne!(hash_of(with_pairs(entry("E", 1), &changed_value)), original);
        let mut removed = pairs.clone();
        removed.remove(index);
        prop_assert_ne!(hash_of(with_pairs(entry("E", 1), &removed)), original);
        if pairs.len() < 16 {
            let key = format!("zz{extra}");
            prop_assume!(!pairs.iter().any(|(k, _)| *k == key));
            prop_assert_ne!(hash_of(base.with_meta(&key, "").unwrap()), original);
        }
    }
    // 付随情報つきの登録・訂正・取消の連鎖を、from_events が検証して復元できる。
    #[test]
    fn chain_with_meta_restores(ops in prop::collection::vec((0u8..6, any::<u8>(), pairs_strategy()), 1..40)) {
        let mut ledger = Ledger::new();
        for (slot, action, pairs) in ops {
            let name = format!("E{slot}");
            let body = with_pairs(entry(&name, i64::from(action) + 1), &pairs);
            match ledger.history(&id(&name)).last().map(|e| e.kind) {
                None => { ledger.post(body, 0).unwrap(); }
                Some(kamoku_ledger::EventKind::Voided) => {}
                Some(_) if action % 3 == 0 => { ledger.void(&id(&name), "取消", 0).unwrap(); }
                Some(_) => { ledger.revise(body, "訂正", 0).unwrap(); }
            }
        }
        let restored = Ledger::from_events(ledger.events().to_vec()).unwrap();
        prop_assert_eq!(restored.head_hash(), ledger.head_hash());
        prop_assert_eq!(restored.current(), ledger.current());
    }
}
