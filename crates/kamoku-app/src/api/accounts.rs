// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 勘定科目と口座（`/accounts`、`/wallets`）。

use super::body::{Fields, object};
use super::{ApiError, Call, Reply, created, ok, to_value};
use crate::state::State;
use kamoku_books::{Kind, TaxCode, Wallet, WalletType, wallet_balances};
use kamoku_store::{AccountPatch, NewAccount};
use serde_json::{Value, json};

/// `GET /accounts?kind=&wallet=1&include_disabled=1`。科目は数が限られるので、一覧を区切らず全部返す。
pub(crate) fn accounts_list(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let kind = call
        .query
        .get("kind")
        .map(|k| {
            Kind::parse(k).ok_or_else(|| {
                ApiError::bad_request(
                    "kind は asset・liability・equity・revenue・expense のどれかにしてください。",
                )
                .with_field("kind")
            })
        })
        .transpose()?;
    let wallet = match call.query.get("wallet") {
        None => None,
        Some("1" | "true") => Some(true),
        Some("0" | "false") => Some(false),
        Some(_) => {
            return Err(
                ApiError::bad_request("wallet は 1 か 0 にしてください。").with_field("wallet")
            );
        }
    };
    let include_disabled = call.query.flag("include_disabled");
    let items = state
        .store
        .chart()
        .iter()
        .filter(|a| kind.is_none_or(|k| a.kind == k))
        .filter(|a| wallet.is_none_or(|w| a.is_wallet() == w))
        .filter(|a| include_disabled || a.enabled)
        .map(to_value)
        .collect::<Result<Vec<Value>, ApiError>>()?;
    ok(json!({ "total": items.len(), "items": items }))
}

fn wallet_type(text: &str) -> Option<WalletType> {
    match text {
        "cash" => Some(WalletType::Cash),
        "bank" => Some(WalletType::Bank),
        "card" => Some(WalletType::Card),
        "ewallet" => Some(WalletType::Ewallet),
        _ => None,
    }
}

fn default_tax(f: &Fields<'_>) -> Result<Option<TaxCode>, ApiError> {
    f.opt_str("default_tax")?
        .filter(|t| !t.is_empty())
        .map(|t| {
            TaxCode::parse(&t).ok_or_else(|| {
                ApiError::new(
                    422,
                    "bad_tax_code",
                    format!("税区分「{t}」は使えません。S10・S8・P10・P8・P10X・P8X・NT のどれかを選ぶか、空にしてください。"),
                )
                .with_field("default_tax")
            })
        })
        .transpose()
}

fn name(f: &Fields<'_>, required: bool) -> Result<Option<String>, ApiError> {
    let value = if required {
        Some(f.req_str("name", "名前")?)
    } else {
        f.opt_str("name")?
    };
    match value {
        Some(v) if v.trim().is_empty() => {
            Err(ApiError::rule("名前を入れてください（空白だけは使えません）。").with_field("name"))
        }
        other => Ok(other),
    }
}

/// `POST /accounts`。口座なら `wallet` に型を入れる。`kind` を省くと、口座の型から決める。
pub(crate) fn account_add(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let name = name(&f, true)?.unwrap_or_default();
    let wallet = match f.opt_object("wallet")? {
        None => None,
        Some(w) => {
            let wf = Fields::nested(w, "wallet.".to_owned());
            let kind = wallet_type(&wf.req_str("type", "口座の型")?).ok_or_else(|| {
                ApiError::bad_request(
                    "口座の型（wallet.type）は cash・bank・card・ewallet のどれかにしてください。",
                )
                .with_field("wallet.type")
            })?;
            Some(Wallet {
                kind,
                institution: wf.text("institution")?,
            })
        }
    };
    let kind = match (f.opt_str("kind")?, &wallet) {
        (Some(k), _) => Kind::parse(&k).ok_or_else(|| {
            ApiError::bad_request(
                "kind は asset・liability・equity・revenue・expense のどれかにしてください。",
            )
            .with_field("kind")
        })?,
        (None, Some(w)) => w.kind.kind(),
        (None, None) => return Err(f.missing("kind", "分類")),
    };
    if wallet.as_ref().is_some_and(|w| w.kind.kind() != kind) {
        return Err(ApiError::rule(
            "口座の型と分類が合いません。クレジットカードは負債、それ以外の口座は資産です。",
        )
        .with_field("kind"));
    }
    let parent = f.opt_str("parent")?.filter(|p| !p.is_empty());
    if let Some(p) = &parent {
        match state.store.chart().get(p) {
            None => {
                return Err(ApiError::new(
                    422,
                    "unknown_account",
                    format!("親の科目「{p}」が見つかりません。"),
                )
                .with_field("parent"));
            }
            Some(a) if a.kind != kind => {
                return Err(ApiError::rule(
                    "親の科目と分類が違います。同じ分類の科目を親にしてください。",
                )
                .with_field("parent"));
            }
            Some(_) => {}
        }
    }
    let account = state.store.add_account(
        NewAccount {
            name,
            reading: f.text("reading")?,
            kind,
            parent,
            wallet,
            default_tax: default_tax(&f)?,
            default_pct: f.opt_pct("default_pct")?.unwrap_or(100),
        },
        call.now,
    )?;
    created(to_value(&account)?)
}

/// `PATCH /accounts/{code}`。変えられるのは名前・読み・有効か・既定の税区分・既定の割合だけ。
/// 分類・親・口座の種類・コードを変えようとしたら409。
pub(crate) fn account_patch(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let code = call.param(0);
    let current = state
        .store
        .chart()
        .get(code)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("科目「{code}」が見つかりません。")))?;
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let fixed = [
        ("kind", json!(current.kind.as_str())),
        ("code", json!(current.code.as_str())),
        (
            "parent",
            json!(
                current
                    .parent
                    .as_ref()
                    .map(kamoku_ledger::AccountCode::as_str)
            ),
        ),
        ("wallet", to_value(&current.wallet)?),
    ];
    for (key, value) in fixed {
        if map.get(key).is_some_and(|given| *given != value) {
            return Err(ApiError::new(
                409,
                "protected_account",
                "分類・親の科目・口座の種類・コードは変えられません。新しい科目や口座を作ってください。",
            )
            .with_field(key));
        }
    }
    let patch = AccountPatch {
        name: name(&f, false)?,
        reading: f.opt_str("reading")?,
        enabled: f.opt_bool("enabled")?,
        default_tax: if f.has("default_tax") {
            Some(default_tax(&f)?)
        } else {
            None
        },
        default_pct: f.opt_pct("default_pct")?,
    };
    let account = state.store.update_account(code, patch, call.now)?;
    ok(to_value(&account)?)
}

/// `GET /wallets?as_of=`（既定は今日）。カードの残高は未払い額（正）。
pub(crate) fn wallets(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let as_of = call.query.date("as_of")?.unwrap_or(call.today);
    let entries = state.store.ledger().current();
    let balances = wallet_balances(&entries, state.store.chart(), as_of)?;
    ok(to_value(&balances)?)
}
