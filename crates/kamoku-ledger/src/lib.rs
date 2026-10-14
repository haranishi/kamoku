// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 追記だけで管理する複式簿記の台帳。訂正と取消も新しいイベントとして残します。
//! SHA-256 の連鎖は改ざんの検知を行いますが、防止はしません。
//! 末尾の削除や全体の再計算は連鎖だけでは検知できません。
//! `head_hash()` を紙・メール・Git など台帳の外に控えて比較してください。
//! 電子帳簿保存法などへの適合は保証しません。呼び出し側が渡す
//! `recorded_at` は信頼できる時刻の証明ではありません。

mod encoding;
mod entry;
mod error;
mod hash;
mod ids;
mod ledger;
mod money;

pub use entry::{Entry, Line, Side};
pub use error::{ChainError, EntryError, HashError, IdError, LedgerError};
pub use hash::Hash;
pub use ids::{AccountCode, EntryId};
pub use ledger::{Balance, Event, EventKind, Ledger};
pub use money::Yen;

#[cfg(test)]
mod chain_tests;
