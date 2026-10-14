// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! デモの合成データ（`kamoku demo`）。2026年1月〜9月の帳簿と、未確認の明細25件を入れる。
//!
//! 乱数の種を固定した線形合同法（依存を足さない）と、日付から決めた記録の時刻で作るので、
//! 毎回同じ台帳（同じ `head_hash`）になる（AC-12.2）。名前は「サンプル」「デモ」を含む架空のものだけ。

mod drafts;
mod plan;

use crate::ids;
use kamoku_books::{
    ComposeSettings, Kind, Rounding, SimpleInput, SimpleKind, Source, TaxMode, Wallet, WalletType,
    compose_opening, compose_simple,
};
use kamoku_ledger::{AccountCode, EntryId, EventKind, Yen};
use kamoku_store::{AccountPatch, NewAccount, NewPartner, Settings, Store};
use std::collections::HashSet;
use std::fmt;
use time::{Date, Month};

/// 合成データを作れなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoError(String);
impl fmt::Display for DemoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DemoError {}
fn fail(e: impl fmt::Display) -> DemoError {
    DemoError(e.to_string())
}

/// 入れた結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoSummary {
    /// 仕訳の数（取消済みを含む）。
    pub entries: usize,
    /// 未確認の明細の数。
    pub drafts: u64,
    /// 台帳の末尾のハッシュ。
    pub head_hash: String,
}

// 2026-01-01T00:00:00Z。マスタの変更の時刻に使う。
const EPOCH_2026: i64 = 1_767_225_600;
const UNIX_JULIAN_DAY: i32 = 2_440_588;
const SEED: u64 = 20_261_014;
const PARTNERS: [&str; 5] = [
    "サンプル商事",
    "デモ企画",
    "サンプル通信",
    "デモ交通",
    "サンプル文具",
];

// 線形合同法（Knuth の MMIX の定数）。上位32ビットを使う。
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        u32::try_from(self.0 >> 32).unwrap_or_default()
    }
    fn below(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn amount(&mut self, lo: i64, hi: i64, step: i64) -> i64 {
        let steps = u32::try_from((hi - lo) / step + 1).unwrap_or(1);
        lo + i64::from(self.below(steps)) * step
    }
}

struct Demo<'a> {
    store: &'a mut Store,
    rng: Lcg,
    clock: i64,
    cash: String,
    bank: String,
    card: String,
    settings: ComposeSettings,
    taken: HashSet<EntryId>,
}

fn day(year: i32, month: u8, day: u8) -> Result<Date, DemoError> {
    let month = Month::try_from(month).map_err(fail)?;
    Date::from_calendar_date(year, month, day).map_err(fail)
}

fn input(kind: SimpleKind, date: Date, amount: i64, account: &str, wallet: &str) -> SimpleInput {
    SimpleInput {
        kind,
        date,
        amount,
        account: (!account.is_empty()).then(|| account.to_owned()),
        wallet: wallet.to_owned(),
        to_wallet: None,
        tax_code: None,
        partner: String::new(),
        description: String::new(),
        business_pct: None,
        memo: String::new(),
    }
}

impl Demo<'_> {
    // その日の18時（日本時間）を基準に、前の記録より後の時刻にする。
    fn tick(&mut self, date: Date) -> i64 {
        let start = i64::from(date.to_julian_day() - UNIX_JULIAN_DAY) * 86_400 + 9 * 3_600;
        self.clock = start.max(self.clock + 1);
        self.clock
    }
    fn next_id(&mut self, at: i64) -> Result<EntryId, DemoError> {
        let millis = u64::try_from(at).unwrap_or_default().saturating_mul(1_000);
        for _ in 0..16 {
            let id = EntryId::new(&ids::entry_id(millis, self.rng.next())).map_err(fail)?;
            if self.taken.insert(id.clone()) {
                return Ok(id);
            }
        }
        Err(DemoError("仕訳IDが重なりました".to_owned()))
    }
    fn post(&mut self, input: &SimpleInput) -> Result<EntryId, DemoError> {
        let composed = compose_simple(input, self.store.chart(), &self.settings).map_err(fail)?;
        let at = self.tick(input.date);
        let id = self.next_id(at)?;
        let entry = composed.to_entry(id.clone()).map_err(fail)?;
        self.store.post(entry, at, None).map_err(fail)?;
        Ok(id)
    }
    fn revise(&mut self, id: &EntryId, input: &SimpleInput, reason: &str) -> Result<(), DemoError> {
        let composed = compose_simple(input, self.store.chart(), &self.settings).map_err(fail)?;
        let at = self.tick(input.date);
        let entry = composed.to_entry(id.clone()).map_err(fail)?;
        self.store
            .revise(entry, 1, reason, at, None)
            .map_err(fail)?;
        Ok(())
    }
}

fn add_wallet(
    store: &mut Store,
    name: &str,
    reading: &str,
    kind: WalletType,
    institution: &str,
) -> Result<String, DemoError> {
    let wallet = Wallet {
        kind,
        institution: institution.to_owned(),
    };
    let account = NewAccount {
        name: name.to_owned(),
        reading: reading.to_owned(),
        kind: kind.kind(),
        parent: None,
        wallet: Some(wallet),
        default_tax: None,
        default_pct: 100,
    };
    let added = store.add_account(account, EPOCH_2026).map_err(fail)?;
    Ok(added.code.as_str().to_owned())
}

fn masters(store: &mut Store) -> Result<(String, String, String), DemoError> {
    let settings = Settings {
        business_name: "サンプル事務所（デモ）".to_owned(),
        owner_name: "デモ利用者".to_owned(),
        tax_mode: "taxable".to_owned(),
        invoice_number: None,
        rounding: "floor".to_owned(),
        opened_on: "2026-01-01".to_owned(),
        locked_until: None,
        setup_done: true,
    };
    store.save_settings(settings, EPOCH_2026).map_err(fail)?;
    let cash = add_wallet(store, "現金", "げんきん", WalletType::Cash, "")?;
    let bank_name = "サンプル銀行 普通預金";
    let bank = add_wallet(
        store,
        bank_name,
        "さんぷるぎんこう",
        WalletType::Bank,
        "サンプル銀行",
    )?;
    let card_name = "サンプルカード";
    let card = add_wallet(
        store,
        card_name,
        "さんぷるかーど",
        WalletType::Card,
        card_name,
    )?;
    for name in PARTNERS {
        let partner = NewPartner {
            name: name.to_owned(),
            ..NewPartner::default()
        };
        store.add_partner(partner, EPOCH_2026).map_err(fail)?;
    }
    // ガソリンの按分（事業30%）を、科目の既定の割合で入れる例にする。
    let vehicle = AccountPatch {
        default_pct: Some(30),
        ..AccountPatch::default()
    };
    store
        .update_account("exp_vehicle", vehicle, EPOCH_2026)
        .map_err(fail)?;
    if store.chart().kind(&cash) != Some(Kind::Asset) {
        return Err(DemoError("現金の口座を作れませんでした".to_owned()));
    }
    Ok((cash, bank, card))
}

/// 空の帳簿に、合成データを入れます。
///
/// # Errors
/// 保存・組み立て・取込の失敗。
pub fn populate(store: &mut Store) -> Result<DemoSummary, DemoError> {
    let (cash, bank, card) = masters(store)?;
    let mut demo = Demo {
        store,
        rng: Lcg(SEED),
        clock: EPOCH_2026,
        cash,
        bank,
        card,
        settings: ComposeSettings {
            rounding: Rounding::Floor,
            tax_mode: TaxMode::Taxable,
            source: Source::Demo,
        },
        taken: HashSet::new(),
    };
    opening(&mut demo)?;
    let marks = plan::months(&mut demo)?;
    plan::corrections(&mut demo, &marks)?;
    drafts::import(&mut demo)?;
    let ledger = demo.store.ledger();
    Ok(DemoSummary {
        entries: ledger
            .events()
            .iter()
            .filter(|e| e.kind == EventKind::Posted)
            .count(),
        drafts: demo.store.draft_counts().map_err(fail)?.draft,
        head_hash: ledger.head_hash().to_hex(),
    })
}

fn opening(demo: &mut Demo<'_>) -> Result<(), DemoError> {
    let date = day(2026, 1, 1)?;
    let cash = AccountCode::new(&demo.cash).map_err(fail)?;
    let bank = AccountCode::new(&demo.bank).map_err(fail)?;
    let rows = [(cash, Yen::new(30_000)), (bank, Yen::new(1_200_000))];
    let composed = compose_opening(date, &rows, demo.store.chart())
        .map_err(fail)?
        .ok_or_else(|| DemoError("期首残高が空です".to_owned()))?;
    let at = demo.tick(date);
    let id = demo.next_id(at)?;
    let entry = composed.to_entry(id).map_err(fail)?;
    demo.store.post(entry, at, None).map_err(fail)?;
    Ok(())
}
