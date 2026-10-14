// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 取込の行の判定（新規・重複・読めない）と、プレビュー・確定（`/imports/preview` `/imports/commit`）。

use super::body::{Fields, object};
use super::imports::{UPLOAD_TTL, Upload, Uploads, wallet_code};
use super::{ApiError, Call, Reply, ok, to_value};
use crate::state::State;
use kamoku_books::format_date;
use kamoku_import::{
    Direction, Mapping, ParsedRow, RowOutcome, Table, apply_mapping, number_occurrences,
};
use kamoku_store::{ImportSummary, NewBatch, NewDraft, Store};
use serde_json::{Value, json};
use std::collections::HashSet;

const PREVIEW_ROWS: usize = 500;
const DUPLICATE_REASON: &str = "同じ口座・日付・金額・摘要（取引番号があれば、その番号）の明細が、すでに取り込んであります。別の取引なら、この行を選んで取り込めます。";

/// 1行の判定。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    New,
    Duplicate,
    Invalid,
}

struct Judged {
    line: usize,
    status: Status,
    reason: Option<String>,
    row: Option<(ParsedRow, String, u32)>,
}

// 行ごとに読み、読めた行に（指紋, 出現番号）を振って、すでにある下書きと突き合わせる。
fn judge(
    store: &Store,
    table: &Table,
    wallet: &str,
    mapping: &Mapping,
) -> Result<Vec<Judged>, ApiError> {
    let outcomes = apply_mapping(table, mapping);
    let parsed: Vec<ParsedRow> = outcomes
        .iter()
        .filter_map(|o| match o {
            RowOutcome::Parsed(p) => Some(p.clone()),
            RowOutcome::Invalid { .. } => None,
        })
        .collect();
    let keys = number_occurrences(wallet, &parsed);
    let known = store.known_fingerprints(&keys)?;
    let mut keyed = parsed.into_iter().zip(keys);
    let mut out = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        match outcome {
            RowOutcome::Parsed(_) => {
                let Some((row, (print, occurrence))) = keyed.next() else {
                    return Err(ApiError::internal("取込の行と指紋の数が合いません"));
                };
                let duplicate = known.existing.contains(&(print.clone(), occurrence));
                out.push(Judged {
                    line: row.line,
                    status: if duplicate {
                        Status::Duplicate
                    } else {
                        Status::New
                    },
                    reason: duplicate.then(|| DUPLICATE_REASON.to_owned()),
                    row: Some((row, print, occurrence)),
                });
            }
            RowOutcome::Invalid { line, reason } => out.push(Judged {
                line,
                status: Status::Invalid,
                reason: Some(reason),
                row: None,
            }),
        }
    }
    Ok(out)
}

fn check_mapping(table: &Table, mapping: &Mapping) -> Result<(), ApiError> {
    let width = table.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    let mut cols = vec![mapping.date_col];
    cols.extend(&mapping.desc_cols);
    cols.extend(mapping.id_col);
    match mapping.amount {
        kamoku_import::AmountMapping::Split { out_col, in_col } => {
            if out_col == in_col {
                return Err(ApiError::bad_request("入金と出金に、同じ列は選べません。")
                    .with_field("mapping"));
            }
            cols.extend([out_col, in_col]);
        }
        kamoku_import::AmountMapping::Signed { col, .. } => cols.push(col),
    }
    if cols.iter().any(|c| *c >= width) {
        return Err(ApiError::bad_request(
            "列の番号が、ファイルの列の数を超えています。列の対応を選び直してください。",
        )
        .with_field("mapping"));
    }
    Ok(())
}

fn read_mapping(f: &Fields<'_>) -> Result<Mapping, ApiError> {
    let value = f
        .value("mapping")
        .ok_or_else(|| f.missing("mapping", "列の対応"))?;
    serde_json::from_value(value.clone()).map_err(|_| {
        ApiError::bad_request(
            "列の対応（mapping）の形が正しくありません。日付と金額の列を選び直してください。",
        )
        .with_field("mapping")
    })
}

// プレビューと確定の両方が使う、本文の読み取り（`upload_id` `wallet` `mapping`）。
fn job<'a>(
    uploads: &'a Uploads,
    store: &Store,
    f: &Fields<'_>,
) -> Result<(&'a Upload, String, Mapping), ApiError> {
    let id = f.req_str("upload_id", "取り込むファイル")?;
    let upload = uploads
        .0
        .get(&id)
        .filter(|u| u.at.elapsed() < UPLOAD_TTL)
        .ok_or_else(|| {
            ApiError::not_found(
                "取り込むファイルの期限が切れました（30分）。もう一度ファイルを選んでください。",
            )
            .with_field("upload_id")
        })?;
    let wallet = wallet_code(store, &f.req_str("wallet", "口座")?, "wallet")?;
    let mapping = read_mapping(f)?;
    check_mapping(&upload.table, &mapping)?;
    Ok((upload, wallet, mapping))
}

fn row_json(j: &Judged) -> Value {
    let row = j.row.as_ref().map(|(r, _, _)| r);
    json!({
        "line": j.line,
        "date": row.map(|r| format_date(r.date)),
        "description": row.map(|r| r.description.clone()),
        "direction": row.map(|r| if r.direction == Direction::In { "in" } else { "out" }),
        "amount": row.map(|r| r.amount),
        "status": match j.status { Status::New => "new", Status::Duplicate => "duplicate", Status::Invalid => "invalid" },
        "reason": j.reason,
        "forceable": j.status == Status::Duplicate,
    })
}

/// `POST /imports/preview`。本文 `{ upload_id, wallet, mapping }`。行は先頭の500行まで。
pub(crate) fn import_preview(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let (upload, wallet, mapping) = job(&state.uploads, &state.store, &f)?;
    let judged = judge(&state.store, &upload.table, &wallet, &mapping)?;
    let count = |s: Status| judged.iter().filter(|j| j.status == s).count();
    let rows: Vec<Value> = judged.iter().take(PREVIEW_ROWS).map(row_json).collect();
    ok(json!({
        "rows": rows,
        "counts": { "new": count(Status::New), "duplicate": count(Status::Duplicate), "invalid": count(Status::Invalid) },
        "truncated": judged.len() > PREVIEW_ROWS,
    }))
}

/// 読んだ表を、下書きに入れます（API とデモが使う）。読めた行をファイルの順に渡す。
/// 重複の判定と、選んで取り込む重複の行の出現番号（その指紋の最大+1）は、保存の層が
/// 挿入の前の状態で決める（新規の行の番号を、選んだ重複の行が先に取ることは無い）。
pub(crate) fn commit_table(
    store: &mut Store,
    table: &Table,
    wallet: &str,
    mapping: &Mapping,
    filename: &str,
    force: HashSet<usize>,
    now: i64,
) -> Result<ImportSummary, ApiError> {
    let judged = judge(store, table, wallet, mapping)?;
    let invalid = judged
        .iter()
        .filter(|j| j.status == Status::Invalid)
        .count();
    let drafts: Vec<NewDraft> = judged
        .iter()
        .filter_map(|j| j.row.as_ref())
        .map(|(row, print, occurrence)| NewDraft {
            line: row.line,
            date: format_date(row.date),
            description: row.description.clone(),
            direction: if row.direction == Direction::In {
                "in"
            } else {
                "out"
            }
            .to_owned(),
            amount: row.amount,
            fingerprint: print.clone(),
            occurrence: *occurrence,
            external_id: row.id.clone(),
        })
        .collect();
    let batch = NewBatch {
        filename: filename.to_owned(),
        wallet: wallet.to_owned(),
        recorded_at: now,
        invalid: u64::try_from(invalid).map_err(ApiError::internal)?,
    };
    Ok(store.import_commit(batch, drafts, force)?)
}

/// `POST /imports/commit`。本文はプレビューと同じに `save_profile` と `force_lines` を足したもの。
pub(crate) fn import_commit(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let save_profile = f.opt_bool("save_profile")?.unwrap_or(false);
    let mut force = HashSet::new();
    for (i, v) in f
        .opt_array("force_lines")?
        .into_iter()
        .flatten()
        .enumerate()
    {
        let line = v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                ApiError::bad_request(
                    "force_lines は、ファイルの行番号（1以上の整数）の配列にしてください。",
                )
                .with_field(format!("force_lines[{i}]"))
            })?;
        force.insert(line);
    }
    let (upload, wallet, mapping) = job(&state.uploads, &state.store, &f)?;
    // 列の対応を先に保存する。下書きを入れたあとで対応の保存に失敗すると、失敗を返すのに取り込みが
    // 残ってしまう。逆の順なら、残るのは対応だけで、害が無い（保存の層は2つを別々に書く）。
    if save_profile {
        state
            .store
            .save_import_profile(&wallet, mapping.clone(), call.now)?;
    }
    let summary = commit_table(
        &mut state.store,
        &upload.table,
        &wallet,
        &mapping,
        &upload.filename,
        force,
        call.now,
    )?;
    ok(to_value(&summary)?)
}
