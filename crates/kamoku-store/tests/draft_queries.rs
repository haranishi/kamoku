// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 未確認の集計と選択行の読み取り。確定時の状態検査は保存のトランザクションで行う。

mod common;
use common::*;
use kamoku_books::{MAX_AMOUNT, MAX_REPORT_AMOUNT};
use kamoku_store::*;
use rusqlite::Connection;
use std::collections::HashSet;

fn rows(count: usize) -> Vec<NewDraft> {
    (1..=count)
        .map(|i| draft(i, u32::try_from(i).unwrap()))
        .collect()
}

fn insert(store: &mut Store, wallet: &str, rows: Vec<NewDraft>) -> Vec<DraftId> {
    let count = rows.len();
    store
        .import_commit(batch(wallet), rows, HashSet::new())
        .unwrap();
    (0..count)
        .step_by(500)
        .flat_map(|offset| {
            store
                .drafts(DraftFilter {
                    limit: 500,
                    offset: u64::try_from(offset).unwrap(),
                    ..DraftFilter::default()
                })
                .unwrap()
                .items
                .into_iter()
                .map(|row| row.id)
        })
        .collect()
}

fn ready(store: &mut Store, id: DraftId, account: &str) {
    store
        .update_draft(
            id,
            DraftPatch {
                account: Some(Some(account.into())),
                ..DraftPatch::default()
            },
            2,
        )
        .unwrap();
}

#[test]
fn pending_amount_only_counts_outgoing_unconfirmed_rows() {
    let (mut store, _) = Store::open_in_memory().unwrap();
    assert_eq!(store.pending_out_amount().unwrap(), 0);
    let wallet = wallet(&mut store);
    let mut input = rows(6);
    for (i, row) in input.iter_mut().enumerate() {
        row.amount = i64::try_from(i + 1).unwrap();
        row.direction = if i % 2 == 0 { "out" } else { "in" }.into();
    }
    let ids = insert(&mut store, &wallet, input);
    for id in &ids[2..4] {
        store.set_draft_status(*id, DraftAction::Ignore, 3).unwrap();
    }
    ready(&mut store, ids[4], "exp_comm");
    ready(&mut store, ids[5], "sales");
    let entries = vec![
        (ids[4], import_entry(&store, ids[4], "out")),
        (ids[5], import_entry(&store, ids[5], "in")),
    ];
    assert!(
        store
            .confirm_drafts(entries, 4)
            .unwrap()
            .iter()
            .all(|result| result.result.is_ok())
    );
    assert_eq!(store.pending_out_amount().unwrap(), 1);
    assert_eq!(store.draft_counts().unwrap().draft, 2);
}

#[test]
fn selected_drafts_keep_order_duplicates_and_empty_slots() {
    let (mut store, _) = Store::open_in_memory().unwrap();
    assert!(store.pending_drafts(&[]).unwrap().is_empty());
    let wallet = wallet(&mut store);
    let ids = insert(&mut store, &wallet, rows(4));
    store
        .set_draft_status(ids[1], DraftAction::Ignore, 3)
        .unwrap();
    ready(&mut store, ids[3], "exp_comm");
    let entry = import_entry(&store, ids[3], "confirmed");
    assert!(
        store.confirm_drafts(vec![(ids[3], entry)], 4).unwrap()[0]
            .result
            .is_ok()
    );
    let selected = store
        .pending_drafts(&[ids[2], -1, ids[0], ids[2], ids[1], ids[3], i64::MAX, 0])
        .unwrap();
    let found: Vec<_> = selected
        .iter()
        .map(|row| row.as_ref().map(|d| d.id))
        .collect();
    assert_eq!(
        found,
        [
            Some(ids[2]),
            None,
            Some(ids[0]),
            Some(ids[2]),
            None,
            None,
            None,
            None
        ]
    );
    let reference = store.drafts(DraftFilter::default()).unwrap().items;
    assert_eq!(
        serde_json::to_value(&selected[2]).unwrap(),
        serde_json::to_value(&reference[0]).unwrap()
    );
}

#[test]
fn selected_drafts_are_found_among_thousands_of_unrelated_rows() {
    let (mut store, _) = Store::open_in_memory().unwrap();
    let wallet = wallet(&mut store);
    let ids = insert(&mut store, &wallet, rows(2_048));
    let wanted = [ids[2_047], ids[1_024], ids[0], ids[2_047]];
    let selected = store.pending_drafts(&wanted).unwrap();
    let found: Vec<_> = selected.into_iter().map(|row| row.unwrap().id).collect();
    assert_eq!(found, wanted);
    assert_eq!(store.pending_out_amount().unwrap(), 2_048 * 110);
}

#[test]
fn pending_amount_keeps_the_safe_integer_boundary_exact() {
    let (mut store, _) = Store::open_in_memory().unwrap();
    let wallet = wallet(&mut store);
    let mut input = rows(901);
    for row in &mut input[..900] {
        row.amount = MAX_AMOUNT;
    }
    input[900].amount = MAX_REPORT_AMOUNT - 900 * MAX_AMOUNT;
    insert(&mut store, &wallet, input);
    assert_eq!(store.pending_out_amount().unwrap(), MAX_REPORT_AMOUNT);
    let mut extra = draft(902, 902);
    extra.amount = 1;
    store
        .import_commit(batch(&wallet), vec![extra], HashSet::new())
        .unwrap();
    assert!(matches!(
        store.pending_out_amount(),
        Err(StoreError::AmountRange)
    ));
}

#[test]
fn pending_amount_reports_amount_range_before_a_sqlite_integer_sum_can_overflow() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("synthetic.sqlite");
    let mut store = opened(&path);
    let wallet = wallet(&mut store);
    let ids = insert(&mut store, &wallet, rows(2));
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("PRAGMA ignore_check_constraints=ON")
        .unwrap();
    connection
        .execute("UPDATE drafts SET amount=? WHERE id=?", [i64::MAX, ids[0]])
        .unwrap();
    assert!(matches!(
        store.pending_out_amount(),
        Err(StoreError::AmountRange)
    ));
}

#[test]
fn confirmation_rechecks_status_after_selection() {
    let (mut store, _) = Store::open_in_memory().unwrap();
    let wallet = wallet(&mut store);
    let ids = insert(&mut store, &wallet, rows(1));
    ready(&mut store, ids[0], "exp_comm");
    let selected = store.pending_drafts(&ids).unwrap().pop().unwrap().unwrap();
    let entry = import_entry(&store, selected.id, "selected");
    store
        .set_draft_status(selected.id, DraftAction::Ignore, 3)
        .unwrap();
    let rejected = store
        .confirm_drafts(vec![(selected.id, entry.clone())], 4)
        .unwrap();
    assert!(matches!(rejected[0].result, Err(StoreError::Invalid(_))));
    assert_eq!(store.ledger().events(), []);
    assert!(store.pending_drafts(&ids).unwrap()[0].is_none());
    store
        .set_draft_status(selected.id, DraftAction::Restore, 5)
        .unwrap();
    assert!(
        store
            .confirm_drafts(vec![(selected.id, entry.clone())], 6)
            .unwrap()[0]
            .result
            .is_ok()
    );
    let repeated = store.confirm_drafts(vec![(selected.id, entry)], 7).unwrap();
    assert!(matches!(
        repeated[0].result,
        Err(StoreError::AlreadyConfirmed)
    ));
    assert_eq!(store.ledger().events().len(), 1);
}
