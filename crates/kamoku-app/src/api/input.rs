// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 取引の入力（`docs/spec/api-entries.md` の「入力の形」）の読み取りと、組み立ての呼び出し。
//! 組み立ての規則は `kamoku-books` が持つ。ここでは形を読むだけ。

use super::ApiError;
use super::body::Fields;
use kamoku_books::{
    Chart, ComposeSettings, Composed, JournalInput, JournalLineInput, Rounding, SimpleInput,
    SimpleKind, Source, TaxMode, compose_journal, compose_simple,
};
use kamoku_ledger::Side;
use kamoku_store::Settings;
use serde_json::{Map, Value};

/// 取引の入力。
#[derive(Debug, Clone)]
pub(crate) enum EntryInput {
    /// 単純な形（支出・収入・振替）。
    Simple(SimpleInput),
    /// 仕訳の形。
    Journal(JournalInput),
}
impl EntryInput {
    /// 取引先の名前。
    pub(crate) fn partner(&self) -> &str {
        match self {
            Self::Simple(s) => &s.partner,
            Self::Journal(j) => &j.partner,
        }
    }
}

/// 本文から入力を読みます。取引先の名前の前後の空白は落とす（同じ名前の取引先が2つできないように）。
pub(crate) fn parse_entry(map: &Map<String, Value>) -> Result<EntryInput, ApiError> {
    let f = Fields::new(map);
    let kind = f.req_str("kind", "種別")?;
    let kind = match kind.as_str() {
        "expense" => SimpleKind::Expense,
        "income" => SimpleKind::Income,
        "transfer" => SimpleKind::Transfer,
        "journal" => return journal(&f).map(EntryInput::Journal),
        _ => {
            return Err(ApiError::bad_request(
                "種別（kind）は expense・income・transfer・journal のどれかにしてください。",
            )
            .with_field("kind"));
        }
    };
    simple(&f, kind).map(EntryInput::Simple)
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.is_empty())
}

fn date(f: &Fields<'_>) -> Result<time::Date, ApiError> {
    f.opt_date("date")?.ok_or_else(|| f.missing("date", "日付"))
}

fn simple(f: &Fields<'_>, kind: SimpleKind) -> Result<SimpleInput, ApiError> {
    Ok(SimpleInput {
        kind,
        date: date(f)?,
        amount: f
            .opt_amount("amount")?
            .ok_or_else(|| f.missing("amount", "金額"))?,
        account: non_empty(f.opt_str("account")?),
        wallet: f.text("wallet")?,
        to_wallet: non_empty(f.opt_str("to_wallet")?),
        tax_code: non_empty(f.opt_str("tax_code")?),
        partner: f.text("partner")?.trim().to_owned(),
        description: f.text("description")?,
        business_pct: f.opt_pct("business_pct")?,
        memo: f.text("memo")?,
    })
}

fn journal(f: &Fields<'_>) -> Result<JournalInput, ApiError> {
    let rows = f
        .opt_array("lines")?
        .ok_or_else(|| f.missing("lines", "明細の行"))?;
    let mut lines = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let Value::Object(map) = row else {
            return Err(ApiError::bad_request(format!(
                "明細の{}行目は、オブジェクト（{{ account, side, amount, ... }}）にしてください。",
                i + 1
            ))
            .with_field(format!("lines[{i}]")));
        };
        let lf = Fields::nested(map, format!("lines[{i}]."));
        let side = match lf.req_str("side", "貸借")?.as_str() {
            "debit" => Side::Debit,
            "credit" => Side::Credit,
            _ => {
                return Err(ApiError::bad_request(
                    "貸借（side）は debit か credit にしてください。",
                )
                .with_field(lf.name("side")));
            }
        };
        lines.push(JournalLineInput {
            account: lf.text("account")?,
            side,
            amount: lf
                .opt_amount("amount")?
                .ok_or_else(|| lf.missing("amount", "金額"))?,
            tax_code: non_empty(lf.opt_str("tax_code")?),
            tax_amount: lf.opt_int("tax_amount")?,
            memo: lf.text("memo")?,
        });
    }
    Ok(JournalInput {
        date: date(f)?,
        description: f.text("description")?,
        partner: f.text("partner")?.trim().to_owned(),
        lines,
    })
}

/// 設定から、組み立ての設定を作ります。
pub(crate) fn compose_settings(settings: &Settings, source: Source) -> ComposeSettings {
    ComposeSettings {
        rounding: match settings.rounding.as_str() {
            "round" => Rounding::Round,
            "ceil" => Rounding::Ceil,
            _ => Rounding::Floor,
        },
        tax_mode: if settings.tax_mode == "taxable" {
            TaxMode::Taxable
        } else {
            TaxMode::Exempt
        },
        source,
    }
}

/// 入力から明細を組み立てます（`kamoku-books` を呼ぶだけ）。
pub(crate) fn compose(
    input: &EntryInput,
    chart: &Chart,
    settings: ComposeSettings,
) -> Result<Composed, ApiError> {
    match input {
        EntryInput::Simple(s) => compose_simple(s, chart, &settings),
        EntryInput::Journal(j) => compose_journal(j, chart, &settings),
    }
    .map_err(ApiError::from)
}
