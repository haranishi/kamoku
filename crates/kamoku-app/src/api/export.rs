// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 書き出し（`/export/journal.csv` `/export/all.json` `/export/backup.db`）と、CSVの共通の部品。

use super::entry_view::{Current, all_current};
use super::{ApiError, Call, FileReply, Reply};
use crate::csv::{Encoding, Writer, num, text};
use crate::query::Query;
use crate::state::State;
use crate::{clock, paths};
use kamoku_books::format_date;
use kamoku_ledger::{Event, EventKind, Side};
use serde_json::{Value, json};

/// `?encoding=utf8bom|sjis`（既定は `utf8bom`）。
pub(crate) fn csv_encoding(query: &Query) -> Result<Encoding, ApiError> {
    match query.get("encoding") {
        None | Some("utf8bom") => Ok(Encoding::Utf8Bom),
        Some("sjis") => Ok(Encoding::Sjis),
        Some(_) => Err(
            ApiError::bad_request("encoding は utf8bom か sjis にしてください。")
                .with_field("encoding"),
        ),
    }
}

/// CSVのファイルの応答。`Shift_JIS` で表せない文字があれば `X-Kamoku-Warning: unmappable=<数>` を付ける。
pub(crate) fn csv_reply(writer: Writer, encoding: Encoding, filename: String) -> Reply {
    let encoded = writer.finish(encoding);
    Reply::File(FileReply {
        content_type: match encoding {
            Encoding::Utf8Bom => "text/csv; charset=utf-8",
            Encoding::Sjis => "text/csv; charset=Shift_JIS",
        },
        filename,
        body: encoded.bytes,
        warning: (encoded.unmappable > 0).then(|| format!("unmappable={}", encoded.unmappable)),
    })
}

const fn side_label(side: Side) -> &'static str {
    match side {
        Side::Debit => "借方",
        Side::Credit => "貸方",
    }
}

/// `GET /export/journal.csv?from&to&state=active|all`。1行が1明細。既定は有効な仕訳だけ。
pub(crate) fn export_journal(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let encoding = csv_encoding(call.query)?;
    let from = call.query.date("from")?;
    let to = call.query.date("to")?;
    let include_voided = match call.query.get("state") {
        None | Some("active") => false,
        Some("all") => true,
        Some(_) => {
            return Err(
                ApiError::bad_request("state は active か all にしてください。")
                    .with_field("state"),
            );
        }
    };
    let chart = state.store.chart();
    let mut rows: Vec<Current<'_>> = all_current(&state.store)
        .filter(|c| include_voided || !c.voided())
        .filter(|c| {
            from.is_none_or(|d| c.entry.date() >= d) && to.is_none_or(|d| c.entry.date() <= d)
        })
        .collect();
    rows.sort_by_key(|c| (c.entry.date(), c.no));
    let mut w = Writer::default();
    w.row(
        &[
            "仕訳No",
            "版",
            "状態",
            "日付",
            "摘要",
            "取引先",
            "行",
            "貸借",
            "科目コード",
            "科目名",
            "金額",
            "税区分",
            "税額",
            "行メモ",
        ]
        .map(text),
    );
    for c in &rows {
        let no = i64::try_from(c.no).map_err(ApiError::internal)?;
        for (index, line) in (1i64..).zip(c.entry.lines()) {
            let code = line.account.as_str();
            w.row(&[
                num(no),
                num(i64::from(c.latest.revision)),
                text(if c.voided() { "取消" } else { "有効" }),
                text(format_date(c.entry.date())),
                text(c.entry.description()),
                text(c.meta("partner")),
                num(index),
                text(side_label(line.side)),
                text(code),
                text(chart.name(code).unwrap_or(code)),
                num(line.amount.get()),
                text(line.tax_code.clone().unwrap_or_default()),
                num(line.tax_amount.get()),
                text(line.memo.clone()),
            ]);
        }
    }
    // 期間を決めない書き出しも、いつ書き出したかが名前で分かるようにする（バックアップと同じ日付の形）。
    let period = match (from, to) {
        (Some(f), Some(t)) => format!("-{}_{}", format_date(f), format_date(t)),
        _ => format!("-{}", stamp(call.now)),
    };
    Ok(csv_reply(
        w,
        encoding,
        format!("kamoku-journal{period}.csv"),
    ))
}

fn event_json(e: &Event) -> Value {
    json!({
        "seq": e.seq,
        "kind": match e.kind { EventKind::Posted => "posted", EventKind::Revised => "revised", _ => "voided" },
        "entry_id": e.entry_id.as_str(),
        "revision": e.revision,
        "recorded_at": e.recorded_at,
        "reason": e.reason,
        "prev_hash": e.prev_hash.to_hex(),
        "hash": e.hash.to_hex(),
        "entry": e.entry.as_ref().map(|entry| json!({
            "date": format_date(entry.date()),
            "description": entry.description(),
            "lines": entry.lines().iter().map(|l| json!({
                "account": l.account.as_str(),
                "side": if l.side == Side::Debit { "debit" } else { "credit" },
                "amount": l.amount.get(),
                "tax_code": l.tax_code,
                "tax_amount": l.tax_amount.get(),
                "memo": l.memo,
            })).collect::<Vec<Value>>(),
            "meta": entry.meta(),
        })),
    })
}

fn stamp(now: i64) -> String {
    format_date(clock::jst_date(now)).replace('-', "")
}

/// `GET /export/all.json`。設定・科目・取引先・全イベント（明細・付随情報・ハッシュ）。`schema` の版を持つ。
pub(crate) fn export_all(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let store = &state.store;
    let accounts: Vec<Value> = store
        .chart()
        .iter()
        .map(super::to_value)
        .collect::<Result<_, _>>()?;
    let events: Vec<Value> = store.ledger().events().iter().map(event_json).collect();
    let body = json!({
        "app": "kamoku",
        "schema": 1,
        "version": env!("CARGO_PKG_VERSION"),
        "exported_at": call.now,
        "settings": super::to_value(store.settings())?,
        "accounts": accounts,
        "partners": super::to_value(&store.partners()?)?,
        "events": events,
        "head_hash": store.ledger().head_hash().to_hex(),
        "verified": store.read_only().is_none(),
    });
    let bytes = serde_json::to_vec_pretty(&body).map_err(ApiError::internal)?;
    Ok(Reply::File(FileReply {
        content_type: "application/json; charset=utf-8",
        filename: format!("kamoku-all-{}.json", stamp(call.now)),
        body: bytes,
        warning: None,
    }))
}

/// `GET /export/backup.db`。`VACUUM INTO` で一時ファイルに作り、読んで消してから返す。
pub(crate) fn export_backup(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let bytes = paths::backup_bytes(&state.store).map_err(ApiError::internal)?;
    Ok(Reply::File(FileReply {
        content_type: "application/vnd.sqlite3",
        filename: format!("kamoku-backup-{}.db", stamp(call.now)),
        body: bytes,
        warning: None,
    }))
}

// 帳票のCSVが共通に使う、期間の名前。
pub(crate) fn period_name(from: time::Date, to: time::Date) -> String {
    format!("{}_{}", format_date(from), format_date(to))
}
