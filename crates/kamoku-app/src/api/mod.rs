// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `/api/v1` の処理。受け口に依存しない関数として書く（[`Call`] を受け取り、[`Reply`] を返す）。
//! 受け口を差し替えても、ここは変えずに済む。

mod accounts;
mod body;
mod client_ids;
mod dashboard;
mod drafts;
mod entries;
#[cfg(test)]
mod entries_tests;
mod entry_list;
mod entry_view;
mod errors;
mod export;
mod export_reports;
mod import_rows;
mod imports;
mod input;
mod opening;
mod partners;
mod reports;
mod settings;
mod status;
#[cfg(test)]
mod testing;

pub(crate) use accounts::{account_add, account_patch, accounts_list, wallets};
pub(crate) use client_ids::ClientIds;
#[cfg(test)]
pub(crate) use client_ids::ClientLimits;
pub(crate) use dashboard::dashboard;
pub(crate) use drafts::{draft_confirm, draft_ignore, draft_patch, draft_restore, drafts_list};
pub(crate) use entries::{entry_create, entry_preview, entry_revise, entry_void};
pub(crate) use entry_list::{entries_list, entry_detail};
pub(crate) use errors::ApiError;
pub(crate) use export::{export_all, export_backup, export_journal};
pub(crate) use export_reports::{
    export_balance_sheet, export_income_statement, export_ledger, export_tax_summary,
    export_trial_balance,
};
pub(crate) use import_rows::{commit_table, import_commit, import_preview};
pub(crate) use imports::{Uploads, import_batches, import_profile, import_upload};
pub(crate) use opening::{opening_get, opening_put};
pub(crate) use partners::{
    partner_add, partner_patch, partners_list, template_add, template_delete, templates_list,
};
pub(crate) use reports::{balance_sheet, income_statement, ledger, tax_summary, trial_balance};
pub(crate) use settings::{settings_get, settings_put};
pub(crate) use status::{status, verify};

use crate::query::Query;
use crate::state::State;
use serde::Serialize;
use serde_json::Value;
use time::Date;

/// 1つの要求の中身（受け口に依存しない形）。
pub(crate) struct Call<'a> {
    /// パスの `{...}` の値（パーセント符号化を戻したもの）。
    pub params: &'a [String],
    /// 問い合わせ。
    pub query: &'a Query,
    /// 本文。
    pub body: &'a [u8],
    /// いまのUNIX秒（記録の時刻）。
    pub now: i64,
    /// 日本時間の今日。
    pub today: Date,
}
impl Call<'_> {
    /// パスの値（無ければ空）。
    pub(crate) fn param(&self, index: usize) -> &str {
        self.params.get(index).map_or("", String::as_str)
    }
}

/// 書き出すファイル。
pub(crate) struct FileReply {
    pub content_type: &'static str,
    pub filename: String,
    pub body: Vec<u8>,
    /// 応答の `X-Kamoku-Warning`（`Shift_JIS` で表せない文字があったときなど）。
    pub warning: Option<String>,
}

/// 処理の結果。
pub(crate) enum Reply {
    /// JSON（状態、本文）。
    Json(u16, Value),
    /// ファイル。
    File(FileReply),
}

/// 処理の関数の形。
pub(crate) type Run = fn(&mut State, &Call<'_>) -> Result<Reply, ApiError>;

/// 値をJSONにします（失敗は想定外として500）。
pub(crate) fn to_value<T: Serialize>(value: &T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(ApiError::internal)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "処理の関数の最後の式として、そのまま返せるようにするため"
)]
pub(crate) fn ok(value: Value) -> Result<Reply, ApiError> {
    Ok(Reply::Json(200, value))
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "処理の関数の最後の式として、そのまま返せるようにするため"
)]
pub(crate) fn created(value: Value) -> Result<Reply, ApiError> {
    Ok(Reply::Json(201, value))
}
