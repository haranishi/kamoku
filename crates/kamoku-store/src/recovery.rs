// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 別の接続の変更に合わせ直す処理と、途中で止まった処理からの立て直し。

use crate::{Store, StoreError, VerifyFailure, db_masters, sql};
use kamoku_ledger::Ledger;
use rusqlite::OptionalExtension;

impl Store {
    /// 設定と科目の集合を、DBから読み直します。仕訳を組み立てる前に呼ぶと、別の接続（別のプロセス）が
    /// 変えたマスタで組み立てられる（古い既定の割合などで、黙って保存しない）。
    /// 台帳の末尾がDBと違えば、台帳も読み直してから `Stale` を返す（書き込みの前の検査と同じ扱い）。
    ///
    /// # Errors
    /// 台帳の末尾が変わっていたとき `Stale`（読み直しは済んでいる）、`SQLite`またはマスタの読み取りの失敗。
    pub fn refresh_masters(&mut self) -> Result<(), StoreError> {
        // 末尾とマスタを同じ時点で読む（読む間の外部の書き込みで、別々の時点にならないように）。
        self.conn.execute_batch("BEGIN DEFERRED")?;
        let read = (|| {
            let tail: Option<(u64, String)> = self
                .conn
                .query_row(
                    "SELECT seq,hash FROM ledger_events ORDER BY seq DESC LIMIT 1",
                    [],
                    |r| Ok((sql::unsigned(r, 0)?, r.get(1)?)),
                )
                .optional()?;
            let chart = db_masters::chart(&self.conn)?;
            let settings = db_masters::settings(&self.conn)?;
            Ok::<_, StoreError>((tail, chart, settings))
        })();
        self.conn.execute_batch("ROLLBACK")?;
        let (tail, chart, settings) = read?;
        let memory = self
            .ledger
            .events()
            .last()
            .map(|e| (e.seq, e.hash.to_hex()));
        if tail != memory {
            self.reload()?;
            return Err(StoreError::Stale);
        }
        self.chart = chart;
        self.settings = settings;
        Ok(())
    }

    /// 処理の途中で止まった（panicした）あとに、DBの状態からやり直します。開いたままの
    /// トランザクションがあれば巻き戻し、台帳とマスタを読み直す。読み直せなければ、保存されて
    /// いないかもしれないメモリの仕訳を公開しないよう、台帳を空にして読み取り専用にする。
    pub fn recover(&mut self) {
        if !self.conn.is_autocommit() {
            // 巻き戻しに失敗しても、続く読み直しが失敗して読み取り専用になるので、ここでは止めない。
            let _ = self.conn.execute_batch("ROLLBACK");
        }
        if let Err(error) = self.reload() {
            self.ledger = Ledger::new();
            self.numbers.clear();
            self.failure = Some(VerifyFailure::row(
                0,
                format!("途中で止まった処理のあとの再読込に失敗しました: {error}"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{OpenOptions, Store, StoreError};
    use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn entry(id: &str) -> Entry {
        Entry::new(
            EntryId::new(id).unwrap(),
            time::macros::date!(2026 - 10 - 01),
            "試験の仕訳",
            vec![
                Line::new(
                    AccountCode::new("exp_comm").unwrap(),
                    Side::Debit,
                    Yen::new(110),
                ),
                Line::new(
                    AccountCode::new("cash").unwrap(),
                    Side::Credit,
                    Yen::new(110),
                ),
            ],
        )
        .unwrap()
    }

    fn open(path: &std::path::Path) -> Store {
        Store::open(
            path,
            OpenOptions {
                create_if_missing: true,
            },
        )
        .unwrap()
        .0
    }

    // B2: 書き込みの途中で panic すると、トランザクションが開いたまま、メモリには保存されていない
    // 仕訳が残る。recover は巻き戻して読み直すので、未保存の仕訳は読み取りに出ず、次の書き込みが通る。
    #[test]
    fn recover_rolls_back_and_drops_unsaved_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kamoku.db");
        let mut store = open(&path);
        store.post(entry("e_saved"), 1, None).unwrap();
        let injected = catch_unwind(AssertUnwindSafe(|| {
            let _ = store.atomic(|s| -> Result<(), StoreError> {
                s.post_inside(entry("e_unsaved"), 2, None)?;
                panic!("試験で注入した停止");
            });
        }));
        assert!(injected.is_err());
        assert!(!store.conn.is_autocommit(), "トランザクションが開いたまま");
        assert_eq!(store.ledger().events().len(), 2, "メモリには未保存の仕訳");
        store.recover();
        assert!(store.conn.is_autocommit());
        assert_eq!(store.ledger().events().len(), 1);
        assert!(store.read_only().is_none());
        store.post(entry("e_after"), 3, None).unwrap();
        drop(store);
        let reopened = open(&path);
        let ids: Vec<&str> = reopened
            .ledger()
            .events()
            .iter()
            .map(|e| e.entry_id.as_str())
            .collect();
        assert_eq!(ids, ["e_saved", "e_after"]);
    }

    // B2: 読み直せないときは、台帳を空にして読み取り専用にする（未保存かもしれない仕訳を出さない）。
    #[test]
    fn recover_goes_read_only_when_it_cannot_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(&dir.path().join("kamoku.db"));
        store.post(entry("e_saved"), 1, None).unwrap();
        store.conn.execute_batch("DROP TABLE settings").unwrap();
        store.recover();
        assert!(store.read_only().is_some());
        assert_eq!(store.ledger().events().len(), 0);
        assert!(matches!(
            store.post(entry("e_after"), 2, None),
            Err(StoreError::ReadOnly)
        ));
    }
}
