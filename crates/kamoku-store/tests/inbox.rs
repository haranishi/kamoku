// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 保存層の受け入れ条件の回帰試験。

mod common;
use common::*;
use kamoku_ledger::EntryId;
use kamoku_store::*;
use rusqlite::Connection;
use std::collections::HashSet;

// AC-04.4 AC-04.5 AC-04.7: 二度目、部分重複、同じ内容の二取引、強制指定。
#[test]
fn duplicates_and_force() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let w = wallet(&mut s);
    let first = s
        .import_commit(batch(&w), vec![draft(2, 1), draft(3, 2)], HashSet::new())
        .unwrap();
    assert_eq!(first.created, 2);
    assert_eq!(first.invalid, 1);
    let repeated = s
        .import_commit(batch(&w), vec![draft(2, 1), draft(3, 2)], HashSet::new())
        .unwrap();
    assert_eq!(repeated.created, 0);
    assert_eq!(repeated.duplicates, 2);
    let mut different = draft(4, 1);
    different.fingerprint = "b".repeat(64);
    let overlap = s
        .import_commit(batch(&w), vec![draft(2, 1), different], HashSet::new())
        .unwrap();
    assert_eq!(overlap.created, 1);
    assert_eq!(overlap.duplicates, 1);
    let forced = s
        .import_commit(
            batch(&w),
            vec![draft(2, 1), draft(3, 2)],
            HashSet::from([2, 3]),
        )
        .unwrap();
    assert_eq!(forced.created, 2);
    assert_eq!(forced.duplicates, 0);
    let known = s
        .known_fingerprints(&[("a".repeat(64), 4), ("a".repeat(64), 5)])
        .unwrap();
    assert_eq!(known.maxima.get(&"a".repeat(64)), Some(&4));
    assert_eq!(known.existing.len(), 1);
    assert_eq!(s.draft_counts().unwrap().draft, 5);
    let rows = s.drafts(DraftFilter::default()).unwrap();
    assert_eq!(rows.total, 5);
    assert_eq!(rows.items[0].external_id.as_deref(), Some("デモ取引番号"));
}
// AC-05.4 AC-05.5: 科目未選択は飛ばし、有効な行だけ確定し、metaも永続化。
#[test]
fn confirm_partial_and_restore() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let w = wallet(&mut s);
    s.import_commit(batch(&w), vec![draft(2, 1), draft(3, 2)], HashSet::new())
        .unwrap();
    let rows = s.drafts(DraftFilter::default()).unwrap().items;
    let id = rows[0].id;
    let missing = rows[1].id;
    s.update_draft(
        id,
        DraftPatch {
            account: Some(Some("exp_comm".into())),
            partner: Some("サンプル取引先".into()),
            ..DraftPatch::default()
        },
        2,
    )
    .unwrap();
    let results = s
        .confirm_drafts(
            vec![
                (id, import_entry(&s, id, "a")),
                (missing, entry("b", "サンプル通信")),
            ],
            3,
        )
        .unwrap();
    assert!(results[0].result.is_ok());
    assert!(matches!(results[1].result, Err(StoreError::MissingAccount)));
    let counts = s.draft_counts().unwrap();
    assert_eq!(counts.confirmed, 1);
    assert_eq!(counts.draft, 1);
    assert_eq!(s.partners().unwrap().len(), 1);
    let e = s.ledger().current()[0];
    assert_eq!(e.meta().get("draft"), Some(&id.to_string()));
    assert_eq!(e.meta().get("source").map(String::as_str), Some("import"));
    let confirmed = s
        .drafts(DraftFilter {
            status: Some(DraftStatus::Confirmed),
            ..DraftFilter::default()
        })
        .unwrap()
        .items;
    assert_eq!(confirmed[0].entry_id.as_deref(), Some("a"));
    let wallet_line = s.ledger().current()[0]
        .lines()
        .iter()
        .find(|l| l.account.as_str() == w)
        .unwrap();
    assert_eq!(wallet_line.side, kamoku_ledger::Side::Credit);
    assert!(matches!(
        s.update_draft(id, DraftPatch::default(), 4),
        Err(StoreError::AlreadyConfirmed)
    ));
    assert!(matches!(
        s.set_draft_status(id, DraftAction::Ignore, 4),
        Err(StoreError::AlreadyConfirmed)
    ));
    assert!(matches!(
        s.set_draft_status(id, DraftAction::Restore, 4),
        Err(StoreError::AlreadyConfirmed)
    ));
    let repeated = s.confirm_drafts(vec![(id, entry("c", "デモ"))], 4).unwrap();
    assert!(matches!(
        repeated[0].result,
        Err(StoreError::AlreadyConfirmed)
    ));
    s.void(&EntryId::new("a").unwrap(), 1, "取消", 5).unwrap();
    s.set_draft_status(id, DraftAction::Restore, 6).unwrap();
    let log = s.change_log(1).unwrap();
    assert_eq!(log[0].body["previous_entry_id"], "a");
    let forced = s
        .drafts(DraftFilter::default())
        .unwrap()
        .items
        .into_iter()
        .find(|d| d.id == id)
        .unwrap();
    assert!(forced.entry_id.is_none());
    s.set_draft_status(id, DraftAction::Ignore, 7).unwrap();
    s.set_draft_status(id, DraftAction::Restore, 8).unwrap();
    s.update_draft(
        id,
        DraftPatch {
            date: Some("2026-10-02".into()),
            ..DraftPatch::default()
        },
        9,
    )
    .unwrap();
    let forced = s
        .drafts(DraftFilter {
            from: Some("2026-10-02".into()),
            q: Some("サンプル".into()),
            wallet: Some(w),
            ..DraftFilter::default()
        })
        .unwrap();
    assert_eq!(forced.total, 1);
    assert_eq!(forced.items[0].fingerprint, "a".repeat(64));
}
// AC-05.5: 状態更新で失敗すると、複数の仕訳も取引先も履歴も全体で戻る。
#[test]
fn confirmation_failure_rolls_back_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    let w = wallet(&mut s);
    s.import_commit(batch(&w), vec![draft(2, 1), draft(3, 2)], HashSet::new())
        .unwrap();
    let ids = s
        .drafts(DraftFilter::default())
        .unwrap()
        .items
        .iter()
        .map(|d| d.id)
        .collect::<Vec<_>>();
    for id in &ids {
        s.update_draft(
            *id,
            DraftPatch {
                account: Some(Some("exp_comm".into())),
                partner: Some("デモ取引先".into()),
                ..DraftPatch::default()
            },
            2,
        )
        .unwrap();
    }
    let before = s.change_log(500).unwrap().len();
    Connection::open(&path).unwrap().execute_batch(&format!("CREATE TRIGGER fail BEFORE UPDATE ON drafts WHEN NEW.status='confirmed' AND NEW.id={} BEGIN SELECT RAISE(ABORT,'test'); END;",ids[1])).unwrap();
    assert!(
        s.confirm_drafts(
            vec![
                (ids[0], import_entry(&s, ids[0], "a")),
                (ids[1], import_entry(&s, ids[1], "b"))
            ],
            3
        )
        .is_err()
    );
    assert_eq!(s.ledger().events(), []);
    assert_eq!(s.draft_counts().unwrap().draft, 2);
    assert_eq!(s.draft_counts().unwrap().confirmed, 0);
    assert!(s.partners().unwrap().is_empty());
    assert_eq!(s.change_log(500).unwrap().len(), before);
    assert!(s.verify().is_ok());
}
#[test]
fn import_failure_rolls_back_batch_and_rows() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let w = wallet(&mut s);
    let mut bad = draft(3, 2);
    bad.date = "2026-02-30".into();
    assert!(
        s.import_commit(batch(&w), vec![draft(2, 1), bad], HashSet::new())
            .is_err()
    );
    assert!(s.import_batches(50).unwrap().is_empty());
    assert_eq!(s.draft_counts().unwrap().draft, 0);
}

// AC-05.5: 入金は口座の借方。古い下書きから組み立てた仕訳は確定しない。
#[test]
fn incoming_wallet_and_changed_draft() {
    let (mut store, _) = Store::open_in_memory().unwrap();
    let wallet_code = wallet(&mut store);
    let mut row = draft(2, 1);
    row.direction = "in".into();
    store
        .import_commit(batch(&wallet_code), vec![row], HashSet::new())
        .unwrap();
    let id = store.drafts(DraftFilter::default()).unwrap().items[0].id;
    store
        .update_draft(
            id,
            DraftPatch {
                account: Some(Some("sales".into())),
                ..DraftPatch::default()
            },
            2,
        )
        .unwrap();
    let old = import_entry(&store, id, "a");
    store
        .update_draft(
            id,
            DraftPatch {
                date: Some("2026-10-02".into()),
                ..DraftPatch::default()
            },
            3,
        )
        .unwrap();
    let results = store.confirm_drafts(vec![(id, old)], 4).unwrap();
    assert!(results[0].result.is_err());
    assert_eq!(store.draft_counts().unwrap().draft, 1);
    let fresh = import_entry(&store, id, "a");
    let results = store.confirm_drafts(vec![(id, fresh)], 5).unwrap();
    assert!(results[0].result.is_ok());
    let event = store.ledger().current()[0];
    let line = event
        .lines()
        .iter()
        .find(|l| l.account.as_str() == wallet_code)
        .unwrap();
    assert_eq!(line.side, kamoku_ledger::Side::Debit);
    assert_eq!(line.amount.get(), 110);
    assert_eq!(event.date(), time::macros::date!(2026 - 10 - 02));
}
