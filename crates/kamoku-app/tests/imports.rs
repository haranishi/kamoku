// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 取込と未確認（`docs/spec/api-inbox.md`）。素材は kamoku-import の合成のCSV。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::{Value, json};

fn upload(c: &Client, name: &str, query: &str) -> Value {
    let r = c.upload(query, &fixture(name));
    assert_eq!(r.status, 200, "{name}: {}", r.text());
    r.json()
}

fn job(up: &Value, wallet: &str) -> Value {
    json!({ "upload_id": up["upload_id"], "wallet": wallet, "mapping": up["guess"] })
}

fn ledger_total(c: &Client) -> i64 {
    c.api("/entries?state=all").json()["total"]
        .as_i64()
        .unwrap()
}

// AC-04.1・AC-04.3: UTF-8 と Shift_JIS を判定して読み、列の対応の初期値を返す。
#[test]
fn upload_detects_encoding_and_guesses_columns() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let utf8 = upload(&c, "utf8.csv", "?name=meisai.csv&wallet=w02");
    assert_eq!(utf8["encoding"], "utf-8");
    assert_eq!(utf8["delimiter"], ",");
    assert_eq!(utf8["rows"], 5);
    assert_eq!(
        utf8["header"],
        json!(["日付", "摘要", "出金", "入金", "残高"])
    );
    assert_eq!(utf8["sample"][0][1], "サンプルストア");
    assert_eq!(utf8["guess"]["amount"]["mode"], "split");
    assert_eq!(utf8["filename"], "meisai.csv");
    assert!(utf8["saved"].is_null());
    assert!(utf8["upload_id"].as_str().unwrap().starts_with("u_"));
    let sjis = upload(&c, "sjis.csv", "");
    assert_eq!(sjis["encoding"], "shift_jis");
    assert_eq!(sjis["sample"][0][1], "デモ商店①");
    let forced = upload(&c, "sjis.csv", "?encoding=utf-8");
    assert_eq!(forced["encoding"], "utf-8");
    assert!(
        !forced["warnings"].as_array().unwrap().is_empty(),
        "読めないバイトの件数を知らせる"
    );
    let card = upload(&c, "card.csv", "");
    assert_eq!(card["guess"]["skip_rows"], 2);
    assert_eq!(card["guess"]["amount"]["mode"], "signed");
    assert_eq!(
        c.upload("?encoding=latin1", &fixture("utf8.csv")).status,
        400
    );
    let broken = c.upload("", &fixture("broken_quote.csv"));
    assert_eq!(
        (broken.status, broken.code().as_str()),
        (400, "invalid_csv")
    );
    assert_eq!(broken.json()["error"]["line"], 3);
    assert_eq!(c.upload("", b"").code(), "invalid_csv");
    assert_eq!(
        c.upload("?wallet=exp_comm", &fixture("utf8.csv")).code(),
        "unknown_account"
    );
}

// AC-04.4: 同じファイルを2回取り込んでも二重に入らない。AC-04.5: 同じ内容の本物の2件は2件とも入る。
// AC-04.8: 重複の行は理由つきで出て、選べば取り込める。AC-04.7: 確定するまで台帳に入らない。
#[test]
fn preview_commit_and_duplicates() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let first = upload(&c, "utf8.csv", "");
    let preview = c.post("/imports/preview", &job(&first, "w02")).json();
    assert_eq!(
        preview["counts"],
        json!({ "new": 4, "duplicate": 0, "invalid": 0 })
    );
    assert_eq!(preview["truncated"], false);
    assert_eq!(preview["rows"][0]["description"], "サンプルストア");
    assert_eq!(preview["rows"][0]["direction"], "out");
    assert_eq!(preview["rows"][2]["direction"], "in");
    let mut commit = job(&first, "w02");
    commit["save_profile"] = json!(true);
    let done = c.post("/imports/commit", &commit);
    assert_eq!(done.status, 200, "{}", done.text());
    let done = done.json();
    assert_eq!(
        (done["created"].as_i64(), done["duplicates"].as_i64()),
        (Some(4), Some(0))
    );
    assert!(done["batch_id"].as_i64().unwrap() > 0);
    assert_eq!(ledger_total(&c), 0, "AC-04.7: 確定前は台帳に入らない");
    let again = upload(&c, "utf8.csv", "?wallet=w02");
    assert_eq!(
        again["saved"], first["guess"],
        "AC-04.6: 口座ごとに列の対応を覚える"
    );
    let preview = c.post("/imports/preview", &job(&again, "w02")).json();
    assert_eq!(
        preview["counts"],
        json!({ "new": 0, "duplicate": 4, "invalid": 0 })
    );
    assert_eq!(preview["rows"][3]["status"], "duplicate");
    assert_eq!(preview["rows"][3]["forceable"], true);
    assert!(
        preview["rows"][3]["reason"]
            .as_str()
            .unwrap()
            .contains("取り込")
    );
    let mut force = job(&again, "w02");
    force["force_lines"] = json!([5]);
    let forced = c.post("/imports/commit", &force).json();
    assert_eq!(
        (forced["created"].as_i64(), forced["duplicates"].as_i64()),
        (Some(1), Some(3))
    );
    let drafts = c.api("/drafts?limit=500").json();
    assert_eq!(drafts["total"], 5);
    assert_eq!(drafts["counts"]["draft"], 5);
    let same: Vec<&Value> = drafts["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["amount"] == 55000)
        .collect();
    assert_eq!(same.len(), 3, "本物の2件と、選んで取り込んだ1件");
    assert_eq!(c.api("/imports/profile?wallet=w02").json(), first["guess"]);
    assert!(c.api("/imports/profile?wallet=w01").json().is_null());
    let batches = c.api("/imports/batches").json();
    assert_eq!(batches["total"], 2);
}

// 取込の確定: 選んだ重複の行が、同じ指紋の新規の行の出現番号を奪わない。
// 新規の行を先に入れるのは保存層の役目で、ここでは画面からの通しで確かめる。
#[test]
fn forced_duplicates_do_not_swallow_new_rows() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let one = "日付,摘要,出金,入金\r\n2026/10/05,デモ店,,500\r\n";
    let two = "日付,摘要,出金,入金\r\n2026/10/05,デモ店,,500\r\n2026/10/05,デモ店,,500\r\n";
    let a = c.upload("", one.as_bytes()).json();
    c.post("/imports/commit", &job(&a, "w02"));
    let b = c.upload("", two.as_bytes()).json();
    let preview = c.post("/imports/preview", &job(&b, "w02")).json();
    assert_eq!(
        preview["counts"],
        json!({ "new": 1, "duplicate": 1, "invalid": 0 })
    );
    let mut force = job(&b, "w02");
    force["force_lines"] = json!([2]);
    let done = c.post("/imports/commit", &force).json();
    assert_eq!(done["created"], 2, "{done}");
    assert_eq!(c.api("/drafts").json()["total"], 3);
}

fn draft_ids(c: &Client) -> Vec<i64> {
    c.api("/drafts?limit=500").json()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_i64().unwrap())
        .collect()
}

// AC-05.4: 科目が空の行は確定せず、理由を行ごとに返す。残りは確定する。
// AC-05.5: 出金なら口座が貸方、入金なら借方。確定した行は仕訳帳と試算表に出る。
#[test]
fn confirm_drafts_row_by_row() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let up = upload(&c, "utf8.csv", "");
    c.post("/imports/commit", &job(&up, "w02"));
    let ids = draft_ids(&c);
    let out_id = ids[0];
    let in_id = ids[2];
    let patched = c.patch(&format!("/drafts/{out_id}"), &json!({ "account": "exp_supplies", "tax_code": "P10", "business_pct": 50, "date": "2026-10-02", "partner": "サンプルストア" }));
    assert_eq!(patched.status, 200, "{}", patched.text());
    assert_eq!(patched.json()["wallet_name"], "サンプル銀行");
    assert_eq!(patched.json()["date"], "2026-10-02");
    c.patch(
        &format!("/drafts/{in_id}"),
        &json!({ "account": "sales", "tax_code": "S10" }),
    );
    assert_eq!(
        c.patch(&format!("/drafts/{in_id}"), &json!({ "tax_code": "Q1" }))
            .code(),
        "bad_tax_code"
    );
    assert_eq!(
        c.patch(
            &format!("/drafts/{in_id}"),
            &json!({ "account": "nothing" })
        )
        .code(),
        "unknown_account"
    );
    let tb_before = c.api("/reports/trial-balance?year=2026").json();
    assert_eq!(tb_before["total_debit"], 0, "確定前は試算表に出ない");
    let result = c.post(
        "/drafts/confirm",
        &json!({ "ids": [out_id, in_id, ids[1], 99_999] }),
    );
    assert_eq!(result.status, 200, "{}", result.text());
    let result = result.json();
    let confirmed = result["confirmed"].as_array().unwrap();
    assert_eq!(confirmed.len(), 2);
    let failed = result["failed"].as_array().unwrap();
    assert_eq!(failed[0]["id"], ids[1]);
    assert_eq!(failed[0]["error"]["code"], "missing_account");
    assert_eq!(failed[1]["error"]["code"], "not_found");
    let out_entry = c
        .api(&format!(
            "/entries/{}",
            confirmed[0]["entry_id"].as_str().unwrap()
        ))
        .json();
    let wallet_line = out_entry["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["account"] == "w02")
        .unwrap()
        .clone();
    assert_eq!(wallet_line["side"], "credit");
    assert_eq!(out_entry["meta"]["source"], "import");
    assert_eq!(out_entry["meta"]["draft"], out_id.to_string());
    assert_eq!(out_entry["date"], "2026-10-02");
    assert_eq!(
        out_entry["lines"].as_array().unwrap().len(),
        3,
        "割合50%なので事業主貸の行がある"
    );
    let in_entry = c
        .api(&format!(
            "/entries/{}",
            confirmed[1]["entry_id"].as_str().unwrap()
        ))
        .json();
    assert_eq!(in_entry["lines"][0]["account"], "w02");
    assert_eq!(in_entry["lines"][0]["side"], "debit");
    let tb = c.api("/reports/trial-balance?year=2026").json();
    assert_eq!(tb["total_debit"], 1650 + 55000);
    assert_eq!(c.api("/entries?source=import").json()["total"], 2);
    let twice = c
        .post("/drafts/confirm", &json!({ "ids": [out_id] }))
        .json();
    assert_eq!(twice["failed"][0]["error"]["code"], "already_confirmed");
    assert_eq!(
        c.patch(&format!("/drafts/{out_id}"), &json!({ "memo": "x" }))
            .code(),
        "already_confirmed"
    );
    assert_eq!(
        c.bare("POST", &format!("/drafts/{out_id}/ignore")).code(),
        "already_confirmed"
    );
    assert_eq!(c.api("/drafts?status=confirmed").json()["total"], 2);
}

// AC-05.4 AC-05.5 AC-07.2: 選んだ行だけを確定し、状態の違い・重複・不存在が混ざっても
// 送った順に結果を返す。ホームの合計には未確認の出金だけを含める。
#[test]
fn selected_draft_confirmation_preserves_order_and_dashboard_status_rules() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let csv = "日付,摘要,出金,入金\n\
        2026/10/01,未確認の出金,1,\n\
        2026/10/01,未確認の入金,,2\n\
        2026/10/01,無視する出金,3,\n\
        2026/10/01,無視する入金,,4\n\
        2026/10/01,確定する出金,5,\n\
        2026/10/01,確定する入金,,6\n";
    let uploaded = c.upload("", csv.as_bytes()).json();
    let imported = c.post("/imports/commit", &job(&uploaded, "w02"));
    assert_eq!(imported.status, 200, "{}", imported.text());
    assert_eq!(imported.json()["created"], 6);
    let ids = draft_ids(&c);
    for (id, account, tax) in [
        (ids[1], "sales", "S10"),
        (ids[4], "exp_comm", "P10"),
        (ids[5], "sales", "S10"),
    ] {
        let patched = c.patch(
            &format!("/drafts/{id}"),
            &json!({ "account": account, "tax_code": tax }),
        );
        assert_eq!(patched.status, 200, "{}", patched.text());
    }
    let confirmed = c
        .post("/drafts/confirm", &json!({ "ids": [ids[4], ids[5]] }))
        .json();
    assert_eq!(
        confirmed["confirmed"].as_array().unwrap().len(),
        2,
        "{confirmed}"
    );
    assert_eq!(confirmed["failed"], json!([]));
    for id in &ids[2..4] {
        assert_eq!(c.bare("POST", &format!("/drafts/{id}/ignore")).status, 200);
    }
    let dashboard = c.api("/reports/dashboard?month=2026-10").json();
    assert_eq!(
        dashboard["next_actions"],
        json!({ "inbox_draft": 2, "inbox_out_amount": 1 })
    );
    assert_eq!(
        c.post("/drafts/confirm", &json!({ "ids": [] })).json(),
        json!({ "confirmed": [], "failed": [] })
    );
    let selected = [ids[1], 99_999, ids[2], ids[4], ids[0], ids[1], ids[3], -1];
    let result = c.post("/drafts/confirm", &json!({ "ids": selected }));
    assert_eq!(result.status, 200, "{}", result.text());
    let result = result.json();
    assert_eq!(result["confirmed"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(result["confirmed"][0]["id"], ids[1]);
    let failed: Vec<_> = result["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_i64().unwrap(),
                row["error"]["code"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        failed,
        [
            (99_999, "not_found"),
            (ids[2], "bad_request"),
            (ids[4], "already_confirmed"),
            (ids[0], "missing_account"),
            (ids[3], "bad_request"),
            (-1, "not_found")
        ]
    );
    assert_eq!(
        c.api("/reports/dashboard?month=2026-10").json()["next_actions"],
        json!({ "inbox_draft": 1, "inbox_out_amount": 1 })
    );
}

// api-inbox の状態の移り方: 取り込まない→戻す。確定済みは、仕訳を取り消したときだけ戻せる。
// 取込から確定した仕訳を直しても、出どころ（source と draft）は残る。
#[test]
fn ignore_restore_and_voided_confirmations() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let up = upload(&c, "utf8.csv", "");
    c.post("/imports/commit", &job(&up, "w02"));
    let ids = draft_ids(&c);
    let ignored = c.bare("POST", &format!("/drafts/{}/ignore", ids[1]));
    assert_eq!(ignored.status, 200);
    assert_eq!(ignored.json()["status"], "ignored");
    assert_eq!(c.api("/drafts").json()["total"], 3);
    assert_eq!(c.api("/drafts?status=ignored").json()["total"], 1);
    assert_eq!(
        c.api("/status").json()["inbox"],
        json!({ "draft": 3, "ignored": 1 })
    );
    assert_eq!(
        c.bare("POST", &format!("/drafts/{}/restore", ids[1]))
            .json()["status"],
        "draft"
    );
    c.patch(
        &format!("/drafts/{}", ids[0]),
        &json!({ "account": "exp_supplies", "tax_code": "P10" }),
    );
    let done = c
        .post("/drafts/confirm", &json!({ "ids": [ids[0]] }))
        .json();
    let entry_id = done["confirmed"][0]["entry_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        c.bare("POST", &format!("/drafts/{}/restore", ids[0]))
            .code(),
        "already_confirmed"
    );
    let fix = json!({ "kind": "expense", "date": "2026-10-01", "amount": 1650, "account": "exp_meeting", "wallet": "w02",
        "tax_code": "P10", "description": "サンプルストア", "reason": "科目の訂正", "expected_revision": 1 });
    let revised = c.put(&format!("/entries/{entry_id}"), &fix);
    assert_eq!(revised.status, 200, "{}", revised.text());
    assert_eq!(revised.json()["meta"]["source"], "import");
    assert_eq!(revised.json()["meta"]["draft"], ids[0].to_string());
    c.post(
        &format!("/entries/{entry_id}/void"),
        &json!({ "reason": "取込の誤り", "expected_revision": 2 }),
    );
    let restored = c.bare("POST", &format!("/drafts/{}/restore", ids[0]));
    assert_eq!(restored.status, 200, "{}", restored.text());
    let back = c.api("/drafts?limit=500").json();
    let row = back["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == ids[0])
        .unwrap()
        .clone();
    assert_eq!(row["status"], "draft");
    assert!(row["entry_id"].is_null());
}

// api-inbox: 期限切れや知らない取込、列の対応の誤り。
#[test]
fn import_jobs_are_checked() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let missing = c.post(
        "/imports/preview",
        &json!({ "upload_id": "u_nothing", "wallet": "w02", "mapping": {} }),
    );
    assert_eq!(missing.status, 404);
    let up = upload(&c, "utf8.csv", "");
    let mut bad = job(&up, "w02");
    bad["mapping"]["date_col"] = json!(40);
    assert_eq!(c.post("/imports/preview", &bad).status, 400);
    bad["mapping"] = json!("x");
    assert_eq!(c.post("/imports/preview", &bad).status, 400);
    let not_wallet = job(&up, "exp_comm");
    assert_eq!(
        c.post("/imports/preview", &not_wallet).code(),
        "unknown_account"
    );
    let mut lines = job(&up, "w02");
    lines["force_lines"] = json!([0]);
    assert_eq!(c.post("/imports/commit", &lines).status, 400);
}
