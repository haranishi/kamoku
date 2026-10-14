// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 失敗の形（`{ "error": { code, message, field? } }`）と、下の部品の失敗からの対応づけ。

use kamoku_books::{ComposeError, ReportError};
use kamoku_import::ImportError;
use kamoku_ledger::LedgerError;
use kamoku_store::StoreError;
use serde_json::{Map, Value, json};

/// 応答にする失敗。`detail` は想定外の失敗（500）の中身で、標準エラーにだけ書く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    pub field: Option<String>,
    pub line: Option<usize>,
    pub detail: Option<String>,
}
impl ApiError {
    pub(crate) fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            field: None,
            line: None,
            detail: None,
        }
    }
    /// 400 `bad_request`（形が壊れている）。
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "bad_request", message)
    }
    /// 422 `bad_request`（形は正しいが、規則に反する）。
    pub(crate) fn rule(message: impl Into<String>) -> Self {
        Self::new(422, "bad_request", message)
    }
    /// 404 `not_found`。
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }
    /// 500 `internal`。中身は応答に出さず、標準エラーに書く。
    pub(crate) fn internal(detail: impl std::fmt::Display) -> Self {
        let mut e = Self::new(
            500,
            "internal",
            "サーバーで想定外の失敗がありました。もう一度試してください。続くときは、サーバーを起動したターミナルの表示を確かめてください。",
        );
        e.detail = Some(detail.to_string());
        e
    }
    pub(crate) fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
    /// 応答の本文。
    pub(crate) fn body(&self) -> Value {
        let mut error = Map::new();
        error.insert("code".into(), json!(self.code));
        error.insert("message".into(), json!(self.message));
        if let Some(field) = &self.field {
            error.insert("field".into(), json!(field));
        }
        if let Some(line) = self.line {
            error.insert("line".into(), json!(line));
        }
        json!({ "error": error })
    }
}

impl From<ComposeError> for ApiError {
    fn from(e: ComposeError) -> Self {
        let mut out = Self::new(e.http_status(), e.code(), e.message());
        out.field = e.field().map(str::to_owned);
        out
    }
}
impl From<ReportError> for ApiError {
    fn from(e: ReportError) -> Self {
        Self::new(e.http_status(), e.code(), e.message())
    }
}
impl From<ImportError> for ApiError {
    fn from(e: ImportError) -> Self {
        let mut out = Self::new(400, e.code(), e.message());
        out.line = e.line();
        out
    }
}
impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        store_error(e)
    }
}

fn conflict(code: &'static str, message: &str) -> ApiError {
    ApiError::new(409, code, message)
}
fn rule(code: &'static str, message: &str, field: &str) -> ApiError {
    ApiError::new(422, code, message).with_field(field)
}

/// 保存の層の失敗を、通信の約束の状態と `code` にします。
pub(crate) fn store_error(e: StoreError) -> ApiError {
    match e {
        StoreError::ReadOnly => conflict(
            "read_only",
            "台帳の検証で不正が見つかったため、読み取り専用で開いています。書き込みはできません。設定の画面で検査の結果を確かめてください。",
        ),
        StoreError::Stale => conflict(
            "stale",
            "ほかの場所で台帳が更新されました。読み込み直してから、もう一度試してください。",
        ),
        StoreError::StaleRevision => conflict(
            "stale_revision",
            "この仕訳は、ほかの場所で直されています。最新の版を開き直してから、もう一度試してください。",
        ),
        StoreError::AlreadyConfirmed => conflict(
            "already_confirmed",
            "この行は確定済みです。直すときは、仕訳帳で仕訳を取り消してから、未確認に戻してください。",
        ),
        StoreError::DuplicateName => conflict(
            "duplicate_name",
            "同じ名前がすでにあります。別の名前にしてください。",
        )
        .with_field("name"),
        StoreError::ProtectedAccount => conflict(
            "protected_account",
            "事業主貸・事業主借・元入金は変えられません。帳簿に必ず要る科目は、無効にできません。",
        ),
        StoreError::NotFound => ApiError::not_found("見つかりません。一覧から選び直してください。"),
        StoreError::LockedPeriod => rule(
            "locked_period",
            "締めた日（設定の「締め」）以前の日付には、登録・訂正・取消ができません。日付を直すか、締めた日を見直してください。",
            "date",
        ),
        StoreError::MissingAccount => rule(
            "missing_account",
            "科目が選ばれていません。科目を選んでください。",
            "account",
        ),
        StoreError::UnknownAccount => rule(
            "unknown_account",
            "科目・口座が見つかりません。一覧から選び直してください。",
            "account",
        ),
        StoreError::DisabledAccount => rule(
            "disabled_account",
            "無効にした科目・口座は使えません。勘定科目の画面で有効に戻すか、別の科目を選んでください。",
            "account",
        ),
        StoreError::AmountRange => ApiError::new(
            422,
            "amount_range",
            "金額は1円から9,999,999,999,999円までです。",
        ),
        StoreError::BadInvoiceNumber => rule(
            "bad_invoice_number",
            "登録番号は、半角の T と13桁の数字で入れてください（例: T1234567890123）。免税事業者の設定では入れられません。",
            "invoice_number",
        ),
        StoreError::BadTaxCode => rule(
            "bad_tax_code",
            "税区分は S10・S8・P10・P8・P10X・P8X・NT のどれかを選ぶか、空にしてください。",
            "tax_code",
        ),
        StoreError::Invalid(message) => ApiError::rule(message),
        StoreError::Compose(e) => e.into(),
        StoreError::Ledger(e) => ledger_error(e),
        other @ (StoreError::Sql(_)
        | StoreError::Io(_)
        | StoreError::Json(_)
        | StoreError::Entry(_)
        | StoreError::FutureSchema
        | StoreError::BackupExists) => ApiError::internal(other),
    }
}

fn ledger_error(e: LedgerError) -> ApiError {
    match e {
        LedgerError::AlreadyVoided(_) => conflict(
            "already_voided",
            "この仕訳は取消済みです。取り消した仕訳は、直すことも、もう一度取り消すこともできません。",
        ),
        LedgerError::EmptyReason => rule(
            "reason_required",
            "理由を入れてください（空白だけは使えません）。",
            "reason",
        ),
        other => ApiError::internal(other),
    }
}
