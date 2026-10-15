// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    Cell, Context, Element, Filters, HtmlElement, HtmlInputElement, HtmlSelectElement, JsCast,
    KeyboardEvent, Rc, RefCell, State, Summary, Value, append, attrs, button, class, disabled,
    document, focus, group, h, icon, ime_ignore, input, is_mac, is_wallet, node, pause, replace,
    select, set_text, text,
};
use crate::app::prelude::phrase;

pub(super) struct Loading {
    stopped: Rc<Cell<bool>>,
    shown: Rc<RefCell<Option<Element>>>,
}
impl Loading {
    pub fn cancel(&self) -> Rc<dyn Fn()> {
        let stopped = self.stopped.clone();
        let shown = self.shown.clone();
        Rc::new(move || {
            stopped.set(true);
            if let Some(node) = shown.borrow_mut().take() {
                node.remove();
            }
        })
    }
    pub fn new(ctx: &Context, container: &Element, lines: usize, label: &str) -> Self {
        let stopped = Rc::new(Cell::new(false));
        let shown = Rc::new(RefCell::new(None));
        let waiting = stopped.clone();
        let node = shown.clone();
        let container = container.clone();
        let label = label.to_owned();
        let context = ctx.clone();
        ctx.task(async move {
            pause(300).await;
            if waiting.get() || !context.alive() {
                return Ok(());
            }
            let block = h("div", "skel-block", "");
            attrs(&block, &[("role", "status"), ("aria-label", &label)]);
            for i in 0..lines {
                let bar = h("span", "skel", "");
                if let Some(el) = bar.dyn_ref::<HtmlElement>() {
                    let _ = el
                        .style()
                        .set_property("width", ["40%", "90%", "70%", "85%", "60%"][i % 5]);
                }
                append(&block, &[bar]);
            }
            append(&container, std::slice::from_ref(&block));
            *node.borrow_mut() = Some(block);
            Ok(())
        });
        Self { stopped, shown }
    }
}
impl Drop for Loading {
    fn drop(&mut self) {
        self.cancel()();
    }
}
pub(super) struct Tools {
    pub el: Element,
    pub panel: Element,
    pub draft: Element,
    pub ignored: Element,
    wallet: HtmlSelectElement,
    search: HtmlInputElement,
}
impl Tools {
    pub fn new() -> Self {
        let draft = button("未確認", "ghost");
        let ignored = button("無視した行", "ghost");
        for (el, key) in [(&draft, "draft"), (&ignored, "ignored")] {
            el.set_class_name("tab");
            attrs(
                el,
                &[
                    ("role", "tab"),
                    ("data-tab", key),
                    ("id", &format!("inbox-tab-{key}")),
                    ("aria-controls", "inbox-panel"),
                ],
            );
        }
        let tabs = node("div", "tabs", &[draft.clone(), ignored.clone()]);
        attrs(
            &tabs,
            &[("role", "tablist"), ("aria-label", "表示する明細")],
        );
        let wallet = select(&[("", "すべての口座")], "");
        wallet.set_class_name("select inbox-wallet");
        attrs(&wallet, &[("aria-label", "口座で絞り込む")]);
        let search = input("search", "");
        attrs(
            &search,
            &[
                ("placeholder", "摘要・取引先・金額で探す"),
                ("aria-label", "明細を文字で探す"),
                ("aria-describedby", "inbox-search-hint"),
                ("data-search", "1"),
            ],
        );
        let hint = h("span", "search-hint", "Enter で探します");
        attrs(&hint, &[("id", "inbox-search-hint")]);
        let kbd = h("kbd", "", "/");
        attrs(&kbd, &[("aria-hidden", "true")]);
        let search_box = node(
            "div",
            "search-box inbox-search",
            &[icon("search"), search.clone().into(), kbd, hint],
        );
        let el = node(
            "div",
            "card inbox-tools",
            &[
                tabs,
                node("div", "inbox-filters", &[wallet.clone().into(), search_box]),
            ],
        );
        attrs(
            &el,
            &[("role", "search"), ("aria-label", "未確認の絞り込み")],
        );
        let panel = h("div", "tabpanel inbox-panel", "");
        attrs(
            &panel,
            &[
                ("id", "inbox-panel"),
                ("role", "tabpanel"),
                ("aria-labelledby", "inbox-tab-draft"),
            ],
        );
        Self {
            el,
            panel,
            draft,
            ignored,
            wallet,
            search,
        }
    }
    pub fn bind(&self, s: &Rc<State>) {
        for (el, status) in [(&self.draft, "draft"), (&self.ignored, "ignored")] {
            let weak = Rc::downgrade(s);
            s.ctx.on(el, "click", move |_| {
                if let Some(s) = weak.upgrade() {
                    s.navigate(|f| f.status = status.into());
                }
            });
            let weak = Rc::downgrade(s);
            s.ctx.on(el, "keydown", move |e| {
                let Ok(k) = e.dyn_into::<KeyboardEvent>() else {
                    return;
                };
                if ime_ignore(&k) {
                    return;
                }
                if matches!(
                    k.key().as_str(),
                    "ArrowLeft" | "ArrowRight" | "Home" | "End"
                ) {
                    k.prevent_default();
                    if let Some(s) = weak.upgrade() {
                        let status = match k.key().as_str() {
                            "Home" => "draft",
                            "End" => "ignored",
                            _ => {
                                if status == "draft" {
                                    "ignored"
                                } else {
                                    "draft"
                                }
                            }
                        };
                        focus(if status == "draft" {
                            &s.tools.draft
                        } else {
                            &s.tools.ignored
                        });
                        s.navigate(|f| f.status = status.into());
                    }
                }
            });
        }
        let weak = Rc::downgrade(s);
        s.ctx.on(&self.wallet, "change", move |_| {
            if let Some(s) = weak.upgrade() {
                let v = s.tools.wallet.value();
                s.navigate(|f| f.wallet = v);
            }
        });
        let commit = |s: &Rc<State>| {
            let q = s.tools.search.value().trim().to_owned();
            if q != s.filters.borrow().q {
                s.navigate(|f| f.q = q);
            }
        };
        let weak = Rc::downgrade(s);
        s.ctx.on(&self.search, "change", move |_| {
            if let Some(s) = weak.upgrade() {
                commit(&s);
            }
        });
        let weak = Rc::downgrade(s);
        s.ctx.on(&self.search, "keydown", move |e| {
            let Ok(k) = e.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&k) {
                return;
            }
            if let Some(s) = weak.upgrade() {
                match k.key().as_str() {
                    "Enter" => {
                        k.prevent_default();
                        commit(&s);
                    }
                    "Escape" => {
                        k.prevent_default();
                        let _ = s.tools.search.blur();
                    }
                    _ => {}
                }
            }
        });
    }
    pub fn sync(&self, f: &Filters) {
        for (el, selected) in [
            (&self.draft, f.status == "draft"),
            (&self.ignored, f.status == "ignored"),
        ] {
            attrs(
                el,
                &[
                    ("aria-selected", if selected { "true" } else { "false" }),
                    ("tabindex", if selected { "0" } else { "-1" }),
                ],
            );
            class(el, "is-active", selected);
        }
        attrs(
            &self.panel,
            &[(
                "aria-labelledby",
                if f.status == "draft" {
                    "inbox-tab-draft"
                } else {
                    "inbox-tab-ignored"
                },
            )],
        );
        self.wallet.set_value(&f.wallet);
        if document().active_element() != Some(self.search.clone().into()) {
            self.search.set_value(&f.q);
        }
    }
    pub fn wallets(&self, accounts: &[Value]) {
        replace(&self.wallet, &[]);
        let all = h("option", "", "すべての口座");
        attrs(&all, &[("value", "")]);
        append(&self.wallet, &[all]);
        for a in accounts
            .iter()
            .filter(|a| is_wallet(a) && a["enabled"] != false)
        {
            let o = h("option", "", text(a, "name"));
            attrs(&o, &[("value", text(a, "code"))]);
            append(&self.wallet, &[o]);
        }
    }
}
pub(super) struct Bar {
    pub el: Element,
    pub confirm: Element,
    count: Element,
    in_total: Element,
    out_total: Element,
    note: Element,
    live: Element,
    pub save: Element,
    generation: Rc<Cell<u64>>,
    save_generation: Rc<Cell<u64>>,
    spoken: Rc<RefCell<Option<String>>>,
}
impl Bar {
    pub fn new() -> Self {
        let count = h("span", "num", "0");
        let incoming = h("span", "num", "0");
        let outgoing = h("span", "num", "0");
        let sums = h("dl", "inbox-sums", "");
        for (label, n, unit, extra) in [
            ("確定する行", count.clone(), "件", " inbox-sum--count"),
            ("入金", incoming.clone(), "円", ""),
            ("出金", outgoing.clone(), "円", ""),
        ] {
            append(
                &sums,
                &[node(
                    "div",
                    &format!("inbox-sum{extra}"),
                    &[
                        h("dt", "", label),
                        node(
                            "dd",
                            "",
                            &[node("span", "money", &[n, h("span", "unit", unit)])],
                        ),
                    ],
                )],
            );
        }
        let confirm = button("確定する", "primary");
        confirm.set_class_name("btn btn--primary inbox-confirm");
        attrs(
            &confirm,
            &[("aria-keyshortcuts", "Control+Enter Meta+Enter")],
        );
        let kbd = h("kbd", "", if is_mac() { "⌘ Enter" } else { "Ctrl Enter" });
        attrs(&kbd, &[("aria-hidden", "true")]);
        append(&confirm, &[kbd]);
        let save = h("p", "inbox-save", "");
        attrs(&save, &[("role", "status")]);
        let note = h("p", "field-hint inbox-note", "");
        attrs(&note, &[("id", "inbox-block-note")]);
        let live = h("p", "visually-hidden inbox-live", "");
        attrs(&live, &[("aria-live", "polite")]);
        let el = node(
            "section",
            "inbox-bar",
            &[
                node("div", "inbox-bar-main", &[sums, confirm.clone()]),
                node("div", "inbox-bar-foot", &[save.clone(), note.clone()]),
                live.clone(),
            ],
        );
        attrs(
            &el,
            &[("data-bottom-bar", "true"), ("aria-label", "確定する行")],
        );
        Self {
            el,
            confirm,
            count,
            in_total: incoming,
            out_total: outgoing,
            note,
            live,
            save,
            generation: Rc::new(Cell::new(0)),
            save_generation: Rc::new(Cell::new(0)),
            spoken: Rc::new(RefCell::new(None)),
        }
    }
    pub fn bind(&self, s: &Rc<State>) {
        let weak = Rc::downgrade(s);
        s.ctx.on(&self.confirm, "click", move |_| {
            if let Some(s) = weak.upgrade() {
                s.confirm();
            }
        });
    }
    pub fn empty_attempt(&self, ctx: &Context) {
        let reason = "確定する行がありません。行で Enter を押して、確定する行にしてください。";
        self.update(ctx, &Summary::default(), reason, true);
        replace(
            &self.note,
            &[
                node("span", "note-wide", &[phrase(reason)]),
                node(
                    "span",
                    "note-narrow",
                    &[phrase(
                        "確定する行がありません。左の印を押して、確定する行を選んでください。",
                    )],
                ),
            ],
        );
    }
    pub fn update(&self, ctx: &Context, s: &Summary, reason: &str, narrow: bool) {
        set_text(&self.count, &group(s.count as i128));
        set_text(&self.in_total, &group(s.incoming));
        set_text(&self.out_total, &group(s.outgoing));
        if narrow {
            replace(
                &self.note,
                &[
                    node("span", "note-wide", &[phrase(reason)]),
                    node(
                        "span",
                        "note-narrow",
                        &[phrase("左の印を押すと、確定する行になります。")],
                    ),
                ],
            );
        } else {
            replace(&self.note, &[phrase(reason)]);
        }
        class(&self.el, "is-empty", s.count == 0);
        disabled(&self.confirm, !reason.is_empty());
        if reason.is_empty() {
            let _ = self.confirm.remove_attribute("aria-describedby");
        } else {
            attrs(&self.confirm, &[("aria-describedby", "inbox-block-note")]);
        }
        let text = s.speech();
        let generation = self.generation.clone();
        let n = generation.get() + 1;
        generation.set(n);
        if self.spoken.borrow().is_none() {
            *self.spoken.borrow_mut() = Some(text);
            return;
        }
        if self.spoken.borrow().as_ref() == Some(&text) {
            return;
        }
        let spoken = self.spoken.clone();
        let live = self.live.clone();
        let ctx2 = ctx.clone();
        ctx.task(async move {
            pause(1000).await;
            if ctx2.alive() && generation.get() == n {
                *spoken.borrow_mut() = Some(text.clone());
                set_text(&live, &text);
            }
            Ok(())
        });
    }
    pub fn saved(&self, ctx: &Context, kind: &str) {
        let generation = self.save_generation.get() + 1;
        self.save_generation.set(generation);
        attrs(&self.save, &[("data-state", kind)]);
        let (text, icon_name) = match kind {
            "saving" => ("保存しています…", "clock"),
            "saved" => ("保存済み", "check-circle"),
            "failed" => (
                "保存できなかった欄があります。行の下の理由を見てください。",
                "alert",
            ),
            _ => ("", ""),
        };
        let children = if text.is_empty() {
            vec![]
        } else {
            let mark = icon(icon_name);
            attrs(&mark, &[("width", "16"), ("height", "16")]);
            vec![
                mark,
                if text.chars().count() > 20 {
                    node("span", "", &[phrase(text)])
                } else {
                    h("span", "", text)
                },
            ]
        };
        replace(&self.save, &children);
        if kind == "saved" {
            let current = self.save_generation.clone();
            let save = self.save.clone();
            let context = ctx.clone();
            ctx.task(async move {
                pause(4000).await;
                if context.alive() && current.get() == generation {
                    replace(&save, &[]);
                }
                Ok(())
            });
        }
    }
}
