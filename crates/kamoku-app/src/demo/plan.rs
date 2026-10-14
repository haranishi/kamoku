// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 毎月の取引の予定（売上1〜2件、経費10〜25件、口座の移動、事業主貸・借）と、訂正・取消。

use super::{Demo, DemoError, PARTNERS, day, fail, input};
use kamoku_books::{SimpleInput, SimpleKind};
use kamoku_ledger::EntryId;

#[derive(Clone, Copy)]
enum Pay {
    Cash,
    Bank,
    Card,
}

struct Item {
    account: &'static str,
    text: &'static str,
    partner: &'static str,
    pay: Pay,
    lo: i64,
    hi: i64,
    step: i64,
}

// 毎月、ここから選んで入れる経費（課税仕入10%）。ガソリンは車両費の既定の割合（30%）で按分される。
#[rustfmt::skip]
const CATALOG: [Item; 9] = [
    Item { account: "exp_travel", text: "電車代", partner: "デモ交通", pay: Pay::Cash, lo: 160, hi: 1_200, step: 10 },
    Item { account: "exp_travel", text: "バス代", partner: "デモ交通", pay: Pay::Cash, lo: 200, hi: 600, step: 10 },
    Item { account: "exp_supplies", text: "文房具", partner: "サンプル文具", pay: Pay::Cash, lo: 200, hi: 3_000, step: 10 },
    Item { account: "exp_supplies", text: "プリンターのインク", partner: "サンプル文具", pay: Pay::Card, lo: 2_000, hi: 6_000, step: 100 },
    Item { account: "exp_meeting", text: "打ち合わせのコーヒー", partner: "", pay: Pay::Cash, lo: 600, hi: 2_400, step: 10 },
    Item { account: "exp_vehicle", text: "ガソリン", partner: "", pay: Pay::Card, lo: 3_000, hi: 8_000, step: 1 },
    Item { account: "exp_books", text: "技術書", partner: "", pay: Pay::Card, lo: 2_000, hi: 5_000, step: 10 },
    Item { account: "exp_fee", text: "振込手数料", partner: "", pay: Pay::Bank, lo: 145, hi: 440, step: 5 },
    Item { account: "exp_outsource", text: "デザインの外注", partner: "", pay: Pay::Bank, lo: 33_000, hi: 110_000, step: 1_100 },
];

// 訂正と取消に使う仕訳（3月の文房具、4月の回線、5月に二重に入れた携帯電話の料金）。
#[derive(Default)]
pub(super) struct Marks {
    stationery: Option<(EntryId, SimpleInput)>,
    internet: Option<(EntryId, SimpleInput)>,
    doubled: Option<EntryId>,
}

fn costed(mut p: SimpleInput, text: &str, partner: &str) -> SimpleInput {
    p.tax_code = Some("P10".to_owned());
    partner.clone_into(&mut p.partner);
    text.clone_into(&mut p.description);
    p
}

/// 1月〜9月の取引を、月ごとに日付の順で入れます。
pub(super) fn months(demo: &mut Demo<'_>) -> Result<Marks, DemoError> {
    let mut marks = Marks::default();
    let mut card_spent = 0i64;
    for month in 1u8..=9 {
        let mut plans = sales(demo, month)?;
        plans.extend(fixed_costs(demo, month)?);
        plans.extend(random_costs(demo, month)?);
        plans.extend(movements(demo, month, card_spent)?);
        card_spent = plans
            .iter()
            .filter(|p| p.kind == SimpleKind::Expense && p.wallet == demo.card)
            .map(|p| p.amount)
            .sum();
        plans.sort_by_key(|p| p.date);
        for plan in plans {
            let id = demo.post(&plan)?;
            if month == 3 && plan.description == "文房具" && marks.stationery.is_none() {
                marks.stationery = Some((id, plan));
            } else if month == 4 && plan.description == "インターネット回線" {
                marks.internet = Some((id, plan));
            } else if month == 5
                && plan.description == "携帯電話の利用料"
                && marks.doubled.is_none()
            {
                marks.doubled = Some(demo.post(&plan)?);
            }
        }
    }
    Ok(marks)
}

fn sales(demo: &mut Demo<'_>, month: u8) -> Result<Vec<SimpleInput>, DemoError> {
    let mut out = Vec::new();
    for k in 0..=demo.rng.below(2) {
        let date = day(2026, month, if k == 0 { 25 } else { 10 })?;
        let amount = demo.rng.amount(220_000, 550_000, 1_100);
        let mut sale = input(SimpleKind::Income, date, amount, "sales", &demo.bank);
        sale.tax_code = Some("S10".to_owned());
        let client = usize::from(month % 2) ^ usize::try_from(k).unwrap_or_default();
        PARTNERS
            .get(client)
            .copied()
            .unwrap_or_default()
            .clone_into(&mut sale.partner);
        sale.description = if k == 0 {
            format!("{month}月分 業務委託料")
        } else {
            "デザイン制作費".to_owned()
        };
        out.push(sale);
    }
    Ok(out)
}

fn fixed_costs(demo: &mut Demo<'_>, month: u8) -> Result<Vec<SimpleInput>, DemoError> {
    let power = demo.rng.amount(6_000, 12_000, 10);
    let rows = [
        (
            10,
            7_700,
            "exp_comm",
            "携帯電話の利用料",
            "サンプル通信",
            demo.card.clone(),
            70,
        ),
        (
            27,
            5_500,
            "exp_comm",
            "インターネット回線",
            "サンプル通信",
            demo.bank.clone(),
            50,
        ),
        (
            20,
            power,
            "exp_utility",
            "電気代（自宅兼事務所）",
            "",
            demo.bank.clone(),
            30,
        ),
    ];
    let mut out = Vec::new();
    for (d, amount, account, text, partner, wallet, pct) in rows {
        let date = day(2026, month, d)?;
        let mut p = costed(
            input(SimpleKind::Expense, date, amount, account, &wallet),
            text,
            partner,
        );
        p.business_pct = Some(pct);
        out.push(p);
    }
    Ok(out)
}

fn random_costs(demo: &mut Demo<'_>, month: u8) -> Result<Vec<SimpleInput>, DemoError> {
    let mut out = Vec::new();
    for _ in 0..7 + demo.rng.below(16) {
        let item = CATALOG
            .get(usize::try_from(demo.rng.below(9)).unwrap_or_default())
            .ok_or_else(|| DemoError("経費の表".to_owned()))?;
        let date = day(
            2026,
            month,
            u8::try_from(1 + demo.rng.below(28)).unwrap_or(1),
        )?;
        let amount = demo.rng.amount(item.lo, item.hi, item.step);
        let wallet = match item.pay {
            Pay::Cash => demo.cash.clone(),
            Pay::Bank => demo.bank.clone(),
            Pay::Card => demo.card.clone(),
        };
        let p = input(SimpleKind::Expense, date, amount, item.account, &wallet);
        out.push(costed(p, item.text, item.partner));
    }
    Ok(out)
}

// 口座の移動（現金の引き出し、前の月のカードの引き落とし）と、事業主貸・事業主借。
fn movements(demo: &Demo<'_>, month: u8, card_spent: i64) -> Result<Vec<SimpleInput>, DemoError> {
    let bank = demo.bank.as_str();
    let mut withdraw = input(SimpleKind::Transfer, day(2026, month, 1)?, 10_000, "", bank);
    withdraw.to_wallet = Some(demo.cash.clone());
    "現金の引き出し".clone_into(&mut withdraw.description);
    "サンプル銀行のATM".clone_into(&mut withdraw.memo);
    let mut out = vec![withdraw];
    if card_spent > 0 {
        let mut pay = input(
            SimpleKind::Transfer,
            day(2026, month, 27)?,
            card_spent,
            "",
            bank,
        );
        pay.to_wallet = Some(demo.card.clone());
        "サンプルカードの引き落とし".clone_into(&mut pay.description);
        out.push(pay);
    }
    if matches!(month, 2 | 5 | 8) {
        let date = day(2026, month, 15)?;
        let mut draw = input(SimpleKind::Expense, date, 100_000, "owner_draw", bank);
        "生活費として引き出し".clone_into(&mut draw.description);
        out.push(draw);
    }
    if matches!(month, 3 | 7) {
        let date = day(2026, month, 5)?;
        let mut put = input(SimpleKind::Income, date, 50_000, "owner_inv", bank);
        "個人の口座から入金".clone_into(&mut put.description);
        out.push(put);
    }
    Ok(out)
}

/// 訂正2件（金額と摘要）と、二重に入れた仕訳の取消1件。
pub(super) fn corrections(demo: &mut Demo<'_>, marks: &Marks) -> Result<(), DemoError> {
    if let Some((id, original)) = &marks.stationery {
        let fixed = SimpleInput {
            amount: original.amount + 110,
            ..original.clone()
        };
        demo.revise(id, &fixed, "金額の入力ミス")?;
    }
    if let Some((id, original)) = &marks.internet {
        let fixed = SimpleInput {
            description: "インターネット回線（4月分）".to_owned(),
            ..original.clone()
        };
        demo.revise(id, &fixed, "摘要の書き直し")?;
    }
    if let Some(id) = &marks.doubled {
        let at = demo.tick(day(2026, 9, 30)?);
        demo.store
            .void(id, 1, "二重に登録したため", at)
            .map_err(fail)?;
    }
    Ok(())
}
