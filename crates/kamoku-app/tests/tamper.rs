// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 改ざん（`docs/spec/api-common.md` の読み取り専用の状態）。DBのファイルのバイトを書き換えてから起動すると、
//! 読み取り専用になり、`/status` に最初の不正が出て、書き込みは409。読み取りは壊れた連番の手前まで。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::json;
use std::path::Path;

/// DBの本体のファイルの中の文字列を、同じバイト数の別の文字列に書き換える（SQLite の行の形は保つ）。
fn corrupt(db: &Path, needle: &str, replacement: &str) {
    assert_eq!(needle.len(), replacement.len());
    let wal = format!("{}-wal", db.display());
    assert!(
        !Path::new(&wal).exists() || std::fs::metadata(&wal).unwrap().len() == 0,
        "WALが残っている"
    );
    let mut bytes = std::fs::read(db).unwrap();
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .expect("書き換える文字列が無い");
    bytes[at..at + needle.len()].copy_from_slice(replacement.as_bytes());
    std::fs::write(db, bytes).unwrap();
}

fn tampered() -> TestServer {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    create(&c, &expense("2026-10-01", 1100, "exp_comm", "w01"));
    let mut target = expense("2026-10-02", 2200, "exp_comm", "w01");
    target["description"] = json!("改ざんの的です");
    create(&c, &target);
    create(&c, &expense("2026-10-03", 3300, "exp_comm", "w01"));
    let (dir, db) = s.stop();
    corrupt(&db, "改ざんの的です", "改ざんの跡です");
    TestServer::on(dir, db, |_| {})
}

// AC-09.1: 起動時の検証に失敗したら読み取り専用。/status に最初の不正（連番と規則）が出る。
#[test]
fn tampered_database_is_read_only_with_the_first_failure() {
    let s = tampered();
    let c = s.login();
    let st = c.api("/status").json();
    assert_eq!(st["ledger"]["verified"], false);
    assert_eq!(st["ledger"]["error"]["seq"], 2);
    assert_eq!(st["ledger"]["error"]["kind"], "bad_hash");
    assert_eq!(st["ledger"]["events_loaded"], 1);
    assert_eq!(st["ledger"]["events_in_db"], 3);
    assert_eq!(st["ledger"]["events"], 1);
    // 読み取りは、壊れた連番の手前までの台帳で答える。
    assert_eq!(c.api("/entries").json()["total"], 1);
    assert_eq!(
        c.api("/reports/trial-balance?year=2026").json()["total_debit"],
        1100
    );
    let verify = c.post("/verify", &json!({ "noted_hash": null }));
    assert_eq!(verify.status, 200);
    assert_eq!(verify.json()["ok"], false);
    assert_eq!(verify.json()["error"]["seq"], 2);
    // 書き込みでない要求（プレビュー）は通る。
    assert_eq!(
        c.post(
            "/entries/preview",
            &expense("2026-10-05", 100, "exp_comm", "w01")
        )
        .status,
        200
    );
}

// api-common: 読み取り専用のときは、書き込みの要求をすべて409 read_only で断る（入力の検査より先に）。
#[test]
fn tampered_database_refuses_every_write() {
    let s = tampered();
    let c = s.login();
    let writes = [
        c.post("/entries", &expense("2026-10-05", 100, "exp_comm", "w01")),
        c.post("/entries", &json!({ "kind": "nonsense" })),
        c.put("/settings", &json!({})),
        c.post("/accounts", &json!({ "name": "x", "kind": "expense" })),
        c.patch("/accounts/exp_comm", &json!({ "name": "x" })),
        c.post("/partners", &json!({ "name": "サンプル商事" })),
        c.post("/templates", &json!({ "name": "x", "body": {} })),
        c.bare("DELETE", "/templates/1"),
        c.put("/opening", &json!({ "balances": [] })),
        c.put("/entries/e_x", &json!({})),
        c.post("/entries/e_x/void", &json!({})),
        c.upload("", &fixture("utf8.csv")),
        c.post("/imports/commit", &json!({})),
        c.patch("/drafts/1", &json!({})),
        c.bare("POST", "/drafts/1/ignore"),
        c.bare("POST", "/drafts/1/restore"),
        c.post("/drafts/confirm", &json!({ "ids": [1] })),
    ];
    for (i, r) in writes.iter().enumerate() {
        assert_eq!(
            (r.status, r.code().as_str()),
            (409, "read_only"),
            "書き込み{i}: {}",
            r.text()
        );
    }
    assert_eq!(c.api("/status").json()["ledger"]["events_in_db"], 3);
}
