// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 勘定科目・仕訳の組み立て・帳票（試算表、損益計算書、貸借対照表、消費税の集計）を計算する。入出力を持たない。
//!
//! - 科目: [`Account`]、検査済みの集合 [`Chart`]、標準の科目 [`standard_accounts`]。
//! - 組み立て: [`compose_simple`]（支出・収入・振替）、[`compose_journal`]、[`compose_opening`]。
//!   画面のプレビューと保存は、どちらも [`Composed`] から作る。訂正の初期値は [`reconstruct_simple`]。
//! - 帳票: [`trial_balance`] [`income_statement`] [`balance_sheet`] [`account_ledger`]
//!   [`tax_summary`] [`monthly_series`] [`wallet_balances`]。出力の型は、そのままJSONにできる。
//!
//! 金額は整数の円（`Yen`）で持ち、桁あふれは `Result` で返します。消費税の集計は参考値で、
//! 税理士の確認を受けていません。

mod account;
mod chart;
mod compose;
mod compose_error;
mod compose_parts;
mod input;
mod journal;
mod reconstruct;
mod reports;
mod ser;
mod standard;
mod tax;

pub use account::{Account, Kind, PROTECTED_CODES, Wallet, WalletType};
pub use chart::{Chart, ChartError, REQUIRED_ACCOUNTS};
pub use compose::compose_simple;
pub use compose_error::ComposeError;
pub use compose_parts::{Composed, LineView, MAX_AMOUNT};
pub use input::{
    ComposeSettings, JournalInput, JournalLineInput, SimpleInput, SimpleKind, Source, TaxMode,
};
pub use journal::{compose_journal, compose_opening};
pub use reconstruct::reconstruct_simple;
pub use reports::{
    AccountLedger, BalanceSheet, BsRow, IncomeStatement, IsRow, LedgerRow, MAX_REPORT_AMOUNT,
    MonthPoint, MonthTotal, NET_INCOME_ROW_CODE, ReportError, Section, TaxRow, TaxSummary,
    TransitionalRow, TrialBalance, TrialRow, WalletBalances, WalletItem, account_ledger,
    balance_sheet, income_statement, monthly_series, tax_disclaimer, tax_summary, trial_balance,
    wallet_balances,
};
pub use ser::{format_date, parse_date};
pub use standard::{BLANK_FORM_LINE, standard_accounts, standard_accounts_markdown};
pub use tax::{TaxClass, TaxCode, tax_included};

/// 端数処理の種類（`kamoku-jp-tax` のものをそのまま使う）。
pub use kamoku_jp_tax::Rounding;
