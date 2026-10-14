-- SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
-- SPDX-License-Identifier: MIT OR Apache-2.0
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE settings(id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL CHECK(json_valid(body))) STRICT;
CREATE TABLE accounts(
 code TEXT PRIMARY KEY CHECK(length(code) BETWEEN 1 AND 16), name TEXT NOT NULL CHECK(length(name)>0),
 reading TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('asset','liability','equity','revenue','expense')),
 parent TEXT REFERENCES accounts(code), wallet TEXT CHECK(wallet IS NULL OR json_valid(wallet)),
 default_tax TEXT CHECK(default_tax IS NULL OR default_tax IN ('S10','S8','P10','P8','P10X','P8X','NT')),
 default_pct INTEGER NOT NULL CHECK(default_pct BETWEEN 0 AND 100), form_line TEXT,
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)), system INTEGER NOT NULL CHECK(system IN (0,1)),
 sort INTEGER NOT NULL CHECK(sort BETWEEN 0 AND 4294967295)) STRICT;
CREATE TABLE partners(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE CHECK(length(name)>0),
 reading TEXT NOT NULL, invoice_number TEXT, note TEXT NOT NULL, enabled INTEGER NOT NULL CHECK(enabled IN (0,1))) STRICT;
CREATE TABLE ledger_events(seq INTEGER PRIMARY KEY CHECK(seq>0), kind INTEGER NOT NULL CHECK(kind IN (1,2,3)),
 entry_id TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision BETWEEN 1 AND 4294967295), recorded_at INTEGER NOT NULL,
 reason TEXT, date TEXT, description TEXT, prev_hash TEXT NOT NULL CHECK(length(prev_hash)=64),
 hash TEXT NOT NULL CHECK(length(hash)=64), UNIQUE(entry_id,revision),
 CHECK((kind=3 AND date IS NULL AND description IS NULL) OR (kind IN (1,2) AND date IS NOT NULL AND description IS NOT NULL))) STRICT;
CREATE TABLE ledger_lines(seq INTEGER NOT NULL REFERENCES ledger_events(seq), position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 199),
 account TEXT NOT NULL REFERENCES accounts(code), side TEXT NOT NULL CHECK(side IN ('debit','credit')),
 amount INTEGER NOT NULL CHECK(amount BETWEEN 1 AND 9999999999999), tax_code TEXT,
 tax_amount INTEGER NOT NULL CHECK(tax_amount>=0 AND tax_amount<=amount AND (tax_code IS NOT NULL OR tax_amount=0)),
 memo TEXT NOT NULL, PRIMARY KEY(seq,position)) STRICT;
CREATE TABLE ledger_meta(seq INTEGER NOT NULL REFERENCES ledger_events(seq), key TEXT NOT NULL CHECK(length(key) BETWEEN 1 AND 32),
 value TEXT NOT NULL CHECK(length(CAST(value AS BLOB))<=1000), PRIMARY KEY(seq,key)) STRICT;
CREATE TABLE import_batches(id INTEGER PRIMARY KEY, filename TEXT NOT NULL, wallet TEXT NOT NULL REFERENCES accounts(code),
 recorded_at INTEGER NOT NULL, created INTEGER NOT NULL CHECK(created>=0), duplicates INTEGER NOT NULL CHECK(duplicates>=0), invalid INTEGER NOT NULL CHECK(invalid>=0)) STRICT;
CREATE TABLE drafts(id INTEGER PRIMARY KEY, batch_id INTEGER NOT NULL REFERENCES import_batches(id),
 wallet TEXT NOT NULL REFERENCES accounts(code), line INTEGER NOT NULL CHECK(line>0), date TEXT NOT NULL,
 description TEXT NOT NULL, direction TEXT NOT NULL CHECK(direction IN ('in','out')),
 amount INTEGER NOT NULL CHECK(amount BETWEEN 1 AND 9999999999999), fingerprint TEXT NOT NULL CHECK(length(fingerprint)=64),
 occurrence INTEGER NOT NULL CHECK(occurrence BETWEEN 1 AND 4294967295), external_id TEXT,
 account TEXT REFERENCES accounts(code), tax_code TEXT CHECK(tax_code IS NULL OR tax_code IN ('S10','S8','P10','P8','P10X','P8X','NT')),
 partner TEXT NOT NULL DEFAULT '', memo TEXT NOT NULL DEFAULT '', business_pct INTEGER NOT NULL DEFAULT 100 CHECK(business_pct BETWEEN 0 AND 100),
 status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','confirmed','ignored')), entry_id TEXT,
 CHECK((status='confirmed' AND entry_id IS NOT NULL) OR (status!='confirmed' AND entry_id IS NULL)), UNIQUE(fingerprint,occurrence)) STRICT;
CREATE INDEX drafts_filter ON drafts(status,wallet,date,id);
CREATE TABLE import_profiles(wallet TEXT PRIMARY KEY REFERENCES accounts(code), body TEXT NOT NULL CHECK(json_valid(body))) STRICT;
CREATE TABLE templates(id INTEGER PRIMARY KEY, name TEXT NOT NULL CHECK(length(name)>0), body TEXT NOT NULL CHECK(json_valid(body))) STRICT;
CREATE TABLE change_log(id INTEGER PRIMARY KEY, actor TEXT NOT NULL, recorded_at INTEGER NOT NULL, kind TEXT NOT NULL, body TEXT NOT NULL CHECK(json_valid(body))) STRICT;
CREATE TRIGGER ledger_events_no_update BEFORE UPDATE ON ledger_events BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER ledger_events_no_delete BEFORE DELETE ON ledger_events BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER ledger_lines_no_update BEFORE UPDATE ON ledger_lines BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER ledger_lines_no_delete BEFORE DELETE ON ledger_lines BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER ledger_meta_no_update BEFORE UPDATE ON ledger_meta BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER ledger_meta_no_delete BEFORE DELETE ON ledger_meta BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER change_log_no_update BEFORE UPDATE ON change_log BEGIN SELECT RAISE(ABORT,'append only'); END;
CREATE TRIGGER change_log_no_delete BEFORE DELETE ON change_log BEGIN SELECT RAISE(ABORT,'append only'); END;

-- REPLACEの暗黙削除でも、recursive_triggersの設定によらず既存行を保護する。
CREATE TRIGGER ledger_events_no_replace BEFORE INSERT ON ledger_events
WHEN EXISTS (SELECT 1 FROM ledger_events
 WHERE seq=NEW.seq OR (entry_id=NEW.entry_id AND revision=NEW.revision))
BEGIN SELECT RAISE(ABORT, 'append only'); END;
CREATE TRIGGER ledger_lines_no_replace BEFORE INSERT ON ledger_lines
WHEN EXISTS (SELECT 1 FROM ledger_lines WHERE seq=NEW.seq AND position=NEW.position)
BEGIN SELECT RAISE(ABORT, 'append only'); END;
CREATE TRIGGER ledger_meta_no_replace BEFORE INSERT ON ledger_meta
WHEN EXISTS (SELECT 1 FROM ledger_meta WHERE seq=NEW.seq AND key=NEW.key)
BEGIN SELECT RAISE(ABORT, 'append only'); END;
CREATE TRIGGER change_log_no_replace BEFORE INSERT ON change_log
WHEN EXISTS (SELECT 1 FROM change_log WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT, 'append only'); END;
