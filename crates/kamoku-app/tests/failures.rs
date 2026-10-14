// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 保存の途中の失敗（コードレビューの指摘 B6）。失敗は、同じ帳簿を別の接続で開いて起こす。

mod common;
use common::*;
use serde_json::json;

// B6: 列の対応の保存に失敗したら、取り込みも起きない。列の対応を先に保存し、そのあとで下書きを入れる
// （列の対応だけが残っても害は無い）。失敗のあと、保存しない取り込みは通る。
#[test]
fn a_failed_profile_save_imports_nothing() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let other = rusqlite::Connection::open(&s.db).unwrap();
    other
        .execute_batch(
            "CREATE TRIGGER fail_profile BEFORE INSERT ON import_profiles
             BEGIN SELECT RAISE(ABORT, '試験で起こす失敗'); END;",
        )
        .unwrap();
    let up = c
        .upload(
            "",
            "日付,摘要,出金,入金\r\n2026/10/05,デモ店,,500\r\n".as_bytes(),
        )
        .json();
    let mut job = json!({ "upload_id": up["upload_id"], "wallet": "w02", "mapping": up["guess"],
                          "save_profile": true });
    let failed = c.post("/imports/commit", &job);
    assert_eq!(failed.status, 500, "{}", failed.text());
    assert_eq!(c.api("/drafts").json()["total"], 0);
    assert_eq!(c.api("/imports/batches").json()["total"], 0);
    job["save_profile"] = json!(false);
    let done = c.post("/imports/commit", &job);
    assert_eq!(done.status, 200, "{}", done.text());
    assert_eq!(c.api("/drafts").json()["total"], 1);
}
