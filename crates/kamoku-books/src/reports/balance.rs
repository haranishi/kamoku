// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReportError, Section, add, carried, sub};
use crate::{Account, Chart, Kind, ser};
use kamoku_ledger::{Entry, Yen};
use serde::Serialize;
use std::collections::BTreeMap;
use time::Date;

/// 資本の区分の最後に置く「当期純利益」の行のコード。科目のコード（16バイトまで）と重ならない長さにしてある。
pub const NET_INCOME_ROW_CODE: &str = "net_income_current";

/// 貸借対照表の1行。親の `amount` は、自分の `own` と子の `amount` の合計。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BsRow {
    /// 科目のコード（当期純利益の行は [`NET_INCOME_ROW_CODE`]）。
    pub code: String,
    /// 科目の名前。
    pub name: String,
    /// 自分と子の合計。
    #[serde(serialize_with = "ser::yen")]
    pub amount: Yen,
    /// 自分に直接記帳した分の残高。
    #[serde(serialize_with = "ser::yen")]
    pub own: Yen,
    /// 子の科目の行。残高のある子が無ければ省く。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<BsRow>,
}
/// 貸借対照表（`docs/spec/api-reports.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BalanceSheet {
    /// いつの時点か。
    #[serde(with = "ser::date")]
    pub as_of: Date,
    /// 資産。事業主貸は、その年の動きだけ。
    pub assets: Section<BsRow>,
    /// 負債。事業主借は、その年の動きだけ。
    pub liabilities: Section<BsRow>,
    /// 資本。最後の行が当期純利益で、合計はそれを含む。
    pub equity: Section<BsRow>,
    /// その年の1月1日から `as_of` までの、収益−費用。
    #[serde(serialize_with = "ser::yen")]
    pub net_income_current: Yen,
    /// 資産の合計 = 負債の合計 + 資本の合計 か。
    pub balanced: bool,
}

/// 貸借対照表を作ります（AC-08.2）。前の年までの損益と事業主勘定は、仮想の繰越で元入金に入ります。
///
/// # Errors
/// 知らない科目、集計値の上限超えを返します。
pub fn balance_sheet(
    entries: &[&Entry],
    chart: &Chart,
    as_of: Date,
) -> Result<BalanceSheet, ReportError> {
    let book = carried(entries, chart, as_of.year(), Some(as_of))?;
    let mut net = Yen::ZERO;
    for acc in chart {
        let own = book.get(acc.code.as_str()).copied().unwrap_or_default();
        match acc.kind {
            Kind::Revenue => net = add(net, own)?,
            Kind::Expense => net = sub(net, own)?,
            Kind::Asset | Kind::Liability | Kind::Equity => {}
        }
    }
    let assets = section(chart, Kind::Asset, &book)?;
    let liabilities = section(chart, Kind::Liability, &book)?;
    let mut equity = section(chart, Kind::Equity, &book)?;
    equity.rows.push(BsRow {
        code: NET_INCOME_ROW_CODE.to_owned(),
        name: "当期純利益".to_owned(),
        amount: net,
        own: net,
        children: Vec::new(),
    });
    equity.total = add(equity.total, net)?;
    let balanced = assets.total == add(liabilities.total, equity.total)?;
    Ok(BalanceSheet {
        as_of,
        assets,
        liabilities,
        equity,
        net_income_current: net,
        balanced,
    })
}

fn section(
    chart: &Chart,
    kind: Kind,
    balances: &BTreeMap<&str, Yen>,
) -> Result<Section<BsRow>, ReportError> {
    let mut out = Section {
        rows: Vec::new(),
        total: Yen::ZERO,
    };
    for root in chart
        .iter()
        .filter(|a| a.kind == kind && a.parent.is_none())
    {
        if let Some(row) = tree_row(chart, root, balances)? {
            out.total = add(out.total, row.amount)?;
            out.rows.push(row);
        }
    }
    Ok(out)
}

// 科目の集合は循環しない（Chart::new が検査済み）ので、再帰は必ず終わる。
fn tree_row(
    chart: &Chart,
    acc: &Account,
    balances: &BTreeMap<&str, Yen>,
) -> Result<Option<BsRow>, ReportError> {
    let code = acc.code.as_str();
    let own = balances.get(code).copied().unwrap_or_default();
    let mut amount = own;
    let mut children = Vec::new();
    for child in chart.children(code) {
        if let Some(row) = tree_row(chart, child, balances)? {
            amount = add(amount, row.amount)?;
            children.push(row);
        }
    }
    if own == Yen::ZERO && children.is_empty() {
        return Ok(None);
    }
    Ok(Some(BsRow {
        code: code.to_owned(),
        name: acc.name.clone(),
        amount,
        own,
        children,
    }))
}
