// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Candidate matching and popup geometry shared by the browser combobox.

use crate::input::normalize_search;
use serde_json::{Value, json};

pub(crate) const OPTION_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComboItem {
    pub value: String,
    pub label: String,
    pub reading: String,
    pub meta: Value,
}

pub(crate) fn fold(text: &str) -> String {
    normalize_search(text)
}

/// Prefix matches precede substring matches; the caller's order breaks ties.
pub(crate) fn match_rank(folded_query: &str, item: &ComboItem) -> Option<u8> {
    if folded_query.is_empty() {
        return Some(0);
    }
    let mut rank = None;
    for text in [
        item.label.as_str(),
        item.reading.as_str(),
        item.value.as_str(),
        item.meta["keywords"].as_str().unwrap_or(""),
    ] {
        let text = fold(text);
        if text.starts_with(folded_query) {
            return Some(0);
        }
        if text.contains(folded_query) {
            rank = Some(1);
        }
    }
    rank
}

pub(crate) fn filter_items<'a>(items: &'a [ComboItem], query: &str) -> Vec<&'a ComboItem> {
    let query = fold(query);
    let mut ranked = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match_rank(&query, item).map(|rank| (rank, index, item)))
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    ranked.into_iter().map(|(_, _, item)| item).collect()
}

pub(crate) fn filter_options(items: &[ComboItem], query: &str, limit: usize) -> Vec<ComboItem> {
    filter_items(items, query)
        .into_iter()
        .take(limit)
        .cloned()
        .collect()
}

pub(crate) fn exact_matches<'a>(items: &'a [ComboItem], text: &str) -> Vec<&'a ComboItem> {
    let query = fold(text);
    if query.is_empty() {
        return Vec::new();
    }
    items
        .iter()
        .filter(|item| {
            fold(&item.label) == query || (!item.reading.is_empty() && fold(&item.reading) == query)
        })
        .collect()
}

pub(crate) fn pick_on_blur(items: &[ComboItem], text: &str) -> Option<ComboItem> {
    let matches = exact_matches(items, text);
    (matches.len() == 1).then(|| matches[0].clone())
}

pub(crate) fn same_name<'a>(items: &'a [ComboItem], text: &str) -> Vec<&'a ComboItem> {
    let query = fold(text);
    if query.is_empty() {
        return Vec::new();
    }
    items
        .iter()
        .filter(|item| fold(&item.label) == query)
        .collect()
}

pub(crate) fn options_with_new(
    items: &[ComboItem],
    text: &str,
    create_label: &dyn Fn(&str) -> String,
    limit: usize,
) -> Vec<ComboItem> {
    let query = fold(text);
    let mut found = filter_options(items, text, limit);
    found.sort_by_key(|item| query.is_empty() || fold(&item.label) != query);
    let name = text.trim();
    // Search every item, including candidates beyond the visible limit, for a duplicate name.
    if !name.is_empty() && same_name(items, name).is_empty() {
        found.push(ComboItem {
            value: name.into(),
            label: name.into(),
            reading: String::new(),
            meta: json!({"isNew":true,"is_new":true,"optionLabel":create_label(name)}),
        });
    }
    found
}

pub(crate) fn move_active(active: Option<usize>, count: usize, down: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match active {
        None => 0,
        Some(index) if down => index.saturating_add(1).min(count - 1),
        Some(index) => index.saturating_sub(1).min(count - 1),
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct InputRect {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub width: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ListPlacement {
    pub above: bool,
    pub width: f64,
    pub left: f64,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    pub max_height: f64,
}

pub(crate) fn list_placement(
    rect: InputRect,
    viewport_width: f64,
    viewport_height: f64,
) -> ListPlacement {
    let width = rect.width.clamp(240.0, 420.0).min(viewport_width - 16.0);
    let left = rect.left.min(viewport_width - width - 8.0).max(8.0);
    let below = viewport_height - rect.bottom - 8.0;
    let above = rect.top - 8.0;
    let opens_above = below < 288.0 && above > below;
    ListPlacement {
        above: opens_above,
        width,
        left,
        top: (!opens_above).then_some(rect.bottom + 4.0),
        bottom: opens_above.then_some(viewport_height - rect.top + 4.0),
        max_height: if opens_above {
            above.min(288.0)
        } else {
            below.clamp(120.0, 288.0)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(value: &str, label: &str, reading: &str) -> ComboItem {
        ComboItem {
            value: value.into(),
            label: label.into(),
            reading: reading.into(),
            meta: Value::Null,
        }
    }

    fn kana_items() -> Vec<ComboItem> {
        vec![
            item("exp_comm", "通信費", "つうしんひ"),
            item("exp_supplies", "消耗品費", "しょうもうひんひ"),
            item("exp_travel", "旅費交通費", "りょひこうつうひ"),
            item("exp_fee", "支払手数料", "しはらいてすうりょう"),
            item("sales", "売上高", "うりあげだか"),
        ]
    }

    fn accounts() -> Vec<ComboItem> {
        vec![
            item("exp_comm", "通信費", "つうしんひ"),
            item("exp_travel", "旅費交通費", "りょひこうつうひ"),
            item("exp_supplies", "消耗品費", "しょうもうひんひ"),
            item("exp_utility", "水道光熱費", "すいどうこうねつひ"),
            item("sales", "売上高", "うりあげだか"),
            item("w03", "サンプルカード", "さんぷるかーど"),
        ]
    }

    fn partners() -> Vec<ComboItem> {
        vec![
            item("サンプル通信社", "サンプル通信社", "さんぷるつうしんしゃ"),
            item("サンプル通信", "サンプル通信", "さんぷるつうしん"),
        ]
    }

    fn values(items: &[ComboItem]) -> Vec<&str> {
        items.iter().map(|item| item.value.as_str()).collect()
    }

    fn borrowed_values<'a>(items: &[&'a ComboItem]) -> Vec<&'a str> {
        items.iter().map(|item| item.value.as_str()).collect()
    }

    fn new_label(name: &str) -> String {
        format!("「{name}」を新しい取引先として足す")
    }

    fn is_new(item: &ComboItem) -> bool {
        item.meta["isNew"] == true
    }

    fn assert_px(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn ac_05_1_fold_katakana_half_width_hiragana_and_ascii() {
        for text in ["ツウシン", "ﾂｳｼﾝ", "つうしん"] {
            assert_eq!(fold(text), "つうしん");
        }
        assert_eq!(fold("ＡＢＣ ｄｅｆ"), "abcdef");
        assert_eq!(fold("Exp_Comm"), "exp_comm");
    }

    #[test]
    fn ac_05_1_filter_name_reading_and_code_substrings() {
        let items = kana_items();
        for query in ["通信", "つうしん", "ツウシン"] {
            assert_eq!(borrowed_values(&filter_items(&items, query)), ["exp_comm"]);
        }
        assert_eq!(borrowed_values(&filter_items(&items, "exp_f")), ["exp_fee"]);
        assert_eq!(
            filter_items(&items, "まったく無い"),
            [] as [&crate::choices::ComboItem; 0]
        );
    }

    #[test]
    fn ac_05_1_prefix_rank_and_stable_ties() {
        let items = kana_items();
        assert_eq!(
            borrowed_values(&filter_items(&items, "こうつう")),
            ["exp_travel"]
        );
        let matches = filter_items(&items, "し");
        assert_eq!(&borrowed_values(&matches)[..2], ["exp_supplies", "exp_fee"]);
        assert!(
            matches
                .iter()
                .position(|item| item.value == "exp_comm")
                .unwrap()
                > 1
        );
        assert_eq!(match_rank(&fold("つう"), &items[0]), Some(0));
        assert_eq!(match_rank(&fold("しん"), &items[0]), Some(1));
        assert_eq!(match_rank(&fold("存在しない"), &items[0]), None);
    }

    #[test]
    fn blank_query_retains_every_item_in_original_order() {
        let items = kana_items();
        assert_eq!(borrowed_values(&filter_items(&items, "")), values(&items));
        assert_eq!(
            borrowed_values(&filter_items(&items, " \t　")),
            values(&items)
        );
    }

    #[test]
    fn exact_name_or_reading_is_required_for_blur_matching() {
        let items = kana_items();
        for text in ["通信費", "つうしんひ"] {
            assert_eq!(borrowed_values(&exact_matches(&items, text)), ["exp_comm"]);
        }
        assert_eq!(
            exact_matches(&items, "通信"),
            [] as [&crate::choices::ComboItem; 0]
        );
        assert_eq!(
            exact_matches(&items, ""),
            [] as [&crate::choices::ComboItem; 0]
        );
    }

    #[test]
    fn sc_02_popup_uses_the_roomier_side_and_stays_in_viewport() {
        let rect = |top, bottom| InputRect {
            top,
            bottom,
            left: 33.0,
            width: 324.0,
        };
        let low = list_placement(rect(599.0, 643.0), 390.0, 844.0);
        assert!(low.above);
        assert_px(low.max_height, 288.0);
        assert_px(low.bottom.unwrap(), 249.0);
        assert!(low.top.is_none());
        let high = list_placement(rect(200.0, 244.0), 390.0, 844.0);
        assert!(!high.above);
        assert_px(high.top.unwrap(), 248.0);
        assert_px(high.max_height, 288.0);
        assert!(high.bottom.is_none());
        let short = list_placement(rect(120.0, 164.0), 640.0, 450.0);
        assert!(!short.above);
        assert_px(short.max_height, 278.0);
        let both = list_placement(rect(250.0, 294.0), 640.0, 450.0);
        assert!(both.above);
        assert_px(both.max_height, 242.0);
        let edge = list_placement(
            InputRect {
                top: 0.0,
                bottom: 44.0,
                left: 300.0,
                width: 200.0,
            },
            390.0,
            844.0,
        );
        assert_px(edge.left, 142.0);
        assert_px(edge.width, 240.0);
        let wide = list_placement(
            InputRect {
                width: 900.0,
                ..rect(200.0, 244.0)
            },
            390.0,
            844.0,
        );
        assert_px(wide.width, 374.0);
        let desktop = list_placement(
            InputRect {
                width: 900.0,
                ..rect(200.0, 244.0)
            },
            1440.0,
            900.0,
        );
        assert_px(desktop.width, 420.0);
    }

    #[test]
    fn ac_05_1_options_match_hiragana_before_conversion() {
        let items = accounts();
        assert_eq!(
            values(&filter_options(&items, "つうしん", OPTION_LIMIT)),
            ["exp_comm"]
        );
        assert_eq!(
            values(&filter_options(&items, "こうつう", OPTION_LIMIT)),
            ["exp_travel"]
        );
    }

    #[test]
    fn ac_05_1_options_match_kanji_katakana_and_codes() {
        let items = accounts();
        for (text, value) in [
            ("光熱", "exp_utility"),
            ("カード", "w03"),
            ("exp_s", "exp_supplies"),
        ] {
            assert_eq!(values(&filter_options(&items, text, OPTION_LIMIT)), [value]);
        }
    }

    #[test]
    fn ac_05_1_options_preserve_caller_order_after_prefix_ranking() {
        let items = accounts();
        assert_eq!(
            values(&filter_options(&items, "ひ", OPTION_LIMIT)),
            ["exp_comm", "exp_travel", "exp_supplies", "exp_utility"]
        );
        let matches = filter_options(&items, "う", OPTION_LIMIT);
        assert_eq!(matches[0].value, "sales");
        assert!(matches.iter().any(|item| item.value == "exp_comm"));
    }

    #[test]
    fn option_limit_is_one_hundred_or_the_requested_limit() {
        let items = (0..300)
            .map(|index| item(&format!("x{index}"), &format!("科目{index}"), ""))
            .collect::<Vec<_>>();
        assert_eq!(filter_options(&items, "", OPTION_LIMIT).len(), 100);
        assert_eq!(filter_options(&items, "", 20).len(), 20);
        assert_eq!(
            filter_options(&items, "", 0),
            [] as [crate::choices::ComboItem; 0]
        );
    }

    #[test]
    fn blur_selects_only_one_exact_name_or_reading() {
        let mut items = accounts();
        for text in ["通信費", "ツウシンヒ"] {
            assert_eq!(pick_on_blur(&items, text).unwrap().value, "exp_comm");
        }
        assert!(pick_on_blur(&items, "通信").is_none());
        items.push(item("x01", "通信費", "つうしんひ"));
        assert!(pick_on_blur(&items, "通信費").is_none());
        assert!(pick_on_blur(&items, "ツウシンヒ").is_none());
    }

    #[test]
    fn sc_02_new_partner_is_appended_only_for_a_nonblank_new_name() {
        let items = partners();
        let list = options_with_new(&items, "通信", &new_label, OPTION_LIMIT);
        assert_eq!(values(&list), ["サンプル通信社", "サンプル通信", "通信"]);
        assert!(is_new(list.last().unwrap()));
        assert_eq!(
            list.last().unwrap().meta["optionLabel"],
            "「通信」を新しい取引先として足す"
        );
        assert!(list[..list.len() - 1].iter().all(|item| !is_new(item)));
        let new = options_with_new(&items, " 山田商店 ", &new_label, OPTION_LIMIT);
        assert_eq!(values(&new), ["山田商店"]);
        assert!(is_new(&new[0]));
        assert!(
            options_with_new(&items, "  ", &new_label, OPTION_LIMIT)
                .iter()
                .all(|item| !is_new(item))
        );
    }

    #[test]
    fn sc_02_same_name_is_first_and_width_or_kana_variants_are_not_new() {
        let items = partners();
        assert_eq!(
            values(&options_with_new(
                &items,
                "サンプル通信",
                &new_label,
                OPTION_LIMIT
            )),
            ["サンプル通信", "サンプル通信社"]
        );
        assert!(
            options_with_new(&items, "ｻﾝﾌﾟﾙ通信", &new_label, OPTION_LIMIT)
                .iter()
                .all(|item| !is_new(item))
        );
        assert!(is_new(
            options_with_new(&items, "さんぷるつうしん", &new_label, OPTION_LIMIT)
                .last()
                .unwrap()
        ));
        assert_eq!(
            borrowed_values(&same_name(&items, "サンプル通信")),
            ["サンプル通信"]
        );
        assert_eq!(same_name(&items, ""), [] as [&crate::choices::ComboItem; 0]);
    }

    #[test]
    fn duplicate_name_beyond_the_visible_limit_prevents_new_partner() {
        let mut items = (0..100)
            .map(|index| item(&format!("x{index}"), &format!("通信{index}"), ""))
            .collect::<Vec<_>>();
        items.push(item("exact", "通信", ""));
        let list = options_with_new(&items, "通信", &new_label, OPTION_LIMIT);
        assert_eq!(list.len(), 100);
        assert!(list.iter().all(|item| !is_new(item)));
        items.pop();
        let list = options_with_new(&items, "通信", &new_label, OPTION_LIMIT);
        assert_eq!(
            list.len(),
            101,
            "the new-name action follows the capped existing candidates"
        );
        assert!(is_new(list.last().unwrap()));
    }

    #[test]
    fn keyword_matches_participate_in_the_same_stable_ranking() {
        let mut items = accounts();
        items[0].meta = json!({"keywords":"でんわ インターネット"});
        assert_eq!(
            values(&filter_options(&items, "ﾃﾞﾝﾜ", OPTION_LIMIT)),
            ["exp_comm"]
        );
        assert_eq!(match_rank(&fold("でん"), &items[0]), Some(0));
        assert_eq!(match_rank(&fold("ねっと"), &items[0]), Some(1));
    }

    #[test]
    fn arrows_start_at_the_first_candidate_and_stop_at_both_ends() {
        assert_eq!(move_active(None, 3, true), Some(0));
        assert_eq!(move_active(None, 3, false), Some(0));
        assert_eq!(move_active(Some(0), 3, true), Some(1));
        assert_eq!(move_active(Some(0), 3, false), Some(0));
        assert_eq!(move_active(Some(2), 3, true), Some(2));
        assert_eq!(move_active(Some(2), 3, false), Some(1));
        assert_eq!(move_active(None, 0, true), None);
    }
}
