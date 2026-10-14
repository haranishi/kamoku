// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 単純な形の入力から明細を組み立てる（`docs/spec/api-entries.md` の「明細の組み立て」）。

use crate::compose_parts::{
    Composed, Lookup, OWNER_DRAW, business_share, check_amount, check_text, parse_tax, taxed,
};
use crate::{
    Account, Chart, ComposeError, ComposeSettings, Kind, SimpleInput, SimpleKind, TaxClass,
    TaxCode, TaxMode,
};
use kamoku_jp_tax::transitional_percent;
use kamoku_ledger::{Line, Side, Yen};

/// 単純な形の入力（支出・収入・振替）から明細を組み立てます。保存はしません。
///
/// # Errors
/// 金額・科目・口座・税区分・割合・文字の長さの違反を、通信の約束の `code` つきで返します。
pub fn compose_simple(
    input: &SimpleInput,
    chart: &Chart,
    settings: &ComposeSettings,
) -> Result<Composed, ComposeError> {
    compose_simple_with(input, chart, *settings, true)
}

// 組み立て直し（reconstruct_simple）は `check_enabled = false` で呼ぶ。記帳した後に無効にした
// 科目でも、過去の仕訳を訂正の画面に戻せるようにするため。
pub(crate) fn compose_simple_with(
    input: &SimpleInput,
    chart: &Chart,
    settings: ComposeSettings,
    check_enabled: bool,
) -> Result<Composed, ComposeError> {
    check_amount("amount", input.amount)?;
    check_text("description", &input.description, "摘要")?;
    check_text("partner", &input.partner, "取引先の名前")?;
    check_text("memo", &input.memo, "メモ")?;
    if input.business_pct.is_some_and(|p| p > 100) {
        return Err(ComposeError::bad(
            "business_pct",
            "事業の割合は0から100までの整数で入れてください。",
        ));
    }
    if input.kind != SimpleKind::Transfer
        && input.to_wallet.as_deref().is_some_and(|w| !w.is_empty())
    {
        return Err(ComposeError::bad(
            "to_wallet",
            "移す先の口座は、振替のときだけ選べます。種別を振替にするか、移す先を空にしてください。",
        ));
    }
    let lookup = Lookup {
        chart,
        check_enabled,
    };
    let wallet = lookup.wallet("wallet", Some(&input.wallet))?;
    let mut composed = Composed::new(
        input.date,
        &input.description,
        input.kind.as_str(),
        settings.source,
        &input.partner,
    );
    match input.kind {
        SimpleKind::Expense => expense(&mut composed, input, wallet, &lookup, settings)?,
        SimpleKind::Income => income(&mut composed, input, wallet, &lookup, settings)?,
        SimpleKind::Transfer => transfer(&mut composed, input, wallet, &lookup)?,
    }
    composed.validated()
}

fn counter_account<'a>(
    input: &SimpleInput,
    wallet: &Account,
    lookup: &Lookup<'a>,
) -> Result<(&'a Account, Option<TaxCode>), ComposeError> {
    let account = lookup.account("account", input.account.as_deref())?;
    if account.code == wallet.code {
        return Err(ComposeError::bad(
            "account",
            "科目と口座が同じです。相手の科目を選び直してください。",
        ));
    }
    let code = parse_tax("tax_code", input.tax_code.as_deref())?;
    if account.is_wallet() {
        // 口座どうしのお金の移動に税区分や割合を付けると、カードの購入と引き落としで二重に数えるため。
        if code.is_some() {
            return Err(ComposeError::bad(
                "tax_code",
                format!(
                    "相手が口座（{}）のときは、税区分を付けられません。カードで買ったときは「費用／カード」、引き落としは「カード／預金」と、2回に分けて入れてください。",
                    account.name
                ),
            ));
        }
        if input.business_pct.is_some_and(|p| p != 100) {
            return Err(ComposeError::bad(
                "business_pct",
                format!(
                    "相手が口座（{}）のときは、事業の割合を使えません。割合を100にするか、空にしてください。",
                    account.name
                ),
            ));
        }
    }
    Ok((account, code))
}

fn expense(
    c: &mut Composed,
    input: &SimpleInput,
    wallet: &Account,
    lookup: &Lookup<'_>,
    settings: ComposeSettings,
) -> Result<(), ComposeError> {
    let (account, code) = counter_account(input, wallet, lookup)?;
    let pct = if account.is_wallet() {
        100
    } else {
        input.business_pct.unwrap_or(account.default_pct).min(100)
    };
    let amount = Yen::new(input.amount);
    let business = business_share(amount, pct).ok_or_else(|| ComposeError::AmountRange {
        field: "amount".to_owned(),
        amount: input.amount,
    })?;
    let private = amount.checked_sub(business).unwrap_or(Yen::ZERO);
    c.meta.insert("business_pct".to_owned(), pct.to_string());
    if business > Yen::ZERO {
        let line = Line::new(account.code.clone(), Side::Debit, business);
        c.lines.push(taxed(line, code, settings.rounding)?);
        tax_warnings(c, code, account, false, settings);
    } else {
        c.warnings.push(format!(
            "事業分が0円です（割合{pct}%）。全額を事業主貸に入れ、「{}」の明細は作りません。",
            account.name
        ));
    }
    if private > Yen::ZERO {
        let owner = lookup.system_code(OWNER_DRAW)?;
        c.lines.push(
            Line::new(owner, Side::Debit, private).with_memo(&format!("家事按分 {}%", 100 - pct)),
        );
    }
    c.lines
        .push(Line::new(wallet.code.clone(), Side::Credit, amount).with_memo(&input.memo));
    if account.kind == Kind::Revenue {
        c.warnings.push(format!(
            "支出の科目に、収益の「{}」が選ばれています。経費なら費用の科目を選んでください。",
            account.name
        ));
    }
    Ok(())
}

fn income(
    c: &mut Composed,
    input: &SimpleInput,
    wallet: &Account,
    lookup: &Lookup<'_>,
    settings: ComposeSettings,
) -> Result<(), ComposeError> {
    if input.business_pct.is_some_and(|p| p != 100) {
        return Err(ComposeError::bad(
            "business_pct",
            "収入には事業の割合を付けられません。割合を100にするか、空にしてください。",
        ));
    }
    let (account, code) = counter_account(input, wallet, lookup)?;
    let amount = Yen::new(input.amount);
    c.lines
        .push(Line::new(wallet.code.clone(), Side::Debit, amount).with_memo(&input.memo));
    let line = Line::new(account.code.clone(), Side::Credit, amount);
    c.lines.push(taxed(line, code, settings.rounding)?);
    tax_warnings(c, code, account, true, settings);
    if account.kind == Kind::Expense {
        c.warnings.push(format!(
            "収入の科目に、費用の「{}」が選ばれています。売上なら収益の科目を選んでください。",
            account.name
        ));
    }
    Ok(())
}

fn transfer(
    c: &mut Composed,
    input: &SimpleInput,
    wallet: &Account,
    lookup: &Lookup<'_>,
) -> Result<(), ComposeError> {
    if input.account.as_deref().is_some_and(|a| !a.is_empty()) {
        return Err(ComposeError::bad(
            "account",
            "振替には科目を付けません。科目を空にするか、種別を支出か収入にしてください。",
        ));
    }
    if input.tax_code.as_deref().is_some_and(|t| !t.is_empty()) {
        return Err(ComposeError::bad(
            "tax_code",
            "振替には税区分を付けられません。税区分を空にしてください。",
        ));
    }
    if input.business_pct.is_some_and(|p| p != 100) {
        return Err(ComposeError::bad(
            "business_pct",
            "振替には事業の割合を付けられません。割合を100にするか、空にしてください。",
        ));
    }
    let to = lookup.wallet("to_wallet", input.to_wallet.as_deref())?;
    if to.code == wallet.code {
        return Err(ComposeError::bad(
            "to_wallet",
            "移す元と移す先が同じ口座です。別の口座を選んでください。",
        ));
    }
    let amount = Yen::new(input.amount);
    c.lines
        .push(Line::new(to.code.clone(), Side::Debit, amount));
    c.lines
        .push(Line::new(wallet.code.clone(), Side::Credit, amount).with_memo(&input.memo));
    Ok(())
}

pub(crate) fn tax_warnings(
    c: &mut Composed,
    code: Option<TaxCode>,
    account: &Account,
    income: bool,
    settings: ComposeSettings,
) {
    let profit_or_loss = matches!(account.kind, Kind::Revenue | Kind::Expense);
    let Some(code) = code else {
        if settings.tax_mode == TaxMode::Taxable && profit_or_loss && !account.is_wallet() {
            c.warnings.push(format!(
                "「{}」の税区分が空です。課税事業者の設定なので、消費税の対象か確かめてください（対象外なら空のままで構いません）。",
                account.name
            ));
        }
        return;
    };
    match (income, code.class()) {
        (false, TaxClass::Sales) => c.warnings.push(format!(
            "支出に、売上の税区分（{}）が付いています。仕入の税区分か確かめてください。",
            code.label()
        )),
        (true, TaxClass::Purchase | TaxClass::TransitionalPurchase) => c.warnings.push(format!(
            "収入に、仕入の税区分（{}）が付いています。売上の税区分か確かめてください。",
            code.label()
        )),
        _ => {}
    }
    if code.class() == TaxClass::TransitionalPurchase {
        let percent = transitional_percent(c.date);
        if percent == 0 {
            c.warnings.push(
                "経過措置の期間の外の日付です。この取引の控除は0円として集計します。".to_owned(),
            );
        } else {
            c.warnings.push(format!(
                "経過措置の割合（{percent}%）は、この日付で決まります。日付が課税仕入れの日（引渡しや役務の完了の日）か確かめてください。"
            ));
        }
    }
}
