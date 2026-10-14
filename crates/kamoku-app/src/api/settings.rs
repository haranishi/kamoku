// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 設定（`GET /settings`、`PUT /settings`）。

use super::body::{Fields, object};
use super::{ApiError, Call, Reply, ok, to_value};
use crate::state::State;
use kamoku_store::Settings;

/// `GET /settings`
pub(crate) fn settings_get(state: &mut State, _call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(to_value(state.store.settings())?)
}

fn one_of(f: &Fields<'_>, key: &str, label: &str, choices: &[&str]) -> Result<String, ApiError> {
    let value = f.req_str(key, label)?;
    if choices.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(ApiError::bad_request(format!(
            "{label}（{key}）は {} のどれかにしてください。",
            choices.join("・")
        ))
        .with_field(key))
    }
}

/// `PUT /settings`（全項目を置き換え）。`tax_mode` `rounding` `setup_done` は必須。
/// 文字列の欄は省くと空、`invoice_number` と `locked_until` は省くと `null`。
pub(crate) fn settings_put(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let tax_mode = one_of(&f, "tax_mode", "課税の区分", &["exempt", "taxable"])?;
    let rounding = one_of(&f, "rounding", "端数処理", &["floor", "round", "ceil"])?;
    let setup_done = f
        .opt_bool("setup_done")?
        .ok_or_else(|| f.missing("setup_done", "初期設定の完了"))?;
    let opened_on = f.opt_date("opened_on")?.map(kamoku_books::format_date);
    if opened_on.is_none() && setup_done {
        return Err(ApiError::rule(
            "開業日を入れてください。初期設定を終えるには、開業日が要ります。",
        )
        .with_field("opened_on"));
    }
    let locked_until = f.opt_date("locked_until")?.map(kamoku_books::format_date);
    let invoice_number = f
        .opt_str("invoice_number")?
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    if invoice_number.is_some() && tax_mode == "exempt" {
        return Err(ApiError::new(
            422,
            "bad_invoice_number",
            "登録番号は、課税事業者の設定のときだけ入れられます。課税の区分を確かめてください。",
        )
        .with_field("invoice_number"));
    }
    let settings = Settings {
        business_name: f.text("business_name")?,
        owner_name: f.text("owner_name")?,
        tax_mode,
        invoice_number,
        rounding,
        opened_on: opened_on.unwrap_or_default(),
        locked_until,
        setup_done,
    };
    state.store.save_settings(settings, call.now)?;
    ok(to_value(state.store.settings())?)
}
