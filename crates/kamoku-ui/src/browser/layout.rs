// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{dom, view::View};
use dom::{append, element};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlInputElement, HtmlSelectElement};

struct Controls {
    search: HtmlInputElement,
    from: HtmlInputElement,
    to: HtmlInputElement,
    state: HtmlSelectElement,
    shortcuts: HtmlInputElement,
}

struct JournalParts {
    summary: Element,
    tbody: Element,
    results: Element,
    empty: Element,
    error: Element,
    retry: Element,
    previous: Element,
    next: Element,
    page_count: Element,
}

pub(super) fn create() -> Result<View, JsValue> {
    let root = element("main", "shell", "")?;
    root.set_attribute("id", "journal")?;
    root.set_attribute("tabindex", "-1")?;
    let brand = element("header", "brand", "")?;
    append(&brand, &element("span", "brand-name", "kamoku")?)?;
    append(&brand, &element("span", "preview-label", "Rustプレビュー")?)?;
    append(&brand, &dom::link("通常の仕訳帳へ", "/#/journal", "")?)?;
    append(&root, &brand)?;
    let heading = element("div", "page-head", "")?;
    append(&heading, &element("h1", "", "仕訳帳")?)?;
    append(&root, &heading)?;
    append(
        &root,
        &element(
            "p",
            "intro",
            "仕訳を探して、内容を確かめます。このプレビューでは検索と一覧をRustで動かしています。登録・訂正・詳細は通常の画面で開きます。",
        )?,
    )?;
    let Controls {
        search,
        from,
        to,
        state,
        shortcuts,
    } = controls(&root)?;
    let JournalParts {
        summary,
        tbody,
        results,
        empty,
        error,
        retry,
        previous,
        next,
        page_count,
    } = results(&root)?;
    append(
        &root,
        &element(
            "p",
            "footer",
            "会計の計算と保存は従来と同じRustエンジンです。検索条件はアドレスに残ります。",
        )?,
    )?;
    Ok(View {
        root,
        search,
        from,
        to,
        state,
        shortcuts,
        summary,
        tbody,
        results,
        empty,
        error,
        retry,
        previous,
        next,
        page_count,
    })
}

fn controls(root: &Element) -> Result<Controls, JsValue> {
    let form = element("form", "filters", "")?;
    let label = element("label", "search-label", "文字で探す")?;
    label.set_attribute("for", "search")?;
    append(&form, &label)?;
    let search_line = element("div", "search-line", "")?;
    let search = dom::input("search", "search")?;
    search.set_placeholder("摘要・取引先・科目・金額");
    search.set_attribute("aria-describedby", "search-hint")?;
    append(&search_line, search.as_ref())?;
    append(&search_line, &dom::button("検索", "search", "primary")?)?;
    append(&search_line, &dom::button("条件を消す", "clear", "")?)?;
    append(&form, &search_line)?;
    let hint = element(
        "p",
        "hint",
        "Enterで検索、↓で最初の仕訳へ。/で検索欄、j・kで仕訳を移動できます。",
    )?;
    hint.set_attribute("id", "search-hint")?;
    append(&form, &hint)?;
    let grid = element("div", "filter-grid", "")?;
    let from = dom::input("date", "from")?;
    let to = dom::input("date", "to")?;
    labelled(&grid, "期間の始め", "from", from.as_ref())?;
    labelled(&grid, "期間の終わり", "to", to.as_ref())?;
    let state: HtmlSelectElement = element("select", "", "")?.dyn_into()?;
    state.set_id("state");
    for (value, text) in [
        ("all", "取消済みも出す"),
        ("active", "有効だけ"),
        ("voided", "取消済みだけ"),
    ] {
        let option = element("option", "", text)?;
        option.set_attribute("value", value)?;
        append(state.as_ref(), &option)?;
    }
    labelled(&grid, "状態", "state", state.as_ref())?;
    append(&form, &grid)?;
    let shortcut_label = element("label", "shortcut-toggle", "")?;
    let shortcuts = dom::input("checkbox", "shortcuts")?;
    shortcuts.set_checked(true);
    append(&shortcut_label, shortcuts.as_ref())?;
    append(
        &shortcut_label,
        &element("span", "", "1文字のショートカットを使う")?,
    )?;
    append(&form, &shortcut_label)?;
    append(root, &form)?;
    Ok(Controls {
        search,
        from,
        to,
        state,
        shortcuts,
    })
}

fn results(root: &Element) -> Result<JournalParts, JsValue> {
    let summary = element("p", "summary", "仕訳を読み込んでいます…")?;
    summary.set_attribute("role", "status")?;
    summary.set_attribute("aria-live", "polite")?;
    append(root, &summary)?;
    let pager = element("nav", "pager", "")?;
    pager.set_attribute("aria-label", "ページ送り")?;
    let page_count = element("span", "page-count", "")?;
    let previous = dom::button("前の50件", "previous", "")?;
    let next = dom::button("次の50件", "next", "")?;
    for el in [&page_count, &previous, &next] {
        append(&pager, el)?;
    }
    append(root, &pager)?;
    let results = element("div", "table-card", "")?;
    results.set_attribute("aria-busy", "true")?;
    let table = element("table", "journal-table", "")?;
    table.set_attribute("role", "table")?;
    append(
        &table,
        &element(
            "caption",
            "",
            "日付の新しい順。詳細は通常の画面で開き、ブラウザの戻る操作でここに戻れます。",
        )?,
    )?;
    let head = element("thead", "", "")?;
    head.set_attribute("role", "rowgroup")?;
    let row = element("tr", "", "")?;
    row.set_attribute("role", "row")?;
    for (class, label) in [
        ("c-no number", "No"),
        ("c-date", "日付"),
        ("c-desc", "摘要・取引先"),
        ("c-side", "借方"),
        ("c-side", "貸方"),
        ("c-amount number", "金額（円）"),
        ("c-detail", "詳細"),
    ] {
        let cell = element("th", class, label)?;
        cell.set_attribute("scope", "col")?;
        cell.set_attribute("role", "columnheader")?;
        if label == "日付" {
            cell.set_attribute("aria-sort", "descending")?;
        }
        append(&row, &cell)?;
    }
    append(&head, &row)?;
    append(&table, &head)?;
    let tbody = element("tbody", "", "")?;
    tbody.set_attribute("role", "rowgroup")?;
    append(&table, &tbody)?;
    append(&results, &table)?;
    let empty = element(
        "p",
        "empty",
        "条件に合う仕訳はありません。条件を変えるか、「条件を消す」で一覧に戻ってください。",
    )?;
    empty.set_attribute("hidden", "")?;
    append(&results, &empty)?;
    append(root, &results)?;
    let error = element("p", "error", "")?;
    error.set_attribute("role", "alert")?;
    error.set_attribute("hidden", "")?;
    let retry = dom::button("もう一度読み込む", "retry", "")?;
    retry.set_attribute("hidden", "")?;
    append(root, &error)?;
    append(root, &retry)?;
    Ok(JournalParts {
        summary,
        tbody,
        results,
        empty,
        error,
        retry,
        previous,
        next,
        page_count,
    })
}

fn labelled(parent: &Element, text: &str, id: &str, control: &Element) -> Result<(), JsValue> {
    let field = element("div", "filter-field", "")?;
    let label = element("label", "", text)?;
    label.set_attribute("for", id)?;
    append(&field, &label)?;
    append(&field, control)?;
    append(parent, &field)
}
