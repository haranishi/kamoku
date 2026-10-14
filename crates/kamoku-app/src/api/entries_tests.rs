// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 再送防止の印（`client_id`）の表の試験（コードレビューの指摘 A1・A2、AC-02.7）。

use super::testing::{expense, run, state};
use super::{ClientIds, ClientLimits, entry_create};
use std::time::Duration;

fn entries(state: &crate::state::State) -> usize {
    state.store.ledger().events().len()
}

// A1: 記録は本文を持たない。知らない項目で約0.5MBに膨らませた成功の要求を20回送っても
// （本文を持てば10MB）、表のバイト数は上限（8MB）以内で、1件あたりは小さい（本文の指紋と応答だけ）。
#[test]
fn big_unknown_fields_do_not_grow_the_table() {
    let mut state = state(ClientIds::default());
    let padding = "x".repeat(512 * 1024);
    for i in 0..20 {
        let mut input = expense(1100, &format!("c_big_{i}"));
        input["unknown_padding"] = serde_json::json!(padding);
        assert_eq!(run(&mut state, entry_create, &input).0, 201);
    }
    assert_eq!(state.clients.len(), 20);
    assert!(
        state.clients.bytes() <= ClientLimits::default().bytes,
        "{}",
        state.clients.bytes()
    );
    assert!(
        state.clients.bytes() / 20 < 4096,
        "1件 {}",
        state.clients.bytes() / 20
    );
}

// A2: 表がいっぱいでも、期限内の記録は追い出さない。新しい印の要求は、保存の前に 503 busy で断る。
// 上限＋1件の後に最初の要求を再送しても、最初の結果が返り、仕訳の数は増えない。
#[test]
fn a_full_table_refuses_new_ids_and_still_answers_resends() {
    let limits = ClientLimits {
        count: 3,
        ..ClientLimits::default()
    };
    let mut state = state(ClientIds::with_limits(limits));
    let first = run(&mut state, entry_create, &expense(1100, "c_1"));
    assert_eq!(first.0, 201);
    for id in ["c_2", "c_3"] {
        assert_eq!(run(&mut state, entry_create, &expense(1100, id)).0, 201);
    }
    let over = run(&mut state, entry_create, &expense(1100, "c_4"));
    assert_eq!((over.0, &over.1["error"]["code"]), (503, &"busy".into()));
    assert_eq!(entries(&state), 3, "断った要求は保存しない");
    let again = run(&mut state, entry_create, &expense(1100, "c_1"));
    assert_eq!(again.0, 200);
    assert_eq!(again.1, first.1);
    assert_eq!(entries(&state), 3, "再送で二重に登録しない");
    let changed = run(&mut state, entry_create, &expense(2200, "c_1"));
    assert_eq!(changed.1["error"]["code"], "client_id_reused");
}

// A2: 期限（10分）を過ぎた記録は掃除され、空いた分だけ新しい印を受け付ける。
#[test]
fn expired_records_make_room() {
    let limits = ClientLimits {
        count: 1,
        ttl: Duration::from_millis(50),
        ..ClientLimits::default()
    };
    let mut state = state(ClientIds::with_limits(limits));
    assert_eq!(run(&mut state, entry_create, &expense(1100, "c_a")).0, 201);
    assert_eq!(run(&mut state, entry_create, &expense(1100, "c_b")).0, 503);
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(run(&mut state, entry_create, &expense(1100, "c_b")).0, 201);
    assert_eq!(state.clients.len(), 1);
}

// A1: client_id は64バイトまで。超えたら400で、保存しない。
#[test]
fn client_id_is_at_most_64_bytes() {
    let mut state = state(ClientIds::default());
    assert_eq!(
        run(&mut state, entry_create, &expense(1100, &"a".repeat(64))).0,
        201
    );
    let long = run(&mut state, entry_create, &expense(1100, &"a".repeat(65)));
    assert_eq!(
        (long.0, &long.1["error"]["field"]),
        (400, &"client_id".into())
    );
    assert_eq!(entries(&state), 1);
}

// A1: バイト数の上限に達したら、件数に余りがあっても新しい印は断る。
#[test]
fn the_byte_cap_also_refuses_new_ids() {
    let limits = ClientLimits {
        bytes: 4096,
        ..ClientLimits::default()
    };
    let mut state = state(ClientIds::with_limits(limits));
    let mut created = 0;
    for i in 0..100 {
        match run(&mut state, entry_create, &expense(1100, &format!("c_{i}"))).0 {
            201 => created += 1,
            503 => break,
            other => panic!("想定外の状態 {other}"),
        }
    }
    assert!((1..100).contains(&created), "{created}");
    assert_eq!(entries(&state), created);
}
