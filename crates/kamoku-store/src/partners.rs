// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    ChangeLog, ImportProfile, NewPartner, Partner, PartnerPatch, Store, StoreError, Template,
    masters,
};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

const SAVE_PROFILE: &str = "
    INSERT INTO import_profiles (wallet, body) VALUES (?, ?)
    ON CONFLICT(wallet) DO UPDATE SET body=excluded.body";

fn partner_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Partner> {
    Ok(Partner {
        id: r.get(0)?,
        name: r.get(1)?,
        reading: r.get(2)?,
        invoice_number: r.get(3)?,
        note: r.get(4)?,
        enabled: r.get(5)?,
    })
}
impl Store {
    /// 全取引先を名前順で返します。検索は呼び出し側で行えます。
    ///
    /// # Errors
    /// `SQLite`の読み取りの失敗。
    pub fn partners(&self) -> Result<Vec<Partner>, StoreError> {
        Ok(self
            .conn
            .prepare(
                "
                    SELECT id, name, reading, invoice_number, note, enabled
                    FROM partners ORDER BY name, id",
            )?
            .query_map([], partner_row)?
            .collect::<Result<_, _>>()?)
    }
    pub(crate) fn partner_insert(
        &self,
        input: &NewPartner,
        now: i64,
        ensure: bool,
    ) -> Result<Partner, StoreError> {
        masters::name(&input.name)?;
        masters::invoice(input.invoice_number.as_deref())?;
        let found = self
            .conn
            .query_row(
                "
                    SELECT id, name, reading, invoice_number, note, enabled
                    FROM partners WHERE name=?",
                [&input.name],
                partner_row,
            )
            .optional()?;
        if let Some(p) = found {
            if ensure {
                return Ok(p);
            }
            return Err(StoreError::DuplicateName);
        }
        self.conn.execute(
            "
                INSERT INTO partners(name,reading,invoice_number,note,enabled) VALUES(?,?,?,?,1)",
            params![input.name, input.reading, input.invoice_number, input.note],
        )?;
        let p = Partner {
            id: self.conn.last_insert_rowid(),
            name: input.name.clone(),
            reading: input.reading.clone(),
            invoice_number: input.invoice_number.clone(),
            note: input.note.clone(),
            enabled: true,
        };
        self.log("partner.add", &json!(p), now)?;
        Ok(p)
    }
    /// 名前が重複しない取引先を追加します。
    ///
    /// # Errors
    /// 名前の重複、登録番号の不正、保存の失敗。
    // 操作の入力は、仕訳や変更パッチと同じく所有権を受け取る公開契約にそろえる。
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_partner(&mut self, input: NewPartner, now: i64) -> Result<Partner, StoreError> {
        self.atomic(|s| s.partner_insert(&input, now, false))
    }
    /// 取引先を変更して履歴に残します。
    ///
    /// # Errors
    /// 対象なし、名前の重複、登録番号の不正、保存の失敗。
    pub fn update_partner(
        &mut self,
        id: i64,
        patch: PartnerPatch,
        now: i64,
    ) -> Result<Partner, StoreError> {
        self.atomic(|s| {
            let mut p = s
                .conn
                .query_row(
                    "
                        SELECT id, name, reading, invoice_number, note, enabled
                        FROM partners WHERE id=?",
                    [id],
                    partner_row,
                )
                .optional()?
                .ok_or(StoreError::NotFound)?;
            let before = json!(p);
            if let Some(v) = patch.name {
                masters::name(&v)?;
                p.name = v;
            }
            if let Some(v) = patch.reading {
                p.reading = v;
            }
            if let Some(v) = patch.invoice_number {
                masters::invoice(v.as_deref())?;
                p.invoice_number = v;
            }
            if let Some(v) = patch.note {
                p.note = v;
            }
            if let Some(v) = patch.enabled {
                p.enabled = v;
            }
            let duplicate: bool = s.conn.query_row(
                "
                    SELECT EXISTS(SELECT 1 FROM partners WHERE name=? AND id!=?)",
                params![p.name, id],
                |r| r.get(0),
            )?;
            if duplicate {
                return Err(StoreError::DuplicateName);
            }
            s.conn.execute(
                "
                    UPDATE partners SET name=?, reading=?, invoice_number=?, note=?, enabled=?
                    WHERE id=?",
                params![p.name, p.reading, p.invoice_number, p.note, p.enabled, id],
            )?;
            s.log("partner.update", &json!({"before":before,"after":p}), now)?;
            Ok(p)
        })
    }
    /// テンプレートを一覧にします。
    ///
    /// # Errors
    /// `SQLite`またはJSONの読み取りの失敗。
    pub fn templates(&self) -> Result<Vec<Template>, StoreError> {
        let mut stmt = self.conn.prepare(
            "
                SELECT id,name,body FROM templates ORDER BY id",
        )?;
        stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .map(|r| {
            let (id, name, body) = r?;
            Ok(Template {
                id,
                name,
                body: serde_json::from_str(&body)?,
            })
        })
        .collect()
    }
    /// 日付を除いた単純な入力をテンプレートとして保存します。
    ///
    /// # Errors
    /// 名前・入力の形の不正、保存の失敗。
    pub fn add_template(
        &mut self,
        name: &str,
        body: Value,
        now: i64,
    ) -> Result<Template, StoreError> {
        self.atomic(|s| {
            masters::name(name)?;
            let mut input = body.as_object().cloned().ok_or_else(|| {
                StoreError::Invalid("テンプレートは取引の入力にしてください".into())
            })?;
            if input.contains_key("date") {
                return Err(StoreError::Invalid(
                    "テンプレートには日付を入れません".into(),
                ));
            }
            input.insert("date".into(), json!("2000-01-01"));
            let parsed: kamoku_books::SimpleInput = serde_json::from_value(Value::Object(input))?;
            kamoku_books::compose_simple(
                &parsed,
                &s.chart,
                &kamoku_books::ComposeSettings {
                    rounding: kamoku_books::Rounding::Floor,
                    tax_mode: kamoku_books::TaxMode::Exempt,
                    source: kamoku_books::Source::Template,
                },
            )
            .map_err(|e| StoreError::Invalid(e.to_string()))?;
            s.conn.execute(
                "
                    INSERT INTO templates(name,body) VALUES(?,?)",
                params![name, serde_json::to_string(&body)?],
            )?;
            let template = Template {
                id: s.conn.last_insert_rowid(),
                name: name.into(),
                body,
            };
            s.log("template.add", &json!(template), now)?;
            Ok(template)
        })
    }
    /// テンプレートを削除して、消した内容を履歴に残します。
    ///
    /// # Errors
    /// 対象なしまたは保存の失敗。
    pub fn delete_template(&mut self, id: i64, now: i64) -> Result<(), StoreError> {
        self.atomic(|s| {
            let body: String = s
                .conn
                .query_row(
                    "
                    SELECT body FROM templates WHERE id=?",
                    [id],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or(StoreError::NotFound)?;
            s.conn.execute(
                "
                DELETE FROM templates WHERE id=?",
                [id],
            )?;
            s.log(
                "template.delete",
                &json!({"id":id,"body":serde_json::from_str::<Value>(&body)?}),
                now,
            )
        })
    }
    /// 口座の列対応。
    ///
    /// # Errors
    /// `SQLite`またはJSONの読み取りの失敗。
    pub fn import_profile(&self, wallet: &str) -> Result<Option<ImportProfile>, StoreError> {
        let body: Option<String> = self
            .conn
            .query_row(
                "
                    SELECT body FROM import_profiles WHERE wallet=?",
                [wallet],
                |r| r.get(0),
            )
            .optional()?;
        body.map(|b| serde_json::from_str(&b).map_err(StoreError::from))
            .transpose()
    }
    /// 口座に列対応を覚えさせます。
    ///
    /// # Errors
    /// 口座なし・無効または保存の失敗。
    // 操作の入力は、仕訳や変更パッチと同じく所有権を受け取る公開契約にそろえる。
    #[allow(clippy::needless_pass_by_value)]
    pub fn save_import_profile(
        &mut self,
        wallet: &str,
        profile: ImportProfile,
        now: i64,
    ) -> Result<(), StoreError> {
        self.atomic(|s| {
            s.wallet_check(wallet)?;
            s.conn.execute(
                SAVE_PROFILE,
                params![wallet, serde_json::to_string(&profile)?],
            )?;
            s.log(
                "import_profile",
                &json!({"wallet": wallet, "mapping": profile}),
                now,
            )
        })
    }
    /// 新しい順の変更履歴。上限は1〜500件。
    ///
    /// # Errors
    /// 範囲外の上限、`SQLite`またはJSONの読み取りの失敗。
    pub fn change_log(&self, limit: u32) -> Result<Vec<ChangeLog>, StoreError> {
        if !(1..=500).contains(&limit) {
            return Err(StoreError::Invalid("件数は1〜500にしてください".into()));
        }
        let mut stmt = self.conn.prepare(
            "
                SELECT id,actor,recorded_at,kind,body FROM change_log ORDER BY id DESC LIMIT ?",
        )?;
        let mut rows = stmt.query([limit])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let body: String = r.get(4)?;
            out.push(ChangeLog {
                id: r.get(0)?,
                actor: r.get(1)?,
                recorded_at: r.get(2)?,
                kind: r.get(3)?,
                body: serde_json::from_str(&body)?,
            });
        }
        Ok(out)
    }
}
