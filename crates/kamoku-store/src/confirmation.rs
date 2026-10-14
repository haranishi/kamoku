// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Draft, Store, StoreError, masters};
use kamoku_books::{
    ComposeSettings, Rounding, SimpleInput, SimpleKind, Source, TaxMode, compose_simple,
};
use kamoku_ledger::Entry;

impl Store {
    pub(crate) fn match_draft(&self, draft: &Draft, entry: &Entry) -> Result<(), StoreError> {
        let input = SimpleInput {
            kind: if draft.direction == "out" {
                SimpleKind::Expense
            } else {
                SimpleKind::Income
            },
            date: kamoku_books::parse_date(&draft.date)
                .ok_or_else(|| StoreError::Invalid("下書きの日付".into()))?,
            amount: draft.amount,
            account: draft.account.clone(),
            wallet: draft.wallet.clone(),
            to_wallet: None,
            tax_code: draft.tax_code.clone(),
            partner: draft.partner.clone(),
            description: draft.description.clone(),
            business_pct: Some(draft.business_pct),
            memo: draft.memo.clone(),
        };
        let settings = ComposeSettings {
            rounding: match self.settings.rounding.as_str() {
                "round" => Rounding::Round,
                "ceil" => Rounding::Ceil,
                _ => Rounding::Floor,
            },
            tax_mode: if self.settings.tax_mode == "taxable" {
                TaxMode::Taxable
            } else {
                TaxMode::Exempt
            },
            source: Source::Import,
        };
        let expected = compose_simple(&input, &self.chart, &settings)?;
        // 別接続による下書きや設定の編集も、古い組み立てで確定しないようにする。
        if entry.date() != expected.date
            || entry.description() != expected.description
            || entry.lines() != expected.lines
        {
            return Err(StoreError::Invalid(
                "下書きと仕訳が一致しません。最新の下書きから組み立て直してください".into(),
            ));
        }
        for (key, value) in &expected.meta {
            if key != "source" && entry.meta().get(key) != Some(value) {
                return Err(StoreError::Invalid("下書きの付随情報が一致しません".into()));
            }
        }
        if entry.meta().keys().any(|key| {
            !expected.meta.contains_key(key)
                && !matches!(key.as_str(), "invoice_no" | "draft" | "source")
        }) {
            return Err(StoreError::Invalid("下書きにない付随情報があります".into()));
        }
        masters::invoice(entry.meta().get("invoice_no").map(String::as_str))?;
        Ok(())
    }
}
