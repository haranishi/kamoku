// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phase 0 の受け入れ試験。

use kamoku_ledger::{AccountCode, Entry, EntryId, Ledger, Line, Side, Yen};
use time::macros::date;

fn line(account: &str, side: Side, amount: i64, tax: Option<(&str, i64)>, memo: &str) -> Line {
    let line =
        Line::new(AccountCode::new(account).unwrap(), side, Yen::new(amount)).with_memo(memo);
    match tax {
        Some((code, amount)) => line.with_tax(code, Yen::new(amount)),
        None => line,
    }
}
#[test]
fn fixed_event_hashes() {
    let mut ledger = Ledger::new();
    let id1 = EntryId::new("E-0001").unwrap();
    let id2 = EntryId::new("E-0002").unwrap();
    ledger
        .post(
            Entry::new(
                id1.clone(),
                date!(2026 - 01 - 05),
                "売上入金 テスト",
                vec![
                    line("101", Side::Debit, 110_000, None, ""),
                    line(
                        "401",
                        Side::Credit,
                        110_000,
                        Some(("TAX10", 10_000)),
                        "テスト売上",
                    ),
                ],
            )
            .unwrap(),
            1_767_571_200,
        )
        .unwrap();
    ledger
        .revise(
            Entry::new(
                id1,
                date!(2026 - 01 - 05),
                "売上入金 テスト(訂正)",
                vec![
                    line("101", Side::Debit, 108_000, None, ""),
                    line(
                        "401",
                        Side::Credit,
                        108_000,
                        Some(("TAX10", 9818)),
                        "テスト売上",
                    ),
                ],
            )
            .unwrap(),
            "金額訂正",
            1_767_657_600,
        )
        .unwrap();
    ledger
        .post(
            Entry::new(
                id2.clone(),
                date!(2026 - 01 - 10),
                "消耗品",
                vec![
                    line("510", Side::Debit, 3300, Some(("TAX10", 300)), ""),
                    line("101", Side::Credit, 3300, None, ""),
                ],
            )
            .unwrap(),
            1_768_003_200,
        )
        .unwrap();
    ledger.void(&id2, "重複入力", 1_768_089_600).unwrap();
    // 初回の実装実行で出力した値を固定。検証時には再生成しない。
    let expected = [
        "da1a12a8de1711fc809f39fe6a43ee6575dc806dd36d4f9baeafb8f2719f52df",
        "70c56e42f59ba121307722e8809d38bf0aec1496b5627e78975716693b588b8d",
        "5b338c6959ece6bf4a02f771046d6a2ba42b263a39f26ca88a5ab07f8a466794",
        "3ef7a0df71a3a48267fc3c26599758f1b1ebf96a8b1fab9e8f91b36c9b53a93a",
    ];
    assert_eq!(ledger.events().len(), expected.len());
    for (event, hash) in ledger.events().iter().zip(expected) {
        assert_eq!(event.hash.to_hex(), hash);
    }
    assert_eq!(
        ledger.head_hash().to_hex(),
        "3ef7a0df71a3a48267fc3c26599758f1b1ebf96a8b1fab9e8f91b36c9b53a93a"
    );
    ledger.verify().unwrap();
}
