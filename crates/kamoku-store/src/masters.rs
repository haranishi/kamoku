// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{AccountPatch, NewAccount, Settings, Store, StoreError, db_masters};
use kamoku_books::{Account, Chart, REQUIRED_ACCOUNTS};
use kamoku_ledger::AccountCode;
use rusqlite::params;
use serde_json::json;

pub(crate) fn name(s: &str) -> Result<(), StoreError> {
    if s.trim().is_empty() || s.len() > 1000 || s.contains('\0') {
        Err(StoreError::Invalid(
            "名前は空でない1000バイト以下にしてください".into(),
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn invoice(number: Option<&str>) -> Result<(), StoreError> {
    if number.is_some_and(|s| {
        s.len() != 14
            || !s
                .strip_prefix('T')
                .is_some_and(|v| v.bytes().all(|b| b.is_ascii_digit()))
    }) {
        Err(StoreError::BadInvoiceNumber)
    } else {
        Ok(())
    }
}
pub(crate) fn date(s: &str) -> Result<(), StoreError> {
    if kamoku_books::parse_date(s).is_none() {
        Err(StoreError::Invalid(
            "日付は実在するYYYY-MM-DDにしてください".into(),
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn validate_settings(s: &Settings) -> Result<(), StoreError> {
    if !matches!(s.tax_mode.as_str(), "exempt" | "taxable")
        || !matches!(s.rounding.as_str(), "floor" | "round" | "ceil")
    {
        return Err(StoreError::Invalid(
            "課税区分か端数処理を選び直してください".into(),
        ));
    }
    invoice(s.invoice_number.as_deref())?;
    if s.tax_mode == "exempt" && s.invoice_number.is_some() {
        return Err(StoreError::BadInvoiceNumber);
    }
    if !s.opened_on.is_empty() {
        date(&s.opened_on)?;
    } else if s.setup_done {
        return Err(StoreError::Invalid("開業日を指定してください".into()));
    }
    if let Some(d) = &s.locked_until {
        date(d)?;
    }
    Ok(())
}
impl Store {
    /// 設定全体を検査して置き換え、変更履歴に残します。
    ///
    /// # Errors
    /// 課税区分、登録番号、端数処理、日付または保存の失敗。
    pub fn save_settings(&mut self, settings: Settings, now: i64) -> Result<(), StoreError> {
        self.atomic(|s| {
            validate_settings(&settings)?;
            s.conn.execute(
                "UPDATE settings SET body=? WHERE id=1",
                [serde_json::to_string(&settings)?],
            )?;
            s.log(
                "settings",
                &json!({"before":s.settings,"after":settings}),
                now,
            )?;
            s.settings = settings;
            Ok(())
        })
    }
    /// 口座w01〜、科目x01〜で採番して追加します。
    ///
    /// # Errors
    /// 分類・親・割合・名前の不正、保存の失敗。
    pub fn add_account(&mut self, input: NewAccount, now: i64) -> Result<Account, StoreError> {
        self.atomic(|s| {
            name(&input.name)?;
            let prefix = if input.wallet.is_some() { "w" } else { "x" };
            let max = s
                .chart
                .iter()
                .filter_map(|a| a.code.as_str().strip_prefix(prefix)?.parse::<u32>().ok())
                .max()
                .unwrap_or(0);
            let next = max.checked_add(1).ok_or(StoreError::AmountRange)?;
            let code = AccountCode::new(&format!("{prefix}{next:02}"))
                .map_err(|e| StoreError::Invalid(e.to_string()))?;
            let parent = input
                .parent
                .or_else(|| {
                    input
                        .wallet
                        .as_ref()
                        .map(|w| w.kind.default_parent().into())
                })
                .map(|p| AccountCode::new(&p))
                .transpose()
                .map_err(|e| StoreError::Invalid(e.to_string()))?;
            let account = Account {
                code,
                name: input.name,
                reading: input.reading,
                kind: input.kind,
                parent,
                wallet: input.wallet,
                default_tax: input.default_tax,
                default_pct: input.default_pct,
                form_line: None,
                enabled: true,
                system: false,
                sort: 1000_u32.checked_add(next).ok_or(StoreError::AmountRange)?,
            };
            let mut all = s.chart.iter().cloned().collect::<Vec<_>>();
            all.push(account.clone());
            let chart = Chart::new(all).map_err(|e| StoreError::Invalid(e.to_string()))?;
            db_masters::insert_account(&s.conn, &account)?;
            s.log("account.add", &json!(account), now)?;
            s.chart = chart;
            Ok(account)
        })
    }
    /// 許可された科目の属性だけを変更します。必須の科目は無効にできません。
    ///
    /// # Errors
    /// 対象なし、保護された変更、不正な割合または保存の失敗。
    pub fn update_account(
        &mut self,
        code: &str,
        patch: AccountPatch,
        now: i64,
    ) -> Result<Account, StoreError> {
        self.atomic(|s|{
            let mut a=s.chart.get(code).cloned().ok_or(StoreError::NotFound)?;
            let before=a.clone();
            if a.is_protected() {return Err(StoreError::ProtectedAccount);}
            if patch.enabled==Some(false)&&REQUIRED_ACCOUNTS.iter().any(|(c,_)|*c==code){return Err(StoreError::ProtectedAccount);}
            if let Some(v)=patch.name{name(&v)?;a.name=v;}
            if let Some(v)=patch.reading{a.reading=v;}
            if let Some(v)=patch.enabled{a.enabled=v;}
            if let Some(v)=patch.default_tax{a.default_tax=v;}
            if let Some(v)=patch.default_pct{a.default_pct=v;}
            let all=s.chart.iter().map(|x|if x.code==a.code{a.clone()}else{x.clone()}).collect();
            let chart=Chart::new(all).map_err(|e|StoreError::Invalid(e.to_string()))?;
            s.conn.execute("UPDATE accounts SET name=?,reading=?,enabled=?,default_tax=?,default_pct=? WHERE code=?",params![a.name,a.reading,a.enabled,a.default_tax.map(kamoku_books::TaxCode::as_str),a.default_pct,code])?;
            s.log("account.update",&json!({"before":before,"after":a}),now)?;s.chart=chart;Ok(a)
        })
    }
}
