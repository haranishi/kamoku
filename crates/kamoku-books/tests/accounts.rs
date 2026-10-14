// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 標準の科目、科目の集合の検査、`docs/accounts.md` とコードのずれの試験。

// JSONの例は、通信の約束（docs/spec/）と同じ書き方で比べたいので、数字に区切りを入れない。
#![allow(clippy::unreadable_literal)]
mod common;
use common::{accounts, wallet};
use kamoku_books::{
    BLANK_FORM_LINE, Chart, ChartError, Kind, REQUIRED_ACCOUNTS, TaxClass, TaxCode, WalletType,
    standard_accounts, standard_accounts_markdown,
};
use kamoku_ledger::{AccountCode, Side};

// docs/accounts.md は、コードから作った文書と1文字も違わない。違えば、例の手順で作り直す。
#[test]
fn accounts_doc_matches_code() {
    let on_disk = include_str!("../../../docs/accounts.md");
    assert_eq!(
        on_disk,
        standard_accounts_markdown(),
        "docs/accounts.md が古い。`cargo run -p kamoku-books --example accounts_doc > docs/accounts.md` で作り直す"
    );
    assert!(on_disk.lines().count() <= 100);
}

#[test]
fn standard_accounts_shape() {
    let list = standard_accounts();
    assert_eq!(list.len(), 53);
    let count = |k: Kind| list.iter().filter(|a| a.kind == k).count();
    assert_eq!(
        Kind::ALL.map(count),
        [17, 8, 1, 3, 24],
        "資産・負債・資本・収益・費用の数"
    );
    // 並び順は分類ごとに100・200・300・400・500から10刻み。
    for kind in Kind::ALL {
        let base = match kind {
            Kind::Asset => 100,
            Kind::Liability => 200,
            Kind::Equity => 300,
            Kind::Revenue => 400,
            Kind::Expense => 500,
        };
        let sorts: Vec<u32> = list
            .iter()
            .filter(|a| a.kind == kind)
            .map(|a| a.sort)
            .collect();
        let expected: Vec<u32> = (0u32..).map(|i| base + i * 10).take(sorts.len()).collect();
        assert_eq!(sorts, expected);
    }
    // AC-03.3: 既定の割合はすべて100（利用者が科目ごとに変える）。読みはひらがな。
    for a in &list {
        assert_eq!(a.default_pct, 100);
        assert!(a.system && a.parent.is_none() && a.wallet.is_none());
        assert!(
            !a.reading.is_empty()
                && a.reading
                    .chars()
                    .all(|c| ('ぁ'..='ゖ').contains(&c) || c == 'ー'),
            "{}",
            a.reading
        );
        assert!(a.form_line.is_some());
    }
    let find = |code: &str| list.iter().find(|a| a.code.as_str() == code).unwrap();
    assert_eq!(find("exp_comm").default_tax, Some(TaxCode::P10));
    assert_eq!(find("exp_insurance").default_tax, Some(TaxCode::NT));
    assert_eq!(find("exp_interest").default_tax, Some(TaxCode::NT));
    assert_eq!(find("sales").default_tax, Some(TaxCode::S10));
    for code in [
        "exp_tax",
        "exp_depr",
        "exp_salary",
        "exp_welfare",
        "exp_dues",
        "exp_rent",
        "exp_books",
        "exp_travel",
    ] {
        assert_eq!(find(code).default_tax, None, "{code}");
    }
    for code in [
        "exp_meeting",
        "exp_books",
        "exp_dues",
        "exp_vehicle",
        "exp_fee",
    ] {
        assert_eq!(
            find(code).form_line.as_deref(),
            Some(BLANK_FORM_LINE),
            "{code}"
        );
    }
    assert_eq!(find("exp_outsource").form_line.as_deref(), Some("外注工賃"));
    assert!(!find("land").enabled && find("cash").enabled);
    assert!(
        find("owner_draw").is_protected()
            && find("capital").is_protected()
            && !find("cash").is_protected()
    );
    assert!(Chart::new(list).is_ok());
}

// 科目の表現は、通信の約束（docs/spec/api-masters.md）の例と同じ項目名と値の形になる。
#[test]
fn account_json_matches_spec() {
    let list = accounts();
    let comm = list.iter().find(|a| a.code.as_str() == "exp_comm").unwrap();
    assert_eq!(
        serde_json::to_value(comm).unwrap(),
        serde_json::json!({ "code": "exp_comm", "name": "通信費", "reading": "つうしんひ", "kind": "expense",
            "normal_side": "debit", "parent": null, "wallet": null, "default_tax": "P10",
            "default_pct": 100, "form_line": "通信費", "enabled": true, "system": true, "sort": 550 })
    );
    let card = list.iter().find(|a| a.code.as_str() == "w02").unwrap();
    assert_eq!(
        serde_json::to_value(card).unwrap(),
        serde_json::json!({ "code": "w02", "name": "サンプルカード", "reading": "", "kind": "liability",
            "normal_side": "credit", "parent": "accrued", "wallet": { "type": "card", "institution": "" },
            "default_tax": null, "default_pct": 100, "form_line": null, "enabled": true, "system": false, "sort": 1002 })
    );
}

#[test]
fn kinds_tax_codes_and_wallet_types() {
    assert_eq!(Kind::Asset.normal_side(), Side::Debit);
    assert_eq!(Kind::Expense.normal_side(), Side::Debit);
    for k in [Kind::Liability, Kind::Equity, Kind::Revenue] {
        assert_eq!(k.normal_side(), Side::Credit);
    }
    for k in Kind::ALL {
        assert_eq!(Kind::parse(k.as_str()), Some(k));
        assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
    }
    for code in TaxCode::ALL {
        assert_eq!(TaxCode::parse(code.as_str()), Some(code));
        assert_eq!(serde_json::to_value(code).unwrap(), code.as_str());
        assert_eq!(
            serde_json::from_value::<TaxCode>(serde_json::json!(code.as_str())).unwrap(),
            code
        );
        assert_ne!(code.label(), "");
    }
    assert_eq!(TaxCode::parse("p10"), None);
    assert!(serde_json::from_value::<TaxCode>(serde_json::json!("X")).is_err());
    assert_eq!(TaxCode::P10X.class(), TaxClass::TransitionalPurchase);
    assert_eq!(TaxCode::NT.rate(), None);
    assert_eq!(WalletType::Card.kind(), Kind::Liability);
    assert_eq!(WalletType::Bank.default_parent(), "dep_other");
    assert_eq!(WalletType::Ewallet.default_parent(), "cash");
}

fn code(s: &str) -> AccountCode {
    AccountCode::new(s).unwrap()
}

// 科目の集合の検査: 重複、親の存在と分類、循環、必須の科目、口座の型、既定の割合。
#[test]
fn chart_validation() {
    let with = |change: &dyn Fn(&mut Vec<kamoku_books::Account>)| {
        let mut list = accounts();
        change(&mut list);
        Chart::new(list)
    };
    assert_eq!(
        with(&|l| l.push(wallet("w01", "二重", WalletType::Cash, 1))).unwrap_err(),
        ChartError::DuplicateCode("w01".to_owned())
    );
    assert!(matches!(
        with(&|l| l
            .iter_mut()
            .find(|a| a.code.as_str() == "w01")
            .unwrap()
            .parent = Some(code("zzz")))
        .unwrap_err(),
        ChartError::MissingParent { .. }
    ));
    assert!(matches!(
        with(&|l| l
            .iter_mut()
            .find(|a| a.code.as_str() == "w01")
            .unwrap()
            .parent = Some(code("accrued")))
        .unwrap_err(),
        ChartError::ParentKindMismatch { .. }
    ));
    assert!(matches!(
        with(&|l| {
            l.iter_mut()
                .find(|a| a.code.as_str() == "cash")
                .unwrap()
                .parent = Some(code("w01"));
        })
        .unwrap_err(),
        ChartError::Cycle(_)
    ));
    for (required, _) in REQUIRED_ACCOUNTS {
        // 子の口座も一緒に外す（外さないと、先に「親が無い」で断られる）。
        let drop = |a: &kamoku_books::Account| {
            a.code.as_str() == required || a.parent.as_ref().is_some_and(|p| p.as_str() == required)
        };
        assert_eq!(
            with(&|l| l.retain(|a| !drop(a))).unwrap_err(),
            ChartError::MissingRequired(required.to_owned())
        );
    }
    assert_eq!(
        with(&|l| l
            .iter_mut()
            .find(|a| a.code.as_str() == "sales")
            .unwrap()
            .kind = Kind::Expense)
        .unwrap_err(),
        ChartError::RequiredKindMismatch("sales".to_owned())
    );
    assert_eq!(
        with(&|l| l
            .iter_mut()
            .find(|a| a.code.as_str() == "exp_comm")
            .unwrap()
            .default_pct = 101)
        .unwrap_err(),
        ChartError::BadDefaultPct("exp_comm".to_owned())
    );
    assert!(matches!(
        with(&|l| {
            let w = l.iter_mut().find(|a| a.code.as_str() == "w02").unwrap();
            w.kind = Kind::Asset;
            w.parent = Some(code("cash"));
        })
        .unwrap_err(),
        ChartError::WalletKindMismatch(_)
    ));
    let chart = Chart::new(accounts()).unwrap();
    let children: Vec<&str> = chart.children("cash").map(|a| a.code.as_str()).collect();
    assert_eq!(children, vec!["w01", "w04"]);
    assert_eq!(chart.name("w03"), Some("デモ銀行"));
    assert_eq!(chart.kind("w02"), Some(Kind::Liability));
    assert_eq!(chart.wallets().count(), 4);
    assert_eq!(chart.len(), 57);
}
