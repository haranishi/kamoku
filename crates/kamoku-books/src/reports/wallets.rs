// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReportError, add, carried};
use crate::{Chart, Kind, WalletType, ser};
use kamoku_ledger::{Entry, Yen};
use serde::Serialize;
use time::Date;

/// 口座と、その残高。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletItem {
    /// 口座のコード。
    pub code: String,
    /// 口座の名前。
    pub name: String,
    /// 口座の型。
    #[serde(rename = "type")]
    pub kind: WalletType,
    /// 残高。カードは未払い額（正）。
    #[serde(serialize_with = "ser::yen")]
    pub balance: Yen,
}
/// 口座の残高の一覧（`GET /wallets` とホームの `wallets`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletBalances {
    /// 有効な口座と、残高の残っている無効の口座。科目の並び順。
    pub items: Vec<WalletItem>,
    /// 現金・預金・電子マネーの残高の合計。
    #[serde(serialize_with = "ser::yen")]
    pub cash_total: Yen,
    /// カードの未払い額の合計。
    #[serde(serialize_with = "ser::yen")]
    pub owed_total: Yen,
}

/// `as_of` の日の終わりの、口座の残高を返します。
///
/// # Errors
/// 知らない科目、集計値の上限超えを返します。
pub fn wallet_balances(
    entries: &[&Entry],
    chart: &Chart,
    as_of: Date,
) -> Result<WalletBalances, ReportError> {
    let balances = carried(entries, chart, as_of.year(), Some(as_of))?;
    let mut out = WalletBalances {
        items: Vec::new(),
        cash_total: Yen::ZERO,
        owed_total: Yen::ZERO,
    };
    for acc in chart.wallets() {
        let Some(wallet) = &acc.wallet else {
            continue;
        };
        let balance = balances.get(acc.code.as_str()).copied().unwrap_or_default();
        if !acc.enabled && balance == Yen::ZERO {
            continue;
        }
        // 残高は通常の側から見た値なので、カード（負債）は未払い額がそのまま正になる。
        match acc.kind {
            Kind::Liability => out.owed_total = add(out.owed_total, balance)?,
            _ => out.cash_total = add(out.cash_total, balance)?,
        }
        out.items.push(WalletItem {
            code: acc.code.as_str().to_owned(),
            name: acc.name.clone(),
            kind: wallet.kind,
            balance,
        });
    }
    Ok(out)
}
