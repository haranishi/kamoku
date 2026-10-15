// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込（SC-04、AC-04.3）。合成のCSV（crates/kamoku-import/tests/fixtures/synthetic/）で3段階を通す。
// 同じファイルをもう一度取り込むと重複と出て、行ごとに「この行も取り込む」を選べる。文字コードの選び直し、取引番号の列、期限切れ、壊れたCSV。
// 取込の重複は口座ごとなので、試験ごとに新しい口座を作る（前の実行の取込と混ざらないように）。

import { test, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { open, uniqueText, newWallet, apiCall } from './helpers.mjs';

const FIXTURES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'crates', 'kamoku-import', 'tests', 'fixtures', 'synthetic');
const fixture = (name) => path.join(FIXTURES, name);

async function start(page, file, type = 'bank') {
  await open(page, '#/');
  const w = await newWallet(page, type, uniqueText('E2E 取込口座'));
  await open(page, '#/import');
  await page.getByLabel('取り込む口座').selectOption(w.code);
  await page.locator('input[type="file"]').setInputFiles(fixture(file));
  await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  return w;
}

test('AC-04.3 UTF-8 の明細（入金と出金が別の列）を3段階で取り込み、同じファイルをもう一度取り込むと重複と出る', async ({ page }) => {
  const w = await start(page, 'utf8.csv');
  const card = page.locator('.import-card');
  // 判定した文字コードは、文字コードの選択欄の「自動で判定（…）」で見せる（2026-10-15 の第3回で、横に並んでいた同じ文字を1つにした）。
  await expect(page.getByLabel('文字コード').locator('option:checked')).toHaveText('自動で判定（UTF-8）');
  await expect(card).toContainText('見出しの言葉から、列の対応を推測しました');
  await expect(page.getByLabel('日付の列')).toHaveValue('0');
  await expect(page.getByRole('radio', { name: '入金と出金が別の列' })).toBeChecked();
  await expect(page.getByLabel('出金の列')).toHaveValue('2');
  await expect(page.getByLabel('入金の列')).toHaveValue('3');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.getByRole('heading', { name: '3. 確かめて取り込む' })).toBeVisible();
  await expect(page.locator('.check-counts')).toContainText('新規4件');
  await expect(page.locator('.check-counts')).toContainText('重複0件');
  await page.getByRole('button', { name: '4件を取り込む' }).click();
  await expect(page.locator('.import-done')).toContainText('4件を未確認に入れました');
  await page.locator('.import-done').getByRole('link', { name: /未確認を開く/ }).click();
  await expect(page).toHaveURL(new RegExp(`#/inbox\\?wallet=${w.code}`));
  await expect(page.locator('tr.inbox-row')).toHaveCount(4);

  // 同じファイルをもう一度。口座の列の対応を覚えているので、その旨が出る。全部が重複になる。
  await page.goto(`/#/import?wallet=${w.code}`);
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await expect(page.locator('.import-card')).toContainText('この口座で前に使った列の対応を入れました');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規0件');
  await expect(page.locator('.check-counts')).toContainText('重複4件');
  // 全部が重複なら、冒頭に1行で言い、行ごとの理由は短くする（4行すべてに同じ長い文を並べない。2026-10-15 の直し g2 の8）。
  await expect(page.locator('.check-all-dup')).toHaveText('このファイルの4件は、すべて取り込み済みです。');
  await expect(page.locator('.check-table .reason-text')).toHaveText(Array(4).fill('取り込み済みの明細と同じです。'));
  await expect(page.locator('.check-summary')).toBeHidden();
  const commit = page.getByRole('button', { name: /件を取り込む/ });
  await expect(commit).toBeDisabled();
  // 重複の行も、選べば取り込める（既定は取り込まない）。選ぶと、取り込む行の期間と合計に入る。
  await page.getByRole('checkbox', { name: /^ファイルの5行目も取り込む/ }).check();
  await expect(commit).toHaveText(/1件を取り込む/);
  await expect(page.locator('.check-summary')).toHaveText('取り込む1件: 2026-10-05・入金 1件 55,000円');
  await commit.click();
  await expect(page.locator('.import-done')).toContainText('1件を未確認に入れました');
  await expect(page.locator('.import-done')).toContainText('重複 3件');
  const drafts = (await apiCall(page, 'GET', `/drafts?wallet=${w.code}&limit=50`)).json;
  expect(drafts.total).toBe(5);
});

test('AC-04.3 Shift_JIS の明細（元号の日付・機種依存文字）を読み、文字コードを選び直すと読み直す', async ({ page }) => {
  await start(page, 'sjis.csv');
  const card = page.locator('.import-card');
  const enc = page.getByLabel('文字コード');
  await expect(enc.locator('option:checked')).toHaveText('自動で判定（Shift_JIS）');
  await expect(page.locator('.sample-table')).toContainText('デモ商店①');
  await expect(page.locator('.sample-table')).toContainText('令和8年10月2日');
  // UTF-8 を選ぶと、読めない文字があると出る（選び直しが効いている）。自分で選んだときは、自動の選択肢に判定を添えない。
  await enc.selectOption('utf-8');
  await expect(card.locator('.banner')).toContainText('読めない文字がありました');
  await expect(enc.locator('option:checked')).toHaveText('UTF-8');
  await expect(enc.locator('option[value="auto"]')).toHaveText('自動で判定');
  // 文字コードの文字は、選択欄の外にもう1つ出さない（同じことを2回言わない）。
  await expect(card.locator('.enc-dd')).toHaveText(/^自動で判定UTF-8Shift_JIS$/);
  await page.getByLabel('文字コード').selectOption('shift_jis');
  await expect(card.locator('.banner')).toHaveCount(0);
  await expect(page.locator('.sample-table')).toContainText('デモ商店①');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規3件');
  const table = page.locator('.check-table');
  await expect(table).toContainText('2026-10-01');
  await expect(table).toContainText('2026-10-02');
  await expect(table).toContainText('+300,000');
});

test('AC-04.3 符号つきの1列は、入金の向きを直せる（正の数が入金）', async ({ page }) => {
  await start(page, 'signed.csv');
  await expect(page.getByRole('radio', { name: '符号つきの1列' })).toBeChecked();
  await expect(page.getByLabel('金額の列')).toHaveValue('2');
  // 推測はカードの明細の向き（正の数が出金）。銀行の明細なので直す。
  await expect(page.getByRole('radio', { name: /正の数が出金/ })).toBeChecked();
  // 切り替えの選択肢は、見た目のラベルを押して選ぶ（中の入力欄は見えない）。
  await page.locator('.seg-option', { hasText: '正の数が入金' }).click();
  await expect(page.getByRole('radio', { name: /正の数が入金/ })).toBeChecked();
  await page.getByRole('button', { name: '確かめる' }).click();
  const rows = page.locator('.check-table tbody tr');
  await expect(rows.filter({ hasText: 'サンプル給与' })).toContainText('+300,000');
  await expect(rows.filter({ hasText: '手数料 デモ' })).toContainText('-220');
});

test('SC-04 取引番号の列（任意）を選ぶと、摘要が変わっていても同じ番号の行を重複と見分ける', async ({ page }) => {
  // 番号の列がある明細を、試験の中で作る（合成の固定ファイルには番号の列が無いため）。
  const csv = (desc) => ({ name: 'with-ref.csv', mimeType: 'text/csv', buffer: Buffer.from(`日付,摘要,出金,入金,取引番号\n2026/10/01,${desc} 1,1650,,A-0001\n2026/10/02,${desc} 2,420,,A-0002\n`) });
  await open(page, '#/');
  const w = await newWallet(page, 'bank', uniqueText('E2E 取込番号'));
  const load = async (desc) => {
    // 取り込んだ後もアドレスが同じなので、読み込み直して最初の段階から始める（# だけの移動では画面が変わらない）。
    await page.goto(`/#/import?wallet=${w.code}`);
    await page.reload();
    await page.locator('input[type="file"]').setInputFiles(csv(desc));
    await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  };
  await load('サンプル店');
  // 番号の列は推測しない（既定は「使わない」）。
  const idSel = page.getByLabel('取引番号の列（任意）');
  await expect(idSel).toHaveValue('');
  await idSel.selectOption('4');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規2件');
  await page.getByRole('button', { name: '2件を取り込む' }).click();
  await expect(page.locator('.import-done')).toContainText('2件を未確認に入れました');
  // 摘要だけ変えた同じ番号の明細。保存した対応（番号の列）で、2行とも重複になる。
  await load('デモ店');
  await expect(idSel).toHaveValue('4');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規0件');
  await expect(page.locator('.check-counts')).toContainText('重複2件');
});

test('SC-04 口座が選ばれるまではファイルを置けない。壊れたCSVは行の番号つきで断る', async ({ page }) => {
  await open(page, '#/import');
  const pick = page.getByRole('button', { name: 'ファイルを選ぶ' });
  await expect(pick).toBeDisabled();
  await expect(page.locator('.drop-zone')).toContainText('先に、上で口座を選んでください');
  // 口座を選ばずに置こうとすると、理由を出して受け取らない。
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await expect(page.locator('.import-step .field-error').last()).toContainText('先に口座を選んでください');
  await expect(page.getByRole('heading', { name: '1. 口座とファイル' })).toBeVisible();
  const w = await newWallet(page, 'bank', uniqueText('E2E 取込壊れ'));
  await page.reload();
  await page.getByLabel('取り込む口座').selectOption(w.code);
  await expect(pick).toBeEnabled();
  const failed = page.waitForResponse((r) => r.url().includes('/api/v1/imports/upload'));
  await page.locator('input[type="file"]').setInputFiles(fixture('broken_quote.csv'));
  // サーバーは invalid_csv と行の番号（line）で断る。文面は模擬サーバーと本物で違うので、画面は行の番号が出ることを見る。
  const res = await failed;
  const body = await res.json();
  expect([res.status(), body.error.code, body.error.line]).toEqual([400, 'invalid_csv', 3]);
  await expect(page.locator('.import-step .field-error').last()).toContainText('3行目');
});

test('SC-04 r3-1 「確かめる」の後に「戻る」を押すと、自分で変えた列の対応が残る（ファイルを選び直したときだけ推測に戻る）', async ({ page }) => {
  await start(page, 'utf8.csv');
  const out = page.getByLabel('出金の列');
  const balanceDesc = page.getByRole('checkbox', { name: '5列目 残高' });
  await expect(out).toHaveValue('2');
  await expect(balanceDesc).not.toBeChecked();
  // E2 の再現と同じく、出金の列を「5列目 残高」に変える。摘要の列にも1つ足す（選択欄とチェックの両方が残ることを見る）。
  await out.selectOption('4');
  await balanceDesc.check();
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.getByRole('heading', { name: '3. 確かめて取り込む' })).toBeVisible();
  await page.getByRole('button', { name: '戻る' }).click();
  await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  await expect(out).toHaveValue('4');
  await expect(balanceDesc).toBeChecked();
  await expect(page.getByLabel('入金の列')).toHaveValue('3');
  await expect(page.locator('.import-source')).toHaveText('直前に選んだ列の対応のままです。直したら、もう一度「確かめる」を押してください。');
  // もう一度「確かめる」→「戻る」でも残る。
  await page.getByRole('button', { name: '確かめる' }).click();
  await page.getByRole('button', { name: '戻る' }).click();
  await expect(out).toHaveValue('4');
  // 最初の段階へ戻ってファイルを選び直すと、推測から始める（直前の対応は、そのファイルのものなので使わない）。
  await page.getByRole('button', { name: '戻る' }).click();
  await expect(page.getByRole('heading', { name: '1. 口座とファイル' })).toBeVisible();
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  await expect(out).toHaveValue('2');
  await expect(balanceDesc).not.toBeChecked();
  await expect(page.locator('.import-source')).toContainText('見出しの言葉から、列の対応を推測しました');
});

/** 40行の銀行の明細（E2 の statement-40.csv と同じ形。見出しは「お支払金額」「お預り金額」）。 */
function statement40() {
  const lines = ['日付,摘要,お支払金額,お預り金額,残高'];
  for (let i = 0; i < 40; i += 1) lines.push(`2026/09/${String((i % 30) + 1).padStart(2, '0')},サンプル店 ${i + 1},${1000 + i},,${500000 - i * 1000}`);
  return { name: 'statement-40.csv', mimeType: 'text/csv', buffer: Buffer.from(lines.join('\r\n'), 'utf8') };
}

/** 下に付く帯の位置: 帯の上と下、画面の下の端（幅767px以下は下のバーの上の端）。 */
async function barPlace(page) {
  return page.evaluate(() => {
    const r = document.querySelector('.import-actions').getBoundingClientRect();
    const nav = document.querySelector('.bottomnav');
    const edge = nav && nav.getBoundingClientRect().height > 0 ? nav.getBoundingClientRect().top : window.innerHeight;
    return { top: r.top, bottom: r.bottom, edge };
  });
}

for (const [width, height] of [[1440, 900], [390, 844]]) {
  test(`SC-04 r3-3 幅${width}: 40行の明細でも、(2)の「確かめる」と(3)の「40件を取り込む」は画面の下に付いた帯にあり、送っても見えている`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, '#/');
    const w = await newWallet(page, 'bank', uniqueText('E2E 取込帯'));
    await open(page, `#/import?wallet=${w.code}`);
    await page.locator('input[type="file"]').setInputFiles(statement40());
    await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
    const next = page.getByRole('button', { name: '確かめる' });
    await expect(next).toBeInViewport({ ratio: 1 });
    let place = await barPlace(page);
    expect(Math.abs(place.bottom - place.edge)).toBeLessThan(1);
    // 通知は、帯に重ならない（data-bottom-bar を見て持ち上がる）。文字コードを選び直したときの通知で見る。
    await page.getByLabel('文字コード').selectOption('utf-8');
    const notice = page.locator('.toast').filter({ hasText: '文字コードを変えて、読み直しました。' });
    await expect(notice).toBeVisible();
    await expect
      .poll(async () => {
        const [a, b] = await Promise.all([notice.boundingBox(), page.locator('.import-actions').boundingBox()]);
        return a && b && a.y < b.y + b.height && a.y + a.height > b.y && a.x < b.x + b.width && a.x + a.width > b.x ? 'overlap' : 'clear';
      })
      .toBe('clear');
    await next.click();
    await expect(page.getByRole('heading', { name: '3. 確かめて取り込む' })).toBeVisible();
    const commit = page.getByRole('button', { name: '40件を取り込む' });
    await expect(commit).toBeInViewport({ ratio: 1 });
    // 表の途中まで送っても、帯は画面の下の端に付いたまま。
    await page.evaluate(() => window.scrollTo(0, 1200));
    await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(600);
    place = await barPlace(page);
    expect(Math.abs(place.bottom - place.edge)).toBeLessThan(1);
    await expect(commit).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole('button', { name: '戻る' })).toBeInViewport({ ratio: 1 });
    // 押せる（ほかの要素に覆われていない）ことだけを確かめ、取り込みはしない。通し試験は1つの帳簿を全部の試験で使うので、
    // 40行を2回取り込むと、後の試験の前提（未確認が500件以下。reports.spec）を超えた。
    await commit.click({ trial: true });
  });
}

test('SC-04 r3-3 (2)の欄を Tab でたどっても、フォーカスした欄が下に付いた帯の裏に隠れない（幅390）', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/');
  const w = await newWallet(page, 'bank', uniqueText('E2E 取込Tab'));
  await open(page, `#/import?wallet=${w.code}`);
  await page.locator('input[type="file"]').setInputFiles(statement40());
  await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  await page.getByRole('heading', { name: '2. 列の対応' }).focus();
  const hidden = [];
  for (let i = 0; i < 30; i += 1) {
    await page.keyboard.press('Tab');
    const spot = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el.closest('.import-step') || el.closest('.import-actions')) return null;
      // 見た目を作り替えたラジオ（金額の形など）は、中の入力ではなく、包むラベルの位置で見る。
      const box = (el.closest('label') || el).getBoundingClientRect();
      const bar = document.querySelector('.import-actions').getBoundingClientRect();
      const top = document.querySelector('.topbar').getBoundingClientRect().bottom;
      return { name: el.getAttribute('aria-label') || el.closest('label, .field')?.textContent.slice(0, 20) || el.tagName, top: box.top, bottom: box.bottom, barTop: bar.top, topEdge: top };
    });
    if (spot && (spot.bottom > spot.barTop + 1 || spot.top < spot.topEdge - 1)) hidden.push(spot);
  }
  expect(hidden).toEqual([]);
});

test('SC-04 r3-3 新規が0件（全部が重複）なら、要約のすぐ下に「未確認を開く」を出す（口座で絞った未確認へ）', async ({ page }) => {
  const w = await start(page, 'utf8.csv');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-next')).toHaveCount(0);
  await page.getByRole('button', { name: '4件を取り込む' }).click();
  await expect(page.locator('.import-done')).toContainText('4件を未確認に入れました');
  await page.goto(`/#/import?wallet=${w.code}`);
  await page.reload();
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規0件');
  const link = page.locator('.check-head').getByRole('link', { name: '未確認を開く' });
  await expect(link).toBeInViewport({ ratio: 1 });
  await expect(link).toHaveAttribute('href', `#/inbox?wallet=${w.code}`);
  await link.click();
  await expect(page).toHaveURL(new RegExp(`#/inbox\\?wallet=${w.code}`));
  await expect(page.locator('tr.inbox-row')).toHaveCount(4);
});

/** 文の区切り（phrase の塊）の間に入る空白を許して、文を比べる正規表現。 */
const loose = (text) => new RegExp(`^${[...text].map((c) => c.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('\\s*')}$`);

test('SC-04 r3-6 前に取り込んだ口座を、次の取込の初期値にする（無効にした口座は使わない）。押せない「ファイルを選ぶ」には理由の一言を結ぶ', async ({ page }) => {
  await open(page, '#/import');
  const walletSel = page.getByLabel('取り込む口座');
  const pick = page.getByRole('button', { name: 'ファイルを選ぶ' });
  await expect(walletSel).toHaveValue('');
  await expect(pick).toBeDisabled();
  await expect(pick).toHaveAccessibleDescription(loose('先に、上で口座を選んでください。口座が決まると、ファイルを置けます。'));
  const w = await newWallet(page, 'bank', uniqueText('E2E 取込前回'));
  const other = await newWallet(page, 'bank', uniqueText('E2E 取込選ぶだけ'));
  await page.reload();
  // 選んだだけでは覚えない（取り込めたときに覚える）。
  await walletSel.selectOption(other.code);
  await expect(pick).toHaveAccessibleDescription(/^ドラッグして\s*置くか/);
  await page.goto('/#/import');
  await page.reload();
  await expect(walletSel).toHaveValue('');
  await walletSel.selectOption(w.code);
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await page.getByRole('button', { name: '確かめる' }).click();
  await page.getByRole('button', { name: '4件を取り込む' }).click();
  await expect(page.locator('.import-done')).toContainText('4件を未確認に入れました');
  // メニューから入り直す（アドレスに口座が無い）と、前に取り込んだ口座が選ばれていて、すぐファイルを選べる。
  await page.goto('/#/import');
  await page.reload();
  await expect(walletSel).toHaveValue(w.code);
  await expect(pick).toBeEnabled();
  await expect(pick).toBeFocused();
  // アドレスの口座が先。
  await page.goto(`/#/import?wallet=${other.code}`);
  await page.reload();
  await expect(walletSel).toHaveValue(other.code);
  // 無効にした口座は、初期値にしない。
  expect((await apiCall(page, 'PATCH', `/accounts/${w.code}`, { enabled: false })).status).toBe(200);
  await page.goto('/#/import');
  await page.reload();
  await expect(walletSel).toHaveValue('');
  await expect(pick).toBeDisabled();
});

test('SC-04 r3-9 日付の列に日付らしくない列（摘要）を選ぶと、「確かめる」の前に、欄の下へ「日付に読めない行が多い」と出る', async ({ page }) => {
  await start(page, 'utf8.csv');
  const dateSel = page.getByLabel('日付の列');
  const warn = page.locator('.col-warn');
  await expect(warn).toBeHidden();
  const checked = page.waitForResponse((r) => r.url().includes('/api/v1/imports/preview'));
  await dateSel.selectOption('1');
  await checked;
  await expect(warn).toHaveText('この列は日付に読めない行が多いようです（4行のうち4行）。日付の列を選び直してください。');
  await expect(dateSel).toHaveAccessibleDescription(/日付に\s*読めない\s*行が\s*多い/);
  // まだ取り込んでいない（確かめだけ）。段階も(2)のまま。
  await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  // 日付の列に戻すと、消える。
  await dateSel.selectOption('0');
  await expect(warn).toBeHidden();
  // 選び直した後の「確かめる」は、今までどおり進む。
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規4件');
});

test('SC-04 期限切れ（404）は「もう一度ファイルを選んでください」と出して、最初の段階に戻る', async ({ page }) => {
  await start(page, 'utf8.csv');
  await page.route('**/api/v1/imports/preview', (route) => route.fulfill({ status: 404, json: { error: { code: 'not_found', message: 'not found' } } }));
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.getByRole('heading', { name: '1. 口座とファイル' })).toBeVisible();
  await expect(page.locator('.import-step')).toContainText('もう一度ファイルを選んでください');
});

test('SC-04 g2-8 確かめる画面: 件数の下に、取り込む行の期間と出金・入金の合計を出す。表示位置はページの上、フォーカスは見出しへ', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 700 });
  await start(page, 'utf8.csv');
  // 「確かめる」は列の対応のいちばん下にある（押す前に、画面が下まで送られる）。
  await page.getByRole('button', { name: '確かめる' }).click();
  const heading = page.getByRole('heading', { name: '3. 確かめて取り込む' });
  await expect(heading).toBeFocused();
  await expect(page.locator('.check-summary')).toHaveText('取り込む4件: 2026-10-01〜2026-10-05・出金 2件 2,070円・入金 2件 110,000円');
  // 件数の要約と、期間・合計の行が、画面の中に見えている。
  await expect(page.locator('.check-counts')).toBeInViewport();
  await expect(page.locator('.check-summary')).toBeInViewport();
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
});

test('SC-04 g2-8 行の番号は、列の対応の見本と確かめる画面で同じ（ファイルの行番号。欄の中の改行も数える）', async ({ page }) => {
  await start(page, 'quoted.csv');
  const sample = page.locator('.sample-table');
  await expect(sample.locator('thead th').first()).toHaveText('ファイルの行');
  const sampleNos = await sample.locator('tbody td.line-col').allInnerTexts();
  expect(sampleNos).toEqual(['2', '3', '4', '6']);
  await page.getByRole('button', { name: '確かめる' }).click();
  const check = page.locator('.check-table');
  await expect(check.locator('thead th').first()).toHaveText('ファイルの行');
  await expect(check.locator('tbody tr td:first-child')).toHaveText(sampleNos);
});

test('SC-04 g2-8 読み飛ばす行の数を推測より増やすと、見本のその行に「読み飛ばす」と付く', async ({ page }) => {
  await start(page, 'utf8.csv');
  const rows = page.locator('.sample-table tbody tr');
  await expect(rows.first().locator('td').first()).toHaveText('2');
  await expect(page.locator('.sample-table .skip-tag')).toHaveCount(0);
  await page.getByLabel('読み飛ばす行の数').fill('2');
  await expect(rows.first()).toHaveClass(/is-skipped/);
  await expect(rows.first().locator('.skip-tag')).toHaveText('読み飛ばす');
  await expect(rows.nth(1)).not.toHaveClass(/is-skipped/);
  await page.getByLabel('読み飛ばす行の数').fill('1');
  await expect(page.locator('.sample-table .skip-tag')).toHaveCount(0);
});
