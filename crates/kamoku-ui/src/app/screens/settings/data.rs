// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use super::super::home::common::{box_node, el, n, s, text_input, yes};
use super::model::{normalize_hash, noted_text, verify_text};
use crate::app::prelude::*;
fn result(ok: bool, title: &str, body: &str) -> Element {
    let node = box_node(
        "div",
        if ok {
            "check-result check-result--ok"
        } else {
            "check-result check-result--ng"
        },
        &[
            icon(if ok { "shield-check" } else { "shield-alert" }),
            box_node(
                "div",
                "",
                &[h("p", "banner-title", title), h("p", "", body)],
            ),
        ],
    );
    attrs(&node, &[("role", "status")]);
    node
}
fn verify_result(value: &Value) -> Element {
    let text = verify_text(value);
    result(text.ok, text.title, &text.body)
}
fn small(text: &str) -> Element {
    h("p", "small muted", text)
}
fn block(title: &str, children: &[Element]) -> Element {
    let node = box_node("div", "settings-block", &[h("h3", "", title)]);
    append(&node, children);
    node
}
fn download(ctx: &Context, label: &str, path: &str) -> Element {
    let node = link(label, &format!("/api/v1{path}"), "btn");
    attrs(&node, &[("download", "")]);
    let context_copy = ctx.clone();
    let p = path.to_owned();
    let button_copy = node.clone();
    ctx.on(&node, "click", move |e| {
        if let Some(m) = e.dyn_ref::<web_sys::MouseEvent>()
            && (m.button() != 0 || m.ctrl_key() || m.meta_key() || m.shift_key() || m.alt_key())
        {
            return;
        }
        e.prevent_default();
        if button_copy.get_attribute("aria-busy").as_deref() == Some("true") {
            return;
        }
        busy(&button_copy, true);
        let context_copy = context_copy.clone();
        let end = button_copy.clone();
        let path = p.clone();
        let task = context_copy.clone();
        context_copy.task(async move {
            let out = task.download(&path, &[]).await;
            busy(&end, false);
            out
        });
    });
    node
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn mount(ctx: &Context, status: &Value) -> Element {
    let root = h("div", "settings-data stack", "");
    let verify_out = h("div", "check-out", "");
    let verify = button("台帳を検査する", "");
    let head = h("code", "mono hash-full", s(&status["ledger"], "head_hash"));
    let facts = small(&format!(
        "いちばん新しい記録（{}番目）までの中身から作った、64文字の控えです。このパソコンの外に書き残しておくと、あとで照合できます。",
        n(&status["ledger"], "head_seq")
    ));
    let manual = h("div", "hash-manual", "");
    let copy = button("写し取る", "");
    let context_copy = ctx.clone();
    let out = verify_out.clone();
    let head2 = head.clone();
    let facts2 = facts.clone();
    let button_copy = verify.clone();
    ctx.on(&verify,"click",move |_|{disabled(&button_copy,true);replace(&out,&[h("p","muted","最初の記録から順に、たどっています…")]);let context_copy=context_copy.clone();let out=out.clone();let button_copy=button_copy.clone();let head=head2.clone();let facts=facts2.clone();let task=context_copy.clone();context_copy.task(async move{let response=task.post("/verify",&json!({"noted_hash":null})).await;if task.alive(){match response{Ok(value)=>{replace(&out,&[verify_result(&value)]);if let Ok(status)=task.load("status",true).await&& task.alive(){set_text(&head,s(&status["ledger"],"head_hash"));set_text(&facts,&format!("いちばん新しい記録（{}番目）までの中身から作った、64文字の控えです。このパソコンの外に書き残しておくと、あとで照合できます。",n(&status["ledger"],"head_seq")));}},Err(e)=>replace(&out,&[result(false,"検査できませんでした",&e.message)])}disabled(&button_copy,false);}Ok(())});});
    let context_copy = ctx.clone();
    let code = head.clone();
    let host = manual.clone();
    ctx.on(&copy,"click",move |_|{let text=code.text_content().unwrap_or_default();let context_copy=context_copy.clone();let host=host.clone();let task=context_copy.clone();context_copy.task(async move{if task.copy_text(&text).await.is_ok(){if task.alive(){task.success("照合用の控えを写し取りました。紙やメモなど、このパソコンの外に残しておいてください。");}}else if task.alive(){let input=text_input(&text,"末尾のハッシュ（全文）");input.set_read_only(true);replace(&host,&[el(&input),h("p","field-hint","自動で写せませんでした。選んである文字を Ctrl+C（Macは ⌘+C）で写してください。")]);let _=input.focus();input.select();}Ok(())});});
    let noted = text_input("", "控えたハッシュ");
    noted.set_placeholder("写し取った64文字を貼り付け");
    noted.set_max_length(100);
    let noted_field = field(
        "控えたハッシュ",
        &el(&noted),
        "前に写し取った控えを貼り付けると、台帳の何番目の記録にあたるかを調べます。",
        false,
    );
    let noted_out = h("div", "check-out", "");
    let compare = button("照合する", "");
    let pending = Rc::new(Cell::new(false));
    let action: Rc<dyn Fn()> = {
        let context_copy = ctx.clone();
        let noted = noted.clone();
        let field = noted_field.clone();
        let out = noted_out.clone();
        let pending = pending.clone();
        let button_copy = compare.clone();
        Rc::new(move || {
            if pending.get() {
                return;
            }
            replace(&out, &[]);
            let hash = match normalize_hash(&noted.value()) {
                Ok(v) => {
                    field.set_error("");
                    noted.set_value(&v);
                    v
                }
                Err(e) => {
                    field.set_error(&e);
                    let _ = noted.focus();
                    return;
                }
            };
            pending.set(true);
            disabled(&button_copy, true);
            let context_copy = context_copy.clone();
            let out = out.clone();
            let p = pending.clone();
            let button_copy = button_copy.clone();
            let task = context_copy.clone();
            context_copy.task(async move {
                let response = task.post("/verify", &json!({"noted_hash":hash})).await;
                if task.alive() {
                    match response {
                        Ok(value) => {
                            let node = if yes(&value, "ok") {
                                let (found, text) =
                                    noted_text(&value["noted"], n(&value, "events"));
                                result(
                                    found,
                                    if found {
                                        "一致する記録がありました"
                                    } else {
                                        "一致する記録がありません"
                                    },
                                    &text,
                                )
                            } else {
                                verify_result(&value)
                            };
                            replace(&out, &[node]);
                        }
                        Err(e) => {
                            replace(&out, &[result(false, "照合できませんでした", &e.message)]);
                        }
                    }
                    disabled(&button_copy, false);
                }
                p.set(false);
                Ok(())
            });
        })
    };
    let a = action.clone();
    ctx.on(&compare, "click", move |_| a());
    ctx.on(&el(&noted), "keydown", move |e| {
        if let Some(k) = e.dyn_ref::<KeyboardEvent>()
            && !ime_ignore(k)
            && k.key() == "Enter"
        {
            k.prevent_default();
            action();
        }
    });
    let field_copy = noted_field.clone();
    ctx.on(&el(&noted), "input", move |_| field_copy.set_error(""));
    let encoding = select(
        &[("utf8bom", "UTF-8（BOMつき）"), ("sjis", "Shift_JIS")],
        "utf8bom",
    );
    attrs(&el(&encoding), &[("aria-label", "仕訳帳のCSVの文字コード")]);
    let state = select(
        &[("active", "有効な仕訳だけ"), ("all", "取消済みも入れる")],
        "active",
    );
    attrs(&el(&state), &[("aria-label", "仕訳帳のCSVに入れる仕訳")]);
    let csv = link("仕訳帳のCSV", "", "btn export-wide");
    attrs(&csv, &[("download", "")]);
    let paint: Rc<dyn Fn()> = {
        let e = encoding.clone();
        let s = state.clone();
        let link = csv.clone();
        Rc::new(move || {
            attrs(
                &link,
                &[(
                    "href",
                    &format!(
                        "/api/v1/export/journal.csv?state={}&encoding={}",
                        s.value(),
                        e.value()
                    ),
                )],
            );
        })
    };
    paint();
    for target in [el(&encoding), el(&state)] {
        let p = paint.clone();
        ctx.on(&target, "change", move |_| p());
    }
    let context_copy = ctx.clone();
    let button_copy = csv.clone();
    let encoding2 = encoding.clone();
    let state2 = state.clone();
    ctx.on(&csv, "click", move |e| {
        if let Some(m) = e.dyn_ref::<web_sys::MouseEvent>()
            && (m.button() != 0 || m.ctrl_key() || m.meta_key() || m.shift_key() || m.alt_key())
        {
            return;
        }
        e.prevent_default();
        if button_copy.get_attribute("aria-busy").as_deref() == Some("true") {
            return;
        }
        busy(&button_copy, true);
        let e = encoding2.value();
        let s = state2.value();
        let context_copy = context_copy.clone();
        let button_copy = button_copy.clone();
        let task = context_copy.clone();
        context_copy.task(async move {
            let out = task
                .download("/export/journal.csv", &[("state", &s), ("encoding", &e)])
                .await;
            busy(&button_copy, false);
            out
        });
    });
    append(
        &root,
        &[
            block(
                "台帳の整合性",
                &[
                    small(
                        "記録は書き足すだけで、前の記録と鎖のようにつながっています。最初からたどって、壊れていないかを確かめます。",
                    ),
                    box_node("div", "", &[verify]),
                    verify_out,
                ],
            ),
            block(
                "照合用の控え（末尾のハッシュ）",
                &[facts, head, box_node("div", "", &[copy]), manual],
            ),
            block(
                "控えと照合する",
                &[noted_field.el, box_node("div", "", &[compare]), noted_out],
            ),
            block(
                "書き出し",
                &[
                    small("このパソコンの中に、ファイルとして保存します。外へは送りません。"),
                    h("p", "export-title", "帳簿の控え・全件のデータ"),
                    small(
                        "控えは、取った時点の帳簿を、ファイルごと複製したものです。戻すには、ファイルを置き換えます（手順は「よくある質問」にあります）。",
                    ),
                    small(
                        "全件のデータ（JSON）は、このソフト独自の形式です。確定した記録と設定が入り、未確認の明細・テンプレート・取込の設定は入りません。ほかのソフトで使うには、変換が必要です。",
                    ),
                    box_node(
                        "div",
                        "export-grid",
                        &[
                            download(ctx, "帳簿の控え（バックアップ）", "/export/backup.db"),
                            download(ctx, "全件のデータ（JSON）", "/export/all.json"),
                        ],
                    ),
                    h("p", "export-title", "仕訳帳（表計算ソフト向けのCSV）"),
                    box_node("div", "export-grid", &[el(&state), el(&encoding), csv]),
                ],
            ),
        ],
    );
    root
}
