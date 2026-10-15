// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::{
    KINDS, TAX, WALLET_TYPES, invoice, kind_label, name_locked, percentage, required, wallet_label,
    wallet_parent,
};
use crate::app::prelude::*;
use crate::app::screens::home::common::{
    Segmented, checkbox, el, field_failure, n, s, segmented, text_field,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement, KeyboardEvent};

#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn open(
    ctx: &Context,
    kind: &str,
    existing: Option<Value>,
    accounts: Vec<Value>,
    default_type: &str,
    on_saved: impl Fn(Value) + 'static,
) {
    let old = existing.unwrap_or(Value::Null);
    let editing = old.is_object();
    let wallet = kind == "wallet";
    let partner = kind == "partner";
    let account = !wallet && !partner;
    let title = if editing {
        format!("「{}」を直す", s(&old, "name"))
    } else {
        match kind {
            "wallet" => "口座を追加",
            "partner" => "取引先を足す",
            _ => "科目を足す",
        }
        .into()
    };
    let dialog = Dialog::new(ctx, &title);
    let mut modal = ctx.clone();
    modal.scope = dialog.scope.clone();
    let mut fields = BTreeMap::<String, Field>::new();
    let (name, name_f) = text_field(
        "名前",
        s(&old, "name"),
        if account && name_locked(s(&old, "code")) {
            "この科目は帳簿の決まりで使うので、名前を変えられません。"
        } else {
            "画面に出る名前です。自由に付けられます。"
        },
        true,
    );
    name.set_max_length(if partner { 80 } else { 60 });
    if account && name_locked(s(&old, "code")) {
        name.set_disabled(true);
    }
    let (reading, reading_f) = text_field(
        "読み（ひらがな）",
        s(&old, "reading"),
        "候補の欄で、読みでも探せるようになります。",
        false,
    );
    let (institution, institution_f) = text_field("金融機関の名前（任意）", "", "", false);
    let (invoice_in, invoice_f) = text_field(
        "登録番号（任意）",
        s(&old, "invoice_number"),
        "形式（T と13桁の数字）だけを確かめます。",
        false,
    );
    invoice_in.set_max_length(20);
    let (note, note_f) = text_field("メモ", s(&old, "note"), "", false);
    let tax = select(TAX, s(&old, "default_tax"));
    let tax_f = field(
        "既定の税区分",
        &el(&tax),
        "科目を選んだときに入る初期値です。",
        false,
    );
    let (pct, pct_f) = text_field(
        "既定の事業割合",
        &old["default_pct"].as_i64().unwrap_or(100).to_string(),
        "仕事に使う分を0〜100の整数で入れてください。",
        false,
    );
    attrs(&el(&pct), &[("inputmode", "numeric")]);
    pct.set_max_length(3);
    let classification = select(KINDS, "expense");
    let classification_f = field(
        "分類",
        &el(&classification),
        "分類はあとから変えられません。",
        true,
    );
    let parent = select(&[("", "なし（いちばん上）")], "");
    let parent_f = field(
        "親の科目（任意）",
        &el(&parent),
        "帳票で、親の科目の下にまとめて出します。",
        false,
    );
    let (enabled_el, enabled) = checkbox(
        match kind {
            "wallet" => "この口座を使う",
            "partner" => "この取引先を使う",
            _ => "この科目を使う",
        },
        old["enabled"] != false,
    );
    if account && required(s(&old, "code")) {
        enabled.set_disabled(true);
        append(
            &enabled_el,
            &[h(
                "span",
                "field-hint",
                "帳簿に必ず要る科目なので、外せません。",
            )],
        );
    }
    for (key, f) in [
        ("name", name_f),
        ("reading", reading_f),
        ("institution", institution_f),
        ("invoice_number", invoice_f),
        ("note", note_f),
        ("default_tax", tax_f),
        ("default_pct", pct_f),
        ("kind", classification_f),
        ("parent", parent_f),
    ] {
        fields.insert(key.into(), f);
    }
    let body = h("div", "stack", "");
    let type_state = if wallet && !editing {
        let name = name.clone();
        let reading = reading.clone();
        let institution = institution.clone();
        let paint = move |kind: String| {
            let (n, r, i) = examples(&kind);
            name.set_placeholder(&format!("例: {n}"));
            reading.set_placeholder(&format!("例: {r}"));
            institution.set_placeholder(i);
        };
        paint(default_type.into());
        let choices = segmented(&modal, "口座の種類", WALLET_TYPES, default_type, paint);
        append(&body, std::slice::from_ref(&choices.node));
        Some(choices)
    } else {
        None
    };
    if !editing && account {
        append(
            &body,
            &[fields["kind"].el.clone(), fields["parent"].el.clone()],
        );
        let fill = {
            let parent = parent.clone();
            let classification = classification.clone();
            move || {
                let mut options = vec![{
                    let o = h("option", "", "なし（いちばん上）");
                    attrs(&o, &[("value", "")]);
                    o
                }];
                for a in &accounts {
                    if s(a, "kind") == classification.value()
                        && a["wallet"].is_null()
                        && a["enabled"] != false
                    {
                        let option = h("option", "", s(a, "name"));
                        attrs(&option, &[("value", s(a, "code"))]);
                        options.push(option);
                    }
                }
                replace(&el(&parent), &options);
            }
        };
        fill();
        modal.on(&el(&classification), "change", move |_| fill());
    }
    if editing && wallet {
        append(
            &body,
            &[h(
                "p",
                "muted small",
                &format!(
                    "種類: {}（種類はあとから変えられません）",
                    wallet_label(s(&old["wallet"], "type"))
                ),
            )],
        );
    }
    if editing && account {
        append(
            &body,
            &[h(
                "p",
                "muted small",
                &format!(
                    "分類: {}　コード: {}",
                    kind_label(s(&old, "kind")),
                    s(&old, "code")
                ),
            )],
        );
    }
    append(
        &body,
        &[fields["name"].el.clone(), fields["reading"].el.clone()],
    );
    if wallet && !editing {
        append(&body, &[fields["institution"].el.clone()]);
    }
    if partner {
        append(
            &body,
            &[
                fields["invoice_number"].el.clone(),
                fields["note"].el.clone(),
            ],
        );
        let inv = invoice_in.clone();
        let f = fields["invoice_number"].clone();
        modal.on(&el(&invoice_in), "blur", move |_| {
            f.set_error(&invoice(&inv.value()).err().unwrap_or_default());
        });
    }
    if account && old["wallet"].is_null() {
        if !editing || matches!(s(&old, "kind"), "revenue" | "expense") {
            append(&body, &[fields["default_tax"].el.clone()]);
        }
        if !editing || s(&old, "kind") == "expense" {
            append(&body, &[fields["default_pct"].el.clone()]);
        }
    }
    if editing {
        append(&body, &[enabled_el]);
    }
    append(&dialog.body, &[body]);
    let cancel = button("やめる", "");
    let save = button(
        if editing {
            "保存する"
        } else if wallet {
            "追加"
        } else {
            "足す"
        },
        "primary",
    );
    disabled(&save, ctx.is_read_only());
    append(&dialog.footer, &[cancel.clone(), save.clone()]);
    let d = dialog.clone();
    dialog.scope.on(&cancel, "click", move |_| d.close());
    let d = dialog.clone();
    let c = ctx.clone();
    let kind = kind.to_owned();
    let fields = Rc::new(fields);
    let on_saved = Rc::new(on_saved);
    dialog.scope.on(&save, "click", move |_| {
        if c.is_read_only() {
            c.error(&c.read_only_reason());
            return;
        }
        for f in fields.values() {
            f.set_error("");
        }
        d.set_error("");
        let name_value = name.value().trim().to_owned();
        if name_value.is_empty() {
            fields["name"].set_error("名前を入れてください。");
            let _ = name.focus();
            return;
        }
        let invoice_value = if partner {
            match invoice(&invoice_in.value()) {
                Ok(v) => v,
                Err(e) => {
                    fields["invoice_number"].set_error(&e);
                    let _ = invoice_in.focus();
                    return;
                }
            }
        } else {
            None
        };
        let percentage = if account {
            match percentage(&pct.value()) {
                Ok(v) => v,
                Err(e) => {
                    fields["default_pct"].set_error(&e);
                    let _ = pct.focus();
                    return;
                }
            }
        } else {
            100
        };
        let mut patch = json!({"name":name_value,"reading":reading.value().trim()});
        if partner {
            patch["invoice_number"] = json!(invoice_value);
            patch["note"] = json!(note.value().trim());
        }
        if editing {
            patch["enabled"] = json!(enabled.checked());
            if account && name_locked(s(&old, "code")) {
                patch.as_object_mut().unwrap().remove("name");
            }
            if account && old["wallet"].is_null() {
                patch["default_tax"] = if tax.value().is_empty() {
                    Value::Null
                } else {
                    json!(tax.value())
                };
                patch["default_pct"] = json!(percentage);
            }
        } else if !partner {
            let t = type_state.as_ref().map_or(String::new(), Segmented::get);
            patch["kind"] = json!(if wallet {
                if t == "card" { "liability" } else { "asset" }.to_owned()
            } else {
                classification.value()
            });
            patch["parent"] = if wallet {
                json!(wallet_parent(&t))
            } else if parent.value().is_empty() {
                Value::Null
            } else {
                json!(parent.value())
            };
            patch["wallet"] = if wallet {
                json!({"type":t,"institution":institution.value().trim()})
            } else {
                Value::Null
            };
            patch["default_tax"] = if wallet || tax.value().is_empty() {
                Value::Null
            } else {
                json!(tax.value())
            };
            patch["default_pct"] = json!(percentage);
        }
        d.set_busy(true);
        let d = d.clone();
        let c2 = c.clone();
        let fields = fields.clone();
        let on_saved = on_saved.clone();
        let endpoint = if partner {
            if editing {
                format!("/partners/{}", n(&old, "id"))
            } else {
                "/partners".into()
            }
        } else if editing {
            format!("/accounts/{}", s(&old, "code"))
        } else {
            "/accounts".into()
        };
        let kind = kind.clone();
        c.task(async move {
            let result = if editing {
                c2.patch(&endpoint, &patch).await
            } else {
                c2.post(&endpoint, &patch).await
            };
            if !c2.alive() {
                return Ok(());
            }
            d.set_busy(false);
            match result {
                Ok(saved) => {
                    c2.invalidate(&[if partner { "partners" } else { "accounts" }]);
                    c2.success(&if editing {
                        if kind == "wallet" {
                            format!("口座「{}」を保存しました。", s(&saved, "name"))
                        } else {
                            format!("「{}」を保存しました。", s(&saved, "name"))
                        }
                    } else {
                        match kind.as_str() {
                            "wallet" => format!("口座「{}」を追加しました。", s(&saved, "name")),
                            "partner" => format!("取引先「{}」を足しました。", s(&saved, "name")),
                            _ => format!("科目「{}」を足しました。", s(&saved, "name")),
                        }
                    });
                    d.close();
                    on_saved(saved);
                }
                Err(error) => {
                    let named = fields
                        .iter()
                        .map(|(k, f)| (k.as_str(), f))
                        .collect::<Vec<_>>();
                    if !field_failure(&error, &named) {
                        d.set_error(&error.message);
                    }
                }
            }
            Ok(())
        });
    });
    let submit = save.clone();
    dialog.scope.on(&dialog.body, "keydown", move |event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if key.key() != "Enter"
            || ime_ignore(key)
            || key.alt_key()
            || key.ctrl_key()
            || key.meta_key()
        {
            return;
        }
        if let Some(target) = key
            .target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
            && !matches!(target.type_().as_str(), "radio" | "checkbox")
        {
            key.prevent_default();
            submit.unchecked_ref::<HtmlElement>().click();
        }
    });
    dialog.show();
    if let Some(target) = dialog
        .body
        .query_selector("input:not([disabled]):not([type=radio]):not([type=checkbox])")
        .ok()
        .flatten()
    {
        focus(&target);
    }
}
fn examples(kind: &str) -> (&str, &str, &str) {
    match kind {
        "cash" => ("現金", "げんきん", "空のままでかまいません"),
        "card" => ("サンプルカード", "さんぷるかーど", "例: サンプルカード会社"),
        "ewallet" => (
            "サンプル電子マネー",
            "さんぷるでんしまねー",
            "例: サンプル電子マネーの会社",
        ),
        _ => (
            "サンプル銀行 普通預金",
            "さんぷるぎんこう",
            "例: サンプル銀行",
        ),
    }
}
