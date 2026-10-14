// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 日本の消費税計算。**税理士の確認前（未レビュー）**です。
//! 根拠は各表の `source` のURLで、年度の改正により変わります。
//! 改正時は表を更新し、過去の日付への結果は変えないでください。
//! 申告書の計算（国税7.8%と地方消費税2.2%の分離など）は対象外です。
//! 中間計算には整数の `i128` を使用します。

use kamoku_ledger::Yen;
use std::fmt;
use time::{Date, Month};

/// 消費税と地方消費税を合わせた税率。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rate {
    /// 標準税率10%。
    Standard,
    /// 軽減税率8%。
    Reduced,
}
impl Rate {
    /// 消費税と地方消費税の合計割合。
    #[must_use]
    pub const fn percent(self) -> i64 {
        match self {
            Self::Standard => 10,
            Self::Reduced => 8,
        }
    }
}
/// 非負の金額に対する円未満の端数処理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// 切捨て。
    Floor,
    /// 四捨五入（0.5は切上げ）。
    Round,
    /// 切上げ。
    Ceil,
}
/// 税抜金額と税額。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxSplit {
    /// 税抜金額。
    pub net: Yen,
    /// 税額。
    pub tax: Yen,
}
/// 税計算の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaxError {
    /// 負の入力金額。
    NegativeAmount,
    /// 計算結果の桁あふれ。
    Overflow,
    /// 100を超える経過措置割合。
    InvalidPercent,
}
impl fmt::Display for TaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NegativeAmount => "負の金額は使用できません",
            Self::Overflow => "税計算が桁あふれしました",
            Self::InvalidPercent => "割合は100以下で指定してください",
        })
    }
}
impl std::error::Error for TaxError {}
fn nonnegative(value: Yen) -> Result<i128, TaxError> {
    if value < Yen::ZERO {
        return Err(TaxError::NegativeAmount);
    }
    Ok(i128::from(value.get()))
}
fn rounded(numerator: i128, denominator: i128, rounding: Rounding) -> Result<Yen, TaxError> {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let increment = match rounding {
        Rounding::Floor => false,
        Rounding::Round => remainder.checked_mul(2).ok_or(TaxError::Overflow)? >= denominator,
        Rounding::Ceil => remainder != 0,
    };
    let result = quotient
        .checked_add(i128::from(increment))
        .ok_or(TaxError::Overflow)?;
    Ok(Yen::new(
        i64::try_from(result).map_err(|_| TaxError::Overflow)?,
    ))
}
/// 税込金額から税抜金額と税額を計算します。
/// 適格請求書に記載する消費税額等の端数処理は「**一の適格請求書につき、税率ごとに1回**」で、
/// 切捨て・四捨五入・切上げなど、どの方法にするかは事業者が選べます（この関数は3通りに対応します）。
/// 明細ごとに呼んで足し合わせてはいけません。呼び出し側は税率ごとの合計額に1回だけ呼びます。
/// 出典: <https://www.nta.go.jp/taxes/shiraberu/zeimokubetsu/shohi/keigenzeiritsu/pdf/qa/57.pdf>
/// （適格請求書等保存方式のQ&A 問57）。
///
/// # Errors
/// 負の金額または桁あふれを返します。
pub fn split_tax_included(
    gross: Yen,
    rate: Rate,
    rounding: Rounding,
) -> Result<TaxSplit, TaxError> {
    let numerator = nonnegative(gross)?
        .checked_mul(i128::from(rate.percent()))
        .ok_or(TaxError::Overflow)?;
    let tax = rounded(numerator, i128::from(100 + rate.percent()), rounding)?;
    let net = gross.checked_sub(tax).ok_or(TaxError::Overflow)?;
    Ok(TaxSplit { net, tax })
}
/// 税抜金額から税額を計算します。
/// 適格請求書に記載する消費税額等の端数処理は「**一の適格請求書につき、税率ごとに1回**」で、
/// 切捨て・四捨五入・切上げなど、どの方法にするかは事業者が選べます（この関数は3通りに対応します）。
/// 明細ごとに呼んで足し合わせてはいけません。呼び出し側は税率ごとの合計額に1回だけ呼びます。
/// 出典: <https://www.nta.go.jp/taxes/shiraberu/zeimokubetsu/shohi/keigenzeiritsu/pdf/qa/57.pdf>
/// （適格請求書等保存方式のQ&A 問57）。
///
/// # Errors
/// 負の金額または税額・税込合計の桁あふれを返します。
pub fn add_tax_excluded(net: Yen, rate: Rate, rounding: Rounding) -> Result<TaxSplit, TaxError> {
    let numerator = nonnegative(net)?
        .checked_mul(i128::from(rate.percent()))
        .ok_or(TaxError::Overflow)?;
    let tax = rounded(numerator, 100, rounding)?;
    net.checked_add(tax).ok_or(TaxError::Overflow)?;
    Ok(TaxSplit { net, tax })
}
/// 経過措置で認められる円未満の端数処理。切上げは認められません。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditRounding {
    /// 切捨て。
    Floor,
    /// 四捨五入。
    Round,
}
/// 免税事業者等からの課税仕入れで仕入税額とみなす額を計算します。
/// 標準は `payment × 78 × percent / (1100 × 100)`、軽減は
/// `payment × 624 × percent / (10800 × 100)` を整数で計算します。
/// この関数が持つのは、問130の直接計算の式だけです。「積上げ計算」では課税仕入れの都度、
/// 「割戻し計算」では課税期間中の支払対価の合計に対して使います。どちらの方式かは、すでに採用している
/// 仕入税額の計算方式に従って、呼び出し側が決めます。区分管理や仮払消費税額等を通す方法は扱いません。
/// 出典: <https://www.nta.go.jp/taxes/shiraberu/zeimokubetsu/shohi/keigenzeiritsu/pdf/qa/130.pdf>
/// （問130・令和8年4月改訂）。
///
/// # Errors
/// 負の金額・100超の割合・桁あふれを返します。
pub fn transitional_credit(
    payment: Yen,
    rate: Rate,
    percent: u8,
    rounding: CreditRounding,
) -> Result<Yen, TaxError> {
    let payment = nonnegative(payment)?;
    if percent > 100 {
        return Err(TaxError::InvalidPercent);
    }
    let (factor, denominator) = match rate {
        Rate::Standard => (78, 110_000),
        Rate::Reduced => (624, 1_080_000),
    };
    let numerator = payment
        .checked_mul(factor)
        .and_then(|v| v.checked_mul(i128::from(percent)))
        .ok_or(TaxError::Overflow)?;
    rounded(
        numerator,
        denominator,
        match rounding {
            CreditRounding::Floor => Rounding::Floor,
            CreditRounding::Round => Rounding::Round,
        },
    )
}
/// 両端の日付を含む経過措置の割合と根拠。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionalRule {
    /// 適用開始日。
    pub from: Date,
    /// 適用終了日。
    pub to: Date,
    /// 仕入税額とみなす割合。
    pub percent: u8,
    /// 根拠資料のURL。
    pub source: &'static str,
}
const SOURCE: &str =
    "https://www.nta.go.jp/taxes/shiraberu/zeimokubetsu/shohi/keigenzeiritsu/pdf/qa/113-3.pdf";
/// 経過措置の表（[`transitional_rules`]）を、根拠の資料と最後に突き合わせた日（`YYYY-MM-DD`）。
/// 集計の画面に「使った表の最終確認日」として出します。表を確かめ直したら、この日付も更新してください。
pub const TRANSITIONAL_TABLE_CHECKED_ON: &str = "2026-10-14";
/// 経過措置の表。問113-3・令和8年4月追加、確認日2026-10-14。
/// 改正時も過去の日付に適用する行を保持してください。
/// この表が持つのは割合と期間だけです。適用の条件（請求書等と帳簿の保存）、一の仕入先ごとの上限
/// （令和8年10月1日以後に開始する課税期間は税込1億円）、2割特例・3割特例の適用判定は含みません。
#[must_use]
pub fn transitional_rules() -> Vec<TransitionalRule> {
    [
        (2023, 2026, 80),
        (2026, 2028, 70),
        (2028, 2030, 50),
        (2030, 2031, 30),
    ]
    .into_iter()
    .filter_map(|(start, end, percent)| {
        let from = Date::from_calendar_date(start, Month::October, 1).ok()?;
        let to = Date::from_calendar_date(end, Month::September, 30).ok()?;
        Some(TransitionalRule {
            from,
            to,
            percent,
            source: SOURCE,
        })
    })
    .collect()
}
/// 課税仕入れの時期に適用する経過措置の割合。表の外は0です。
/// 判定する日付は**課税仕入れの時期**で、支払日ではありません。原則として、役務の提供は完了した日、
/// 商品は引渡しのあった日です。
#[must_use]
pub fn transitional_percent(purchase_date: Date) -> u8 {
    transitional_rules()
        .iter()
        .find(|r| r.from <= purchase_date && purchase_date <= r.to)
        .map_or(0, |r| r.percent)
}
/// 適格請求書発行事業者の登録番号。形式だけを検証します。
/// チェックデジットは検証しません（法人番号には公開仕様がありますが、個人の登録番号の検証方法は未確認）。
/// 実在確認は別の仕事です。正規化は呼び出し側の仕事です。
/// 請求書での表記（全角・半角など）の細かい扱いは確認していません。半角だけを受け付けるのは、この型の入力の約束です。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvoiceNumber(String);
/// 登録番号の形式の不正。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvoiceNumberError {
    /// 半角大文字Tと半角数字13桁という形式の違反。
    InvalidFormat,
}
impl fmt::Display for InvoiceNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("登録番号は半角大文字Tと半角数字13桁で指定してください")
    }
}
impl std::error::Error for InvoiceNumberError {}
impl InvoiceNumber {
    /// 半角大文字Tと半角数字13桁を検証します。小文字・全角・空白・ハイフンを拒否します。
    ///
    /// # Errors
    /// 形式が異なる場合に失敗します。
    pub fn parse(s: &str) -> Result<Self, InvoiceNumberError> {
        if s.len() != 14
            || !s
                .strip_prefix('T')
                .is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(InvoiceNumberError::InvalidFormat);
        }
        Ok(Self(s.to_owned()))
    }
    /// 保存された登録番号。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for InvoiceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
