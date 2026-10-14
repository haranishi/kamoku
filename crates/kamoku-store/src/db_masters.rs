// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Settings, StoreError};
use kamoku_books::{Account, Chart, Kind, TaxCode, Wallet};
use kamoku_ledger::AccountCode;
use rusqlite::{Connection, params};

pub(crate) fn insert_account(conn: &Connection, a: &Account) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO accounts VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            a.code.as_str(),
            a.name,
            a.reading,
            a.kind.as_str(),
            a.parent.as_ref().map(AccountCode::as_str),
            a.wallet.as_ref().map(serde_json::to_string).transpose()?,
            a.default_tax.map(TaxCode::as_str),
            a.default_pct,
            a.form_line,
            a.enabled,
            a.system,
            a.sort
        ],
    )?;
    Ok(())
}
pub(crate) fn chart(conn: &Connection) -> Result<Chart, StoreError> {
    let mut stmt=conn.prepare("SELECT code,name,reading,kind,parent,wallet,default_tax,default_pct,form_line,enabled,system,sort FROM accounts")?;
    let mut rows = stmt.query([])?;
    let mut accounts = Vec::new();
    while let Some(r) = rows.next()? {
        let code: String = r.get(0)?;
        let kind: String = r.get(3)?;
        let parent: Option<String> = r.get(4)?;
        let wallet: Option<String> = r.get(5)?;
        let tax: Option<String> = r.get(6)?;
        accounts.push(Account {
            code: AccountCode::new(&code).map_err(|e| StoreError::Invalid(e.to_string()))?,
            name: r.get(1)?,
            reading: r.get(2)?,
            kind: Kind::parse(&kind).ok_or_else(|| StoreError::Invalid("科目の分類".into()))?,
            parent: parent
                .map(|p| AccountCode::new(&p))
                .transpose()
                .map_err(|e| StoreError::Invalid(e.to_string()))?,
            wallet: wallet
                .map(|w| serde_json::from_str::<Wallet>(&w))
                .transpose()?,
            default_tax: tax
                .map(|t| TaxCode::parse(&t).ok_or(StoreError::BadTaxCode))
                .transpose()?,
            default_pct: r.get(7)?,
            form_line: r.get(8)?,
            enabled: r.get(9)?,
            system: r.get(10)?,
            sort: r.get(11)?,
        });
    }
    Chart::new(accounts).map_err(|e| StoreError::Invalid(e.to_string()))
}
pub(crate) fn settings(conn: &Connection) -> Result<Settings, StoreError> {
    let body: String = conn.query_row("SELECT body FROM settings WHERE id=1", [], |r| r.get(0))?;
    let settings = serde_json::from_str(&body)?;
    crate::masters::validate_settings(&settings)?;
    Ok(settings)
}
