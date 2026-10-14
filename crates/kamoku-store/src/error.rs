// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use kamoku_ledger::{ChainError, EntryError, LedgerError};
use std::fmt;

/// 保存処理の失敗。`code()` はHTTP層で使う安定した識別名です。
#[derive(Debug)]
pub enum StoreError {
    /// `SQLite`の失敗。詳細は利用者への応答に含めません。
    Sql(rusqlite::Error),
    /// ファイル操作の失敗。
    Io(std::io::Error),
    /// JSONの失敗。
    Json(serde_json::Error),
    /// 仕訳の検査の失敗。
    Entry(EntryError),
    /// 入力の組み立ての検査。
    Compose(kamoku_books::ComposeError),
    /// 台帳の操作の失敗。
    Ledger(LedgerError),
    /// 起動時の検証に失敗したため書き込めません。
    ReadOnly,
    /// 別の接続の更新を読み直しました。入力を確認して再試行してください。
    Stale,
    /// 指定された版は最新の版ではありません。
    StaleRevision,
    /// 締めた日以前への書き込み。
    LockedPeriod,
    /// 確定済みの下書き。
    AlreadyConfirmed,
    /// 名前が重複しています。
    DuplicateName,
    /// 保護された科目の変更または必須科目の無効化。
    ProtectedAccount,
    /// 対象がありません。
    NotFound,
    /// 科目が選ばれていません。
    MissingAccount,
    /// 科目がありません。
    UnknownAccount,
    /// 科目が無効です。
    DisabledAccount,
    /// 金額の上限を超えました。
    AmountRange,
    /// 登録番号の形式が違います。
    BadInvoiceNumber,
    /// 税区分が違います。
    BadTaxCode,
    /// 入力の形またはマスタの規則が違います。
    Invalid(String),
    /// アプリより新しいDBの版です。
    FutureSchema,
    /// 複製先にファイルがあります。
    BackupExists,
}
impl StoreError {
    /// 通信の約束に対応するコード。
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Stale => "stale",
            Self::StaleRevision => "stale_revision",
            Self::LockedPeriod => "locked_period",
            Self::AlreadyConfirmed => "already_confirmed",
            Self::DuplicateName => "duplicate_name",
            Self::ProtectedAccount => "protected_account",
            Self::NotFound => "not_found",
            Self::MissingAccount => "missing_account",
            Self::UnknownAccount => "unknown_account",
            Self::DisabledAccount => "disabled_account",
            Self::AmountRange => "amount_range",
            Self::BadInvoiceNumber => "bad_invoice_number",
            Self::BadTaxCode => "bad_tax_code",
            Self::Invalid(_) => "bad_request",
            Self::Compose(e) => e.code(),
            Self::Ledger(LedgerError::AlreadyVoided(_)) => "already_voided",
            Self::Ledger(LedgerError::EmptyReason) => "reason_required",
            _ => "internal",
        }
    }
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Json(e) => e.fmt(f),
            Self::Entry(e) => e.fmt(f),
            Self::Compose(e) => e.fmt(f),
            Self::Ledger(e) => e.fmt(f),
            Self::Invalid(s) => f.write_str(s),
            _ => f.write_str(match self.code() {
                "read_only" => "台帳が壊れています。バックアップと控えを確認してください。",
                "stale" => "別の接続が更新しました。読み直した内容を確認してください。",
                "stale_revision" => "最新の版を開き直してください。",
                "locked_period" => "締めた日以前には書き込めません。日付を確認してください。",
                "already_confirmed" => "確定済みです。仕訳を取り消してから戻してください。",
                "duplicate_name" => "同じ名前があります。別の名前にしてください。",
                "protected_account" => "保護された科目は変更できません。",
                "not_found" => "対象が見つかりません。",
                "missing_account" => "科目が選ばれていません。",
                "unknown_account" => "科目が見つかりません。",
                "disabled_account" => "無効な科目です。有効な科目を選んでください。",
                "amount_range" => "1明細は9,999,999,999,999円以下にしてください。",
                "bad_invoice_number" => "登録番号はTと半角数字13桁にしてください。",
                "bad_tax_code" => "税区分を選び直してください。",
                _ => "保存処理に失敗しました。",
            }),
        }
    }
}
impl std::error::Error for StoreError {}
macro_rules! from_error {
    ($ty:ty,$v:ident) => {
        impl From<$ty> for StoreError {
            fn from(e: $ty) -> Self {
                Self::$v(e)
            }
        }
    };
}
from_error!(rusqlite::Error, Sql);
from_error!(std::io::Error, Io);
from_error!(serde_json::Error, Json);
from_error!(EntryError, Entry);
from_error!(kamoku_books::ComposeError, Compose);
from_error!(LedgerError, Ledger);

/// 最初の破損。復元不能な行も連番と規則で返します。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerifyFailure {
    /// 最初に壊れた連番（連番の欠落なら期待した番号）。
    pub seq: u64,
    /// `bad_hash`、`broken_link`、`wrong_sequence`、`rule_violation`、`quick_check`。
    pub kind: String,
    /// 詳しい規則。
    pub rule: Option<String>,
}
impl From<ChainError> for VerifyFailure {
    fn from(e: ChainError) -> Self {
        let (seq, kind, rule) = match e {
            ChainError::WrongSequence { expected, .. } => (expected, "wrong_sequence", None),
            ChainError::BrokenLink { seq } => (seq, "broken_link", None),
            ChainError::BadHash { seq } => (seq, "bad_hash", None),
            ChainError::RuleViolation { seq, rule } => {
                (seq, "rule_violation", Some(rule.to_owned()))
            }
            _ => (0, "rule_violation", Some(e.to_string())),
        };
        Self {
            seq,
            kind: kind.to_owned(),
            rule,
        }
    }
}
impl VerifyFailure {
    pub(crate) fn row(seq: u64, rule: impl Into<String>) -> Self {
        Self {
            seq,
            kind: "rule_violation".into(),
            rule: Some(rule.into()),
        }
    }
}
