// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReportError, add, carried_opening, check_period, movements, sub};
use crate::{Chart, Kind, ser};
use kamoku_ledger::{Entry, Side, Yen};
use serde::Serialize;
use time::Date;

/// 試算表の1行。科目ごとに平らで、親と子を足さない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrialRow {
    /// 科目のコード。
    pub code: String,
    /// 科目の名前。
    pub name: String,
    /// 分類。
    pub kind: Kind,
    /// 期首の残高（通常の側から見た値。仮想の繰越を含む）。
    #[serde(serialize_with = "ser::yen")]
    pub opening: Yen,
    /// 期間内の借方の動き。
    #[serde(serialize_with = "ser::yen")]
    pub debit: Yen,
    /// 期間内の貸方の動き。
    #[serde(serialize_with = "ser::yen")]
    pub credit: Yen,
    /// 期末の残高。`opening` に通常の側の動きを足した値。
    #[serde(serialize_with = "ser::yen")]
    pub closing: Yen,
}
/// 試算表（`docs/spec/api-reports.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrialBalance {
    /// 科目の並び順の行。期首・動き・期末がすべて0の科目は出さない。
    pub rows: Vec<TrialRow>,
    /// 借方の動きの合計。
    #[serde(serialize_with = "ser::yen")]
    pub total_debit: Yen,
    /// 貸方の動きの合計。
    #[serde(serialize_with = "ser::yen")]
    pub total_credit: Yen,
    /// 動きの合計の一致と、期末の残高の借方側・貸方側の一致の、両方が成り立つか。
    pub balanced: bool,
}

/// 試算表を作ります（AC-08.1）。期間は1つの暦年の中だけです。
///
/// # Errors
/// 期間の誤り、年をまたぐ期間、知らない科目、集計値の上限超えを返します。
pub fn trial_balance(
    entries: &[&Entry],
    chart: &Chart,
    from: Date,
    to: Date,
) -> Result<TrialBalance, ReportError> {
    check_period(from, to)?;
    let opening = carried_opening(entries, chart, from)?;
    let moves = movements(entries, chart, from, to)?;
    let mut out = TrialBalance {
        rows: Vec::new(),
        total_debit: Yen::ZERO,
        total_credit: Yen::ZERO,
        balanced: false,
    };
    let (mut debit_side, mut credit_side) = (Yen::ZERO, Yen::ZERO);
    for acc in chart {
        let code = acc.code.as_str();
        let start = opening.get(code).copied().unwrap_or_default();
        let m = moves.get(code).copied().unwrap_or_default();
        if start == Yen::ZERO && m.debit == Yen::ZERO && m.credit == Yen::ZERO {
            continue;
        }
        let change = match acc.normal_side() {
            Side::Debit => sub(m.debit, m.credit)?,
            Side::Credit => sub(m.credit, m.debit)?,
        };
        let closing = add(start, change)?;
        match acc.normal_side() {
            Side::Debit => debit_side = add(debit_side, closing)?,
            Side::Credit => credit_side = add(credit_side, closing)?,
        }
        out.total_debit = add(out.total_debit, m.debit)?;
        out.total_credit = add(out.total_credit, m.credit)?;
        out.rows.push(TrialRow {
            code: code.to_owned(),
            name: acc.name.clone(),
            kind: acc.kind,
            opening: start,
            debit: m.debit,
            credit: m.credit,
            closing,
        });
    }
    out.balanced = out.total_debit == out.total_credit && debit_side == credit_side;
    Ok(out)
}
