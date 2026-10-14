// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 保存層の受け入れ条件の回帰試験。

mod common;
use common::*;
use kamoku_store::*;
use rusqlite::Connection;

// AC-09.3 AC-09.4: 本体・WAL・SHM・複製の権限と一貫性。
#[test]
fn permissions_backup_and_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("private/data.sqlite");
    let mut s = opened(&path);
    s.post(entry("a", "サンプル😀"), 1, None).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in [
            path.clone(),
            std::path::PathBuf::from(format!("{}-wal", path.display())),
            std::path::PathBuf::from(format!("{}-shm", path.display())),
        ] {
            assert_eq!(
                std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    let backup = dir.path().join("backup.sqlite");
    s.backup_to(&backup).unwrap();
    assert!(matches!(
        s.backup_to(&backup),
        Err(StoreError::BackupExists)
    ));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let (b, _) = Store::open(&backup, OpenOptions::default()).unwrap();
    assert_eq!(b.ledger().head_hash(), s.ledger().head_hash());
    assert!(b.verify().is_ok());
    assert!(!std::fs::read_dir(dir.path()).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".kamoku-backup")
    }));
    let old = dir.path().join("old.sqlite");
    Connection::open(&old)
        .unwrap()
        .execute_batch(include_str!("../migrations/fixtures/0000.sql"))
        .unwrap();
    let (s, _) = Store::open(&old, OpenOptions::default()).unwrap();
    assert!(s.chart().len() > 40);
    assert_eq!(s.ledger().events(), []);
    let c = Connection::open(&old).unwrap();
    assert_eq!(
        c.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    c.execute_batch("PRAGMA user_version=99").unwrap();
    assert!(matches!(
        Store::open(&old, OpenOptions::default()),
        Err(StoreError::FutureSchema)
    ));
}
#[test]
fn default_options_never_create_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.sqlite");
    assert!(Store::open(&path, OpenOptions::default()).is_err());
    assert!(!path.exists());
}

// AC-09.3: 複製の公開で失敗しても、不完全な本来名や一時ファイルを残さない。
#[cfg(unix)]
#[test]
fn backup_publish_failure_cleans_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = Store::open_in_memory().unwrap();
    let destination = dir.path().join("x".repeat(300));
    assert!(store.backup_to(&destination).is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
