// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録で、欄の下やプレビューに出す一言の決まり（DOM不要。単体試験で確かめる）。
// どれも、黙って入った値や、読み違えやすい値に、利用者が気づけるようにするためのもの。値そのものは変えない。

import { addDays, normalizeWidth } from '../input-parse.js';

export const PCT_ERROR = '事業の割合は、0から100の整数で入れてください。';

/** 事業の割合の欄を読む。0〜100の整数なら数、それ以外（空、小数、「5e1」など）は null。 */
export function parsePct(text) {
  const t = normalizeWidth(text).trim();
  if (!/^\d{1,3}$/.test(t)) return null;
  const n = Number(t);
  return n <= 100 ? n : null;
}

/** 科目の既定の割合が100%未満なら、その出どころ（黙って30%などが入った訳）を伝える一言。 */
export function pctSourceNote(account) {
  if (!account || account.wallet) return '';
  const pct = account.default_pct ?? 100;
  return pct < 100 ? `この科目の既定の割合は、事業${pct}%です。「勘定科目」の画面で変えられます。` : '';
}

/**
 * 課税事業者が、既定の税区分が「対象外」の費用の科目を選び、税区分も「対象外」のままのときの一言。
 * 標準の科目表の既定（旅費交通費などは対象外）は税の判断を含むので変えず、ここで気づけるようにする。
 */
export function taxSourceNote({ taxable, kind, account, taxCode }) {
  if (!taxable || kind !== 'expense' || !account || account.wallet || account.kind !== 'expense') return '';
  if ((account.default_tax ?? null) !== null || (taxCode ?? null) !== null) return '';
  return 'この科目の既定は「対象外」です。課税の取引なら、税区分を選んでください。';
}

/** 「2030-01-01」を「2030年1月1日」に。 */
export function jpDate(iso) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''));
  return m ? `${Number(m[1])}年${Number(m[2])}月${Number(m[3])}日` : String(iso || '');
}

/**
 * 保存の前に確かめる日付（年の打ち間違いなど）。今日より31日を超えて先か、開業日より前なら、確認の文を返す。無ければ空。
 * @param {string} date YYYY-MM-DD
 * @param {{ today: string, openedOn?: string }} p
 */
export function dateConcern(date, { today, openedOn }) {
  const iso = /^\d{4}-\d{2}-\d{2}$/;
  if (!iso.test(date || '')) return '';
  if (iso.test(today || '') && date > addDays(today, 31)) return `${jpDate(date)}の取引として保存します。今日より31日を超えて先の日付です。年は合っていますか。`;
  if (iso.test(openedOn || '') && date < openedOn) return `${jpDate(date)}の取引として保存します。開業日（${jpDate(openedOn)}）より前の日付です。年は合っていますか。`;
  return '';
}

/**
 * 締めた日（設定の locked_until）以前の日付への登録・訂正の断り。締めていない、または当たらなければ空。
 * サーバーの決まり（kamoku-store の posting.rs）と同じ: 日付が締めた日「以前」なら断る。訂正は、元の日付が締めた日以前でも断る。
 * 締めた日が文に無く、保存して初めて断られた（2026-10-15 の体験の評価 E3）ので、欄を確定したときにも出し、断りの文にも日付を入れる。
 * @param {{ date: string|null, lockedUntil?: string|null, correcting?: boolean, originalDate?: string|null, onSave?: boolean }} p
 *   onSave: 保存してサーバーに断られたときの文（直し方まで書く）
 */
export function lockedMessage({ date, lockedUntil, correcting = false, originalDate = null, onSave = false }) {
  const iso = /^\d{4}-\d{2}-\d{2}$/;
  if (!iso.test(lockedUntil || '')) return '';
  const head = `${jpDate(lockedUntil)}まで締めてあるため、`;
  if (correcting && iso.test(originalDate || '') && originalDate <= lockedUntil) {
    return `${head}${jpDate(originalDate)}の仕訳は訂正できません。${onSave ? '締めた日は、設定の「締め」で見直せます。' : ''}`;
  }
  if (!iso.test(date || '') || date > lockedUntil) return '';
  return `${head}この日付には${correcting ? '訂正' : '登録'}できません。${onSave ? '日付を直すか、設定の「締め」で締めた日を見直してください。' : ''}`;
}

/**
 * 仕訳の詳細の板で、締めた日以前の仕訳に出す文。訂正も取消もできない（サーバーの決まり。日付は直せない）ので、開いた時点で伝え、
 * ボタンを止める。理由を書いて確定してから断られ、断る文の「日付を直すか」が取消に合わなかった（2026-10-15 の体験の評価 E3）。
 * 締めていない、または当たらなければ空。
 */
export function lockedDetailNote(entryDate, lockedUntil) {
  const iso = /^\d{4}-\d{2}-\d{2}$/;
  if (!iso.test(lockedUntil || '') || !iso.test(entryDate || '') || entryDate > lockedUntil) return '';
  return `${jpDate(lockedUntil)}まで締めてあるため、この仕訳は訂正も取消もできません。締めた日は、設定の「締め」で見直せます。`;
}

/**
 * 明細のメモ。事業主貸の「家事按分 30%」は私用の分の割合なので、事業の割合（70%）と取り違えないよう「私用の分 30%」と言い換える。
 * サーバーの応えの文字はそのままにして、画面に出すときだけ組み替える。
 */
export function lineMemo(line) {
  if (!line) return '';
  const m = line.account === 'owner_draw' ? /^家事按分 (\d{1,3})%$/.exec(line.memo || '') : null;
  return m ? `私用の分 ${m[1]}%` : line.memo || '';
}
