// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の形の入力と、期首残高の組み立て。

use crate::compose_parts::{
    CAPITAL, Lookup, MAX_AMOUNT, OWNER_DRAW, OWNER_INV, check_amount, check_text, parse_tax,
};
use crate::{
    Chart, ComposeError, ComposeSettings, Composed, JournalInput, JournalLineInput, Kind, Source,
    TaxCode, TaxMode, tax_included,
};
use kamoku_jp_tax::transitional_percent;
use kamoku_ledger::{AccountCode, Line, Side, Yen};
use std::collections::BTreeSet;
use time::Date;

/// 仕訳の形の入力から明細を作ります。税額を省いた行は、その行の金額から計算します（記帳上の目安）。
///
/// # Errors
/// 行の数、金額、科目、税区分と税額、貸借の一致の違反を返します。
pub fn compose_journal(
    input: &JournalInput,
    chart: &Chart,
    settings: &ComposeSettings,
) -> Result<Composed, ComposeError> {
    check_text("description", &input.description, "摘要")?;
    check_text("partner", &input.partner, "取引先の名前")?;
    if !(2..=200).contains(&input.lines.len()) {
        return Err(ComposeError::bad(
            "lines",
            format!(
                "仕訳の行は2行から200行までです。いまは{}行です。",
                input.lines.len()
            ),
        ));
    }
    let lookup = Lookup {
        chart,
        check_enabled: true,
    };
    let mut c = Composed::new(
        input.date,
        &input.description,
        "journal",
        settings.source,
        &input.partner,
    );
    let (mut debit, mut credit) = (0i128, 0i128);
    let mut missing_tax = Vec::new();
    let mut transitional = false;
    for (i, row) in input.lines.iter().enumerate() {
        let (line, kind) = journal_line(i, row, &lookup, *settings)?;
        let code = line.tax_code.as_deref().and_then(TaxCode::parse);
        if code.is_none()
            && settings.tax_mode == TaxMode::Taxable
            && matches!(kind, Kind::Revenue | Kind::Expense)
        {
            missing_tax.push((i + 1).to_string());
        }
        transitional |= code.is_some_and(|c| matches!(c, TaxCode::P10X | TaxCode::P8X));
        match row.side {
            Side::Debit => debit += i128::from(row.amount),
            Side::Credit => credit += i128::from(row.amount),
        }
        c.lines.push(line);
    }
    if debit != credit {
        return Err(ComposeError::Unbalanced {
            debit: Yen::new(i64::try_from(debit).unwrap_or(i64::MAX)),
            credit: Yen::new(i64::try_from(credit).unwrap_or(i64::MAX)),
        });
    }
    if !missing_tax.is_empty() {
        c.warnings.push(format!(
            "{}行目の税区分が空です。課税事業者の設定なので、消費税の対象か確かめてください。",
            missing_tax.join("・")
        ));
    }
    if transitional {
        let percent = transitional_percent(input.date);
        c.warnings.push(if percent == 0 {
            "経過措置の期間の外の日付です。この取引の控除は0円として集計します。".to_owned()
        } else {
            format!(
                "経過措置の割合（{percent}%）は、この日付で決まります。日付が課税仕入れの日か確かめてください。"
            )
        });
    }
    c.validated()
}

fn journal_line(
    i: usize,
    row: &JournalLineInput,
    lookup: &Lookup<'_>,
    settings: ComposeSettings,
) -> Result<(Line, Kind), ComposeError> {
    let field = |name: &str| format!("lines[{i}].{name}");
    check_text(&field("memo"), &row.memo, "メモ")?;
    check_amount(&field("amount"), row.amount)?;
    let account = lookup.account(&field("account"), Some(&row.account))?;
    let code = parse_tax(&field("tax_code"), row.tax_code.as_deref())?;
    let amount = Yen::new(row.amount);
    let line = Line::new(account.code.clone(), row.side, amount).with_memo(&row.memo);
    let line = match (code, row.tax_amount) {
        (None, None | Some(0)) => line,
        (None, Some(_)) => {
            return Err(ComposeError::BadTaxCode {
                field: field("tax_amount"),
                message: "税区分が空の行には、税額を入れられません。税区分を選ぶか、税額を0にしてください。".to_owned(),
            });
        }
        (Some(TaxCode::NT), Some(tax)) if tax != 0 => {
            return Err(ComposeError::BadTaxCode {
                field: field("tax_amount"),
                message: "非課税（NT）の行の税額は0円です。税額を0にしてください。".to_owned(),
            });
        }
        (Some(code), None) => {
            let tax = tax_included(code, amount, settings.rounding).map_err(|_| {
                ComposeError::AmountRange {
                    field: field("amount"),
                    amount: row.amount,
                }
            })?;
            line.with_tax(code.as_str(), tax)
        }
        (Some(code), Some(tax)) => {
            if !(0..=row.amount).contains(&tax) {
                return Err(ComposeError::bad(
                    &field("tax_amount"),
                    "税額は0円から、その行の金額までです。直してください。",
                ));
            }
            line.with_tax(code.as_str(), Yen::new(tax))
        }
    };
    Ok((line, account.kind))
}

/// 期首残高の仕訳を組み立てます。資産は借方、負債は貸方に入れ、差額（資産−負債）を元入金で
/// 合わせます（債務超過なら元入金が借方）。金額が0の行は入れず、1行も残らなければ `None`。
///
/// # Errors
/// 科目が資産・負債でない、事業主貸・事業主借、重複、金額が範囲外のときに返します。
pub fn compose_opening(
    date: Date,
    balances: &[(AccountCode, Yen)],
    chart: &Chart,
) -> Result<Option<Composed>, ComposeError> {
    let lookup = Lookup {
        chart,
        check_enabled: true,
    };
    let mut c = Composed::new(date, "期首残高", "opening", Source::Opening, "");
    let mut seen = BTreeSet::new();
    let mut net = 0i128;
    for (i, (code, amount)) in balances.iter().enumerate() {
        let field = format!("balances[{i}].account");
        let account = lookup.account(&field, Some(code.as_str()))?;
        if !seen.insert(code.as_str()) {
            return Err(ComposeError::bad(
                &field,
                format!(
                    "「{}」が2回入っています。1つにまとめてください。",
                    account.name
                ),
            ));
        }
        let side = match account.kind {
            Kind::Asset => Side::Debit,
            Kind::Liability => Side::Credit,
            other => {
                return Err(ComposeError::bad(
                    &field,
                    format!(
                        "期首残高は、資産と負債の科目だけに入れられます。「{}」は{}です。",
                        account.name,
                        other.label()
                    ),
                ));
            }
        };
        if [OWNER_DRAW, OWNER_INV].contains(&code.as_str()) {
            return Err(ComposeError::bad(
                &field,
                "事業主貸と事業主借は、年の初めに0から始まるので、期首残高に入れません。",
            ));
        }
        if !(0..=MAX_AMOUNT).contains(&amount.get()) {
            return Err(ComposeError::AmountRange {
                field: format!("balances[{i}].amount"),
                amount: amount.get(),
            });
        }
        if *amount == Yen::ZERO {
            continue;
        }
        net += match side {
            Side::Debit => i128::from(amount.get()),
            Side::Credit => -i128::from(amount.get()),
        };
        c.lines.push(Line::new(account.code.clone(), side, *amount));
    }
    if c.lines.is_empty() {
        return Ok(None);
    }
    if net != 0 {
        let capital = i64::try_from(net.abs())
            .ok()
            .filter(|v| *v <= MAX_AMOUNT)
            .ok_or_else(|| ComposeError::AmountRange {
                field: "balances".to_owned(),
                amount: i64::try_from(net).unwrap_or(i64::MAX),
            })?;
        let side = if net > 0 { Side::Credit } else { Side::Debit };
        let code = chart.get(CAPITAL).map(|a| a.code.clone()).ok_or_else(|| {
            ComposeError::UnknownAccount {
                field: "balances".to_owned(),
                code: CAPITAL.to_owned(),
            }
        })?;
        c.lines.push(Line::new(code, side, Yen::new(capital)));
    }
    c.validated().map(Some)
}
