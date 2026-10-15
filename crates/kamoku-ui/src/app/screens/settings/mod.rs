// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
mod data;
pub(crate) mod model;
pub(crate) mod opening;
use super::home::common::{
    Failures, banner_node, box_node, el, field_failure, heading, list, n, s, segmented, set_hidden,
    text_field,
};
use super::masters::model::invoice;
use crate::app::prelude::*;
use opening::OpeningForm;
pub(crate) const TAX_MODE_HINT: &str = "免税事業者かどうかは、前々年の課税売上などで決まります。分からなければ、税務署か税理士に確かめてください。";
pub(crate) const OPENED_HINT: &str = "この日より前の日付で取引を登録するときは、確認が出ます。期首残高の日付の初期値にもなります。\n年の途中から記帳を始めるときは、その年の1月1日にすると、年の帳票の「期首」の列に、期首残高が入ります。";
const SECTIONS: [(&str, &str); 5] = [
    ("business", "事業者"),
    ("lock", "締め"),
    ("opening", "期首残高"),
    ("data", "データ"),
    ("appearance", "見た目"),
];
#[derive(Clone)]
struct Settings {
    value: Rc<RefCell<Value>>,
    failures: Failures,
}
impl Settings {
    fn borrow(&self) -> std::cell::Ref<'_, Value> {
        self.value.borrow()
    }
    fn borrow_mut(&self) -> std::cell::RefMut<'_, Value> {
        self.value.borrow_mut()
    }
}

fn section(
    id: &str,
    title: &str,
    lead: &str,
    side: Option<Element>,
    children: &[Element],
) -> Element {
    let heading = h("h2", "card-title", title);
    attrs(
        &heading,
        &[("id", &format!("set-{id}-title")), ("tabindex", "-1")],
    );
    let node = h("section", "card settings-section", "");
    attrs(
        &node,
        &[
            ("id", &format!("set-{id}")),
            ("aria-labelledby", &heading.id()),
        ],
    );
    append(
        &node,
        &[if let Some(side) = side {
            box_node("div", "settings-head", &[heading, side])
        } else {
            heading
        }],
    );
    if !lead.is_empty() {
        let p = h("p", "small muted settings-lead", "");
        for line in lead.split('\n') {
            append(&p, &[h("span", "hint-line", line)]);
        }
        append(&node, &[p]);
    }
    append(&node, children);
    node
}
fn cols(left: Element, right: Element) -> Element {
    box_node(
        "div",
        "settings-cols",
        &[
            box_node("div", "settings-col", &[left]),
            box_node("div", "settings-col", &[right]),
        ],
    )
}
async fn save(
    ctx: &Context,
    settings: &Settings,
    patch: Value,
    fields: &[(&str, &Field)],
    button: &Element,
    message: &str,
) -> bool {
    if ctx.is_read_only() {
        ctx.error(&ctx.read_only_reason());
        return false;
    }
    for (_, field_copy) in fields {
        field_copy.set_error("");
    }
    disabled(button, true);
    let mut input = settings.borrow().clone();
    if let (Some(target), Some(patch)) = (input.as_object_mut(), patch.as_object()) {
        for (k, v) in patch {
            target.insert(k.clone(), v.clone());
        }
    }
    let output = ctx.put("/settings", &input).await;
    let success = match output {
        Ok(value) => {
            *settings.borrow_mut() = value.clone();
            ctx.shared
                .values
                .borrow_mut()
                .insert("settings".into(), value);
            if ctx.alive() {
                settings.failures.clear();
                ctx.success(message);
            }
            let _ = ctx.load("status", true).await;
            true
        }
        Err(error) => {
            if ctx.alive() && !field_failure(&error, fields) {
                settings.failures.show(&error.message);
            }
            false
        }
    };
    if ctx.alive() {
        disabled(button, ctx.is_read_only());
    }
    success
}
fn business(ctx: &Context, settings: &Settings) -> Element {
    let current = settings.borrow();
    let (name, name_f) = text_field(
        "事業者名",
        s(&current, "business_name"),
        "メニューの上に出ます。",
        true,
    );
    name.set_max_length(80);
    name.set_placeholder("例: サンプル事務所");
    let (opened, opened_f) = text_field("開業日", s(&current, "opened_on"), OPENED_HINT, true);
    let (inv, inv_f) = text_field(
        "登録番号（適格請求書発行事業者）",
        s(&current, "invoice_number"),
        "課税事業者のときだけ入れられます。形式（T と13桁の数字）だけを確かめます。",
        false,
    );
    inv.set_max_length(20);
    inv.set_placeholder("例: T1234567890123");
    let inv_box = inv_f.el.clone();
    let mode = segmented(
        ctx,
        "消費税の課税の区分",
        &[("exempt", "免税事業者"), ("taxable", "課税事業者")],
        s(&current, "tax_mode"),
        move |value| set_hidden(&inv_box, value != "taxable"),
    );
    set_hidden(&inv_f.el, mode.get() != "taxable");
    let mode_hint = h("p", "field-hint", TAX_MODE_HINT);
    attrs(&mode_hint, &[("id", &uid("mode-hint"))]);
    attrs(&mode.node, &[("aria-describedby", &mode_hint.id())]);
    let rounding = select(
        &[
            ("floor", "切捨て"),
            ("round", "四捨五入"),
            ("ceil", "切上げ"),
        ],
        s(&current, "rounding"),
    );
    let rounding_f = field(
        "消費税の端数処理",
        &el(&rounding),
        "記帳の目安の税額を出すときの、1円未満の扱いです。",
        false,
    );
    drop(current);
    let button = button("事業者の設定を保存", "primary");
    disabled(&button, ctx.is_read_only());
    for (control, field_copy) in [
        (name.clone(), name_f.clone()),
        (opened.clone(), opened_f.clone()),
        (inv.clone(), inv_f.clone()),
    ] {
        ctx.on(&el(&control), "input", move |_| field_copy.set_error(""));
    }
    let date_control = opened.clone();
    let field_copy = opened_f.clone();
    ctx.on(&el(&opened), "blur", move |_| {
        match parse_date(&date_control.value(), &today()) {
            Ok(v) => {
                date_control.set_value(&v);
                field_copy.set_error("");
            }
            Err(e) => field_copy.set_error(&e),
        }
    });
    let context_copy = ctx.clone();
    let settings = settings.clone();
    let button_copy = button.clone();
    let mode2 = mode.clone();
    let nf = name_f.clone();
    let of = opened_f.clone();
    let inf = inv_f.clone();
    ctx.on(&button,"click",move |_|{let name_value=name.value().trim().to_owned();if name_value.is_empty(){nf.set_error("事業者名を入れてください。");let _=name.focus();return;}let date=match parse_date(&opened.value(),&today()){Ok(v)=>{opened.set_value(&v);v},Err(e)=>{of.set_error(&e);let _=opened.focus();return;}};let mode=mode2.get();let inv_value=if mode=="taxable"{match invoice(&inv.value()){Ok(v)=>v.map_or(Value::Null, Value::String),Err(e)=>{inf.set_error(&e);let _=inv.focus();return;}}}else{Value::Null};let patch=json!({"business_name":name_value,"opened_on":date,"tax_mode":mode,"invoice_number":inv_value,"rounding":rounding.value()});let context_copy=context_copy.clone();let settings=settings.clone();let button_copy=button_copy.clone();let fields=(nf.clone(),of.clone(),inf.clone());let task=context_copy.clone();context_copy.task(async move{save(&task,&settings,patch,&[("business_name",&fields.0),("opened_on",&fields.1),("invoice_number",&fields.2)],&button_copy,"設定を保存しました。").await;Ok(())});});
    section(
        "business",
        "事業者",
        "課税の区分を「免税事業者」にすると、取引の登録と未確認で税区分の欄を隠します。",
        None,
        &[
            cols(
                box_node("div", "settings-grid", &[name_f.el, opened_f.el]),
                box_node("div", "", &[mode.node, mode_hint, inv_f.el, rounding_f.el]),
            ),
            box_node("div", "settings-actions", &[button]),
        ],
    )
}
fn lock_state(date: &str) -> String {
    if date.is_empty() {
        "締めていません".into()
    } else {
        format!("{date} まで締め済み")
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn lock(ctx: &Context, settings: &Settings) -> Element {
    let (lock, field_copy) = text_field(
        "この日以前は変更しない",
        s(&settings.borrow(), "locked_until"),
        "決算や申告が済んだ期間を締めると、その期間の登録・訂正・取消を断ります。空にすると締めません。",
        false,
    );
    lock.set_placeholder(&format!(
        "例: {}-12-31（締めないなら空）",
        today()[..4].parse::<i32>().unwrap_or(2026) - 1
    ));
    let state = h(
        "span",
        "badge badge--muted lock-state",
        &lock_state(s(&settings.borrow(), "locked_until")),
    );
    let save_button = button("締めた日を保存", "primary");
    disabled(&save_button, ctx.is_read_only());
    let date_control = lock.clone();
    let field = field_copy.clone();
    ctx.on(&el(&lock), "blur", move |_| {
        if date_control.value().trim().is_empty() {
            field.set_error("");
        } else {
            match parse_date(&date_control.value(), &today()) {
                Ok(v) => {
                    date_control.set_value(&v);
                    field.set_error("");
                }
                Err(e) => field.set_error(&e),
            }
        }
    });
    let field = field_copy.clone();
    ctx.on(&el(&lock), "input", move |_| field.set_error(""));
    let context_copy = ctx.clone();
    let settings = settings.clone();
    let button_copy = save_button.clone();
    let field = field_copy.clone();
    let output = state.clone();
    let pending = Rc::new(Cell::new(false));
    ctx.on(&save_button, "click", move |_| {
        if pending.get() || context_copy.is_read_only() {
            return;
        }
        let next = if lock.value().trim().is_empty() {
            String::new()
        } else {
            match parse_date(&lock.value(), &today()) {
                Ok(v) => {
                    lock.set_value(&v);
                    v
                }
                Err(e) => {
                    field.set_error(&e);
                    let _ = lock.focus();
                    return;
                }
            }
        };
        let before = s(&settings.borrow(), "locked_until").to_owned();
        if before == next {
            context_copy.info(&format!(
                "締めた日は変わっていません（{}）。",
                lock_state(&before)
            ));
            return;
        }
        pending.set(true);
        let context_copy = context_copy.clone();
        let settings = settings.clone();
        let button_copy = button_copy.clone();
        let field = field.clone();
        let output = output.clone();
        let pending = pending.clone();
        let task = context_copy.clone();
        context_copy.task(async move {
            let title = if next.is_empty() {
                "締めを外しますか".into()
            } else {
                format!("{next} まで締めますか")
            };
            let body = if next.is_empty() {
                format!("締めを外すと、{before} 以前の仕訳を、また直せるようになります。")
            } else {
                let reopened = if !before.is_empty() && next < before {
                    format!(
                        "{}〜{before} の仕訳は、また直せるようになります。",
                        add_days(&next, 1)
                    )
                } else {
                    String::new()
                };
                format!("{next} 以前の、登録・訂正・取消ができなくなります。{reopened}")
            };
            if confirm(
                &task,
                &title,
                &body,
                if next.is_empty() {
                    "外す"
                } else {
                    "締める"
                },
                false,
            )
            .await
                && task.alive()
            {
                let message = if next.is_empty() {
                    "締めを外しました。".into()
                } else {
                    format!("{next} まで締めました。")
                };
                if save(
                    &task,
                    &settings,
                    json!({"locked_until":if next.is_empty(){Value::Null}else{json!(next)}}),
                    &[("locked_until", &field)],
                    &button_copy,
                    &message,
                )
                .await
                    && task.alive()
                {
                    set_text(&output, &lock_state(s(&settings.borrow(), "locked_until")));
                }
            }
            pending.set(false);
            Ok(())
        });
    });
    section(
        "lock",
        "締め",
        "",
        Some(state),
        &[
            field_copy.el,
            box_node("div", "settings-actions", &[save_button]),
        ],
    )
}
fn opening_section(
    ctx: &Context,
    settings: &Settings,
    accounts: &[Value],
    opening: &Value,
) -> Element {
    let form = OpeningForm::new(
        ctx,
        accounts,
        opening,
        s(&settings.borrow(), "opened_on"),
        None,
    );
    let button = button("期首残高を保存", "primary");
    disabled(&button, ctx.is_read_only());
    let context_copy = ctx.clone();
    let field_copy = form.clone();
    let failures = settings.failures.clone();
    let button_copy = button.clone();
    ctx.on(&button,"click",move |_|{if context_copy.is_read_only(){return;}let Some(input)=field_copy.collect()else{return};disabled(&button_copy,true);let context_copy=context_copy.clone();let field_copy=field_copy.clone();let failures=failures.clone();let button_copy=button_copy.clone();let task=context_copy.clone();context_copy.task(async move{let response=task.put("/opening",&input).await;if task.alive(){match response{Ok(value)=>{failures.clear();
                        task.success(&if value["entry_id"].is_null(){"期首残高をすべて0にしました（仕訳はありません）。".into()}else{format!("期首残高を保存しました（元入金 {}円）。",number(n(&value,"capital")))});let _=task.load("status",true).await;},Err(e)if e.is_stale()=>{let _=task.get("/opening",&[]).await;if task.alive(){task.info("他の場所で更新されました。読み込み直しました。入力はそのままです。もう一度「期首残高を保存」を押してください。");}},Err(e)=>{if !field_copy.show_error(&e){failures.show(&e.message);}}}disabled(&button_copy,task.is_read_only());}Ok(())});});
    section(
        "opening",
        "期首残高",
        "開業日（または記帳を始めた日）の、口座などの残高です。\n見直すと、期首残高の仕訳が「期首残高の見直し」という理由で訂正されます。",
        None,
        &[form.el, box_node("div", "settings-actions", &[button])],
    )
}
fn appearance(ctx: &Context) -> Element {
    let context_copy = ctx.clone();
    let theme = segmented(
        ctx,
        "明るさ",
        &[
            ("light", "ライト"),
            ("dark", "ダーク"),
            ("os", "OSに合わせる"),
        ],
        &ctx.get_theme(),
        move |v| context_copy.set_theme(&v),
    );
    let context_copy = ctx.clone();
    let density = segmented(
        ctx,
        "表の行の密度",
        &[("standard", "標準"), ("compact", "狭い")],
        &ctx.get_density(),
        move |v| context_copy.set_density(&v),
    );
    let t = theme.clone();
    let date_control = density.clone();
    let context_copy = ctx.clone();
    ctx.scope.listen(
        web_sys::window().unwrap().as_ref(),
        "kamoku-appearance",
        false,
        move |_| {
            t.set(&context_copy.get_theme());
            date_control.set(&context_copy.get_density());
        },
    );
    section(
        "appearance",
        "見た目",
        "選ぶと、すぐに変わります（保存のボタンはありません）。このブラウザの中にだけ覚え、帳簿には入りません。",
        None,
        &[cols(theme.node, density.node)],
    )
}
fn jump(body: &Element, tabs: &[Element], name: &str) {
    if let Ok(Some(section)) = body.query_selector(&format!("#set-{name}")) {
        section.scroll_into_view();
        if let Ok(Some(heading)) = section.query_selector("h2") {
            focus(&heading);
        }
        for tab in tabs {
            if tab.get_attribute("data-section").as_deref() == Some(name) {
                attrs(tab, &[("aria-current", "true")]);
            } else {
                let _ = tab.remove_attribute("aria-current");
            }
        }
    }
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let root = h("div", "", "");
    let body = h("div", "settings", "");
    let nav = h("nav", "settings-nav", "");
    attrs(&nav, &[("aria-label", "設定の節")]);
    let tabs: Vec<_> = SECTIONS
        .iter()
        .map(|(id, label)| {
            let tab = button(label, "");
            attrs(&tab, &[("class", "settings-tab"), ("data-section", id)]);
            append(&nav, std::slice::from_ref(&tab));
            tab
        })
        .collect();
    if let Some(first) = tabs.first() {
        attrs(first, &[("aria-current", "true")]);
    }
    for (id, _) in SECTIONS {
        let body = body.clone();
        let tabs = tabs.clone();
        let button = tabs
            .iter()
            .find(|t| t.get_attribute("data-section").as_deref() == Some(id))
            .unwrap()
            .clone();
        ctx.on(&button, "click", move |_| jump(&body, &tabs, id));
    }
    append(&root, &[heading("設定", None), nav.clone(), body.clone()]);
    let context_copy = ctx.clone();
    let output = body.clone();
    let tabs2 = tabs.clone();
    ctx.task(async move {
        let response = async {
            let settings = context_copy.load("settings", true).await?;
            let accounts = context_copy.load("accounts", true).await?;
            let opening = context_copy.get("/opening", &[]).await?;
            let status = context_copy.load("status", true).await?;
            Ok::<_, ApiError>((settings, accounts, opening, status))
        }
        .await;
        if !context_copy.alive() {
            return Ok(());
        }
        match response {
            Ok((settings, accounts, opening, status)) => {
                let settings = Settings {
                    value: Rc::new(RefCell::new(settings)),
                    failures: Failures::new(&context_copy),
                };
                let mut children = vec![];
                if context_copy.is_read_only() {
                    children.push(banner_node(
                        "読み取り専用のため、保存はできません",
                        &context_copy.read_only_reason(),
                        true,
                    ));
                }
                children.extend([
                    business(&context_copy, &settings),
                    lock(&context_copy, &settings),
                    opening_section(&context_copy, &settings, &list(&accounts), &opening),
                    section(
                        "data",
                        "データ",
                        "",
                        None,
                        &[data::mount(&context_copy, &status)],
                    ),
                    appearance(&context_copy),
                ]);
                replace(&output, &children);
                let target = context_copy.route.query("section");
                if !target.is_empty() {
                    jump(&output, &tabs2, &target);
                }
            }
            Err(e) => replace(
                &output,
                &[banner_node("設定を読み込めませんでした", &e.message, true)],
            ),
        }
        Ok(())
    });
    let output = body.clone();
    let tabs_scroll = tabs.clone();
    let nav2 = nav.clone();
    ctx.scope.listen(
        web_sys::window().unwrap().as_ref(),
        "scroll",
        false,
        move |_| {
            let line = nav2.get_bounding_client_rect().bottom() + 24.0;
            let mut current = "business";
            for (id, _) in SECTIONS {
                if let Ok(Some(section)) = output.query_selector(&format!("#set-{id}"))
                    && section.get_bounding_client_rect().top() <= line
                {
                    current = id;
                }
            }
            for tab in &tabs_scroll {
                if tab.get_attribute("data-section").as_deref() == Some(current) {
                    attrs(tab, &[("aria-current", "true")]);
                } else {
                    let _ = tab.remove_attribute("aria-current");
                }
            }
        },
    );
    let mut screen = Screen::new(&ctx, root);
    screen.update = Some(Rc::new(move |next| {
        let target = next.route.query("section");
        if !target.is_empty() {
            jump(&body, &tabs, &target);
        }
        true
    }));
    Ok(screen)
}
