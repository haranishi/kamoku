// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::wrap_hint;
use super::{
    CSV_LIMIT, Element, File, JsCast, Rc, State, append, attrs, build_hash, button, class,
    disabled, field, focus, h, icon, input, node, select, set_text, text,
};
use crate::app::prelude::{phrase, replace};
impl State {
    #[allow(
        clippy::too_many_lines,
        reason = "Build the wallet/file controls and register their upload/drop handlers together."
    )]
    pub fn step_file(self: &Rc<Self>, message: Option<&str>) {
        let ctx = self.begin_step();
        self.filename.borrow_mut().clear();
        self.bytes.borrow_mut().clear();
        *self.encoding.borrow_mut() = "auto".into();
        let select = select(&[("", "口座を選んでください")], "");
        for w in self.wallets.borrow().iter() {
            let kind = match text(&w["wallet"], "type") {
                "cash" => "現金",
                "bank" => "預金",
                "card" => "カード",
                "ewallet" => "電子マネー",
                _ => "",
            };
            let option = h("option", "", &format!("{}（{kind}）", text(w, "name")));
            attrs(&option, &[("value", text(w, "code"))]);
            append(&select, &[option]);
        }
        select.set_value(&self.wallet.borrow());
        let wallet_field = field(
            "取り込む口座",
            &select,
            "明細を出した銀行やカードの口座です。口座ごとに、列の対応を覚えます。",
            true,
        );
        wrap_hint(&wallet_field);
        let file = input("file", "");
        file.set_class_name("visually-hidden file-input");
        attrs(
            &file,
            &[
                ("accept", ".csv,.txt,.tsv,text/csv,text/plain"),
                ("tabindex", "-1"),
            ],
        );
        let hint = h("p", "drop-hint", "");
        attrs(&hint, &[("id", "import-drop-hint")]);
        let pick = button("ファイルを選ぶ", "secondary");
        let mark = icon("import");
        let _ = pick.insert_before(&mark, pick.first_child().as_ref());
        attrs(&pick, &[("aria-describedby", "import-drop-hint")]);
        let busy = h("p", "drop-busy", "");
        attrs(&busy, &[("role", "status")]);
        let error = h("p", "field-error", message.unwrap_or(""));
        attrs(&error, &[("role", "alert")]);
        let zone = node(
            "div",
            "drop-zone",
            &[
                node("span", "drop-icon", &[icon("import")]),
                h("p", "drop-title", "CSVのファイルをここに置く"),
                hint.clone(),
                pick.clone(),
                busy.clone(),
            ],
        );
        let sync = {
            let select = select.clone();
            let pick = pick.clone();
            let zone = zone.clone();
            let hint = hint.clone();
            move || {
                let ready = !select.value().is_empty();
                class(&zone, "is-disabled", !ready);
                disabled(&pick, !ready);
                replace(
                    &hint,
                    &[phrase(if ready {
                        "ドラッグして置くか、「ファイルを選ぶ」から選んでください。文字コード（UTF-8・Shift_JIS）は自動で判定します。10MBまで。"
                    } else {
                        "先に、上で口座を選んでください。口座が決まると、ファイルを置けます。"
                    })],
                );
            }
        };
        sync();
        let weak = Rc::downgrade(self);
        let sel = select.clone();
        let err = error.clone();
        ctx.on(&select, "change", move |_| {
            if let Some(s) = weak.upgrade() {
                set_text(&err, "");
                let value = sel.value();
                s.wallet.borrow_mut().clone_from(&value);
                s.ctx
                    .navigate(&build_hash("/import", &[("wallet", &value)]), true);
                sync();
            }
        });
        let f = file.clone();
        ctx.on(&pick, "click", move |_| f.click());
        let take: Rc<dyn Fn(Option<File>)> = {
            let weak = Rc::downgrade(self);
            let select = select.clone();
            let pick = pick.clone();
            let busy = busy.clone();
            let error = error.clone();
            let context = ctx.clone();
            Rc::new(move |file| {
                let Some(s) = weak.upgrade() else {
                    return;
                };
                if !context.alive() {
                    return;
                }
                set_text(&error, "");
                if select.value().is_empty() {
                    set_text(&error, "先に口座を選んでください。");
                    focus(&select);
                    return;
                }
                let Some(file) = file else {
                    return;
                };
                if file.size() > f64::from(CSV_LIMIT) {
                    set_text(
                        &error,
                        &format!(
                            "ファイルが大きすぎます（{}MB）。10MBまでのCSVにしてください。期間を分けて書き出すと小さくなります。",
                            (file.size() / 1024. / 1024.).ceil()
                        ),
                    );
                    return;
                }
                if s.busy.replace(true) {
                    return;
                }
                disabled(&pick, true);
                select.set_disabled(true);
                set_text(&busy, &format!("「{}」を読み込んでいます…", file.name()));
                *s.filename.borrow_mut() = file.name();
                let filename = file.name();
                let wallet = select.value();
                let seq = s.seq.get() + 1;
                s.seq.set(seq);
                let state = s.clone();
                let error = error.clone();
                let pick = pick.clone();
                let select = select.clone();
                let busy = busy.clone();
                let context = context.clone();
                s.ctx.task(async move {
                    let result = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                    if !context.alive() || seq != state.seq.get() {
                        return Ok(());
                    }
                    if let Ok(buffer) = result {
                        *state.bytes.borrow_mut() = js_sys::Uint8Array::new(&buffer).to_vec();
                        let encoding = state.encoding.borrow().clone();
                        let bytes = state.bytes.borrow().clone();
                        let up = state
                            .ctx
                            .upload(
                                "/imports/upload",
                                &[
                                    ("name", &filename),
                                    ("encoding", &encoding),
                                    ("wallet", &wallet),
                                ],
                                &bytes,
                            )
                            .await;
                        if !context.alive() || seq != state.seq.get() {
                            return Ok(());
                        }
                        state.busy.set(false);
                        match up {
                            Ok(upload) => {
                                *state.upload.borrow_mut() = upload;
                                state.step_columns(None);
                            }
                            Err(err) => {
                                disabled(&pick, false);
                                select.set_disabled(false);
                                set_text(&busy, "");
                                set_text(&error, &err.message);
                            }
                        }
                    } else {
                        state.busy.set(false);
                        disabled(&pick, false);
                        select.set_disabled(false);
                        set_text(&busy, "");
                        set_text(
                            &error,
                            "ファイルを読み込めませんでした。もう一度選んでください。",
                        );
                    }
                    Ok(())
                });
            })
        };
        let take_file = take.clone();
        let f = file.clone();
        ctx.on(&file, "change", move |_| {
            take_file(f.files().and_then(|files| files.get(0)));
            f.set_value("");
        });
        let z = zone.clone();
        let sel = select.clone();
        ctx.on(&zone, "dragover", move |e| {
            e.prevent_default();
            if let Some(e) = e.dyn_ref::<web_sys::DragEvent>()
                && let Some(data) = e.data_transfer()
            {
                data.set_drop_effect(if sel.value().is_empty() {
                    "none"
                } else {
                    "copy"
                });
            }
            class(&z, "is-over", true);
        });
        let z = zone.clone();
        ctx.on(&zone, "dragleave", move |_| class(&z, "is-over", false));
        let z = zone.clone();
        ctx.on(&zone, "drop", move |e| {
            e.prevent_default();
            class(&z, "is-over", false);
            let f = e
                .dyn_ref::<web_sys::DragEvent>()
                .and_then(web_sys::DragEvent::data_transfer)
                .and_then(|data| data.files())
                .and_then(|files| files.get(0));
            take(f);
        });
        let initial: Element = if select.value().is_empty() {
            select.into()
        } else {
            pick
        };
        self.show(
            0,
            node(
                "div",
                "import-step stack",
                &[wallet_field.el, zone, file.into(), error],
            ),
            Some(&initial),
        );
    }
}
