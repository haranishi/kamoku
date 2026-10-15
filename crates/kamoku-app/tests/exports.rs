// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 書き出し（`docs/spec/api-reports.md` の「書き出し」）。

// 金額は、仕様の例と同じく区切りなしで書く。
#![allow(clippy::unreadable_literal)]

mod common;
use common::*;
use serde_json::json;

fn utf8_body(r: &Resp) -> String {
    assert_eq!(r.status, 200, "{}", r.text());
    assert!(r.body.starts_with(&[0xEF, 0xBB, 0xBF]), "BOMが無い");
    String::from_utf8(r.body[3..].to_vec()).unwrap()
}

// AC-13.1・AC-09.3: 仕訳帳のCSVは1行が1明細で、仕訳No・版・日付・科目・貸借・金額・税区分・税額・摘要・取引先を含む。
// AC-08.6: BOMつきUTF-8が既定、Shift_JISも選べる。改行はCRLF。既定は有効な仕訳だけ。
#[test]
fn journal_csv_rows_encodings_and_states() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mut phone = expense("2026-10-05", 11000, "exp_comm", "w03");
    phone["business_pct"] = json!(70);
    phone["partner"] = json!("サンプル通信");
    create(&c, &phone);
    let voided = create(&c, &expense("2026-10-06", 500, "exp_supplies", "w01"));
    c.post(
        &format!("/entries/{}/void", voided["id"].as_str().unwrap()),
        &json!({ "reason": "二重", "expected_revision": 1 }),
    );
    let resp = c.api("/export/journal.csv");
    assert_eq!(resp.header("Content-Type"), Some("text/csv; charset=utf-8"));
    // 期間を決めなければ、書き出した日（JST）が名前に入る。
    let disposition = resp.header("Content-Disposition").unwrap();
    assert!(
        disposition.starts_with("attachment; filename=\"kamoku-journal-")
            && disposition.contains(".csv"),
        "{disposition}"
    );
    let stamp = disposition
        .split("kamoku-journal-")
        .nth(1)
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    assert_eq!(stamp.len(), 8, "{disposition}");
    assert!(stamp.chars().all(|ch| ch.is_ascii_digit()), "{disposition}");
    assert_eq!(resp.header("Cache-Control"), Some("no-store"));
    let text = utf8_body(&resp);
    let lines: Vec<&str> = text.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines[0],
        "仕訳No,版,状態,日付,摘要,取引先,行,貸借,科目コード,科目名,金額,税区分,税額,行メモ"
    );
    assert_eq!(
        lines.len(),
        4,
        "見出し＋1仕訳の3明細（取消済みは既定で出さない）"
    );
    assert_eq!(
        lines[1],
        "1,1,有効,2026-10-05,デモの支出,サンプル通信,1,借方,exp_comm,通信費,7700,P10,700,"
    );
    assert!(lines[2].contains(",借方,owner_draw,事業主貸,3300,,0,家事按分 30%"));
    assert!(!text.replace("\r\n", "").contains('\n'));
    let all = utf8_body(&c.api("/export/journal.csv?state=all"));
    assert!(all.contains(",取消,"));
    let sjis = c.api("/export/journal.csv?encoding=sjis");
    assert_eq!(
        sjis.header("Content-Type"),
        Some("text/csv; charset=Shift_JIS")
    );
    let (decoded, _, errors) = encoding_rs::SHIFT_JIS.decode(&sjis.body);
    assert!(!errors);
    assert_eq!(decoded, text);
    assert!(sjis.header("X-Kamoku-Warning").is_none());
    let period = utf8_body(&c.api("/export/journal.csv?from=2026-10-06&to=2026-10-31&state=all"));
    assert_eq!(period.split("\r\n").filter(|l| !l.is_empty()).count(), 3);
    assert_eq!(c.api("/export/journal.csv?encoding=latin1").status, 400);
}

// nfr 安全: 文字列の欄の先頭が = + - @、タブ、CR なら ' を足す。数値の欄は対象外。
#[test]
fn csv_guards_formulas_in_text_cells() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let minus = c
        .post(
            "/accounts",
            &json!({ "name": "-マイナス費", "kind": "expense" }),
        )
        .json();
    let code = minus["code"].as_str().unwrap().to_owned();
    let entry = json!({ "kind": "journal", "date": "2026-10-05", "description": "=HYPERLINK(\"http://example.invalid\")",
        "partner": "+81 デモ", "lines": [
            { "account": code, "side": "debit", "amount": 1000, "memo": "@SUM(1)" },
            { "account": "w01", "side": "credit", "amount": 600, "memo": "\tタブ" },
            { "account": "w02", "side": "credit", "amount": 400, "memo": "\r改行" } ] });
    create(&c, &entry);
    let text = utf8_body(&c.api("/export/journal.csv"));
    assert!(
        text.contains(",\"'=HYPERLINK(\"\"http://example.invalid\"\")\","),
        "{text}"
    );
    assert!(text.contains(",'+81 デモ,"));
    assert!(text.contains(",'-マイナス費,"));
    assert!(text.contains(",'@SUM(1)\r\n"));
    assert!(text.contains(",'\tタブ\r\n"));
    assert!(text.contains(",\"'\r改行\"\r\n"));
    assert!(!text.contains(",'1000,"), "数値の欄には足さない");
    let tb = utf8_body(&c.api("/export/trial-balance.csv?year=2026"));
    assert!(tb.contains("'-マイナス費"));
    assert!(
        tb.contains(",-600\r\n") || tb.contains(",-600,") || tb.contains("-600"),
        "負の数はそのまま: {tb}"
    );
}

// Shift_JIS で表せない文字は ? にして、X-Kamoku-Warning で知らせる。
#[test]
fn sjis_marks_unmappable_characters() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let mut e = expense("2026-10-05", 100, "exp_comm", "w01");
    e["description"] = json!("絵文字😀と①");
    create(&c, &e);
    let r = c.api("/export/journal.csv?encoding=sjis");
    // 摘要は明細の行ごとに出るので、2行ぶんで2文字。
    assert_eq!(r.header("X-Kamoku-Warning"), Some("unmappable=2"));
    let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&r.body);
    assert!(decoded.contains("絵文字?と①"));
}

// AC-08.6: どの帳票もCSVで出せ、数字は画面の帳票と同じ。
#[test]
fn report_csvs_match_report_numbers() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    create(
        &c,
        &json!({ "kind": "income", "date": "2026-05-10", "amount": 330000, "account": "sales", "wallet": "w02", "tax_code": "S10" }),
    );
    create(&c, &expense("2026-06-01", 33000, "exp_outsource", "w02"));
    let tb = c.api("/reports/trial-balance?year=2026").json();
    let tb_csv = utf8_body(&c.api("/export/trial-balance.csv?year=2026"));
    assert!(tb_csv.starts_with("コード,科目名,分類,期首残高,借方,貸方,期末残高\r\n"));
    assert!(tb_csv.contains(&format!(
        "合計,,,,{},{},",
        tb["total_debit"], tb["total_credit"]
    )));
    let is_csv = utf8_body(&c.api("/export/income-statement.csv?year=2026&by=month"));
    assert!(is_csv.starts_with("区分,コード,科目名,1月,2月,"));
    assert!(
        is_csv.contains("当期純利益,,,0,0,0,0,330000,-33000,0,0,0,0,0,0,297000\r\n"),
        "{is_csv}"
    );
    let bs_csv = utf8_body(&c.api("/export/balance-sheet.csv?as_of=2026-12-31"));
    assert!(
        bs_csv.contains("資本,1,net_income_current,当期純利益,297000,297000\r\n"),
        "{bs_csv}"
    );
    let ledger_csv = utf8_body(&c.api("/export/ledger.csv?account=w02&year=2026"));
    assert!(
        ledger_csv.contains(",,期末残高,,,,297000\r\n"),
        "{ledger_csv}"
    );
    let tax_csv = utf8_body(&c.api("/export/tax-summary.csv?year=2026"));
    assert!(tax_csv.contains("売上税額,,,30000\r\n"));
    assert!(tax_csv.contains("参考値です。"));
    assert_eq!(c.api("/export/ledger.csv?year=2026").status, 400);
}

// AC-09.3: 全件のJSON（設定・科目・取引先・全イベント）と、開ける複製（同じ head_hash）。
#[test]
fn all_json_and_backup_db_match_the_ledger() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let first = create(&c, &expense("2026-10-05", 1100, "exp_comm", "w01"));
    let id = first["id"].as_str().unwrap();
    c.put(&format!("/entries/{id}"), &json!({ "kind": "expense", "date": "2026-10-05", "amount": 2200, "account": "exp_comm", "wallet": "w01", "reason": "訂正", "expected_revision": 1 }));
    let head = c.api("/status").json()["ledger"]["head_hash"].clone();
    let all = c.api("/export/all.json");
    assert_eq!(
        all.header("Content-Type"),
        Some("application/json; charset=utf-8")
    );
    assert!(
        all.header("Content-Disposition")
            .unwrap()
            .contains("kamoku-all-")
    );
    let all = all.json();
    assert_eq!(all["schema"], 1);
    assert_eq!(all["head_hash"], head);
    assert_eq!(all["settings"]["business_name"], "サンプル事務所");
    assert!(all["accounts"].as_array().unwrap().len() > 50);
    let events = all["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["kind"], "revised");
    assert_eq!(events[1]["prev_hash"], events[0]["hash"]);
    assert_eq!(events[1]["entry"]["lines"][0]["amount"], 2200);
    assert_eq!(events[0]["entry"]["meta"]["kind"], "expense");
    let backup = c.api("/export/backup.db");
    assert_eq!(backup.status, 200);
    assert_eq!(
        backup.header("Content-Type"),
        Some("application/vnd.sqlite3")
    );
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("copy.db");
    std::fs::write(&copy, &backup.body).unwrap();
    let (store, report) =
        kamoku_store::Store::open(&copy, kamoku_store::OpenOptions::default()).unwrap();
    assert!(report.verification.is_ok());
    assert_eq!(json!(store.ledger().head_hash().to_hex()), head);
    // 書き出しの一時ファイルは残さない。
    let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("kamoku-export-")
        })
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}
