// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 消費税の集計（国税と地方消費税の合計での参考値）。式は `docs/spec/api-reports.md` の
//! 「消費税の集計」。経過措置は、明細に含まれる税額（合計）に割合を掛けて、明細ごとに切り捨てる。
//! `kamoku-jp-tax` の `transitional_credit` は国税分だけを返すので、単位が混ざらないよう使わない。

use super::{ReportError, account, add, check_period, sub};
use crate::{Chart, TaxClass, TaxCode, ser};
use kamoku_jp_tax::{TRANSITIONAL_TABLE_CHECKED_ON, transitional_percent};
use kamoku_ledger::{Entry, Side, Yen};
use serde::Serialize;
use std::collections::BTreeMap;
use time::Date;

/// 税区分ごとの行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaxRow {
    /// 税区分。
    pub tax_code: TaxCode,
    /// 税区分の日本語の名前。
    pub label: String,
    /// 税込の課税標準。売上の区分は貸方−借方、仕入の区分は借方−貸方。
    #[serde(serialize_with = "ser::yen")]
    pub gross: Yen,
    /// 含まれる税額（国税と地方消費税の合計）。
    #[serde(serialize_with = "ser::yen")]
    pub tax: Yen,
}
/// 経過措置の割合ごとの内訳。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransitionalRow {
    /// 仕入税額とみなす割合（表の外の日付は0）。
    pub percent: u8,
    /// 税込の支払額（借方−貸方）。
    #[serde(serialize_with = "ser::yen")]
    pub gross: Yen,
    /// 含まれる税額。
    #[serde(serialize_with = "ser::yen")]
    pub tax: Yen,
    /// 控除できる額。明細ごとの `signum(tax) × floor(|tax| × percent / 100)` の合計。
    #[serde(serialize_with = "ser::yen")]
    pub deductible: Yen,
}
/// 消費税の集計（本則課税の参考値）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaxSummary {
    /// 税区分ごとの行（使われた区分だけ）。
    pub rows: Vec<TaxRow>,
    /// 売上税額（`S10` `S8` の税額の合計）。
    #[serde(serialize_with = "ser::yen")]
    pub sales_tax: Yen,
    /// 仕入税額（`P10` `P8` の税額と、経過措置の控除額の合計）。
    #[serde(serialize_with = "ser::yen")]
    pub purchase_tax: Yen,
    /// 見込み額（売上税額−仕入税額）。
    #[serde(serialize_with = "ser::yen")]
    pub estimate: Yen,
    /// 経過措置の、割合ごとの内訳。
    pub transitional: Vec<TransitionalRow>,
    /// 参考値であることと、表の最終確認日。
    pub disclaimer: String,
}

/// 消費税の集計の注意書き（`docs/spec/api-reports.md` の文面）。
#[must_use]
pub fn tax_disclaimer() -> String {
    format!(
        "参考値です。簡易課税と特例、経過措置の適用条件（請求書等と帳簿の保存、一の仕入先ごとの上限）は反映していません。課税の区分は現在の設定を使っています。経過措置の表の最終確認日は{TRANSITIONAL_TABLE_CHECKED_ON}です。"
    )
}

fn oriented(side: Side, value: Yen, toward: Side) -> Result<Yen, ReportError> {
    if side == toward {
        Ok(value)
    } else {
        sub(Yen::ZERO, value)
    }
}

/// 消費税を集計します（AC-08.5）。期間は1つの暦年の中だけです。
///
/// # Errors
/// 期間の誤り、年をまたぐ期間、知らない科目・税区分、集計値の上限超えを返します。
pub fn tax_summary(
    entries: &[&Entry],
    chart: &Chart,
    from: Date,
    to: Date,
) -> Result<TaxSummary, ReportError> {
    check_period(from, to)?;
    let mut rows = BTreeMap::<TaxCode, (Yen, Yen)>::new();
    let mut transitional = BTreeMap::<u8, (Yen, Yen, Yen)>::new();
    for entry in entries
        .iter()
        .filter(|e| from <= e.date() && e.date() <= to)
    {
        for line in entry.lines() {
            let Some(text) = line.tax_code.as_deref() else {
                continue;
            };
            let code =
                TaxCode::parse(text).ok_or_else(|| ReportError::UnknownTaxCode(text.to_owned()))?;
            // 非課税は売上にも仕入にもあるので、科目の通常の側で向きを決める。
            let toward = match code.class() {
                TaxClass::Sales => Side::Credit,
                TaxClass::Purchase | TaxClass::TransitionalPurchase => Side::Debit,
                TaxClass::NonTaxable => account(chart, line.account.as_str())?.normal_side(),
            };
            let gross = oriented(line.side, line.amount, toward)?;
            let tax = oriented(line.side, line.tax_amount, toward)?;
            let slot = rows.entry(code).or_insert((Yen::ZERO, Yen::ZERO));
            *slot = (add(slot.0, gross)?, add(slot.1, tax)?);
            if code.class() == TaxClass::TransitionalPurchase {
                let percent = transitional_percent(entry.date());
                // 0へ向かって切り捨てる。仕入と全額の返品で端数も相殺するため。
                let deductible = i64::try_from((i128::from(tax.get()) * i128::from(percent)) / 100)
                    .map(Yen::new)
                    .map_err(|_| ReportError::AmountRange)?;
                let slot = transitional
                    .entry(percent)
                    .or_insert((Yen::ZERO, Yen::ZERO, Yen::ZERO));
                *slot = (
                    add(slot.0, gross)?,
                    add(slot.1, tax)?,
                    add(slot.2, deductible)?,
                );
            }
        }
    }
    let tax_of = |codes: &[TaxCode]| -> Result<Yen, ReportError> {
        codes.iter().try_fold(Yen::ZERO, |sum, code| {
            add(sum, rows.get(code).map_or(Yen::ZERO, |r| r.1))
        })
    };
    let sales_tax = tax_of(&[TaxCode::S10, TaxCode::S8])?;
    let mut purchase_tax = tax_of(&[TaxCode::P10, TaxCode::P8])?;
    for (_, _, deductible) in transitional.values() {
        purchase_tax = add(purchase_tax, *deductible)?;
    }
    Ok(TaxSummary {
        rows: rows
            .iter()
            .map(|(code, (gross, tax))| TaxRow {
                tax_code: *code,
                label: code.label().to_owned(),
                gross: *gross,
                tax: *tax,
            })
            .collect(),
        sales_tax,
        purchase_tax,
        estimate: sub(sales_tax, purchase_tax)?,
        transitional: transitional
            .into_iter()
            .map(|(percent, (gross, tax, deductible))| TransitionalRow {
                percent,
                gross,
                tax,
                deductible,
            })
            .collect(),
        disclaimer: tax_disclaimer(),
    })
}
