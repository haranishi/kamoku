// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 下書きの確定の守り。渡された仕訳が、下書きから組み立て直した仕訳と違えば断る。
//! 日付・摘要・明細・付随情報の4つを、1つずつ別の試験にする（どれか1つの検査を外しても
//! 赤くなるように）。変異試験で、明細の照合を外しても試験が赤くならない穴が見つかったため。

mod common;
use common::*;
use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use kamoku_store::*;
use std::collections::HashSet;
use time::macros::date;

/// 科目を付けた下書きを1件作り、(Store, 下書きID, 正しい仕訳) を返す。
fn prepared() -> (Store, DraftId, Entry) {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let w = wallet(&mut s);
    s.import_commit(batch(&w), vec![draft(2, 1)], HashSet::new())
        .unwrap();
    let id = s.drafts(DraftFilter::default()).unwrap().items[0].id;
    s.update_draft(
        id,
        DraftPatch {
            account: Some(Some("exp_comm".into())),
            partner: Some("サンプル取引先".into()),
            ..DraftPatch::default()
        },
        2,
    )
    .unwrap();
    let good = import_entry(&s, id, "g1");
    (s, id, good)
}

/// 正しい仕訳の日付・摘要・明細・付随情報のうち、指定したものだけを入れ替えた仕訳を作る。
fn variant(
    good: &Entry,
    date: Option<time::Date>,
    description: Option<&str>,
    lines: Option<Vec<Line>>,
    keep_meta: bool,
) -> Entry {
    let mut e = Entry::new(
        good.id().clone(),
        date.unwrap_or_else(|| good.date()),
        description.unwrap_or_else(|| good.description()),
        lines.unwrap_or_else(|| good.lines().to_vec()),
    )
    .unwrap();
    if keep_meta {
        for (k, v) in good.meta() {
            e = e.with_meta(k, v).unwrap();
        }
    }
    e
}

fn rejected(s: &mut Store, id: DraftId, e: Entry) {
    let results = s.confirm_drafts(vec![(id, e)], 3).unwrap();
    assert!(
        matches!(results[0].result, Err(StoreError::Invalid(_))),
        "照合で断られるはず: {:?}",
        results[0].result
    );
    // 断られた下書きは未確認のまま、台帳には何も入らない。
    assert_eq!(s.draft_counts().unwrap().draft, 1);
    assert_eq!(s.draft_counts().unwrap().confirmed, 0);
    assert_eq!(s.ledger().current().len(), 0);
}

// AC-05.5: 正しい仕訳は確定できる（比較の基準になる試験）。
#[test]
fn matching_entry_is_confirmed() {
    let (mut s, id, good) = prepared();
    let results = s.confirm_drafts(vec![(id, good)], 3).unwrap();
    assert!(results[0].result.is_ok());
    assert_eq!(s.draft_counts().unwrap().confirmed, 1);
}

// 明細だけが違う（科目が違う）仕訳は、日付と摘要が同じでも断る。
#[test]
fn different_lines_are_rejected() {
    let (mut s, id, good) = prepared();
    let lines = vec![
        Line::new(
            AccountCode::new("exp_supplies").unwrap(),
            Side::Debit,
            Yen::new(110),
        ),
        good.lines()[1].clone(),
    ];
    rejected(&mut s, id, variant(&good, None, None, Some(lines), true));
}

// 明細の金額だけが違う仕訳も断る（貸借は合っている）。
#[test]
fn different_amount_is_rejected() {
    let (mut s, id, good) = prepared();
    let lines = vec![
        Line::new(
            AccountCode::new("exp_comm").unwrap(),
            Side::Debit,
            Yen::new(111),
        ),
        Line::new(good.lines()[1].account.clone(), Side::Credit, Yen::new(111)),
    ];
    rejected(&mut s, id, variant(&good, None, None, Some(lines), true));
}

// 日付だけが違う仕訳は断る。
#[test]
fn different_date_is_rejected() {
    let (mut s, id, good) = prepared();
    rejected(
        &mut s,
        id,
        variant(&good, Some(date!(2026 - 10 - 02)), None, None, true),
    );
}

// 摘要だけが違う仕訳は断る。
#[test]
fn different_description_is_rejected() {
    let (mut s, id, good) = prepared();
    rejected(
        &mut s,
        id,
        variant(&good, None, Some("別の摘要"), None, true),
    );
}

// 付随情報（取引先など）が欠けた仕訳は断る。
#[test]
fn missing_meta_is_rejected() {
    let (mut s, id, good) = prepared();
    assert!(good.meta().contains_key("partner"));
    rejected(&mut s, id, variant(&good, None, None, None, false));
}

// 仕訳の識別子が違っても、中身が合っていれば確定できる（識別子は呼び出し側が決める）。
#[test]
fn entry_id_is_chosen_by_caller() {
    let (mut s, id, good) = prepared();
    let renamed = Entry::new(
        EntryId::new("another-id").unwrap(),
        good.date(),
        good.description(),
        good.lines().to_vec(),
    )
    .unwrap();
    let mut e = renamed;
    for (k, v) in good.meta() {
        e = e.with_meta(k, v).unwrap();
    }
    let results = s.confirm_drafts(vec![(id, e)], 3).unwrap();
    assert!(results[0].result.is_ok());
    assert_eq!(s.draft_counts().unwrap().confirmed, 1);
}
