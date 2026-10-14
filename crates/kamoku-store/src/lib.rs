// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 台帳・下書き・マスタの`SQLite`保存。書き込みは接続ごとに直列で行います。
//! 起動時に破損があれば検証済みの接頭部分だけを公開し、すべての変更を拒みます。
//! 操作者は `set_actor`、マスタの変更時刻は各関数の `now` で指定します。

mod confirmation;
mod db_event;
mod db_ledger;
mod db_masters;
mod draft_types;
mod drafts;
mod error;
mod files;
mod inbox;
mod masters;
mod partners;
mod posting;
mod recovery;
mod sql;
mod types;

pub use draft_types::*;
pub use error::{StoreError, VerifyFailure};
use kamoku_books::Chart;
use kamoku_ledger::{EntryId, EventKind, Ledger};
use rusqlite::{Connection, OptionalExtension};
use std::{collections::HashMap, path::Path, time::Instant};
pub use types::*;

/// `SQLite`接続と検証済み台帳。外部の変更は書き込み前に検出します。
pub struct Store {
    conn: Connection,
    ledger: Ledger,
    chart: Chart,
    settings: Settings,
    numbers: HashMap<EntryId, u64>,
    failure: Option<VerifyFailure>,
    actor: String,
}
impl Store {
    /// DBを開き、移行と全件の検証を行います。破損はErrでなく読み取り専用として返します。
    ///
    /// # Errors
    /// ファイル操作、`SQLite`、未来のスキーマ、マスタ復元の失敗。
    pub fn open(
        path: impl AsRef<Path>,
        options: OpenOptions,
    ) -> Result<(Self, LoadReport), StoreError> {
        let path = path.as_ref();
        files::prepare(path, options.create_if_missing)?;
        let mut flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE;
        if options.create_if_missing {
            flags |= rusqlite::OpenFlags::SQLITE_OPEN_CREATE;
        }
        let conn = Connection::open_with_flags(path, flags)?;
        let result = Self::from_connection(conn);
        files::secure_db(path)?;
        result
    }
    /// 試験用のインメモリDBを開きます。
    ///
    /// # Errors
    /// `SQLite`または移行の失敗。
    pub fn open_in_memory() -> Result<(Self, LoadReport), StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }
    fn from_connection(mut conn: Connection) -> Result<(Self, LoadReport), StoreError> {
        let start = Instant::now();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000;")?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(StoreError::FutureSchema);
        }
        if version == 0 {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            tx.execute_batch(include_str!("../migrations/0001_init.sql"))?;
            tx.execute(
                "INSERT INTO settings VALUES(1,?)",
                [serde_json::to_string(&Settings::default())?],
            )?;
            tx.execute("INSERT INTO meta VALUES('app','kamoku')", [])?;
            tx.execute("INSERT INTO meta VALUES('schema','1')", [])?;
            tx.execute(
                "INSERT INTO meta VALUES('created_at',?)",
                [std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| StoreError::Invalid(e.to_string()))?
                    .as_secs()
                    .to_string()],
            )?;
            for account in kamoku_books::standard_accounts() {
                db_masters::insert_account(&tx, &account)?;
            }
            tx.execute_batch("PRAGMA user_version=1")?;
            tx.commit()?;
        }
        // 読み込み中の外部追記で、イベントと明細が別時点になるのを防ぐ。
        conn.execute_batch("BEGIN DEFERRED")?;
        let loaded = db_ledger::load(&conn);
        let chart = db_masters::chart(&conn);
        let settings = db_masters::settings(&conn);
        conn.execute_batch("ROLLBACK")?;
        let loaded = loaded?;
        let mut store = Self {
            conn,
            ledger: loaded.ledger,
            chart: chart?,
            settings: settings?,
            numbers: HashMap::new(),
            failure: loaded.failure,
            actor: "local".into(),
        };
        store.rebuild_numbers();
        let report = LoadReport {
            events_in_db: loaded.count,
            events_loaded: store.ledger.events().len() as u64,
            entries: store.numbers.len() as u64,
            elapsed: start.elapsed(),
            verification: store
                .failure
                .clone()
                .map_or_else(|| Ok(store.verify_ok()), Err),
        };
        Ok((store, report))
    }
    /// 検証済み台帳。
    #[must_use]
    pub const fn ledger(&self) -> &Ledger {
        &self.ledger
    }
    /// 科目集合。
    #[must_use]
    pub const fn chart(&self) -> &Chart {
        &self.chart
    }
    /// 設定。
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }
    /// 読み取り専用なら最初の破損。
    #[must_use]
    pub const fn read_only(&self) -> Option<&VerifyFailure> {
        self.failure.as_ref()
    }
    /// 登録順の仕訳No。
    #[must_use]
    pub fn entry_no(&self, id: &EntryId) -> Option<u64> {
        self.numbers.get(id).copied()
    }
    /// 次の操作に記録する操作者。未指定は `local`。
    ///
    /// # Errors
    /// 空または1000バイト超の名前。
    pub fn set_actor(&mut self, actor: &str) -> Result<(), StoreError> {
        masters::name(actor)?;
        actor.clone_into(&mut self.actor);
        Ok(())
    }
    fn rebuild_numbers(&mut self) {
        self.numbers.clear();
        let mut no = 0;
        for e in self.ledger.events() {
            if e.kind == EventKind::Posted {
                no += 1;
                self.numbers.insert(e.entry_id.clone(), no);
            }
        }
    }
    fn reload(&mut self) -> Result<(), StoreError> {
        self.conn.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let loaded = db_ledger::load(&self.conn)?;
            let chart = db_masters::chart(&self.conn)?;
            let settings = db_masters::settings(&self.conn)?;
            Ok::<_, StoreError>((loaded, chart, settings))
        })();
        self.conn.execute_batch("ROLLBACK")?;
        let (loaded, chart, settings) = result?;
        self.ledger = loaded.ledger;
        self.failure = loaded.failure;
        self.chart = chart;
        self.settings = settings;
        self.rebuild_numbers();
        Ok(())
    }
    fn writable(&self) -> Result<(), StoreError> {
        if self.failure.is_some() {
            Err(StoreError::ReadOnly)
        } else {
            Ok(())
        }
    }
    fn atomic<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.writable()?;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let tail: Option<(u64, String)> = self
                .conn
                .query_row(
                    "SELECT seq,hash FROM ledger_events ORDER BY seq DESC LIMIT 1",
                    [],
                    |r| Ok((sql::unsigned(r, 0)?, r.get(1)?)),
                )
                .optional()?;
            let memory = self
                .ledger
                .events()
                .last()
                .map(|e| (e.seq, e.hash.to_hex()));
            if tail != memory {
                return Err(StoreError::Stale);
            }
            // 締めや科目の有効性も、ロックを取った時点のマスタを使う。
            self.chart = db_masters::chart(&self.conn)?;
            self.settings = db_masters::settings(&self.conn)?;
            let value = operation(self)?;
            self.conn.execute_batch("COMMIT")?;
            Ok(value)
        })();
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                if !self.conn.is_autocommit() {
                    self.conn.execute_batch("ROLLBACK")?;
                }
                if let Err(reload_error) = self.reload() {
                    // 保存されなかったメモリの仕訳を、検証済みとして公開しない。
                    self.ledger = Ledger::new();
                    self.numbers.clear();
                    self.failure = Some(VerifyFailure::row(
                        0,
                        format!("巻き戻し後の再読込に失敗しました: {reload_error}"),
                    ));
                }
                Err(error)
            }
        }
    }
    fn log(&self, kind: &str, body: &serde_json::Value, now: i64) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO change_log(actor,recorded_at,kind,body) VALUES(?,?,?,?)",
            rusqlite::params![self.actor, now, kind, serde_json::to_string(&body)?],
        )?;
        Ok(())
    }
    fn verify_ok(&self) -> VerifyOk {
        VerifyOk {
            events: self.ledger.events().len() as u64,
            head_hash: self.ledger.head_hash().to_hex(),
        }
    }
    /// メモリとDBの連鎖、件数、連番、`quick_check` を検査します。
    ///
    /// # Errors
    /// 最初に見つけた破損またはメモリとDBの不一致。
    pub fn verify(&self) -> Result<VerifyOk, VerifyFailure> {
        if let Some(f) = &self.failure {
            return Err(f.clone());
        }
        self.ledger.verify().map_err(VerifyFailure::from)?;
        self.conn
            .execute_batch("BEGIN DEFERRED")
            .map_err(|e| VerifyFailure::row(0, e.to_string()))?;
        let result = (|| {
            let quick: String = self
                .conn
                .query_row("PRAGMA quick_check", [], |r| r.get(0))
                .map_err(|e| VerifyFailure::row(0, e.to_string()))?;
            if quick != "ok" {
                return Err(VerifyFailure {
                    seq: 0,
                    kind: "quick_check".into(),
                    rule: Some(quick),
                });
            }
            let loaded =
                db_ledger::load(&self.conn).map_err(|e| VerifyFailure::row(0, e.to_string()))?;
            if let Some(f) = loaded.failure {
                return Err(f);
            }
            if loaded.ledger.events() != self.ledger.events() {
                return Err(VerifyFailure::row(loaded.count, "DBとメモリの不一致"));
            }
            Ok(self.verify_ok())
        })();
        self.conn
            .execute_batch("ROLLBACK")
            .map_err(|e| VerifyFailure::row(0, e.to_string()))?;
        result
    }
    /// 控えたハッシュが、検証済み部分のどのイベントかを返します。
    #[must_use]
    pub fn noted_hash_lookup(&self, hash: &str) -> Option<NotedHash> {
        let hash = kamoku_ledger::Hash::from_hex(hash).ok()?;
        self.ledger
            .events()
            .iter()
            .find(|e| e.hash == hash)
            .map(|e| NotedHash {
                seq: e.seq,
                is_head: e.hash == self.ledger.head_hash(),
            })
    }
}
