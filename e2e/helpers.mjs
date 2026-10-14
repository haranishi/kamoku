// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 試験で共通に使う手順。本物のサーバーで動かすときは KAMOKU_TOKEN に起動時の合言葉を入れる（Cookieを受け取るため）。

import { expect } from '@playwright/test';

/** 画面を開き、本文の見出しが出るまで待つ。 */
export async function open(page, hash = '#/') {
  if (process.env.KAMOKU_TOKEN && !page.kamokuAuthed) {
    await page.goto(`/?t=${process.env.KAMOKU_TOKEN}`);
    page.kamokuAuthed = true;
  }
  await page.goto(`/${hash}`);
  await expect(page.locator('main h1')).toBeVisible();
}

/** 操作の試験のサーバー（KAMOKU_URL があれば本物、無ければ模擬サーバー）。 */
const FLOWS_ORIGIN = process.env.KAMOKU_URL ? new URL(process.env.KAMOKU_URL).origin : 'http://127.0.0.1:8766';

/**
 * /api/v1 を直接呼ぶ（準備と後片付け用）。変更の要求には、自分のオリジンを Origin に付ける。
 * まだ画面を開いていなければ、操作の試験のサーバーへ送る。本物のサーバーなら、先に合言葉で入る（Cookie が要る）。
 */
export async function apiCall(page, method, path, body) {
  if (!page.url().startsWith('http') && process.env.KAMOKU_TOKEN && !page.kamokuAuthed) {
    await page.goto(`${FLOWS_ORIGIN}/?t=${process.env.KAMOKU_TOKEN}`);
    page.kamokuAuthed = true;
  }
  const origin = page.url().startsWith('http') ? new URL(page.url()).origin : FLOWS_ORIGIN;
  const res = await page.request.fetch(`${origin}/api/v1${path}`, {
    method,
    data: body === undefined ? undefined : body,
    headers: method === 'GET' ? {} : { Origin: origin, 'Content-Type': 'application/json' },
  });
  const status = res.status();
  return { status, json: status === 204 ? null : await res.json() };
}

/** 設定を一時的に変えて fn を動かし、最後に元へ戻す。 */
export async function withSettings(page, patch, fn) {
  const before = (await apiCall(page, 'GET', '/settings')).json;
  await apiCall(page, 'PUT', '/settings', { ...before, ...patch });
  try {
    await fn();
  } finally {
    await apiCall(page, 'PUT', '/settings', before);
  }
}

/**
 * 科目の既定の税区分と割合を一時的に変えて fn を動かし、最後に元へ戻す。
 * 模擬サーバーと本物の合成データで既定値が違う（通信費の割合が70と100）ので、試験が前提にする値は自分で入れる。
 */
export async function withAccountDefaults(page, code, patch, fn) {
  const before = (await apiCall(page, 'GET', '/accounts?include_disabled=1')).json.items.find((a) => a.code === code);
  await apiCall(page, 'PATCH', `/accounts/${code}`, patch);
  try {
    await fn();
  } finally {
    await apiCall(page, 'PATCH', `/accounts/${code}`, { default_tax: before.default_tax, default_pct: before.default_pct });
  }
}

/** 日本の今日（YYYY-MM-DD）。 */
export function todayTokyo() {
  return new Intl.DateTimeFormat('sv-SE', { timeZone: 'Asia/Tokyo' }).format(new Date());
}

/** 現金で払った支出の仕訳を、APIで1つ作る（試験の準備用）。 */
export async function addCashExpense(page, { description, amount = 500, account = 'exp_supplies', tax_code = 'P10', business_pct = 100 }) {
  const cash = await walletOf(page, 'cash');
  const res = await apiCall(page, 'POST', '/entries', { kind: 'expense', date: todayTokyo(), amount, account, wallet: cash.code, tax_code, partner: '', description, business_pct, memo: '' });
  if (res.status !== 201) throw new Error(`仕訳を作れません: ${JSON.stringify(res.json)}`);
  return res.json;
}

/** 文字で探す試験のために、その文字を摘要に含む仕訳が無ければ1つ作る（合成データに頼らないため）。 */
export async function ensureEntryWith(page, word) {
  if ((await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(word)}&state=active&limit=1`)).json.total > 0) return;
  await addCashExpense(page, { description: `${word}（E2E）`, amount: 66000, account: 'exp_rent', tax_code: 'NT', business_pct: 50 });
}

/** 試験ごとに重ならない摘要（ほかの試験の仕訳と区別するため）。 */
export function uniqueText(prefix) {
  return `${prefix} ${Date.now().toString(36)}`;
}

/** 取引の登録の画面の欄。 */
export function entryFields(page) {
  const form = page.locator('form.entry-form');
  return {
    form,
    kind: (name) => page.getByRole('radio', { name }),
    date: form.getByLabel('日付'),
    amount: form.getByLabel('金額'),
    account: form.getByRole('combobox', { name: /科目/ }),
    wallet: form.getByRole('combobox', { name: /口座/ }).last(),
    tax: form.getByLabel('税区分'),
    partner: form.getByRole('combobox', { name: /取引先/ }),
    description: form.getByLabel('摘要'),
    pct: form.getByLabel('事業の割合'),
    preview: page.locator('.pv-card'),
  };
}

/** いま開いているページのオリジン（API を直に呼ぶため）。開く前なら操作の試験のサーバー。 */
export function originOf(page) {
  return page.url().startsWith('http') ? new URL(page.url()).origin : FLOWS_ORIGIN;
}

/** 型で口座を1つ探す（本物のサーバーでも、合成データの名前に頼らないため）。 */
export async function walletOf(page, type) {
  const items = (await apiCall(page, 'GET', '/accounts')).json.items;
  return items.find((a) => a.wallet && a.wallet.type === type && a.enabled);
}

/** 試験ごとに新しい口座を作る（取込の重複の判定が、前の試験の取込と混ざらないように）。 */
export async function newWallet(page, type, name) {
  const parent = { cash: 'cash', ewallet: 'cash', bank: 'dep_other', card: 'accrued' }[type];
  const res = await apiCall(page, 'POST', '/accounts', { name, reading: '', kind: type === 'card' ? 'liability' : 'asset', parent, wallet: { type, institution: '' }, default_tax: null, default_pct: 100 });
  if (res.status !== 201) throw new Error(`口座を作れません: ${JSON.stringify(res.json)}`);
  return res.json;
}

/** バイト列のCSVを送り、読み取りの結果（upload の応え）を返す。 */
export async function uploadCsv(page, wallet, bytes, name = 'e2e.csv', encoding = 'auto') {
  const origin = originOf(page);
  const res = await page.request.fetch(`${origin}/api/v1/imports/upload?name=${encodeURIComponent(name)}&encoding=${encoding}&wallet=${wallet}`, {
    method: 'POST',
    data: bytes,
    headers: { Origin: origin, 'Content-Type': 'application/octet-stream' },
  });
  return { status: res.status(), json: await res.json() };
}

/**
 * 合成の明細を取り込んで、未確認の行を作る。rows は [日付, 摘要, 出金, 入金] の配列。
 * 画面の試験の準備に使う（取込の画面そのものの試験は import.spec.mjs）。
 */
export async function importRows(page, wallet, rows) {
  const text = ['日付,摘要,出金,入金', ...rows.map((r) => r.join(','))].join('\r\n');
  const up = await uploadCsv(page, wallet, Buffer.from(text, 'utf8'));
  if (up.status !== 200) throw new Error(`取り込めません: ${JSON.stringify(up.json)}`);
  const body = { upload_id: up.json.upload_id, wallet, mapping: up.json.guess };
  const res = await apiCall(page, 'POST', '/imports/commit', { ...body, save_profile: false });
  if (res.status !== 200) throw new Error(`取り込めません: ${JSON.stringify(res.json)}`);
  return res.json;
}

/** 初期設定の試験用のサーバー（空の帳簿）。本物のサーバーで試すときは KAMOKU_SETUP_URL と KAMOKU_SETUP_TOKEN。 */
export const SETUP_URL = process.env.KAMOKU_SETUP_URL || 'http://127.0.0.1:8768';

/** 空の帳簿のサーバーを開く（合言葉があれば先に渡す）。 */
export async function openSetup(page, hash = '#/', base = SETUP_URL, token = process.env.KAMOKU_SETUP_TOKEN) {
  if (token && !page.kamokuSetupAuthed) {
    await page.goto(`${base}/?t=${token}`);
    page.kamokuSetupAuthed = true;
  }
  await page.goto(`${base}/${hash}`);
  await expect(page.locator('main h1')).toBeVisible();
}
