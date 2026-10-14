// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 台帳の復元・改ざん・競合・巻き戻しの回帰試験。

mod common;
use common::*;
use kamoku_ledger::{EntryId, Hash};
use kamoku_store::*;
use proptest::prelude::*;
use rusqlite::Connection;
use std::collections::BTreeMap;

// AC-09.1: 訂正・取消も再起動で同じ列に復元される。
#[test]
fn roundtrip_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    s.post(entry("a", " 日本語😀\n'\"\\ "), 1, None).unwrap();
    s.revise(entry("a", "訂正"), 1, "理由", 2, None).unwrap();
    s.post(entry("b", "デモ"), 3, None).unwrap();
    s.void(&EntryId::new("b").unwrap(), 1, "取消", 4).unwrap();
    let events = s.ledger().events().to_vec();
    let hash = s.ledger().head_hash();
    drop(s);
    let (s, report) = Store::open(&path, OpenOptions::default()).unwrap();
    assert_eq!(s.ledger().events(), events);
    assert_eq!(s.ledger().head_hash(), hash);
    assert_eq!(report.events_loaded, 4);
    assert_eq!(report.entries, 2);
    assert_eq!(s.entry_no(&EntryId::new("a").unwrap()), Some(1));
    assert!(s.verify().is_ok());
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    // AC-09.1: NULを除くUnicodeを正規化せず保存する。日本語・改行・空白・引用符も固定値で追加。
    #[test]
    fn strings_roundtrip(chars in prop::collection::vec(any::<char>().prop_filter("NUL除外",|c|*c!='\0'),0..80), ops in prop::collection::vec(0u8..3,1..12), amount in 1i64..1_000_000, tax in 0u8..100){
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("data.sqlite");let mut s=opened(&path);
        let text=chars.into_iter().collect::<String>();
        let mut e=entry("prop",&text).with_meta("partner",&text).unwrap();
        let mut lines=e.lines().to_vec();for l in &mut lines {l.memo=text.clone();l.amount=kamoku_ledger::Yen::new(amount);if l.side==kamoku_ledger::Side::Debit{l.tax_code=Some("P10".into());l.tax_amount=kamoku_ledger::Yen::new(amount.min(i64::from(tax)));}}
        e=kamoku_ledger::Entry::new(e.id().clone(),e.date(),&text,lines).unwrap().with_meta("partner",&text).unwrap();
        s.post(e.clone(),1,None).unwrap();
        for (revision,op) in (1..).zip(ops) {if op==2{s.void(e.id(),revision,"取消",3).unwrap();break;}s.revise(e.clone(),revision,"訂正",2,None).unwrap();}
        let events=s.ledger().events().to_vec();let head=s.ledger().head_hash();drop(s);
        let s=Store::open(&path,OpenOptions::default()).unwrap().0;
        prop_assert_eq!(s.ledger().events(),events);prop_assert_eq!(s.ledger().head_hash(),head);
    }
}
// AC-09.1: 追記専用トリガーの全8通り。明細とmetaにも実際に行がある。
#[test]
fn append_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    s.post(
        entry("a", "デモ").with_meta("kind", "journal").unwrap(),
        1,
        None,
    )
    .unwrap();
    let conn = Connection::open(path).unwrap();
    let columns = BTreeMap::from([
        ("ledger_events", "recorded_at"),
        ("ledger_lines", "amount"),
        ("ledger_meta", "value"),
        ("change_log", "actor"),
    ]);
    for (table, column) in columns {
        assert!(
            conn.execute(&format!("UPDATE {table} SET {column}={column}"), [])
                .is_err(),
            "{table} UPDATE"
        );
        assert!(
            conn.execute(&format!("DELETE FROM {table}"), []).is_err(),
            "{table} DELETE"
        );
    }
}
// AC-09.1: 改ざんは検証済みの接頭部分を残してread_onlyで開く。
#[test]
fn tampering() {
    for sql in [
        "DROP TRIGGER ledger_lines_no_update; UPDATE ledger_lines SET amount=amount+1 WHERE seq=2",
        "DROP TRIGGER ledger_events_no_update; UPDATE ledger_events SET seq=5 WHERE seq=2",
        "DROP TRIGGER ledger_events_no_update; UPDATE ledger_events SET prev_hash=printf('%064d',0) WHERE seq=2",
        "DROP TRIGGER ledger_events_no_update; UPDATE ledger_events SET reason=NULL WHERE seq=2",
        "DROP TRIGGER ledger_events_no_update; UPDATE ledger_events SET date='2026-02-30' WHERE seq=2",
        "DROP TRIGGER ledger_lines_no_delete; DELETE FROM ledger_lines WHERE seq=2",
        "DROP TRIGGER ledger_lines_no_update; UPDATE ledger_lines SET account='?' WHERE seq=2",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.sqlite");
        let mut s = opened(&path);
        s.post(entry("a", "デモ"), 1, None).unwrap();
        s.revise(entry("a", "訂正"), 1, "理由", 2, None).unwrap();
        drop(s);
        Connection::open(&path)
            .unwrap()
            .execute_batch(&format!("PRAGMA foreign_keys=OFF; {sql}"))
            .unwrap();
        let (mut s, r) = Store::open(&path, OpenOptions::default()).unwrap();
        assert_eq!(r.events_in_db, 2);
        assert_eq!(r.events_loaded, 1);
        assert_eq!(s.read_only().unwrap().seq, 2, "{sql}");
        assert!(matches!(
            s.post(entry("c", "デモ"), 3, None),
            Err(StoreError::ReadOnly)
        ));
        let failure = s.read_only().unwrap();
        let kind = if sql.contains("SET seq=5") {
            "wrong_sequence"
        } else if sql.contains("SET prev_hash") {
            "broken_link"
        } else if sql.contains("SET amount") || sql.contains("SET reason") {
            "bad_hash"
        } else {
            "rule_violation"
        };
        assert_eq!(failure.kind, kind);
        if kind == "rule_violation" {
            assert!(failure.rule.is_some());
        }
        assert!(s.verify().is_err());
    }
}
// AC-09.1: 先頭が復元不能でも起動は成功し、空の台帳を読む。
#[test]
fn bad_first_row() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    s.post(entry("a", "デモ"), 1, None).unwrap();
    drop(s);
    Connection::open(&path).unwrap().execute_batch("DROP TRIGGER ledger_events_no_update; UPDATE ledger_events SET entry_id='?' WHERE seq=1").unwrap();
    let (s, r) = Store::open(&path, OpenOptions::default()).unwrap();
    assert_eq!(s.ledger().events(), []);
    assert_eq!(r.events_loaded, 0);
    assert_eq!(s.read_only().unwrap().seq, 1);
}
// AC-09.1: 末尾削除は連鎖だけでは検知できない。控えたハッシュが消える。
#[test]
fn tail_deletion_needs_noted_hash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    s.post(entry("a", "デモ"), 1, None).unwrap();
    s.void(&EntryId::new("a").unwrap(), 1, "取消", 2).unwrap();
    let hash = s.ledger().head_hash().to_hex();
    assert!(s.noted_hash_lookup(&hash).unwrap().is_head);
    drop(s);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "DROP TRIGGER ledger_events_no_delete; DELETE FROM ledger_events WHERE seq=2",
        )
        .unwrap();
    let (s, _) = Store::open(&path, OpenOptions::default()).unwrap();
    assert!(s.verify().is_ok());
    assert!(s.noted_hash_lookup(&hash).is_none());
    assert_ne!(s.ledger().head_hash(), Hash::from_hex(&hash).unwrap());
}
// AC-06.3: 元の日付と変更先の両方を検査する。
#[test]
fn locked_period() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    s.post(entry("a", "デモ"), 1, None).unwrap();
    let mut settings = s.settings().clone();
    settings.locked_until = Some("2026-10-01".into());
    s.save_settings(settings, 2).unwrap();
    assert!(matches!(
        s.post(entry("b", "デモ"), 3, None),
        Err(StoreError::LockedPeriod)
    ));
    assert!(matches!(
        s.revise(entry("a", "デモ"), 1, "訂正", 3, None),
        Err(StoreError::LockedPeriod)
    ));
    let future = kamoku_ledger::Entry::new(
        EntryId::new("a").unwrap(),
        time::macros::date!(2026 - 10 - 02),
        "デモ",
        entry("a", "").lines().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        s.revise(future, 1, "訂正", 3, None),
        Err(StoreError::LockedPeriod)
    ));
    assert!(matches!(
        s.void(&EntryId::new("a").unwrap(), 1, "取消", 3),
        Err(StoreError::LockedPeriod)
    ));
    let future = kamoku_ledger::Entry::new(
        EntryId::new("c").unwrap(),
        time::macros::date!(2026 - 10 - 02),
        "デモ",
        entry("a", "").lines().to_vec(),
    )
    .unwrap();
    s.post(future, 3, None).unwrap();
    assert!(matches!(
        s.revise(entry("c", "デモ"), 1, "訂正", 4, None),
        Err(StoreError::LockedPeriod)
    ));
}
#[test]
fn stale_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut a = opened(&path);
    let mut b = opened(&path);
    a.post(entry("a", "デモ"), 1, None).unwrap();
    assert!(matches!(
        b.post(entry("b", "デモ"), 2, None),
        Err(StoreError::Stale)
    ));
    assert_eq!(a.ledger().events(), b.ledger().events());
    b.post(entry("b", "デモ"), 2, None).unwrap();
    assert!(matches!(
        a.void(&EntryId::new("a").unwrap(), 1, "取消", 3),
        Err(StoreError::Stale)
    ));
    a.revise(entry("a", "訂正"), 1, "理由", 3, None).unwrap();
    assert!(matches!(
        a.void(&EntryId::new("a").unwrap(), 1, "取消", 4),
        Err(StoreError::StaleRevision)
    ));
}
// AC-09.1: DB失敗でもメモリ、取引先、履歴をすべて戻す。
#[test]
fn rollback_on_database_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail BEFORE INSERT ON ledger_lines BEGIN SELECT RAISE(ABORT,'test'); END;",
    )
    .unwrap();
    let partner = NewPartner {
        name: "サンプル取引先".into(),
        ..NewPartner::default()
    };
    assert!(s.post(entry("a", "デモ"), 1, Some(partner)).is_err());
    assert_eq!(s.ledger().events(), []);
    assert_eq!(s.ledger().head_hash(), Hash::GENESIS);
    assert!(s.partners().unwrap().is_empty());
    assert!(s.change_log(50).unwrap().is_empty());
    assert!(s.verify().is_ok());
    conn.execute_batch("DROP TRIGGER fail").unwrap();
    s.post(entry("a", "デモ"), 1, None).unwrap();
    let events = s.ledger().events().to_vec();
    assert!(
        s.post_many(
            vec![(entry("b", "デモ"), None), (entry("a", "重複"), None)],
            2
        )
        .is_err()
    );
    assert_eq!(s.ledger().events(), events);
    assert!(s.verify().is_ok());
}
