// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createCandidates, chunkIds, confirmInChunks, confirmOutcome, confirmedNos, CONFIRM_LIMIT } from '../js/candidates.js';
import { normalizeHash, notedText, verifyText, ledgerErrorKind } from '../js/ledger-check.js';
import { normalizeInvoice } from '../js/invoice.js';
import { TAX_LABELS } from '../js/account-defaults.js';
import { rowDefaultTax, taxAfterAccount, taxDiffers, touchedOnLoad, taxDiffText, taxChoiceLabel, unsetTaxText, directionWarning, stepOption, rowLabel } from '../js/screens/inbox-rules.js';
import { isTextEntry, isUndoKey } from '../js/screens/inbox-ignore.js';
import { barSpeech } from '../js/screens/inbox-bar.js';

// 標準の科目表と同じ既定の税区分（crates/kamoku-books/src/standard.rs）。
const ACC = {
  comm: { code: 'exp_comm', kind: 'expense', default_tax: 'P10' },
  travel: { code: 'exp_travel', kind: 'expense', default_tax: null },
  sales: { code: 'sales', kind: 'revenue', default_tax: 'S10' },
  misc: { code: 'misc_income', kind: 'revenue', default_tax: null },
  card: { code: 'w_card', kind: 'liability', default_tax: null, wallet: { type: 'card' } },
};

/** 科目を順に選び直したときの税区分（画面の inbox-row.js と同じ順で決まりを当てる）。 */
function pickInOrder(kind, codes, { tax = null, touched = false, exempt = false } = {}) {
  let t = tax;
  let hand = touched;
  return codes.map((name) => {
    const next = ACC[name];
    if (next.wallet) hand = false;
    t = taxAfterAccount({ next, kind, exempt, tax: t, touched: hand });
    return t;
  });
}

test('SC-03 g2-1 入金の行で、科目を通信費→売上高と選び直すと、税区分が売上高の既定（課税売上10%）に入れ直る', () => {
  // 前の決まりでは、通信費の既定（課税仕入10%）が入金の行で選べず空になり、その空が利用者の値とみなされて残った。
  assert.deepEqual(pickInOrder('income', ['comm', 'sales']), [null, 'S10']);
  // 2回変えても追従する（売上高→雑収入→売上高）。
  assert.deepEqual(pickInOrder('income', ['sales', 'misc', 'sales']), ['S10', null, 'S10']);
  // 出金の行でも同じ（売上高を選んでから消耗品費へ）。
  assert.deepEqual(pickInOrder('expense', ['sales', 'comm', 'travel']), [null, 'P10', null]);
});

test('SC-03 g2-1 手で選んだ税区分は、科目を2回変えても残り、科目の既定と違えば「違います」の文を出す', () => {
  assert.deepEqual(pickInOrder('expense', ['comm', 'travel'], { tax: 'P8', touched: true }), ['P8', 'P8']);
  assert.equal(taxDiffers({ account: ACC.comm, kind: 'expense', exempt: false, tax: 'P8' }), true);
  assert.equal(taxDiffers({ account: ACC.comm, kind: 'expense', exempt: false, tax: 'P10' }), false);
  assert.equal(taxDiffText(ACC.comm, 'expense', false), '税区分が、科目の既定（課税仕入10%）と違います。');
  assert.equal(taxDiffText(ACC.travel, 'expense', false), '税区分が、科目の既定（対象外）と違います。');
  // 口座（振替）にすると、手で選んだ値も消える。その後の科目では、また既定に従う。
  assert.deepEqual(pickInOrder('expense', ['card', 'comm'], { tax: 'P8', touched: true }), [null, 'P10']);
  // 免税では、いつも空（税区分の列を隠す）。違いの文も出さない。
  assert.deepEqual(pickInOrder('expense', ['comm', 'sales'], { exempt: true }), [null, null]);
  assert.equal(taxDiffers({ account: ACC.comm, kind: 'expense', exempt: true, tax: 'P8' }), false);
});

test('SC-03 g2-1 科目の既定の税区分は、行の向きで選べる値だけ（入金の行の課税仕入10%は使わない）', () => {
  assert.equal(rowDefaultTax(ACC.comm, 'expense', false), 'P10');
  assert.equal(rowDefaultTax(ACC.comm, 'income', false), null);
  assert.equal(rowDefaultTax(ACC.sales, 'income', false), 'S10');
  assert.equal(rowDefaultTax(ACC.card, 'expense', false), null);
  assert.equal(rowDefaultTax(null, 'expense', false), null);
});

test('SC-03 g2-1 読み込んだ行は、科目の既定と違う税区分（科目が無ければ、空でない税区分）を、手で選んだ値とみなす', () => {
  assert.equal(touchedOnLoad({ account: ACC.comm, kind: 'expense', exempt: false, tax: 'P10' }), false);
  assert.equal(touchedOnLoad({ account: ACC.comm, kind: 'expense', exempt: false, tax: 'P8' }), true);
  // 前の決まりで「対象外」のまま残った売上の行は、違いの文が出る（気づける）。
  assert.equal(touchedOnLoad({ account: ACC.sales, kind: 'income', exempt: false, tax: null }), true);
  assert.equal(touchedOnLoad({ account: null, kind: 'expense', exempt: false, tax: null }), false);
  assert.equal(touchedOnLoad({ account: null, kind: 'expense', exempt: false, tax: 'P8' }), true);
  assert.equal(touchedOnLoad({ account: ACC.comm, kind: 'expense', exempt: true, tax: 'P8' }), false);
});

test('SC-03 g2-4 向きと逆の科目に注意を出す（出金の行に収益、入金の行に費用）。それ以外は出さない', () => {
  assert.match(directionWarning(ACC.sales, 'out'), /^出金の行に、収益の科目が選ばれています。/);
  assert.match(directionWarning(ACC.comm, 'in'), /^入金の行に、費用の科目が選ばれています。/);
  assert.equal(directionWarning(ACC.sales, 'in'), null);
  assert.equal(directionWarning(ACC.comm, 'out'), null);
  assert.equal(directionWarning(ACC.card, 'out'), null);
  assert.equal(directionWarning(null, 'in'), null);
});

test('SC-03 g2-2 税区分の欄の上下の矢印: 選べない仮の値を飛ばし、端で止まる', () => {
  // 先頭は隠した仮の値（科目で決まる）。その後ろに 対象外・課税売上10%・課税売上8%・非課税。
  const opts = [{ disabled: true, hidden: true }, {}, {}, {}, {}];
  assert.equal(stepOption(opts, 0, 1), 1);
  assert.equal(stepOption(opts, 1, 1), 2);
  assert.equal(stepOption(opts, 4, 1), -1);
  assert.equal(stepOption(opts, 1, -1), -1);
  assert.equal(stepOption(opts, 3, -1), 2);
  assert.equal(stepOption([{}, { disabled: true }, {}], 0, 1), 2);
});

test('SC-03 r3-4 税区分の選択肢の名前は、登録・訂正の画面と同じ（TAX_LABELS）。経過措置だけ後ろの括弧を省き、意味は変えない', () => {
  for (const code of Object.keys(TAX_LABELS)) {
    const short = taxChoiceLabel(code);
    // 省くのは末尾の「（免税事業者等から）」だけ（正式な名前の頭と同じ文字列）。
    assert.ok(TAX_LABELS[code].startsWith(short), `${code}: ${short}`);
    assert.equal(TAX_LABELS[code].slice(short.length), code === 'P10X' || code === 'P8X' ? '（免税事業者等から）' : '');
  }
  assert.equal(taxChoiceLabel('P8'), '課税仕入8%（軽減）');
  assert.equal(taxChoiceLabel('S8'), '課税売上8%（軽減）');
  assert.equal(taxChoiceLabel('P10X'), '経過措置の仕入10%');
  assert.equal(taxChoiceLabel(''), '対象外');
  assert.equal(taxChoiceLabel(null), '対象外');
});

test('SC-03 r3-4 空の税区分の仮の値は、確定したときの実際の値（対象外）。既定が対象外の科目なら「対象外（既定）」、科目が無ければ「科目で決まる」', () => {
  assert.deepEqual(unsetTaxText({ account: null, hasAccount: false, kind: 'expense' }), { label: '科目で決まる', title: '科目を選ぶと、科目の既定の税区分が入ります' });
  // 旅費交通費・事業主貸など、既定の税区分が空（勘定科目の画面では「対象外」）の科目。
  assert.deepEqual(unsetTaxText({ account: ACC.travel, hasAccount: true, kind: 'expense' }), { label: '対象外（既定）', title: 'この科目の既定の税区分は「対象外」です。このまま確定すると「対象外」になります。' });
  // 既定（課税仕入10%）が入金の行では選べない科目。既定とは言わない。
  assert.deepEqual(unsetTaxText({ account: ACC.comm, hasAccount: true, kind: 'income' }), { label: '対象外', title: 'この科目の既定の税区分（課税仕入10%）は、入金の行では使えません。このまま確定すると「対象外」になります。' });
  // 科目の一覧に無い科目（無効にした科目など）。既定は分からないが、確定すると対象外。
  assert.deepEqual(unsetTaxText({ account: null, hasAccount: true, kind: 'expense' }), { label: '対象外', title: 'このまま確定すると「対象外」になります。' });
});

test('SC-03 r3-5 確定の通知の番号: 1件なら「No.212」だけ、複数なら小さい番号から大きい番号の範囲', () => {
  assert.equal(confirmedNos([212]), 'No.212');
  assert.equal(confirmedNos([225, 206, 210]), 'No.206〜No.225');
  assert.equal(confirmedNos([]), '');
});

test('SC-03 g2-10 行の読み上げの名前は「日付 摘要 金額」。今年の日付は年を省く', () => {
  assert.equal(rowLabel({ date: '2026-09-18', description: 'デモ電器', amount: 7470 }, '2026'), '09/18 デモ電器 7,470円');
  assert.equal(rowLabel({ date: '2025-12-30', description: '', amount: 120000 }, '2026'), '2025/12/30 摘要なし 120,000円');
});

test('SC-03 g2-7 ⌘Z（Mac）・Ctrl+Z（Mac 以外）で無視を戻す。文字の欄では、欄の取り消しに任せる', () => {
  assert.equal(isUndoKey({ key: 'z', metaKey: true }, true), true);
  assert.equal(isUndoKey({ key: 'z', ctrlKey: true }, true), false);
  assert.equal(isUndoKey({ key: 'z', ctrlKey: true }, false), true);
  assert.equal(isUndoKey({ key: 'Z', metaKey: true, shiftKey: true }, true), false);
  assert.equal(isUndoKey({ key: 'y', metaKey: true }, true), false);
  assert.equal(isTextEntry({ tagName: 'INPUT', type: 'text' }), true);
  assert.equal(isTextEntry({ tagName: 'INPUT', type: 'search' }), true);
  assert.equal(isTextEntry({ tagName: 'TEXTAREA' }), true);
  assert.equal(isTextEntry({ tagName: 'INPUT', type: 'checkbox' }), false);
  assert.equal(isTextEntry({ tagName: 'SELECT' }), false);
  assert.equal(isTextEntry({ tagName: 'BUTTON' }), false);
  assert.equal(isTextEntry(null), false);
});

test('SC-03 g2-5 下の帯の読み上げの文は、件数と入金・出金の合計', () => {
  assert.equal(barSpeech({ count: 3, inTotal: 0, outTotal: 3300 }), '確定する行 3件、入金 0円、出金 3,300円');
});

test('AC-05.3 確定候補の件数と、入金の合計・出金の合計を分けて出す', () => {
  const c = createCandidates();
  c.add({ id: 1, direction: 'out', amount: 1650 });
  c.add({ id: 2, direction: 'out', amount: 420 });
  c.add({ id: 3, direction: 'in', amount: 55000 });
  assert.deepEqual(c.summary(), { count: 3, inTotal: 55000, outTotal: 2070 });
  // 同じ行に2回付けても1件。外すと合計から抜ける。
  c.add({ id: 1, direction: 'out', amount: 1650 });
  assert.equal(c.size, 3);
  assert.equal(c.toggle({ id: 2, direction: 'out', amount: 420 }), false);
  assert.deepEqual(c.summary(), { count: 2, inTotal: 55000, outTotal: 1650 });
  assert.equal(c.toggle({ id: 2, direction: 'out', amount: 420 }), true);
  assert.deepEqual(c.ids().sort(), [1, 2, 3]);
});

test('SC-03 確定は一度に2,000件まで。超えたら2,000件ずつに分けて、送った順のまま送る', () => {
  assert.equal(CONFIRM_LIMIT, 2000);
  const ids = Array.from({ length: 4001 }, (_, i) => i + 1);
  const parts = chunkIds(ids);
  assert.deepEqual(parts.map((p) => p.length), [2000, 2000, 1]);
  assert.deepEqual(parts.flat(), ids);
  assert.deepEqual(chunkIds([]), []);
  assert.deepEqual(chunkIds([1, 2, 3], 2), [[1, 2], [3]]);
});

test('SC-03 分けて送った確定の結果をまとめる。途中で送れなかったら、それまでの結果と失敗の理由を返す', async () => {
  const sent = [];
  const ok = await confirmInChunks([1, 2, 3, 4, 5], async (part) => {
    sent.push(part);
    return { confirmed: part.filter((id) => id !== 4).map((id) => ({ id, no: id + 100 })), failed: part.filter((id) => id === 4).map((id) => ({ id, error: { message: 'だめ' } })) };
  }, 2);
  assert.deepEqual(sent, [[1, 2], [3, 4], [5]]);
  assert.deepEqual(ok.confirmed.map((c) => c.id), [1, 2, 3, 5]);
  assert.deepEqual(ok.failed.map((f) => f.id), [4]);
  assert.equal(ok.error, null);
  const broken = await confirmInChunks([1, 2, 3], async (part) => {
    if (part[0] === 3) throw new Error('通信できません');
    return { confirmed: part.map((id) => ({ id, no: id })), failed: [] };
  }, 2);
  assert.deepEqual(broken.confirmed.map((c) => c.id), [1, 2]);
  assert.equal(broken.error.message, '通信できません');
});

/** 応えを手で返す送り役。呼ばれた組を覚え、resolve と reject を試験の側から押す。 */
function manualSend() {
  const calls = [];
  const send = (part) => new Promise((resolve, reject) => calls.push({ part, resolve, reject }));
  return { calls, send };
}
// 待っている Promise の続きを、全部流し終えるまで待つ。
const settle = () => new Promise((resolve) => setImmediate(resolve));
const done = (part) => ({ confirmed: part.map((id) => ({ id, no: id + 100 })), failed: [] });

// 送り続ける誤りのときに、届かない応えを待って止まらないよう、上限の時間を付ける。
test('SC-03 分けた確定は、前の組の応えが届くまで、次の組を送らない', { timeout: 5000 }, async () => {
  const m = manualSend();
  const result = confirmInChunks([1, 2, 3, 4, 5], m.send, 2);
  await settle();
  assert.equal(m.calls.length, 1, '1組目の応えの前に、2組目を送った');
  m.calls[0].resolve(done([1, 2]));
  await settle();
  assert.equal(m.calls.length, 2, '1組目の応えの後に、2組目を送っていない');
  assert.deepEqual(m.calls[1].part, [3, 4]);
  m.calls[1].resolve({ confirmed: [{ id: 3, no: 103 }], failed: [{ id: 4, error: { code: 'rule', message: 'だめ' } }] });
  await settle();
  assert.equal(m.calls.length, 3);
  m.calls[2].resolve(done([5]));
  const res = await result;
  assert.deepEqual(res.confirmed.map((c) => c.id), [1, 2, 3, 5]);
  assert.deepEqual(res.failed.map((f) => f.id), [4]);
  assert.equal(res.error, null);
});

test('SC-03 途中の組で送れなかったら、後の組は送らず、それまでの結果と理由を返す', { timeout: 5000 }, async () => {
  const m = manualSend();
  const result = confirmInChunks([1, 2, 3, 4, 5], m.send, 2);
  await settle();
  m.calls[0].resolve(done([1, 2]));
  await settle();
  m.calls[1].reject(new Error('通信できません'));
  await settle();
  // 結果を待つ前に数える（後の組を送る誤りなら、その応えは来ないので、待つと止まる）。
  assert.equal(m.calls.length, 2, '失敗した組の後の組を送った');
  const res = await result;
  assert.deepEqual(res.confirmed.map((c) => c.id), [1, 2]);
  assert.deepEqual(res.failed, []);
  assert.equal(res.error.message, '通信できません');
});

test('SC-03 前の組の行ごとの失敗は、後の組で送れなかったときも、理由と一緒に返す（画面は両方を出す）', { timeout: 5000 }, async () => {
  const m = manualSend();
  const result = confirmInChunks([1, 2, 3], m.send, 2);
  await settle();
  m.calls[0].resolve({ confirmed: [], failed: [{ id: 2, error: { code: 'invalid', message: '税区分を見直してください', field: 'tax_code' } }] });
  await settle();
  m.calls[1].reject(new Error('通信できません'));
  const res = await result;
  assert.deepEqual(res.confirmed, []);
  assert.deepEqual(res.failed, [{ id: 2, error: { code: 'invalid', message: '税区分を見直してください', field: 'tax_code' } }]);
  assert.equal(res.error.message, '通信できません');
});

test('SC-03 未確認の全部の行が分かったときは、そこに無い印（ほかで確定・無視された行）を外す', () => {
  const c = createCandidates();
  [1, 2, 3].forEach((id) => c.add({ id, direction: 'out', amount: 100 * id }));
  assert.equal(c.keepOnly(new Set([1, 3, 9])), 1);
  assert.deepEqual(c.ids(), [1, 3]);
  assert.deepEqual(c.summary(), { count: 2, inTotal: 0, outTotal: 400 });
  assert.equal(c.keepOnly(new Set([1, 3])), 0);
});

test('SC-03 確定の結果の仕分け: 確定できた行と、ほかで確定済み・見つからない行は印を外し、直せる失敗だけを行に出す', () => {
  const out = confirmOutcome({
    confirmed: [{ id: 1, no: 101 }],
    failed: [
      { id: 2, error: { code: 'already_confirmed', message: '確定済み' } },
      { id: 3, error: { code: 'not_found', message: '見つかりません' } },
      { id: 4, error: { code: 'missing_account', message: '科目がありません', field: 'account' } },
      { id: 5, error: { code: 'invalid', message: '税区分', field: 'tax_code' } },
    ],
  });
  assert.deepEqual(out.unmark, [1, 2, 3]);
  assert.deepEqual(out.gone.map((f) => f.id), [2, 3]);
  assert.deepEqual(out.fixable.map((f) => f.id), [4, 5]);
  assert.deepEqual(confirmOutcome({ confirmed: [], failed: [] }), { unmark: [], gone: [], fixable: [] });
});

test('SC-03 ちょうど2,000件は1回で送る。2,001件は2,000件と1件。0件は送らない', async () => {
  const sizes = [];
  const send = async (part) => {
    sizes.push(part.length);
    return done(part);
  };
  const ids = (n) => Array.from({ length: n }, (_, i) => i + 1);
  assert.equal((await confirmInChunks(ids(2000), send)).confirmed.length, 2000);
  assert.deepEqual(sizes, [2000]);
  sizes.length = 0;
  await confirmInChunks(ids(2001), send);
  assert.deepEqual(sizes, [2000, 1]);
  sizes.length = 0;
  assert.deepEqual(await confirmInChunks([], send), { confirmed: [], failed: [], error: null });
  assert.deepEqual(sizes, []);
});

test('AC-09.2 控えたハッシュは、空白・改行・大文字を許して64桁の16進にそろえる', () => {
  const h = 'AB'.repeat(32);
  assert.deepEqual(normalizeHash(` ${h.slice(0, 30)}\n${h.slice(30)} `), { ok: true, value: h.toLowerCase() });
  assert.match(normalizeHash('').error, /入れてください/);
  assert.match(normalizeHash('xyz').error, /0〜9 と a〜f/);
  assert.match(normalizeHash('ab12').error, /64文字です（いまは4文字）/);
});

test('AC-09.2 照合の結果の文: 何番目にあたるか、末尾か、見つからないか', () => {
  assert.match(notedText({ found: true, seq: 128, is_head: true }, 128).text, /128番目の記録と一致しました（末尾と同じです）/);
  const mid = notedText({ found: true, seq: 100, is_head: false }, 128).text;
  assert.match(mid, /100番目の記録と一致しました（末尾ではありません）/);
  assert.match(mid, /28件の記録が足されています/);
  assert.equal(notedText({ found: false, seq: null, is_head: false }, 128).found, false);
});

test('AC-09.1 検査の結果の文: 壊れていれば最初の連番と規則を示す', () => {
  assert.equal(verifyText({ ok: true, events: 80 }).ok, true);
  const bad = verifyText({ ok: false, error: { seq: 55, kind: 'bad_hash', rule: null } });
  assert.match(bad.body, /55番目の記録に、ハッシュの不一致/);
  assert.match(verifyText({ ok: false, error: { seq: 7, kind: 'bad_rule', rule: 'unbalanced' } }).body, /決まり: unbalanced/);
});

test('AC-09.1 壊れの種類は、本物のサーバーの値（保存の層）を日本語にし、知らない値でも英字を出さない', () => {
  const kinds = { bad_hash: 'ハッシュの不一致', broken_link: '前の記録とのつながりの切れ', wrong_sequence: '連番の飛びや入れ違い', rule_violation: '記録の決まりへの違反', quick_check: 'データベースのファイルの傷み' };
  for (const [kind, text] of Object.entries(kinds)) assert.equal(ledgerErrorKind(kind), text);
  assert.equal(ledgerErrorKind('something_new'), '記録の不整合');
  assert.equal(ledgerErrorKind(undefined), '記録の不整合');
  const body = verifyText({ ok: false, error: { seq: 12, kind: 'broken_link', rule: null } }).body;
  assert.match(body, /12番目の記録に、前の記録とのつながりの切れ/);
  assert.doesNotMatch(verifyText({ ok: false, error: { seq: 3, kind: 'something_new', rule: null } }).body, /[a-z_]{4,}/);
});

test('SC-09 登録番号は全角・小文字・ハイフンを許して T+13桁にそろえ、形が違えば理由を返す', () => {
  assert.deepEqual(normalizeInvoice('ｔ１２３４－５６７８９０１２３'), { ok: true, value: 'T1234567890123' });
  assert.deepEqual(normalizeInvoice(''), { ok: true, value: null });
  assert.match(normalizeInvoice('T12345').error, /T と13桁/);
  assert.match(normalizeInvoice('1234567890123').error, /T と13桁/);
  // 桁の数の境目（12桁と14桁）も断る。
  assert.match(normalizeInvoice('T123456789012').error, /T と13桁/);
  assert.match(normalizeInvoice('T12345678901234').error, /T と13桁/);
});
