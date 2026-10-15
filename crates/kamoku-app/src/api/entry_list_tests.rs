// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 現在状態とページ選択を、全履歴の再構築・全件ソートの参照結果と照合する。

use super::*;
use crate::api::ClientIds;
use crate::api::testing::state as test_state;
use kamoku_ledger::{AccountCode, Entry, EntryId, Event, EventKind, Line, Side, Yen};
use kamoku_store::{OpenOptions, Store};
use std::collections::BTreeMap;
use time::Month;

fn entry(number: usize, revised: bool) -> Entry {
    let amount = i64::try_from(number).unwrap() * 10 + if revised { 2 } else { 1 };
    let month = if number.is_multiple_of(7) {
        Month::September
    } else {
        Month::October
    };
    let day = u8::try_from(number % 5 + 1 + usize::from(revised)).unwrap();
    let kind = ["expense", "income", "transfer", "journal"][number % 4];
    let wallet = if number.is_multiple_of(3) {
        "w02"
    } else {
        "w01"
    };
    let account = if number.is_multiple_of(2) {
        "exp_comm"
    } else {
        "exp_books"
    };
    let (debit, credit) = match kind {
        "income" => (wallet, "sales"),
        "transfer" => ("w03", wallet),
        _ => (account, wallet),
    };
    let mut debit = Line::new(
        AccountCode::new(debit).unwrap(),
        Side::Debit,
        Yen::new(amount),
    );
    debit.memo = if number.is_multiple_of(2) {
        "memo-even"
    } else {
        "memo-odd"
    }
    .into();
    Entry::new(
        EntryId::new(&format!("E{number}")).unwrap(),
        Date::from_calendar_date(2026, month, day).unwrap(),
        &format!("{}-{number}", if revised { "revised" } else { "original" }),
        vec![
            debit,
            Line::new(
                AccountCode::new(credit).unwrap(),
                Side::Credit,
                Yen::new(amount),
            ),
        ],
    )
    .unwrap()
    .with_meta("kind", kind)
    .unwrap()
    .with_meta(
        "source",
        ["manual", "import", "template", "demo"][number % 4],
    )
    .unwrap()
    .with_meta(
        "partner",
        if number.is_multiple_of(2) {
            "partner-even"
        } else {
            "partner-odd"
        },
    )
    .unwrap()
}

fn populated() -> State {
    let mut state = test_state(ClientIds::default());
    for number in 1..=523 {
        let now = i64::try_from(number).unwrap();
        state.store.post(entry(number, false), now, None).unwrap();
        if number.is_multiple_of(3) {
            state
                .store
                .revise(entry(number - 1, true), 1, "訂正", now, None)
                .unwrap();
        }
        if number.is_multiple_of(5) {
            let id = EntryId::new(&format!("E{}", number - 2)).unwrap();
            let revision = state.store.ledger().history(&id).last().unwrap().revision;
            state.store.void(&id, revision, "取消", now).unwrap();
        }
    }
    state
}

fn list(state: &mut State, raw: &str) -> (u16, Value) {
    let query = Query::parse(raw).unwrap();
    let call = Call {
        params: &[],
        query: &query,
        body: &[],
        now: 1,
        today: time::macros::date!(2026 - 10 - 14),
    };
    match entries_list(state, &call) {
        Ok(Reply::Json(status, value)) => (status, value),
        Ok(Reply::File(_)) => panic!("一覧はJSONで返す"),
        Err(error) => (error.status, error.body()),
    }
}

// 入力の解析・既存のフィルター・JSON表現を共有するが、状態の復元と並べ替えは独立して行う。
fn reference(state: &State, raw: &str) -> Result<Value, ApiError> {
    let query = Query::parse(raw)?;
    let filter = Filter::read(&query)?;
    let (limit, offset) = query.page()?;
    let mut histories = BTreeMap::<&EntryId, Vec<&Event>>::new();
    for event in state.store.ledger().events() {
        histories.entry(&event.entry_id).or_default().push(event);
    }
    let mut rows = Vec::new();
    for (posted, no) in state
        .store
        .ledger()
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Posted)
        .zip(1u64..)
    {
        let history = histories.get(&posted.entry_id).unwrap();
        let current = Current {
            no,
            latest: history.last().unwrap(),
            entry: history
                .iter()
                .rev()
                .find_map(|event| event.entry.as_ref())
                .unwrap(),
        };
        if filter.matches(&current, state.store.chart()) {
            rows.push(current);
        }
    }
    rows.sort_by_key(|row| Reverse((row.entry.date(), row.no)));
    let mut sum = 0i64;
    for row in &rows {
        sum = sum
            .checked_add(row.entry.total().get())
            .ok_or_else(sum_error)?;
    }
    if sum > MAX_REPORT_AMOUNT {
        return Err(sum_error());
    }
    let items: Vec<_> = rows
        .iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap())
        .map(|row| entry_json(row, state.store.chart()))
        .collect();
    Ok(json!({ "items": items, "total": rows.len(), "sum_amount": sum }))
}

fn sum_error() -> ApiError {
    ApiError::new(
        422,
        "amount_range",
        "合計が大きすぎて、正確に表示できません。期間を短くしてください。",
    )
}

fn assert_reference(state: &mut State, raw: &str) {
    let expected =
        reference(state, raw).map_or_else(|error| (error.status, error.body()), |body| (200, body));
    assert_eq!(list(state, raw), expected, "{raw}");
}

#[test]
fn pages_match_full_history_and_sort_for_varied_filters_and_boundaries() {
    let mut state = populated();
    for filter in [
        "",
        "state=all",
        "state=voided",
        "from=2026-10-02&to=2026-10-04",
        "account=exp_comm",
        "wallet=w02",
        "kind=income",
        "kind=opening",
        "source=import",
        "q=partner-even",
        "q=memo-odd",
        "q=通信費",
        "q=1%2C001",
        "q=not-present",
        "q=%20%20",
        "min=1000&max=3000",
        "state=all&source=template&wallet=w01",
    ] {
        for (limit, offset) in [
            (1, 0),
            (7, 1),
            (7, 13),
            (500, 0),
            (500, 500),
            (7, 522),
            (7, 523),
            (7, i64::MAX),
        ] {
            assert_reference(
                &mut state,
                &format!("{filter}&limit={limit}&offset={offset}"),
            );
        }
    }
}

#[test]
fn reconstructed_store_keeps_numbers_revisions_and_voided_last_entries() {
    let mut state = populated();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("synthetic.sqlite");
    let before = list(&mut state, "state=all&limit=500&offset=20");
    state.store.backup_to(&path).unwrap();
    state.store = Store::open(
        &path,
        OpenOptions {
            create_if_missing: false,
        },
    )
    .unwrap()
    .0;
    assert_eq!(list(&mut state, "state=all&limit=500&offset=20"), before);
    assert_reference(&mut state, "state=voided&limit=7&offset=5");
}

#[test]
fn empty_results_and_invalid_query_errors_keep_their_previous_shape() {
    let mut state = test_state(ClientIds::default());
    for query in [
        "",
        "state=all&limit=500&offset=9223372036854775807",
        "limit=0",
        "limit=501",
        "offset=-1",
        "offset=18446744073709551615",
        "state=unknown",
        "source=unknown",
        "kind=unknown",
        "from=2026-02-30",
        "min=not-a-number",
    ] {
        assert_reference(&mut state, query);
    }
}

#[test]
fn page_selection_handles_the_largest_offset_and_partial_final_page() {
    let state = populated();
    for (limit, offset) in [
        (1, u64::MAX),
        (500, u64::MAX),
        (500, 0),
        (500, 500),
        (1, 522),
        (1, 523),
    ] {
        let mut rows: Vec<_> = all_current(&state.store).collect();
        let mut expected: Vec<_> = rows
            .iter()
            .map(|row| (Reverse((row.entry.date(), row.no)), row.no))
            .collect();
        expected.sort_unstable();
        let expected: Vec<_> = expected
            .into_iter()
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .take(usize::try_from(limit).unwrap())
            .map(|(_, no)| no)
            .collect();
        assert_eq!(
            page_rows(&mut rows, limit, offset)
                .iter()
                .map(|row| row.no)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn filtered_sum_is_checked_even_when_the_page_is_empty() {
    let mut state = test_state(ClientIds::default());
    for number in 1..=901 {
        let entry = Entry::new(
            EntryId::new(&format!("L{number}")).unwrap(),
            time::macros::date!(2026 - 10 - 01),
            "large",
            vec![
                Line::new(
                    AccountCode::new("exp_comm").unwrap(),
                    Side::Debit,
                    Yen::new(kamoku_books::MAX_AMOUNT),
                ),
                Line::new(
                    AccountCode::new("w01").unwrap(),
                    Side::Credit,
                    Yen::new(kamoku_books::MAX_AMOUNT),
                ),
            ],
        )
        .unwrap();
        state.store.post(entry, 1, None).unwrap();
    }
    for query in [
        "limit=1",
        "limit=1&offset=10000",
        "limit=500&offset=9223372036854775807",
    ] {
        assert_reference(&mut state, query);
        assert_eq!(list(&mut state, query), (422, sum_error().body()));
    }
    assert_reference(&mut state, "q=not-present&limit=1&offset=10000");
}
