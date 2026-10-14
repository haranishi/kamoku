// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{EntryId, Yen};
use std::fmt;

/// `IdError` に対応する検証失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdError {
    /// 空の識別子。
    Empty,
    /// 長すぎる識別子。
    TooLong,
    /// 許可されない文字。
    BadChar,
}
impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("識別子が空です"),
            Self::TooLong => f.write_str("識別子が長すぎます"),
            Self::BadChar => f.write_str("識別子に使用できない文字があります"),
        }
    }
}
impl std::error::Error for IdError {}

/// `HashError` に対応する検証失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashError {
    /// 64桁でない文字列。
    InvalidLength,
    /// 16進数でない文字列。
    InvalidHex,
}
impl fmt::Display for HashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => f.write_str("ハッシュは64桁で指定してください"),
            Self::InvalidHex => f.write_str("ハッシュに16進数でない文字があります"),
        }
    }
}
impl std::error::Error for HashError {}

/// `EntryError` に対応する検証失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntryError {
    /// 明細が2行未満。
    TooFewLines,
    /// 明細が200行超。
    TooManyLines,
    /// 正でない明細金額。
    NonPositiveAmount {
        /// 明細の0始まりの位置。
        index: usize,
    },
    /// 税額の範囲または税コードとの不整合。
    BadTax {
        /// 明細の0始まりの位置。
        index: usize,
    },
    /// 不正な税コード。
    BadTaxCode {
        /// 明細の0始まりの位置。
        index: usize,
    },
    /// 摘要またはメモが1000バイト超。
    TextTooLong,
    /// 借方または貸方が存在しない。
    MissingSide,
    /// 合計の桁あふれ。
    Overflow,
    /// 貸借の不一致。
    Unbalanced {
        /// 借方合計。
        debit: Yen,
        /// 貸方合計。
        credit: Yen,
    },
}
impl fmt::Display for EntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewLines => f.write_str("明細が少なすぎます"),
            Self::TooManyLines => f.write_str("明細が多すぎます"),
            Self::NonPositiveAmount { .. } => f.write_str("明細金額は正である必要があります"),
            Self::BadTax { .. } => f.write_str("税額が不正です"),
            Self::BadTaxCode { .. } => f.write_str("税コードが不正です"),
            Self::TextTooLong => f.write_str("摘要またはメモが長すぎます"),
            Self::MissingSide => f.write_str("借方と貸方が必要です"),
            Self::Overflow => f.write_str("金額の合計が桁あふれしました"),
            Self::Unbalanced { .. } => f.write_str("貸借が一致しません"),
        }
    }
}
impl std::error::Error for EntryError {}

/// `LedgerError` に対応する検証失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LedgerError {
    /// 登録済みの識別子。
    DuplicateId(EntryId),
    /// 未登録の識別子。
    UnknownId(EntryId),
    /// 取消済みの仕訳。
    AlreadyVoided(EntryId),
    /// 空白だけの理由。
    EmptyReason,
    /// 金額または連番・符号化長の桁あふれ。
    Overflow,
}
impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(..) => f.write_str("仕訳識別子が重複しています"),
            Self::UnknownId(..) => f.write_str("仕訳識別子が未登録です"),
            Self::AlreadyVoided(..) => f.write_str("仕訳は取消済みです"),
            Self::EmptyReason => f.write_str("理由が空です"),
            Self::Overflow => f.write_str("台帳の計算が桁あふれしました"),
        }
    }
}
impl std::error::Error for LedgerError {}

/// `ChainError` に対応する検証失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChainError {
    /// イベント連番の不一致。
    WrongSequence {
        /// 期待する連番。
        expected: u64,
        /// 実際の連番。
        found: u64,
    },
    /// 前のハッシュとの不一致。
    BrokenLink {
        /// 違反したイベントの連番。
        seq: u64,
    },
    /// 再計算したハッシュとの不一致。
    BadHash {
        /// 違反したイベントの連番。
        seq: u64,
    },
    /// イベント規則への違反。
    RuleViolation {
        /// 違反したイベントの連番。
        seq: u64,
        /// 違反した規則。
        rule: &'static str,
    },
}
impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSequence { .. } => f.write_str("イベントの連番が不正です"),
            Self::BrokenLink { .. } => f.write_str("ハッシュ連鎖が切れています"),
            Self::BadHash { .. } => f.write_str("イベントのハッシュが不正です"),
            Self::RuleViolation { .. } => f.write_str("イベント規則に違反しています"),
        }
    }
}
impl std::error::Error for ChainError {}
