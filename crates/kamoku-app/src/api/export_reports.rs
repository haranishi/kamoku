// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 帳票のCSV（`/export/trial-balance.csv` など）。数字は画面の帳票と同じ関数から作る。

use super::export::{csv_encoding, csv_reply, period_name};
use super::reports::{
    make_balance_sheet, make_income_statement, make_ledger, make_tax_summary, make_trial_balance,
    period,
};
use super::{ApiError, Call, Reply};
use crate::csv::{Cell, Writer, num, text};
use crate::state::State;
use kamoku_books::{BsRow, IsRow, format_date};

fn blank() -> Cell {
    text("")
}

/// `GET /export/trial-balance.csv?from&to|year`
pub(crate) fn export_trial_balance(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let encoding = csv_encoding(call.query)?;
    let (from, to) = period(call.query, call.today)?;
    let tb = make_trial_balance(state, call)?;
    let mut w = Writer::default();
    w.row(
        &[
            "コード",
            "科目名",
            "分類",
            "期首残高",
            "借方",
            "貸方",
            "期末残高",
        ]
        .map(text),
    );
    for r in &tb.rows {
        w.row(&[
            text(r.code.clone()),
            text(r.name.clone()),
            text(r.kind.label()),
            num(r.opening.get()),
            num(r.debit.get()),
            num(r.credit.get()),
            num(r.closing.get()),
        ]);
    }
    w.row(&[
        text("合計"),
        blank(),
        blank(),
        blank(),
        num(tb.total_debit.get()),
        num(tb.total_credit.get()),
        blank(),
    ]);
    Ok(csv_reply(
        w,
        encoding,
        format!("kamoku-trial-balance-{}.csv", period_name(from, to)),
    ))
}

fn is_rows(w: &mut Writer, section: &str, rows: &[IsRow], monthly: bool) {
    for r in rows {
        let mut cells = vec![text(section), text(r.code.clone()), text(r.name.clone())];
        if monthly {
            cells.extend(r.months.iter().flatten().map(|v| num(v.get())));
        }
        cells.push(num(r.amount.get()));
        w.row(&cells);
    }
}

/// `GET /export/income-statement.csv?from&to&by=total|month`
pub(crate) fn export_income_statement(
    state: &mut State,
    call: &Call<'_>,
) -> Result<Reply, ApiError> {
    let encoding = csv_encoding(call.query)?;
    let (from, to) = period(call.query, call.today)?;
    let is = make_income_statement(state, call)?;
    let months = is.months.as_ref();
    let mut w = Writer::default();
    let mut header = vec![text("区分"), text("コード"), text("科目名")];
    if months.is_some() {
        header.extend((1..=12).map(|m| text(format!("{m}月"))));
    }
    header.push(text("金額"));
    w.row(&header);
    let monthly = months.is_some();
    is_rows(&mut w, "収益", &is.revenue.rows, monthly);
    is_rows(&mut w, "費用", &is.expense.rows, monthly);
    let total_row = |label: &str, pick: fn(&kamoku_books::MonthTotal) -> i64, total: i64| {
        let mut cells = vec![text(label), blank(), blank()];
        if let Some(months) = months {
            cells.extend(months.iter().map(|m| num(pick(m))));
        }
        cells.push(num(total));
        cells
    };
    w.row(&total_row(
        "収益合計",
        |m| m.revenue.get(),
        is.revenue.total.get(),
    ));
    w.row(&total_row(
        "費用合計",
        |m| m.expense.get(),
        is.expense.total.get(),
    ));
    w.row(&total_row(
        "当期純利益",
        |m| m.net_income.get(),
        is.net_income.get(),
    ));
    Ok(csv_reply(
        w,
        encoding,
        format!("kamoku-income-statement-{}.csv", period_name(from, to)),
    ))
}

fn bs_rows(w: &mut Writer, section: &str, rows: &[BsRow], depth: i64) {
    for r in rows {
        w.row(&[
            text(section),
            num(depth),
            text(r.code.clone()),
            text(r.name.clone()),
            num(r.amount.get()),
            num(r.own.get()),
        ]);
        bs_rows(w, section, &r.children, depth + 1);
    }
}

/// `GET /export/balance-sheet.csv?as_of=`
pub(crate) fn export_balance_sheet(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let encoding = csv_encoding(call.query)?;
    let bs = make_balance_sheet(state, call)?;
    let mut w = Writer::default();
    w.row(&["区分", "階層", "コード", "科目名", "金額", "うち自分の分"].map(text));
    for (label, section) in [
        ("資産", &bs.assets),
        ("負債", &bs.liabilities),
        ("資本", &bs.equity),
    ] {
        bs_rows(&mut w, label, &section.rows, 1);
        w.row(&[
            text(format!("{label}合計")),
            blank(),
            blank(),
            blank(),
            num(section.total.get()),
            blank(),
        ]);
    }
    Ok(csv_reply(
        w,
        encoding,
        format!("kamoku-balance-sheet-{}.csv", format_date(bs.as_of)),
    ))
}

/// `GET /export/ledger.csv?account&from&to`
pub(crate) fn export_ledger(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let encoding = csv_encoding(call.query)?;
    let (from, to) = period(call.query, call.today)?;
    let ledger = make_ledger(state, call)?;
    let mut w = Writer::default();
    w.row(&["日付", "仕訳No", "摘要", "相手科目", "借方", "貸方", "残高"].map(text));
    w.row(&[
        blank(),
        blank(),
        text("期首残高"),
        blank(),
        blank(),
        blank(),
        num(ledger.opening.get()),
    ]);
    for r in &ledger.rows {
        w.row(&[
            text(format_date(r.date)),
            num(i64::try_from(r.no).map_err(ApiError::internal)?),
            text(r.description.clone()),
            text(r.counter.clone()),
            num(r.debit.get()),
            num(r.credit.get()),
            num(r.balance.get()),
        ]);
    }
    w.row(&[
        blank(),
        blank(),
        text("期末残高"),
        blank(),
        blank(),
        blank(),
        num(ledger.closing.get()),
    ]);
    let name = format!(
        "kamoku-ledger-{}-{}.csv",
        ledger.account,
        period_name(from, to)
    );
    Ok(csv_reply(w, encoding, name))
}

/// `GET /export/tax-summary.csv?from&to`
pub(crate) fn export_tax_summary(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let encoding = csv_encoding(call.query)?;
    let (from, to) = period(call.query, call.today)?;
    let t = make_tax_summary(state, call)?;
    let mut w = Writer::default();
    w.row(&["税区分", "名前", "課税標準（税込）", "税額"].map(text));
    for r in &t.rows {
        w.row(&[
            text(r.tax_code.as_str()),
            text(r.label.clone()),
            num(r.gross.get()),
            num(r.tax.get()),
        ]);
    }
    w.row(&[text("売上税額"), blank(), blank(), num(t.sales_tax.get())]);
    w.row(&[
        text("仕入税額"),
        blank(),
        blank(),
        num(t.purchase_tax.get()),
    ]);
    w.row(&[
        text("見込み額（参考）"),
        blank(),
        blank(),
        num(t.estimate.get()),
    ]);
    if !t.transitional.is_empty() {
        w.row(
            &[
                "経過措置の割合（%）",
                "支払額（税込）",
                "税額",
                "控除できる額",
            ]
            .map(text),
        );
        for r in &t.transitional {
            w.row(&[
                num(i64::from(r.percent)),
                num(r.gross.get()),
                num(r.tax.get()),
                num(r.deductible.get()),
            ]);
        }
    }
    w.row(&[text(t.disclaimer.clone())]);
    Ok(csv_reply(
        w,
        encoding,
        format!("kamoku-tax-summary-{}.csv", period_name(from, to)),
    ))
}
