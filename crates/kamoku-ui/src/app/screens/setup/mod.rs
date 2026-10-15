// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod model;
use super::home::common::{
    Failures, amount, banner_node, box_node, date_text, el, field_failure, heading, list, n, s,
    segmented, set_hidden, text_field, yes,
};
use super::masters::{
    forms,
    model::{invoice, wallet_label},
};
use super::settings::{OPENED_HINT, TAX_MODE_HINT, opening::OpeningForm};
use crate::app::prelude::*;
use model::{requested_step, saved_up_to};
const STEPS: [&str; 4] = ["事業者と課税の区分", "口座", "期首残高", "確認して始める"];
struct Setup {
    page: Context,
    view: Option<Scope>,
    failures: Failures,
    settings: Value,
    accounts: Vec<Value>,
    opening: Value,
    step: usize,
    sequence: u64,
    body: Element,
    stepper: Element,
}
type Shared = Rc<RefCell<Setup>>;
fn go(ctx: &Context, step: usize) {
    ctx.allow_leave();
    ctx.navigate(&build_hash("/setup", &[("step", &step.to_string())]), false);
}
fn actions(ctx: &Context, previous: Option<usize>, next: Element) -> Element {
    let node = h("div", "import-actions setup-actions", "");
    if let Some(step) = previous {
        let back = button("戻る", "");
        let context_copy = ctx.clone();
        ctx.on(&back, "click", move |_| go(&context_copy, step));
        append(&node, &[back]);
    }
    append(&node, &[next]);
    node
}
fn card(step: usize, children: &[Element]) -> Element {
    let title = h("h2", "card-title", &format!("{step}. {}", STEPS[step - 1]));
    attrs(&title, &[("id", "setup-title"), ("tabindex", "-1")]);
    let node = box_node("section", "card setup-card", &[title]);
    attrs(&node, &[("aria-labelledby", "setup-title")]);
    append(&node, children);
    node
}
fn paint_steps(state: &Setup) {
    let saved = saved_up_to(&state.settings, &state.opening);
    let items: Vec<_> = STEPS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let number = i + 1;
            let done = number != state.step && (number < state.step || number <= saved);
            let item = h(
                "li",
                if number == state.step {
                    "step is-current"
                } else if done {
                    "step is-done"
                } else {
                    "step"
                },
                "",
            );
            if number == state.step {
                attrs(&item, &[("aria-current", "step")]);
            }
            append(
                &item,
                &[
                    {
                        let no = h("span", "step-no", "");
                        append(
                            &no,
                            &[if done {
                                icon("check")
                            } else {
                                h("span", "", &number.to_string())
                            }],
                        );
                        no
                    },
                    h("span", "step-label", label),
                ],
            );
            if done {
                append(&item, &[h("span", "visually-hidden", "（済み）")]);
            }
            item
        })
        .collect();
    replace(&state.stepper, &items);
}
async fn save_settings(ctx: &Context, state: &Shared, patch: Value) -> Result<Value, ApiError> {
    if ctx.is_read_only() {
        return Err(ApiError::message(ctx.read_only_reason()));
    }
    let mut input = state.borrow().settings.clone();
    if let (Some(target), Some(patch)) = (input.as_object_mut(), patch.as_object()) {
        for (key, v) in patch {
            target.insert(key.clone(), v.clone());
        }
    }
    let value = ctx.put("/settings", &input).await?;
    state.borrow_mut().settings = value.clone();
    ctx.shared
        .values
        .borrow_mut()
        .insert("settings".into(), value.clone());
    Ok(value)
}
fn mode_consequence(mode: &str) -> &'static str {
    if mode == "exempt" {
        "免税事業者にすると、取引の登録で税区分の欄を隠します（「詳細」で出せます）。"
    } else {
        "課税事業者にすると、取引に税区分を付け、ホームと帳票に消費税の見込み（参考）を出します。"
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn step1(ctx: &Context, state: &Shared) -> Element {
    let settings = state.borrow().settings.clone();
    let (name, nf) = text_field(
        "事業者名",
        s(&settings, "business_name"),
        "メニューの上に出ます。",
        true,
    );
    name.set_max_length(80);
    name.set_placeholder("例: サンプル事務所（屋号が無ければ、お名前）");
    let opened_value = if s(&settings, "opened_on").is_empty() {
        format!("{}-01-01", &today()[..4])
    } else {
        s(&settings, "opened_on").to_owned()
    };
    let (opened, of) = text_field("開業日", &opened_value, OPENED_HINT, true);
    let (inv, inf) = text_field(
        "登録番号（任意）",
        s(&settings, "invoice_number"),
        "T と13桁の数字です。分からなければ、あとから設定で入れられます。",
        false,
    );
    inv.set_max_length(20);
    inv.set_placeholder("例: T1234567890123（無ければ空）");
    let inv_node = inf.el.clone();
    let consequence = h("p", "field-hint", "");
    consequence.set_id(&uid("setup-mode-consequence"));
    let consequence2 = consequence.clone();
    let mode = segmented(
        ctx,
        "消費税の課税の区分",
        &[("exempt", "免税事業者"), ("taxable", "課税事業者")],
        if s(&settings, "tax_mode").is_empty() {
            "exempt"
        } else {
            s(&settings, "tax_mode")
        },
        move |v| {
            set_hidden(&inv_node, v != "taxable");
            set_text(&consequence2, mode_consequence(&v));
        },
    );
    set_hidden(&inf.el, mode.get() != "taxable");
    set_text(&consequence, mode_consequence(&mode.get()));
    let hint = h("p", "field-hint", TAX_MODE_HINT);
    attrs(&hint, &[("id", &uid("setup-mode"))]);
    attrs(
        &mode.node,
        &[(
            "aria-describedby",
            &format!("{} {}", consequence.id(), hint.id()),
        )],
    );
    let guide = box_node(
        "details",
        "setup-more",
        &[
            h("summary", "tap", "選び方をくわしく"),
            hint.clone(),
            h(
                "div",
                "setup-mode-guide",
                "免税事業者は消費税の納付を免除される事業者です。課税事業者は消費税の申告・納付が必要な事業者です。インボイスの登録をしている場合は課税事業者になります。判断に迷うときは、税務署か税理士に確かめてください。",
            ),
        ],
    );
    let next = button("保存して次へ", "primary");
    disabled(&next, ctx.is_read_only());
    let context_copy = ctx.clone();
    let state = state.clone();
    let button_copy = next.clone();
    let fields = (nf.clone(), of.clone(), inf.clone());
    let mode2 = mode.clone();
    let name2 = name.clone();
    let date = opened.clone();
    let invoice_input = inv.clone();
    ctx.on(&next,"click",move |_|{if context_copy.is_read_only(){return;}let name=name2.value().trim().to_owned();if name.is_empty(){fields.0.set_error("事業者名を入れてください。");let _=name2.focus();return;}let opened=match parse_date(&date.value(),&today()){Ok(v)=>{date.set_value(&v);v},Err(e)=>{fields.1.set_error(&e);let _=date.focus();return;}};let mode=mode2.get();let invoice=if mode=="taxable"{match invoice(&invoice_input.value()){Ok(v)=>v,Err(e)=>{fields.2.set_error(&e);let _=invoice_input.focus();return;}}}else{None};disabled(&button_copy,true);let context_copy=context_copy.clone();let state=state.clone();let button_copy=button_copy.clone();let fields=fields.clone();let task=context_copy.clone();context_copy.task(async move{match save_settings(&task,&state,json!({"business_name":name,"tax_mode":mode,"opened_on":opened,"invoice_number":invoice,"setup_done":false})).await{Ok(_)=>if task.alive(){state.borrow().failures.clear();go(&task,2)},Err(e)=>if task.alive(){if !field_failure(&e,&[("business_name",&fields.0),("opened_on",&fields.1),("invoice_number",&fields.2)]){state.borrow().failures.show(&e.message);}disabled(&button_copy,task.is_read_only());}}Ok(())});});
    for (control, opening_form) in [
        (name.clone(), nf.clone()),
        (opened.clone(), of.clone()),
        (inv.clone(), inf.clone()),
    ] {
        ctx.on(&el(&control), "input", move |_| opening_form.set_error(""));
    }
    let date_control = opened.clone();
    let opening_form = of.clone();
    ctx.on(&el(&opened), "blur", move |_| {
        match parse_date(&date_control.value(), &today()) {
            Ok(v) => {
                date_control.set_value(&v);
                opening_form.set_error("");
            }
            Err(e) => opening_form.set_error(&e),
        }
    });
    let node = card(
        1,
        &[
            nf.el,
            mode.node.clone(),
            box_node("div", "setup-mode-notes", &[consequence, guide]),
            of.el,
            inf.el.clone(),
            actions(ctx, None, next.clone()),
        ],
    );
    let inv_box = inf.el;
    ctx.on(&node, "keydown", move |event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(key)
            || key.key() != "Enter"
            || key.default_prevented()
            || key.shift_key()
            || key.alt_key()
            || key.ctrl_key()
            || key.meta_key()
        {
            return;
        }
        let Some(target) = key.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        if matches!(target.tag_name().as_str(), "BUTTON" | "A") {
            return;
        }
        let mut order = vec![el(&name), mode.node.clone(), el(&opened)];
        if !inv_box.has_attribute("hidden") {
            order.push(el(&inv));
        }
        if let Some(at) = order
            .iter()
            .position(|n| n == &target || n.contains(Some(&target)))
        {
            key.prevent_default();
            if let Some(next) = order.get(at + 1) {
                if next.tag_name() == "FIELDSET" {
                    if let Ok(Some(radio)) = next.query_selector("input:checked") {
                        focus(&radio);
                    }
                } else {
                    focus(next);
                }
            } else if !next.has_attribute("disabled") {
                next.unchecked_ref::<HtmlElement>().click();
            }
        }
    });
    node
}
fn step2(ctx: &Context, state: &Shared) -> Element {
    let wallets: Vec<_> = state
        .borrow()
        .accounts
        .iter()
        .filter(|a| a["wallet"].is_object() && yes(a, "enabled"))
        .cloned()
        .collect();
    let list = h("ul", "setup-wallets", "");
    for wallet in wallets {
        append(
            &list,
            &[box_node(
                "li",
                "setup-wallet",
                &[
                    icon(if s(&wallet["wallet"], "type") == "card" {
                        "card"
                    } else if s(&wallet["wallet"], "type") == "bank" {
                        "building"
                    } else {
                        "wallet"
                    }),
                    h("span", "setup-wallet-name", s(&wallet, "name")),
                    h(
                        "span",
                        "badge badge--muted",
                        wallet_label(s(&wallet["wallet"], "type")),
                    ),
                ],
            )],
        );
    }
    let add = button("口座を追加", "");
    disabled(&add, ctx.is_read_only());
    let context_copy = ctx.clone();
    let shared = state.clone();
    ctx.on(&add, "click", move |_| {
        let c2 = context_copy.clone();
        let shared2 = shared.clone();
        forms::open(
            &context_copy,
            "wallet",
            None,
            shared.borrow().accounts.clone(),
            "bank",
            move |_| render(&c2, &shared2, Some(2)),
        );
    });
    let next = button("次へ", "primary");
    let context_copy = ctx.clone();
    ctx.on(&next, "click", move |_| go(&context_copy, 3));
    card(
        2,
        &[
            h(
                "p",
                "setup-lead",
                "取引に使うお金の置き場所です。現金の口座は用意してあります。銀行の口座やクレジットカードを使うなら、追加してください。",
            ),
            list,
            box_node("div", "", &[add]),
            h(
                "p",
                "small muted",
                "種類は「現金」「預金」「クレジットカード」「電子マネー」から選び、名前は自由に付けられます。あとから口座の画面でも追加できます。",
            ),
            actions(ctx, Some(1), next),
        ],
    )
}
fn step3(ctx: &Context, state: &Shared) -> Element {
    let next = button("保存して次へ", "primary");
    disabled(&next, ctx.is_read_only());
    let button_copy = next.clone();
    let form = {
        let view = state.borrow();
        OpeningForm::new(
            ctx,
            &view.accounts,
            &view.opening,
            s(&view.settings, "opened_on"),
            Some(Rc::new(move || {
                if !button_copy.has_attribute("disabled") {
                    button_copy.unchecked_ref::<HtmlElement>().click();
                }
            })),
        )
    };
    let context_copy = ctx.clone();
    let shared = state.clone();
    let opening_form = form.clone();
    let button_copy = next.clone();
    ctx.on(&next,"click",move |_|{if context_copy.is_read_only(){return;}let Some(input)=opening_form.collect()else{return};disabled(&button_copy,true);let context_copy=context_copy.clone();let shared=shared.clone();let opening_form=opening_form.clone();let button_copy=button_copy.clone();let task=context_copy.clone();context_copy.task(async move{match task.put("/opening",&input).await{Ok(value)=>{shared.borrow_mut().opening=value;if task.alive(){shared.borrow().failures.clear();go(&task,4);}},Err(e)=>if task.alive(){if e.is_stale(){let _=task.get("/opening",&[]).await;if task.alive(){task.info("他の場所で更新されました。読み込み直しました。入力はそのままです。もう一度「保存して次へ」を押してください。");}}else if !opening_form.show_error(&e){shared.borrow().failures.show(&e.message);}disabled(&button_copy,task.is_read_only());}}Ok(())});});
    card(
        3,
        &[
            h(
                "p",
                "setup-lead",
                "開業日の時点で、それぞれの口座にいくらあったかを入れます。カードは、まだ払っていない額です。",
            ),
            form.el,
            actions(ctx, Some(2), next),
        ],
    )
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn step4(ctx: &Context, state: &Shared) -> Element {
    let view = state.borrow();
    let summary = h("dl", "setup-summary", "");
    for (label, value) in [
        (
            "事業者名",
            h("span", "", s(&view.settings, "business_name")),
        ),
        (
            "課税の区分",
            h(
                "span",
                "",
                if s(&view.settings, "tax_mode") == "taxable" {
                    "課税事業者"
                } else {
                    "免税事業者"
                },
            ),
        ),
        (
            "開業日",
            h("span", "", &date_text(s(&view.settings, "opened_on"))),
        ),
    ] {
        append(
            &summary,
            &[box_node(
                "div",
                "",
                &[h("dt", "", label), box_node("dd", "", &[value])],
            )],
        );
    }
    let wallets = h("dd", "", "");
    for (i, w) in view
        .accounts
        .iter()
        .filter(|a| a["wallet"].is_object() && yes(a, "enabled"))
        .enumerate()
    {
        if i > 0 {
            append(&wallets, &[h("span", "", "、")]);
        }
        append(&wallets, &[h("span", "keep", s(w, "name"))]);
    }
    if wallets.child_element_count() == 0 {
        set_text(&wallets, "なし");
    }
    append(
        &summary,
        &[box_node("div", "", &[h("dt", "", "口座"), wallets])],
    );
    let opening = h("dd", "", "");
    if view.opening["entry_id"].is_null() {
        set_text(&opening, "なし（すべて0円）");
    } else {
        let balances = list(&view.opening["balances"]);
        let list = h("ul", "setup-balances", "");
        for button_copy in &balances {
            let account = view
                .accounts
                .iter()
                .find(|a| s(a, "code") == s(button_copy, "account"));
            let name = h("span", "setup-balance-name", "");
            append(
                &name,
                &[h(
                    "span",
                    "keep",
                    account.map_or(s(button_copy, "account"), |a| s(a, "name")),
                )],
            );
            if account.is_some_and(|a| s(a, "kind") == "liability") {
                append(&name, &[h("span", "badge badge--muted", "負債")]);
            }
            append(
                &list,
                &[box_node(
                    "li",
                    "",
                    &[name, amount(n(button_copy, "amount"))],
                )],
            );
        }
        append(
            &opening,
            &[
                list,
                box_node(
                    "p",
                    "setup-capital",
                    &[
                        h("span", "", "元入金 "),
                        amount(n(&view.opening, "capital")),
                        h("span", "", &format!("（{}つの科目）", balances.len())),
                    ],
                ),
            ],
        );
    }
    append(
        &summary,
        &[box_node("div", "", &[h("dt", "", "期首残高"), opening])],
    );
    drop(view);
    let start = button("記帳を始める", "primary");
    disabled(&start, ctx.is_read_only());
    let context_copy = ctx.clone();
    let shared = state.clone();
    let button_copy = start.clone();
    ctx.on(&start, "click", move |_| {
        if context_copy.is_read_only() {
            return;
        }
        disabled(&button_copy, true);
        let context_copy = context_copy.clone();
        let shared = shared.clone();
        let button_copy = button_copy.clone();
        let task = context_copy.clone();
        context_copy.task(async move {
            match save_settings(&task, &shared, json!({"setup_done":true})).await {
                Ok(_) => {
                    task.load("status", true).await?;
                    if task.alive() {
                        shared.borrow().failures.clear();
                        task.success_for(
                            "初期設定が済みました。取引を登録するか、明細を取り込んでください。",
                            8000,
                        );
                        task.allow_leave();
                        task.navigate("#/", false);
                    }
                }
                Err(e) => {
                    if task.alive() {
                        shared.borrow().failures.show(&e.message);
                        disabled(&button_copy, task.is_read_only());
                    }
                }
            }
            Ok(())
        });
    });
    card(
        4,
        &[
            summary,
            h(
                "p",
                "small muted",
                "どれも、あとから設定と口座の画面で直せます。",
            ),
            actions(ctx, Some(3), start),
        ],
    )
}
fn render(_ctx: &Context, state: &Shared, asked: Option<usize>) {
    let page = state.borrow().page.clone();
    let ctx = &page;
    let seq = {
        let mut view = state.borrow_mut();
        view.sequence = view.sequence.wrapping_add(1);
        if let Some(scope) = view.view.take() {
            scope.dispose();
        }
        if let Some(step) = asked {
            view.step = step;
            paint_steps(&view);
        }
        view.sequence
    };
    let context_copy = ctx.clone();
    let state = state.clone();
    ctx.task(async move{let result=async{let settings=context_copy.load("settings",true).await?;let accounts=context_copy.load("accounts",true).await?;let opening=context_copy.get("/opening",&[]).await?;Ok::<_,ApiError>((settings,list(&accounts),opening))}.await;if !context_copy.alive()||state.borrow().sequence!=seq{return Ok(());}let(settings,mut accounts,opening)=match result{Ok(v)=>v,Err(e)=>{replace(&state.borrow().body,&[banner_node("初期設定の画面を準備できませんでした",&e.message,true)]);return Ok(());}};let step=asked.unwrap_or_else(||(saved_up_to(&settings,&opening)+1).min(4));if step==2&&!accounts.iter().any(|a|a["wallet"].is_object()&&yes(a,"enabled"))&&!context_copy.is_read_only(){let cash=context_copy.post("/accounts",&json!({"name":"現金","reading":"げんきん","kind":"asset","parent":"cash","wallet":{"type":"cash","institution":""},"default_tax":null,"default_pct":100})).await?;if !context_copy.alive()||state.borrow().sequence!=seq{return Ok(());}accounts.push(cash);context_copy.invalidate(&["accounts","wallets"]);}let body={let mut view=state.borrow_mut();view.settings=settings;view.accounts=accounts;view.opening=opening;view.step=step;paint_steps(&view);view.body.clone()};let node=match step{1=>step1(&context_copy,&state),2=>step2(&context_copy,&state),3=>step3(&context_copy,&state),_=>step4(&context_copy,&state)};replace(&body,&[node]);if let Ok(Some(heading))=body.query_selector("h2"){focus(&heading);}Ok(())});
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let root = h("div", "setup", "");
    let body = h("div", "setup-body", "");
    if ctx.peek("status").is_some_and(|v| yes(&v, "setup_done")) {
        append(
            &root,
            &[
                heading("初期設定", None),
                box_node(
                    "div",
                    "card",
                    &[
                        h("h2", "", "初期設定は済んでいます"),
                        h(
                            "p",
                            "",
                            "事業者・締め・期首残高は設定の画面で、口座は口座の画面で見直せます。",
                        ),
                        link("設定を開く", "#/settings", "btn btn--primary"),
                        link("ホームへ", "#/", "btn"),
                    ],
                ),
            ],
        );
        return Ok(Screen::new(&ctx, root));
    }
    if let Some(html) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|date_control| date_control.document_element())
    {
        attrs(&html, &[("data-setup", "on")]);
        ctx.scope.cleanup(move || {
            let _ = html.remove_attribute("data-setup");
        });
    }
    let stepper = h("ol", "stepper", "");
    attrs(&stepper, &[("aria-label", "初期設定の段階")]);
    append(
        &root,
        &[
            h("p", "setup-brand", "kamoku"),
            heading("初期設定", None),
            h(
                "p",
                "muted small",
                "記帳を始める前に、4つだけ決めます。決めた分はその場で保存するので、途中で閉じても続きから再開できます。",
            ),
            stepper.clone(),
            body.clone(),
        ],
    );
    let state = Rc::new(RefCell::new(Setup {
        page: ctx.clone(),
        view: None,
        failures: Failures::new(&ctx),
        settings: json!({}),
        accounts: vec![],
        opening: json!({}),
        step: 1,
        sequence: 0,
        body,
        stepper,
    }));
    render(&ctx, &state, requested_step(&ctx.route.query("step")));
    let cleanup = state.clone();
    ctx.scope.cleanup(move || {
        if let Some(scope) = cleanup.borrow_mut().view.take() {
            scope.dispose();
        }
    });
    let mut screen = Screen::new(&ctx, root);
    screen.manages_focus = true;
    screen.update = Some(Rc::new(move |next| {
        state.borrow_mut().page = next.clone();
        render(&next, &state, requested_step(&next.route.query("step")));
        true
    }));
    Ok(screen)
}
