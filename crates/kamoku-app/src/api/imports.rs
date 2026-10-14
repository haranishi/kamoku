// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CSVの取込（`/imports/upload` `/imports/profile` `/imports/batches`）と、送られたファイルの一時置き場。
//! 読み取りと指紋は `kamoku-import`、保存と重複の判定は `kamoku-store`。行の判定と確定は `import_rows`。

use super::{ApiError, Call, Reply, ok, to_value};
use crate::ids;
use crate::state::State;
use kamoku_import::{EncodingChoice, Table, TextEncoding, decode, guess_mapping, parse_csv};
use kamoku_store::Store;
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub(super) const UPLOAD_TTL: Duration = Duration::from_secs(30 * 60);
// 10MBのCSVを読んだ表は、その数倍のメモリを使う。置いておく数を絞る。
const UPLOAD_MAX: usize = 4;
const SAMPLE_ROWS: usize = 20;

/// 送られたファイル（読んだ表）。
pub(crate) struct Upload {
    pub(super) at: Instant,
    pub(super) filename: String,
    pub(super) table: Table,
}

/// 取込の一時置き場。30分だけメモリに置く。
#[derive(Default)]
pub(crate) struct Uploads(pub(super) HashMap<String, Upload>);
impl Uploads {
    fn purge(&mut self, now: Instant) {
        self.0.retain(|_, u| now.duration_since(u.at) < UPLOAD_TTL);
    }
    fn insert(&mut self, id: String, upload: Upload) {
        self.purge(upload.at);
        while self.0.len() >= UPLOAD_MAX {
            let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, u)| u.at)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.0.remove(&oldest);
        }
        self.0.insert(id, upload);
    }
}

pub(super) fn wallet_code(store: &Store, code: &str, field: &str) -> Result<String, ApiError> {
    match store.chart().get(code) {
        Some(a) if a.is_wallet() && a.enabled => Ok(code.to_owned()),
        Some(a) if a.is_wallet() => Err(ApiError::new(
            422,
            "disabled_account",
            format!(
                "「{}」は無効にしてあります。口座の画面で有効に戻してください。",
                a.name
            ),
        )
        .with_field(field)),
        _ => Err(
            ApiError::new(422, "unknown_account", "取り込む口座を選んでください。")
                .with_field(field),
        ),
    }
}

fn clean_filename(name: Option<&str>) -> String {
    let base = name
        .unwrap_or_default()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect::<String>();
    if base.trim().is_empty() {
        "meisai.csv".to_owned()
    } else {
        base
    }
}

/// `POST /imports/upload?name=&encoding=auto|utf-8|shift_jis&wallet=`。本文はCSVのバイト列。
pub(crate) fn import_upload(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let choice = match call.query.get("encoding").unwrap_or("auto") {
        "auto" => EncodingChoice::Auto,
        "utf-8" => EncodingChoice::Utf8,
        "shift_jis" => EncodingChoice::ShiftJis,
        _ => {
            return Err(ApiError::bad_request(
                "encoding は auto・utf-8・shift_jis のどれかにしてください。",
            )
            .with_field("encoding"));
        }
    };
    let wallet = call
        .query
        .get("wallet")
        .map(|w| wallet_code(&state.store, w, "wallet"))
        .transpose()?;
    if call.body.is_empty() {
        return Err(ApiError::new(
            400,
            "invalid_csv",
            "ファイルが空です。明細のCSVを選んでください。",
        ));
    }
    let decoded = decode(call.body, choice)?;
    let table = parse_csv(&decoded.text)?;
    let guess = guess_mapping(&table);
    let skip = guess.as_ref().map_or(0, |g| g.skip_rows);
    let header = skip
        .checked_sub(1)
        .and_then(|i| table.rows.get(i))
        .map(|r| r.cells.clone());
    let sample: Vec<&Vec<String>> = table
        .rows
        .iter()
        .skip(skip)
        .take(SAMPLE_ROWS)
        .map(|r| &r.cells)
        .collect();
    let mut warnings = Vec::new();
    if decoded.malformed > 0 {
        warnings.push(format!(
            "正しく読めない文字が{}か所ありました（{}として読みました）。文字コードを選び直すと直ることがあります。",
            decoded.malformed,
            encoding_label(decoded.encoding)
        ));
    }
    let saved = match &wallet {
        Some(w) => state.store.import_profile(w)?,
        None => None,
    };
    let upload_id = format!("u_{}", ids::random_hex(12).map_err(ApiError::internal)?);
    let filename = clean_filename(call.query.get("name"));
    let body = json!({
        "upload_id": upload_id,
        "filename": filename,
        "encoding": decoded.encoding.as_str(),
        "delimiter": to_value(&table.delimiter)?,
        "rows": table.rows.len(),
        "header": header,
        "sample": sample,
        "guess": to_value(&guess)?,
        "warnings": warnings,
        "saved": to_value(&saved)?,
    });
    state.uploads.insert(
        upload_id,
        Upload {
            at: Instant::now(),
            filename,
            table,
        },
    );
    ok(body)
}

const fn encoding_label(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Utf8 => "UTF-8",
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => "UTF-16",
        TextEncoding::ShiftJis => "Shift_JIS",
    }
}

/// `GET /imports/profile?wallet=` → 覚えさせた列の対応か `null`。
pub(crate) fn import_profile(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let wallet = call
        .query
        .get("wallet")
        .ok_or_else(|| ApiError::bad_request("wallet を指定してください。").with_field("wallet"))?;
    ok(to_value(&state.store.import_profile(wallet)?)?)
}

/// `GET /imports/batches?limit=` → 取込の記録（新しい順）。
pub(crate) fn import_batches(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let (limit, offset) = call.query.page()?;
    // 保存の層は新しい順の上限つきの読み出ししか持たないので、その上限（500件）まで読んで切り出す。
    // total は読めた件数（履歴が500件を超えると、古いものは一覧に出ない）。
    let all = state.store.import_batches(BATCHES_MAX)?;
    let page: Vec<_> = all
        .iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect();
    ok(json!({ "total": all.len(), "items": to_value(&page)? }))
}

/// 取込の履歴を読む件数の上限（保存の層の上限と同じ）。
const BATCHES_MAX: u32 = 500;
