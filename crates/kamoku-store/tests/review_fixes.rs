// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! AC-04.4 / AC-05.5 / AC-09.1 / AC-09.3: レビュー指摘の回帰試験。
mod common;
use common::*;
use kamoku_ledger::{Entry, EntryId, Yen};
use kamoku_store::*;
use rusqlite::Connection;
use std::collections::HashSet;

#[test]
fn s1_force_reserves_new_occurrences_first() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let w = wallet(&mut s);
    s.import_commit(batch(&w), vec![draft(2, 1)], HashSet::new())
        .unwrap();
    let mut forced = draft(14, 1);
    forced.description = "デモ強制".into();
    let mut fresh = draft(15, 2);
    fresh.description = "デモ新規".into();
    let result = s
        .import_commit(batch(&w), vec![forced, fresh], HashSet::from([14]))
        .unwrap();
    assert_eq!((result.created, result.duplicates), (2, 0));
    let rows = s.drafts(DraftFilter::default()).unwrap().items;
    assert_eq!(
        rows.iter()
            .find(|r| r.description == "デモ新規")
            .unwrap()
            .occurrence,
        2
    );
    assert_eq!(
        rows.iter()
            .find(|r| r.description == "デモ強制")
            .unwrap()
            .occurrence,
        3
    );
}

#[test]
fn s2_replace_cannot_overwrite_append_only_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    s.post(
        entry("a", "デモ").with_meta("kind", "journal").unwrap(),
        1,
        None,
    )
    .unwrap();
    let c = Connection::open(&path).unwrap();
    c.execute_batch("PRAGMA recursive_triggers=OFF").unwrap();
    let cases = [
        (
            "ledger_events",
            "INSERT OR REPLACE INTO ledger_events SELECT seq,kind,entry_id,revision,99,reason,date,description,prev_hash,hash FROM ledger_events WHERE seq=1",
            "SELECT recorded_at FROM ledger_events WHERE seq=1",
            "1",
        ),
        (
            "ledger_events",
            "INSERT OR REPLACE INTO ledger_events SELECT 2,kind,entry_id,revision,99,reason,date,description,prev_hash,hash FROM ledger_events WHERE seq=1",
            "SELECT seq FROM ledger_events WHERE entry_id='a' AND revision=1",
            "1",
        ),
        (
            "ledger_lines",
            "INSERT OR REPLACE INTO ledger_lines SELECT seq,position,account,side,111,tax_code,tax_amount,memo FROM ledger_lines WHERE seq=1 AND position=0",
            "SELECT amount FROM ledger_lines WHERE seq=1 AND position=0",
            "110",
        ),
        (
            "ledger_meta",
            "INSERT OR REPLACE INTO ledger_meta VALUES(1,'kind','other')",
            "SELECT value FROM ledger_meta WHERE seq=1 AND key='kind'",
            "journal",
        ),
        (
            "change_log",
            "INSERT OR REPLACE INTO change_log SELECT id,'other',recorded_at,kind,body FROM change_log WHERE id=1",
            "SELECT actor FROM change_log WHERE id=1",
            "local",
        ),
    ];
    for (table, sql, query, value) in cases {
        let error = c.execute(sql, []).unwrap_err();
        assert!(
            error.to_string().contains("append only"),
            "{table}: {error}"
        );
        let actual: String = c
            .query_row(&format!("SELECT CAST(({query}) AS TEXT)"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(actual, value, "{table}");
    }
    s.post(
        entry("b", "デモ").with_meta("kind", "journal").unwrap(),
        2,
        None,
    )
    .unwrap();
    assert!(s.verify().is_ok());
}

#[test]
fn s3_invalid_meta_opens_verified_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.sqlite");
    let mut s = opened(&path);
    s.post(entry("a", "デモ"), 1, None).unwrap();
    s.post(
        entry("b", "デモ").with_meta("kind", "journal").unwrap(),
        2,
        None,
    )
    .unwrap();
    drop(s);
    Connection::open(&path).unwrap().execute_batch(
        "DROP TRIGGER ledger_meta_no_update; UPDATE ledger_meta SET value=CAST(x'80' AS TEXT) WHERE seq=2"
    ).unwrap();
    let (mut s, report) = Store::open(&path, OpenOptions::default()).unwrap();
    assert_eq!(report.events_loaded, 1);
    assert_eq!(s.read_only().unwrap().seq, 2);
    assert!(matches!(
        s.post(entry("c", "デモ"), 3, None),
        Err(StoreError::ReadOnly)
    ));
}

#[test]
fn s3_orphan_rows_are_corruption_not_open_errors() {
    for seq in [-1, 3] {
        for table in ["ledger_lines", "ledger_meta"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("data.sqlite");
            let mut s = opened(&path);
            s.post(entry("a", "デモ"), 1, None).unwrap();
            s.post(entry("b", "デモ"), 2, None).unwrap();
            drop(s);
            let c = Connection::open(&path).unwrap();
            c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
            if table == "ledger_lines" {
                c.execute(
                    "INSERT INTO ledger_lines VALUES(?,0,'cash','debit',1,NULL,0,'')",
                    [seq],
                )
                .unwrap();
            } else {
                c.execute("INSERT INTO ledger_meta VALUES(?,'kind','journal')", [seq])
                    .unwrap();
            }
            let (mut s, report) = Store::open(&path, OpenOptions::default()).unwrap();
            assert_eq!(report.events_loaded, 2, "{table} {seq}");
            assert!(s.read_only().is_some());
            assert!(matches!(
                s.post(entry("c", "デモ"), 3, None),
                Err(StoreError::ReadOnly)
            ));
        }
    }
}

#[test]
fn s4_failed_reload_clears_unpersisted_memory_and_preserves_save_error() {
    for fail_reload in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.sqlite");
        let mut s = opened(&path);
        s.post(
            entry("a", "デモ").with_meta("kind", "journal").unwrap(),
            1,
            None,
        )
        .unwrap();
        let c = Connection::open(&path).unwrap();
        c.execute_batch("DROP TRIGGER ledger_meta_no_update; UPDATE ledger_meta SET value=CAST(x'80' AS TEXT); CREATE TRIGGER fail BEFORE INSERT ON ledger_lines BEGIN SELECT RAISE(ABORT,'save failed'); END;").unwrap();
        if fail_reload {
            c.execute_batch("DROP TABLE ledger_meta").unwrap();
        }
        let error = s.post(entry("unsaved", "デモ"), 2, None).unwrap_err();
        assert_eq!(s.ledger().events(), []);
        assert!(s.read_only().is_some());
        assert_eq!(s.entry_no(&EntryId::new("unsaved").unwrap()), None);
        assert!(error.to_string().contains("save failed"), "{error}");
        if fail_reload {
            assert!(
                s.read_only()
                    .unwrap()
                    .rule
                    .as_ref()
                    .unwrap()
                    .contains("再読込")
            );
        }
    }
}

fn prepared() -> (Store, DraftId, Entry) {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let w = wallet(&mut s);
    s.import_commit(batch(&w), vec![draft(2, 1)], HashSet::new())
        .unwrap();
    let id = s.drafts(DraftFilter::default()).unwrap().items[0].id;
    s.update_draft(
        id,
        DraftPatch {
            account: Some(Some("exp_comm".into())),
            ..DraftPatch::default()
        },
        2,
    )
    .unwrap();
    let e = import_entry(&s, id, "a");
    (s, id, e)
}

#[test]
fn s5_unexpected_meta_is_rejected() {
    for (key, value) in [("partner", "サンプル別会社"), ("business_pct", "90")] {
        let (mut s, id, e) = if key == "business_pct" {
            let (mut s, _) = Store::open_in_memory().unwrap();
            let w = wallet(&mut s);
            let mut d = draft(2, 1);
            d.direction = "in".into();
            s.import_commit(batch(&w), vec![d], HashSet::new()).unwrap();
            let id = s.drafts(DraftFilter::default()).unwrap().items[0].id;
            s.update_draft(
                id,
                DraftPatch {
                    account: Some(Some("sales".into())),
                    ..DraftPatch::default()
                },
                2,
            )
            .unwrap();
            let e = import_entry(&s, id, "a");
            assert!(!e.meta().contains_key("business_pct"));
            (s, id, e)
        } else {
            prepared()
        };
        let result = s
            .confirm_drafts(vec![(id, e.with_meta(key, value).unwrap())], 3)
            .unwrap();
        assert!(
            matches!(result[0].result, Err(StoreError::Invalid(_))),
            "{key}"
        );
        assert_eq!(s.ledger().events(), []);
    }
}

#[test]
fn s5_invoice_and_store_owned_meta_are_allowed() {
    let (mut s, id, e) = prepared();
    let e = e
        .with_meta("invoice_no", "T0000000000000")
        .unwrap()
        .with_meta("draft", "other")
        .unwrap()
        .with_meta("source", "manual")
        .unwrap();
    assert!(
        s.confirm_drafts(vec![(id, e)], 3).unwrap()[0]
            .result
            .is_ok()
    );
    let meta = s.ledger().current()[0].meta().clone();
    assert_eq!(meta.get("source").unwrap(), "import");
    assert_eq!(meta.get("draft").unwrap(), &id.to_string());
}

fn dated(year: i32, tax: Option<(&str, i64)>) -> Entry {
    let base = entry("a", "デモ");
    let mut lines = base.lines().to_vec();
    if let Some((code, amount)) = tax {
        lines[0].tax_code = Some(code.into());
        lines[0].tax_amount = Yen::new(amount);
    }
    Entry::new(
        base.id().clone(),
        time::Date::from_calendar_date(year, time::Month::January, 1).unwrap(),
        "デモ",
        lines,
    )
    .unwrap()
}

#[test]
fn s6_post_and_revise_reject_unreadable_date_and_tax() {
    for invalid in [
        dated(-1, None),
        dated(2026, Some(("TAX10", 0))),
        dated(2026, Some(("NT", 1))),
    ] {
        let (mut s, _) = Store::open_in_memory().unwrap();
        let error = s.post(invalid.clone(), 1, None).unwrap_err();
        assert!(matches!(
            error,
            StoreError::BadTaxCode | StoreError::Invalid(_)
        ));
        assert_eq!(s.ledger().events(), []);
        s.post(entry("a", "デモ"), 1, None).unwrap();
        let error = s.revise(invalid, 1, "デモ訂正", 2, None).unwrap_err();
        assert!(matches!(
            error,
            StoreError::BadTaxCode | StoreError::Invalid(_)
        ));
        assert_eq!(s.ledger().events().len(), 1);
    }
}

#[test]
fn s6_accepted_date_boundaries_roundtrip() {
    for year in [0, 1, 2026, 9999] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.sqlite");
        let mut s = opened(&path);
        let e = dated(year, Some(("NT", 0)));
        s.post(e.clone(), 1, None).unwrap();
        drop(s);
        let (s, _) = Store::open(&path, OpenOptions::default()).unwrap();
        assert_eq!(s.ledger().current(), vec![&e]);
        assert!(s.read_only().is_none());
    }
}

#[cfg(unix)]
#[test]
fn s7_existing_directories_keep_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = dir.path().join("data.sqlite");
    let s = opened(&path);
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(dir.path()), 0o755);
    assert_eq!(mode(&path), 0o600);
    let backup = dir.path().join("backup.sqlite");
    s.backup_to(&backup).unwrap();
    assert_eq!(mode(dir.path()), 0o755);
    assert_eq!(mode(&backup), 0o600);
    let nested = dir.path().join("new/nested/data.sqlite");
    let _s = opened(&nested);
    assert_eq!(mode(nested.parent().unwrap()), 0o700);
    assert_eq!(mode(&dir.path().join("new")), 0o700);
}
