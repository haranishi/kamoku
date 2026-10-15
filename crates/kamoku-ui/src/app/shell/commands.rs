// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::{core::keys, prelude::*};

#[derive(Clone)]
struct Command {
    label: String,
    reading: String,
    keywords: String,
    action: Action,
}
#[derive(Clone)]
enum Action {
    Go(String),
    Theme(&'static str),
    Density,
    Help,
    Single,
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
fn commands(ctx: &Context) -> Vec<Command> {
    let mut list = vec![];
    for (label, reading, hash) in [
        ("ホーム", "ほーむ", "#/"),
        ("取引を登録", "とりひきをとうろく", "#/entry"),
        ("未確認", "みかくにん", "#/inbox"),
        ("取込（明細のCSV）", "とりこみめいさい", "#/import"),
        ("仕訳帳", "しわけちょう", "#/journal"),
        ("帳票", "ちょうひょう", "#/reports"),
        (
            "帳票: 損益計算書",
            "そんえきけいさんしょちょうひょう",
            "#/reports/income-statement",
        ),
        (
            "帳票: 貸借対照表",
            "たいしゃくたいしょうひょうちょうひょう",
            "#/reports/balance-sheet",
        ),
        (
            "帳票: 試算表",
            "しさんひょうちょうひょう",
            "#/reports/trial-balance",
        ),
        (
            "帳票: 総勘定元帳",
            "そうかんじょうもとちょうちょうひょう",
            "#/reports/ledger",
        ),
        (
            "帳票: 消費税",
            "しょうひぜいちょうひょう",
            "#/reports/tax-summary",
        ),
        ("口座", "こうざ", "#/wallets"),
        ("勘定科目", "かんじょうかもく", "#/accounts"),
        ("取引先", "とりひきさき", "#/partners"),
        ("設定", "せってい", "#/settings"),
        (
            "台帳を検査する（設定のデータ）",
            "だいちょうをけんさはっしゅしょうごう",
            "#/settings?section=data",
        ),
        (
            "期首残高を見直す（設定）",
            "きしゅざんだかもといれきん",
            "#/settings?section=opening",
        ),
        ("口座を追加（口座の画面）", "こうざをついか", "#/wallets"),
    ] {
        list.push(Command {
            label: label.into(),
            reading: reading.into(),
            keywords: String::new(),
            action: Action::Go(hash.into()),
        });
    }
    for (label, reading, keywords) in [
        (
            "仕訳を訂正する（仕訳帳で仕訳を開く）",
            "しわけをていせいする",
            "編集 修正 直す 変更 へんしゅう しゅうせい なおす へんこう",
        ),
        (
            "仕訳を取り消す（仕訳帳で仕訳を開く）",
            "しわけをとりけす",
            "削除 消す 取り消し 削る さくじょ けす とりけし",
        ),
    ] {
        list.push(Command {
            label: label.into(),
            reading: reading.into(),
            keywords: keywords.into(),
            action: Action::Go("#/journal".into()),
        });
    }
    for (label, reading, action) in [
        (
            "表示をライトにする",
            "ひょうじらいとあかるい",
            Action::Theme("light"),
        ),
        (
            "表示をダークにする",
            "ひょうじだーくくらい",
            Action::Theme("dark"),
        ),
        (
            "表示をOSに合わせる",
            "ひょうじおーえす",
            Action::Theme("os"),
        ),
        ("行の密度を切り替える", "ぎょうのみつど", Action::Density),
        ("ショートカットの一覧", "しょーとかっと", Action::Help),
        (
            "1文字のショートカットを切り替える",
            "いちもじのしょーとかっとをきりかえる",
            Action::Single,
        ),
    ] {
        list.push(Command {
            label: if matches!(action, Action::Single) {
                format!(
                    "{label}（いまは{}）",
                    if keys::single_enabled() {
                        "使っています"
                    } else {
                        "切ってあります"
                    }
                )
            } else {
                label.into()
            },
            reading: reading.into(),
            keywords: String::new(),
            action,
        });
    }
    if let Some(accounts) = ctx.peek("accounts").and_then(|v| v.as_array().cloned()) {
        for a in accounts {
            if a["enabled"] != false {
                let code = a["code"].as_str().unwrap_or("");
                list.push(Command {
                    label: format!("科目: {}", a["name"].as_str().unwrap_or(code)),
                    reading: a["reading"].as_str().unwrap_or("").into(),
                    keywords: code.into(),
                    action: Action::Go(build_hash("/journal", &[("account", code)])),
                });
            }
        }
    }
    list
}
fn run(ctx: &Context, action: Action) {
    match action {
        Action::Go(hash) => ctx.navigate(&hash, false),
        Action::Theme(t) => ctx.set_theme(t),
        Action::Density => ctx.set_density(if ctx.get_density() == "compact" {
            "standard"
        } else {
            "compact"
        }),
        Action::Help => help(),
        Action::Single => {
            keys::set_single(!keys::single_enabled());
            sync_single();
            ctx.info(if keys::single_enabled() {
                "1文字のショートカットを使います。"
            } else {
                "1文字のショートカットを切りました。"
            });
        }
    }
}
struct Palette {
    all: Vec<Command>,
    shown: Vec<Command>,
    active: usize,
    input: HtmlInputElement,
    list: Element,
}
fn paint(state: &Rc<RefCell<Palette>>) {
    let mut s = state.borrow_mut();
    let query = normalize_search(&s.input.value());
    let mut ranked = s
        .all
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            [c.label.as_str(), c.reading.as_str(), c.keywords.as_str()]
                .iter()
                .filter_map(|v| normalize_search(v).find(&query))
                .map(|n| usize::from(n > 0))
                .min()
                .map(|rank| (rank, i, c.clone()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(rank, i, _)| (*rank, *i));
    s.shown = ranked.into_iter().take(100).map(|(_, _, c)| c).collect();
    let text = s.input.value().trim().to_owned();
    if !text.is_empty() {
        s.shown.push(Command {
            label: format!("仕訳を「{text}」で探す"),
            reading: String::new(),
            keywords: String::new(),
            action: Action::Go(build_hash("/journal", &[("q", &text)])),
        });
    }
    s.active = 0;
    replace(&s.list, &[]);
    for (i, c) in s.shown.iter().enumerate() {
        let row = h("li", "palette-option", &c.label);
        attrs(
            &row,
            &[
                ("role", "option"),
                ("id", &format!("palette-option-{i}")),
                ("data-index", &i.to_string()),
                ("aria-selected", if i == 0 { "true" } else { "false" }),
            ],
        );
        append(&s.list, &[row]);
    }
    if !s.shown.is_empty() {
        attrs(&s.input, &[("aria-activedescendant", "palette-option-0")]);
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
pub(crate) fn palette() {
    if let Some(open) = document()
        .query_selector("dialog.palette[open]")
        .ok()
        .flatten()
    {
        open.unchecked_ref::<web_sys::HtmlDialogElement>().close();
        return;
    }
    if let Ok(old) = document().query_selector_all("dialog.palette:not([open])") {
        for i in 0..old.length() {
            if let Some(e) = old.item(i).and_then(|e| e.dyn_into::<Element>().ok()) {
                e.remove();
            }
        }
    }
    let Some(ctx) = super::super::global_context() else {
        return;
    };
    let d = Dialog::new(&ctx, "コマンド");
    d.el.set_class_name("dialog palette");
    let input = input("text", "");
    input.set_class_name("input palette-input");
    attrs(
        &input,
        &[
            ("role", "combobox"),
            ("aria-label", "コマンドを探す"),
            ("aria-autocomplete", "list"),
            ("aria-expanded", "true"),
            ("aria-controls", "palette-results"),
            ("placeholder", "画面・操作・科目・仕訳を探す"),
        ],
    );
    let list = h("ul", "palette-list", "");
    list.set_id("palette-results");
    attrs(
        &list,
        &[("role", "listbox"), ("aria-label", "コマンドの候補")],
    );
    append(&d.body, &[input.clone().into(), list.clone()]);
    let state = Rc::new(RefCell::new(Palette {
        all: commands(&ctx),
        shown: vec![],
        active: 0,
        input: input.clone(),
        list: list.clone(),
    }));
    paint(&state);
    let s = state.clone();
    d.scope.on(&input, "input", move |_| paint(&s));
    let s = state.clone();
    let modal = d.clone();
    let context = ctx.clone();
    d.scope.on(&input, "keydown", move |event| {
        let Ok(e) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(&e) {
            return;
        }
        match e.key().as_str() {
            "ArrowDown" | "ArrowUp" => {
                e.prevent_default();
                let mut s = s.borrow_mut();
                if s.shown.is_empty() {
                    return;
                }
                s.active = if e.key() == "ArrowDown" {
                    (s.active + 1).min(s.shown.len() - 1)
                } else {
                    s.active.saturating_sub(1)
                };
                let active = s.active;
                for i in 0..s.list.children().length() {
                    if let Some(el) = s.list.children().item(i) {
                        attrs(
                            &el,
                            &[(
                                "aria-selected",
                                if i as usize == active {
                                    "true"
                                } else {
                                    "false"
                                },
                            )],
                        );
                    }
                }
                attrs(
                    &s.input,
                    &[("aria-activedescendant", &format!("palette-option-{active}"))],
                );
            }
            "Enter" => {
                e.prevent_default();
                let command = {
                    let s = s.borrow();
                    s.shown.get(s.active).cloned()
                };
                if let Some(command) = command {
                    modal.close();
                    run(&context, command.action);
                }
            }
            _ => {}
        }
    });
    let s = state.clone();
    let modal = d.clone();
    let context = ctx;
    d.scope.on(&list, "click", move |e| {
        let index = e
            .target()
            .and_then(|e| e.dyn_into::<Element>().ok())
            .and_then(|e| e.closest("[data-index]").ok().flatten())
            .and_then(|e| e.get_attribute("data-index"))
            .and_then(|v| v.parse::<usize>().ok());
        if let Some(action) = index.and_then(|i| s.borrow().shown.get(i).map(|c| c.action.clone()))
        {
            modal.close();
            run(&context, action);
        }
    });
    d.show();
    focus(&input);
}
fn sync_single() {
    let on = keys::single_enabled();
    if let Ok(list) = document().query_selector_all("[data-single-keys]") {
        for i in 0..list.length() {
            if let Some(e) = list
                .item(i)
                .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
            {
                e.set_checked(on);
            }
        }
    }
    if let Ok(list) = document().query_selector_all(".keys-off-mark") {
        for i in 0..list.length() {
            if let Some(e) = list.item(i).and_then(|e| e.dyn_into::<Element>().ok()) {
                if on {
                    attrs(&e, &[("hidden", "")]);
                } else {
                    let _ = e.remove_attribute("hidden");
                }
            }
        }
    }
}
fn section(title: &str, rows: &[(&str, &str, bool)]) -> Element {
    let section = h("section", "keys-section", "");
    let wrap = h("div", "table-wrap", "");
    let table = h("table", "table keys-table", "");
    let head = h("thead", "", "");
    let tr = h("tr", "", "");
    let key = h("th", "", "キー");
    let what = h("th", "", "動き");
    attrs(&key, &[("scope", "col")]);
    attrs(&what, &[("scope", "col")]);
    append(&tr, &[key, what]);
    append(&head, &[tr]);
    let body = h("tbody", "", "");
    for (keys, text, single) in rows {
        let tr = h("tr", "", "");
        let k = h("th", "", "");
        attrs(&k, &[("scope", "row")]);
        for (index, key) in keys
            .replace("Ctrl", keys::mod_label())
            .split_whitespace()
            .enumerate()
        {
            if index > 0 {
                k.append_child(&document().create_text_node(" "))
                    .expect("key separator");
            }
            append(&k, &[h("kbd", "", key)]);
        }
        let text = h("td", "wrap", text);
        if *single {
            let mark = h(
                "span",
                "badge badge--muted keys-off-mark",
                "いま切ってあります",
            );
            attrs(&mark, &[("hidden", "")]);
            append(&text, &[mark]);
        }
        append(&tr, &[k, text]);
        append(&body, &[tr]);
    }
    append(
        &table,
        &[
            h("caption", "", &format!("{title}のショートカット")),
            head,
            body,
        ],
    );
    append(&wrap, &[table]);
    append(&section, &[h("h3", "", title), wrap]);
    section
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
pub(crate) fn help() {
    let Some(ctx) = super::super::global_context() else {
        return;
    };
    let d = Dialog::new(&ctx, "ショートカット");
    let body = h("div", "keys-help", "");
    let toggle = h("div", "keys-toggle", "");
    let label = h("label", "check", "");
    let box_el = input("checkbox", "");
    box_el.set_checked(keys::single_enabled());
    attrs(&box_el, &[("data-single-keys", "")]);
    append(
        &label,
        &[
            box_el.clone().into(),
            h("span", "", "1文字のショートカットを使う"),
        ],
    );
    append(
        &toggle,
        &[
            label,
            h(
                "p",
                "field-hint",
                "切ると、画面のどこでも働く1文字のキーを止めます。CtrlK・CtrlEnter・Escと欄や行の中だけで働くキーは使えます。",
            ),
        ],
    );
    append(
        &body,
        &[
            toggle,
            h(
                "p",
                "muted small",
                "文字を打つ欄にいるとき（変換中を含む）は、1文字のキーは働きません。",
            ),
        ],
    );
    append(
        &body,
        &[
            section(
                "全体",
                &[
                    (
                        "Ctrl K",
                        "コマンド一覧（画面の移動、主な操作、科目と仕訳を探す）",
                        false,
                    ),
                    ("g h", "ホームへ", true),
                    ("g n", "取引の登録へ（nだけでも同じ）", true),
                    ("g i", "未確認へ", true),
                    ("g j", "仕訳帳へ", true),
                    ("g r", "帳票へ", true),
                    ("g w", "口座へ", true),
                    ("g s", "設定へ", true),
                    ("/", "一覧の検索欄へ", true),
                    ("?", "この一覧", true),
                    ("Esc", "閉じる、取り消す、候補を閉じる", false),
                ],
            ),
            section(
                "取引の登録",
                &[
                    ("1 2 3", "種別の欄で、支出・収入・振替を選ぶ", false),
                    ("Tab", "次の欄へ（Enterも次の欄へ進む）", false),
                    (
                        "Ctrl Enter",
                        "保存して続ける（最後の欄のEnterでも保存）",
                        false,
                    ),
                    ("t", "日付の欄で今日にする", false),
                    ("↑ ↓", "日付の欄で1日ずつ動かす（+と-も同じ）", false),
                ],
            ),
            section(
                "仕訳帳",
                &[
                    ("j k", "下の行、上の行へ（上下の矢印も同じ）", true),
                    ("Enter", "詳細を開く", false),
                    ("e", "訂正する", true),
                    ("v", "取り消す", true),
                    ("[ ]", "前の月、次の月へ", true),
                ],
            ),
            section(
                "未確認",
                &[
                    (
                        "Enter",
                        "行を確定する行にして、次の行の科目へ（科目の候補が開いていれば、選んでから進む）",
                        false,
                    ),
                    (
                        "Ctrl Enter",
                        "確定する行を確定する（先に件数と、入金・出金の合計の確認が出る）",
                        false,
                    ),
                    (
                        "↑ ↓",
                        "同じ列の、上の行・下の行へ（候補の一覧が閉じているとき）。税区分の欄では、行を動かさずに税区分を切り替える",
                        false,
                    ),
                    (
                        "Esc",
                        "候補の一覧を閉じる。閉じていれば、行の印へ。検索の欄では、欄から抜ける",
                        false,
                    ),
                    ("x", "無視する（行の印か「無視」のボタンにいるとき）", false),
                    (
                        "Ctrl Z",
                        "最後に無視した行を戻す（文字の欄にいるときは、ブラウザの標準の取り消し）",
                        false,
                    ),
                    (
                        "Shift Space",
                        "確定する行の印を付ける、外す（取引先とメモの欄では空白を打つ）",
                        false,
                    ),
                ],
            ),
            section(
                "帳票",
                &[
                    ("[ ]", "前の期間、次の期間へ（年・月・日付）", true),
                    ("j k", "総勘定元帳で、下の行、上の行へ", true),
                ],
            ),
        ],
    );
    append(&d.body, &[body]);
    let close = button("閉じる", "primary");
    let modal = d.clone();
    d.scope.on(&close, "click", move |_| modal.close());
    append(&d.footer, &[close]);
    let box_copy = box_el.clone();
    let context = ctx;
    d.scope.on(&box_el, "change", move |_| {
        keys::set_single(box_copy.checked());
        sync_single();
        context.info(if keys::single_enabled() {
            "1文字のショートカットを使います。"
        } else {
            "1文字のショートカットを切りました。"
        });
    });
    d.show();
    sync_single();
}
