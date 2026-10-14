// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! コマンド（`kamoku verify|backup`）。実行ファイルで作った帳簿を確かめ、書き出す。

mod cli_support;
mod common;
use cli_support::{Sandbox, Spawned};
use common::*;
use serde_json::json;
use std::path::Path;

fn make_books(sb: &Sandbox) -> String {
    let server = Spawned::start(sb, &["serve", "--no-open", "--port", "0"]);
    let c = server.login();
    books(&c);
    let mut target = expense("2026-10-02", 2200, "exp_comm", "w01");
    target["description"] = json!("改ざんの的です");
    create(&c, &target);
    create(&c, &expense("2026-10-03", 3300, "exp_comm", "w01"));
    let head = c.api("/status").json()["ledger"]["head_hash"]
        .as_str()
        .unwrap()
        .to_owned();
    server.stop();
    head
}

// AC-09.1: verify は件数と末尾のハッシュを出す。壊れていれば最初の連番と規則を出して終了コード1。
#[test]
fn verify_reports_head_and_corruption() {
    let sb = Sandbox::new();
    assert_eq!(sb.run(&["verify"]).0, 1, "帳簿が無い");
    let head = make_books(&sb);
    let (code, out, _) = sb.run(&["verify"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&head));
    assert!(out.contains("イベント数: 2"));
    let db = sb.path("data").join("kamoku.db");
    let mut bytes = std::fs::read(&db).unwrap();
    let needle = "改ざんの的です".as_bytes();
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap();
    bytes[at..at + needle.len()].copy_from_slice("改ざんの跡です".as_bytes());
    std::fs::write(&db, bytes).unwrap();
    let (code, out, _) = sb.run(&["verify"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("最初の連番: 1") && out.contains("bad_hash"),
        "{out}"
    );
}

// AC-09.3・AC-09.4: backup は新しいファイルにだけ書き、0600 にする。既にあれば断る。書き出し先のフォルダの権限は変えない。
#[test]
fn backup_writes_a_private_new_file() {
    let sb = Sandbox::new();
    let head = make_books(&sb);
    let folder = sb.path("home").join("backups");
    std::fs::create_dir(&folder).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = folder.join("kamoku-backup.db");
    let (code, stdout, err) = sb.run(&["backup", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{stdout}{err}");
    let (store, _) = kamoku_store::Store::open(&out, kamoku_store::OpenOptions::default()).unwrap();
    assert_eq!(store.ledger().head_hash().to_hex(), head);
    drop(store);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&folder), 0o755, "書き出し先のフォルダの権限が変わった");
    }
    let leftovers: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert!(
        leftovers
            .iter()
            .all(|n| n.to_string_lossy().starts_with("kamoku-backup.db")),
        "{leftovers:?}"
    );
    let (code, _, err) = sb.run(&["backup", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.contains("すでにあります"), "{err}");
}

#[cfg(unix)]
#[test]
fn backup_file_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let sb = Sandbox::new();
    make_books(&sb);
    let out = sb.path("home").join("b.db");
    assert_eq!(sb.run(&["backup", "--out", out.to_str().unwrap()]).0, 0);
    assert_eq!(
        std::fs::metadata(&out).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
