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
    // GOLDEN-BEGIN fixed_event_hashes
    let expected = [
        "da1a12a8de1711fc809f39fe6a43ee6575dc806dd36d4f9baeafb8f2719f52df",
        "70c56e42f59ba121307722e8809d38bf0aec1496b5627e78975716693b588b8d",
        "5b338c6959ece6bf4a02f771046d6a2ba42b263a39f26ca88a5ab07f8a466794",
        "3ef7a0df71a3a48267fc3c26599758f1b1ebf96a8b1fab9e8f91b36c9b53a93a",
    ];
    // GOLDEN-END
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

fn with_meta(entry: Entry, pairs: &[(&str, &str)]) -> Entry {
    pairs.iter().fold(entry, |entry, (key, value)| {
        entry.with_meta(key, value).unwrap()
    })
}
fn expense_lines() -> Vec<Line> {
    vec![
        line("exp_comm", Side::Debit, 7700, Some(("P10", 700)), ""),
        line("owner_draw", Side::Debit, 3300, None, "家事按分 30%"),
        line("w02", Side::Credit, 11_000, None, ""),
    ]
}
/// 付随情報つきのイベントの固定値。AC-02.2（保存した仕訳の中身をハッシュで守る土台）。
/// 値は Rust の出力ではなく、`tools/verify-ledger-golden.py` が符号化の仕様だけから計算した。
#[test]
fn meta_event_hashes() {
    let mut ledger = Ledger::new();
    let id1 = EntryId::new("E-1001").unwrap();
    let id2 = EntryId::new("E-1002").unwrap();
    let base = |id: &EntryId| {
        Entry::new(
            id.clone(),
            date!(2026 - 10 - 05),
            "10月分 通信費",
            expense_lines(),
        )
        .unwrap()
    };
    // 1組。
    ledger
        .post(with_meta(base(&id1), &[("kind", "expense")]), 1_791_158_400)
        .unwrap();
    // 3組を、鍵の降順に入れる。符号化は鍵の昇順なので、挿入の順は結果に出ない。
    let sale = Entry::new(
        id2,
        date!(2026 - 10 - 06),
        "売上 デモ",
        vec![
            line("w01", Side::Debit, 55_000, None, ""),
            line("sales", Side::Credit, 55_000, Some(("S10", 5000)), ""),
        ],
    )
    .unwrap();
    let sale = with_meta(
        sale,
        &[
            ("source", "import"),
            ("partner", "Sample Shop"),
            ("kind", "income"),
        ],
    );
    ledger.post(sale, 1_791_244_800).unwrap();
    // 日本語の値。訂正のイベントにも付く。
    let revised = with_meta(
        base(&id1),
        &[("partner", "サンプル通信（デモ）"), ("kind", "expense")],
    );
    ledger
        .revise(revised, "取引先の入力漏れ", 1_791_331_200)
        .unwrap();
    // GOLDEN-BEGIN meta_event_hashes
    let expected = [
        "1ecc295f3f2f908bc5b3f3fb15c740e24130144df3f28d7edadbbe1af1a8b104",
        "b860692e7a88415d7465f759878a2a410fe733d0ce5c44f15d590228a2e5e42b",
        "10129a6cd0b4280013974793442b63380c336bba6805522bb969cae2700b6f8b",
    ];
    // GOLDEN-END
    assert_eq!(ledger.events().len(), expected.len());
    for (event, hash) in ledger.events().iter().zip(expected) {
        assert_eq!(event.hash.to_hex(), hash);
    }
    let restored = Ledger::from_events(ledger.events().to_vec()).unwrap();
    assert_eq!(restored.head_hash(), ledger.head_hash());
}
