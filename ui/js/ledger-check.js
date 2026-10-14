// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 台帳の検査と、控えたハッシュの照合の結果を、日本語の文にする（SC-10 のデータ、AC-09.1、AC-09.2）。

// 壊れの種類（サーバーの保存の層の値）。bad_seq と bad_rule は、前の模擬サーバーの値（同じ意味で読む）。
const KIND = {
  bad_hash: 'ハッシュの不一致',
  broken_link: '前の記録とのつながりの切れ',
  wrong_sequence: '連番の飛びや入れ違い',
  rule_violation: '記録の決まりへの違反',
  quick_check: 'データベースのファイルの傷み',
  bad_seq: '連番の飛びや入れ違い',
  bad_rule: '記録の決まりへの違反',
};

/** 壊れの種類を日本語にする。知らない値でも英字を出さない（読み取り専用の帯と、検査の結果で使う）。 */
export function ledgerErrorKind(kind) {
  return KIND[kind] || '記録の不整合';
}

/**
 * 打った（貼った）ハッシュを、比べられる形にそろえる。空白と改行を除き、小文字にする。
 * @returns {{ ok: true, value: string } | { ok: false, error: string }}
 */
export function normalizeHash(text) {
  const v = String(text ?? '').normalize('NFKC').replace(/\s+/g, '').toLowerCase();
  if (!v) return { ok: false, error: '控えたハッシュを入れてください。' };
  if (!/^[0-9a-f]+$/.test(v)) return { ok: false, error: 'ハッシュに使えるのは、0〜9 と a〜f の文字だけです。写し間違いがないか確かめてください。' };
  if (v.length !== 64) return { ok: false, error: `ハッシュは64文字です（いまは${v.length}文字）。全文を貼り付けてください。` };
  return { ok: true, value: v };
}

/**
 * POST /verify の結果を、検査の結果の文にする。
 * @param {{ ok: boolean, events?: number, error?: { seq: number, kind: string, rule: string|null } }} res
 * @returns {{ ok: boolean, title: string, body: string }}
 */
export function verifyText(res) {
  if (res.ok) {
    return { ok: true, title: '台帳は壊れていません', body: `${res.events}件の記録を最初から順にたどり、すべてのハッシュがつながっていることを確かめました。` };
  }
  const e = res.error || {};
  const what = ledgerErrorKind(e.kind);
  const rule = e.rule ? `（決まり: ${e.rule}）` : '';
  return { ok: false, title: '台帳に壊れた記録があります', body: `${e.seq}番目の記録に、${what}が見つかりました${rule}。それより後の記録は信用できません。自動では直さないので、バックアップと見比べてください。` };
}

/**
 * 控えたハッシュの照合の結果を、文にする。
 * @param {{ found: boolean, seq: number|null, is_head: boolean }} noted
 * @param {number} events いまの記録の件数
 * @returns {{ found: boolean, text: string }}
 */
export function notedText(noted, events) {
  if (!noted || !noted.found) {
    return { found: false, text: 'この台帳には、控えたハッシュと一致する記録がありません。写し間違いがないか、別の帳簿のハッシュでないかを確かめてください。どちらでもなければ、控えた後に記録が書き換えられたおそれがあります。' };
  }
  if (noted.is_head) return { found: true, text: `台帳の${noted.seq}番目の記録と一致しました（末尾と同じです）。控えた後に、記録は増えていません。` };
  const after = Math.max(events - noted.seq, 0);
  return { found: true, text: `台帳の${noted.seq}番目の記録と一致しました（末尾ではありません）。控えた後に、${after}件の記録が足されています。` };
}
