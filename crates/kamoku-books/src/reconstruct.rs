// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::compose::compose_simple_with;
use crate::compose_parts::business_share;
use crate::{Chart, ComposeSettings, SimpleInput, SimpleKind, Source, TaxMode};
use kamoku_jp_tax::Rounding;
use kamoku_ledger::{Entry, Side, Yen};
use std::collections::BTreeMap;

// 組み立てが作らない鍵。呼び出し側が足すので、照合では除く。
const ADDED_BY_CALLER: [&str; 3] = ["source", "draft", "invoice_no"];

/// 単純な形で作った仕訳から、元の入力を組み立て直します（訂正の画面の初期値）。
/// 組み立て直した入力をもう一度組み立てて、明細・摘要・付随情報が一致したときだけ返します。
/// 仕訳の形・期首残高・事業分が0円の支出（科目の行が無い）・古い形の仕訳は `None`。
/// 端数処理の設定が後で変わっても読めるよう、3通りの端数処理を順に試します。
#[must_use]
pub fn reconstruct_simple(entry: &Entry, chart: &Chart) -> Option<SimpleInput> {
    let meta = entry.meta();
    let kind = match meta.get("kind")?.as_str() {
        "expense" => SimpleKind::Expense,
        "income" => SimpleKind::Income,
        "transfer" => SimpleKind::Transfer,
        _ => return None,
    };
    let lines = entry.lines();
    let first = lines.first()?;
    let last = lines.last()?;
    let mut input = SimpleInput {
        kind,
        date: entry.date(),
        amount: entry.total().get(),
        account: None,
        wallet: String::new(),
        to_wallet: None,
        tax_code: None,
        partner: meta.get("partner").cloned().unwrap_or_default(),
        description: entry.description().to_owned(),
        business_pct: None,
        memo: String::new(),
    };
    match kind {
        SimpleKind::Expense => {
            let pct: u8 = meta.get("business_pct")?.parse().ok()?;
            // 事業分が0円なら科目の行が無く、元の科目は分からない。
            if business_share(entry.total(), pct)? == Yen::ZERO {
                return None;
            }
            input.account = Some(first.account.as_str().to_owned());
            input.tax_code.clone_from(&first.tax_code);
            last.account.as_str().clone_into(&mut input.wallet);
            input.memo.clone_from(&last.memo);
            input.business_pct = Some(pct);
        }
        SimpleKind::Income => {
            first.account.as_str().clone_into(&mut input.wallet);
            input.memo.clone_from(&first.memo);
            input.account = Some(last.account.as_str().to_owned());
            input.tax_code.clone_from(&last.tax_code);
        }
        SimpleKind::Transfer => {
            input.to_wallet = Some(first.account.as_str().to_owned());
            last.account.as_str().clone_into(&mut input.wallet);
            input.memo.clone_from(&last.memo);
        }
    }
    if first.side != Side::Debit || last.side != Side::Credit {
        return None;
    }
    let source = meta
        .get("source")
        .and_then(|s| Source::parse(s))
        .unwrap_or(Source::Manual);
    let expected = without_caller_keys(meta);
    [Rounding::Floor, Rounding::Round, Rounding::Ceil]
        .into_iter()
        .any(|rounding| {
            let settings = ComposeSettings {
                rounding,
                tax_mode: TaxMode::Taxable,
                source,
            };
            compose_simple_with(&input, chart, settings, false).is_ok_and(|c| {
                c.lines == lines
                    && c.description == entry.description()
                    && without_caller_keys(&c.meta) == expected
            })
        })
        .then_some(input)
}

fn without_caller_keys(meta: &BTreeMap<String, String>) -> BTreeMap<&str, &str> {
    meta.iter()
        .filter(|(k, _)| !ADDED_BY_CALLER.contains(&k.as_str()))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}
