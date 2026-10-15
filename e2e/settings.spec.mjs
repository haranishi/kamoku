// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 設定（SC-10）: 台帳の検査（AC-09.1）、末尾のハッシュの写し取りと、控えたハッシュの照合（AC-09.2）、事業者の保存、
// 期首残高の見直し（仕訳帳の詳細からの案内、ほかの場所の書き込みで断られたときに入力を保つ）、見た目。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, addCashExpense } from './helpers.mjs';

test('AC-09.1 台帳を検査すると結果が出る。壊れていれば最初の連番と規則を示す', async ({ page }) => {
  await open(page, '#/settings?section=data');
  await page.getByRole('button', { name: '台帳を検査する' }).click();
  const out = page.locator('.settings-data .check-out').first();
  await expect(out).toContainText('台帳は壊れていません');
  await expect(out).toContainText(/\d+件の記録を最初から順にたどり/);
  await page.route('**/api/v1/verify', (route) => route.fulfill({ json: { ok: false, error: { seq: 55, kind: 'bad_hash', rule: null } } }));
  await page.getByRole('button', { name: '台帳を検査する' }).click();
  await expect(out).toContainText('台帳に壊れた記録があります');
  await expect(out).toContainText('55番目の記録に、ハッシュの不一致');
});

test('AC-09.2 末尾のハッシュを写し取れて、控えたハッシュを入れると台帳の何番目にあたるかが分かる', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page, '#/settings?section=data');
  const status = (await apiCall(page, 'GET', '/status')).json;
  await expect(page.locator('.hash-full')).toHaveText(status.ledger.head_hash);
  // 64文字の控えは、ほぼ同じ長さの2行に分かれる（枠いっぱいに並べると、末尾の数文字だけが次の行に落ちた。見た目の採点 第3回、幅1440）。
  const widths = await page.locator('.hash-full').evaluate((el) => {
    const r = document.createRange();
    r.selectNodeContents(el);
    return [...r.getClientRects()].map((x) => x.width).filter((w) => w > 0);
  });
  expect(widths, '2行に分かれる').toHaveLength(2);
  expect(Math.min(...widths) / Math.max(...widths), '2行の長さがほぼ同じ（末尾の数文字だけが落ちない）').toBeGreaterThan(0.8);
  await page.getByRole('button', { name: '写し取る' }).click();
  await expect(page.locator('.toast').filter({ hasText: '写し取りました' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(status.ledger.head_hash);

  const input = page.getByLabel('控えたハッシュ');
  const out = page.locator('.settings-data .check-out').last();
  // 末尾のハッシュ（大文字と前後の空白を混ぜても読める）。
  await input.fill(` ${status.ledger.head_hash.toUpperCase()} `);
  await page.keyboard.press('Enter');
  await expect(out).toContainText(`台帳の${status.ledger.head_seq}番目の記録と一致しました（末尾と同じです）`);
  // 前に控えた、途中の記録のハッシュ。
  const list = (await apiCall(page, 'GET', '/entries?state=all&limit=500')).json.items;
  const oldest = list[list.length - 1];
  const ev = (await apiCall(page, 'GET', `/entries/${oldest.id}`)).json.history[0];
  await input.fill(ev.hash);
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(out).toContainText(`台帳の${ev.seq}番目の記録と一致しました（末尾ではありません）`);
  await expect(out).toContainText(`${status.ledger.events - ev.seq}件の記録が足されています`);
  // どの記録とも一致しない。
  await input.fill('0'.repeat(64));
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(out).toContainText('一致する記録がありません');
  // 形が違えば、欄の下に理由。
  await input.fill('abc');
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(page.locator('#set-data .field-error').filter({ hasText: '64文字です（いまは3文字）' })).toBeVisible();
});

test('SC-10 事業者の設定を保存できる。免税にすると登録番号の欄を隠す', async ({ page }) => {
  await open(page, '#/settings');
  const before = (await apiCall(page, 'GET', '/settings')).json;
  try {
    const name = page.getByLabel('事業者名');
    await name.fill('デモ事務所（試験）');
    await page.getByRole('button', { name: '事業者の設定を保存' }).click();
    await expect(page.locator('.toast').filter({ hasText: '設定を保存しました' })).toBeVisible();
    await expect(page.locator('.sidebar-business')).toHaveText('デモ事務所（試験）');
    expect((await apiCall(page, 'GET', '/settings')).json.business_name).toBe('デモ事務所（試験）');
    await page.locator('#set-business .seg-option', { hasText: '課税事業者' }).click();
    await expect(page.getByLabel('登録番号（適格請求書発行事業者）')).toBeVisible();
    await page.getByLabel('登録番号（適格請求書発行事業者）').fill('T123');
    await page.getByRole('button', { name: '事業者の設定を保存' }).click();
    await expect(page.locator('#set-business .field-error').filter({ hasText: 'T と13桁' })).toBeVisible();
    await page.locator('#set-business .seg-option', { hasText: '免税事業者' }).click();
    await expect(page.getByLabel('登録番号（適格請求書発行事業者）')).toBeHidden();
  } finally {
    await apiCall(page, 'PUT', '/settings', before);
  }
});

test('期首残高の仕訳は、仕訳帳の詳細で訂正・取消を出さず、設定の「期首残高」へ案内する。設定から見直せる', async ({ page }) => {
  await open(page, '#/');
  const opening = (await apiCall(page, 'GET', '/opening')).json;
  test.skip(!opening.entry_id, '期首残高の仕訳が無い帳簿');
  await open(page, `#/journal/${opening.entry_id}`);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText('期首残高は、設定の「期首残高」から見直します');
  await expect(drawer.getByRole('button', { name: /訂正する/ })).toHaveCount(0);
  await expect(drawer.getByRole('button', { name: /取り消す/ })).toHaveCount(0);
  // 仕訳帳の e と v も働かない（詳細の中でも）。
  await page.keyboard.press('e');
  await expect(page).toHaveURL(new RegExp(`#/journal/${opening.entry_id}`));
  await drawer.getByRole('link', { name: '設定の「期首残高」を開く' }).click();
  await expect(page).toHaveURL(/#\/settings\?section=opening/);
  await expect(page.locator('#set-opening h2')).toBeFocused();
  const first = opening.balances[0];
  const field = page.locator('#set-opening input[aria-label$="の残高"]').first();
  const original = await field.inputValue();
  try {
    await field.fill(String(first.amount + 1000));
    await page.getByRole('button', { name: '期首残高を保存' }).click();
    await expect(page.locator('.toast').filter({ hasText: '期首残高を保存しました' })).toBeVisible();
    const after = (await apiCall(page, 'GET', '/opening')).json;
    expect(after.entry_id).toBe(opening.entry_id);
    expect(after.capital).toBe(opening.capital + 1000);
    const detail = (await apiCall(page, 'GET', `/entries/${opening.entry_id}`)).json;
    expect(detail.history.at(-1).reason).toBe('期首残高の見直し');
  } finally {
    await apiCall(page, 'PUT', '/opening', { date: opening.date, balances: opening.balances });
    expect(original).not.toBe('');
  }
});

test('409 stale: 期首残高の保存が、ほかの場所の書き込みで断られたら「他の場所で更新されました」と出し、入力を失わない', async ({ page }) => {
  await open(page, '#/settings?section=opening');
  const before = (await apiCall(page, 'GET', '/opening')).json;
  // 保存（PUT）だけを、別のプロセスが先に書いたときの応えにする。読み込み（GET）はそのまま通す。
  await page.route('**/api/v1/opening', (route) =>
    route.request().method() === 'PUT'
      ? route.fulfill({ status: 409, json: { error: { code: 'stale', message: '台帳がほかの場所で更新されました。', field: null } } })
      : route.continue(),
  );
  const field = page.locator('#set-opening input[aria-label$="の残高"]').first();
  await field.fill('123456');
  await page.getByRole('button', { name: '期首残高を保存' }).click();
  await expect(page.locator('.toast').filter({ hasText: '他の場所で更新されました' })).toBeVisible();
  await expect(field).toHaveValue(/123,?456/);
  expect((await apiCall(page, 'GET', '/opening')).json).toEqual(before);
});

test('AC-08.6 の画面側: Shift_JIS で書き出すと、無い文字を「?」に置き換えたことを知らせ、ファイルは保存される', async ({ page }) => {
  await open(page, '#/settings');
  // Shift_JIS に無い文字（𠮷）を摘要に入れた仕訳を作る。
  await addCashExpense(page, { description: uniqueText('E2E 𠮷の書き出し') });
  await page.getByLabel('仕訳帳のCSVの文字コード').selectOption('sjis');
  const link = page.getByRole('link', { name: '仕訳帳のCSV' });
  await expect(link).toHaveAttribute('href', /encoding=sjis/);
  const download = page.waitForEvent('download');
  await link.click();
  expect((await download).suggestedFilename()).toMatch(/\.csv$/);
  const warned = page.locator('.toast').filter({ hasText: '「?」に置き換えて書き出しました' });
  await expect(warned).toBeVisible();
  // UTF-8 なら置き換えは起きないので、知らせは増えない（前の知らせは、時間で消えることがある）。
  await page.getByLabel('仕訳帳のCSVの文字コード').selectOption('utf8bom');
  const before = await warned.count();
  const second = page.waitForEvent('download');
  await link.click();
  expect((await second).suggestedFilename()).toMatch(/\.csv$/);
  await page.waitForTimeout(300);
  expect(await warned.count()).toBeLessThanOrEqual(before);
});

test('SC-10 見た目: ライト・ダーク・OSに合わせるを選べて、ブラウザの中に残る', async ({ page }) => {
  await open(page, '#/settings?section=appearance');
  await page.locator('#set-appearance .seg-option', { hasText: 'ダーク' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.locator('#set-appearance .seg-option', { hasText: '狭い' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await page.locator('#set-appearance .seg-option', { hasText: 'OSに合わせる' }).click();
  await page.locator('#set-appearance .seg-option', { hasText: '標準' }).click();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /./);
  await expect(page.locator('html')).not.toHaveAttribute('data-density', /./);
});

// 左下の「表示」ボタンで明るさを変えたとき、設定の「明るさ」の選択も追随する（OSに合わせる、に回しても「ダーク」のまま残り、
// 画面は明るいのに設定はダークと出た。2026-10-15 の体験の評価 E3）。
test('SC-10 見た目: 左下の「表示」ボタンで変えても、設定の「明るさ」の選択が合う', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, '#/settings?section=appearance');
  const sec = page.locator('#set-appearance');
  const checked = () => sec.locator('input[name]:checked').first().evaluate((el) => el.closest('.seg-option').textContent.trim());
  await sec.locator('.seg-option', { hasText: 'ダーク' }).click();
  await expect.poll(checked).toContain('ダーク');
  const button = page.locator('.sidebar [data-action="theme"]');
  await button.click(); // ダーク → OSに合わせる
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /./);
  await expect.poll(checked).toContain('OSに合わせる');
  await button.click(); // OSに合わせる → ライト
  await expect.poll(checked).toContain('ライト');
  await button.click(); // ライト → ダーク
  await expect.poll(checked).toContain('ダーク');
  await sec.locator('.seg-option', { hasText: 'OSに合わせる' }).click();
});

// SC-10 締め: 締める・外すときは確認の小窓（何ができなくなるか・できるようになるか）を出し、承認したら保存する。
// 通知は「…まで締めました。」「締めを外しました。」。見出しの横に、いまの状態をいつも出す。
test('SC-10 締める・外すときは確認を出し、通知と見出しの横の状態で結果を伝える', async ({ page }) => {
  await open(page, '#/settings?section=lock');
  const before = (await apiCall(page, 'GET', '/settings')).json;
  // 帳簿の仕訳に関わらない日（開業日の前日）で締める。最後に元へ戻す。
  const opened = new Date(`${before.opened_on}T00:00:00Z`);
  const day = new Date(opened.getTime() - 86400000).toISOString().slice(0, 10);
  try {
    if (before.locked_until) {
      await apiCall(page, 'PUT', '/settings', { ...before, locked_until: null });
      await page.reload();
    }
    const sec = page.locator('#set-lock');
    const stateBadge = sec.locator('.lock-state');
    await expect(stateBadge).toHaveText('締めていません');
    const input = sec.getByRole('textbox', { name: 'この日以前は変更しない' });
    const save = sec.getByRole('button', { name: '締めた日を保存' });
    await input.fill(day);
    await save.click();
    let dialog = page.getByRole('dialog', { name: `${day} まで締めますか` });
    await expect(dialog).toBeVisible();
    // 説明の文は、小窓の説明として結び付いている（開いたときに題名と一緒に読まれる）。
    const describedBy = await dialog.getAttribute('aria-describedby');
    await expect(page.locator(`[id="${describedBy}"]`)).toHaveText(`${day} 以前の、登録・訂正・取消ができなくなります。`);
    // やめたら保存しない。
    await dialog.getByRole('button', { name: 'やめる' }).click();
    await expect(dialog).toHaveCount(0);
    expect((await apiCall(page, 'GET', '/settings')).json.locked_until).toBeNull();
    await expect(stateBadge).toHaveText('締めていません');
    // 承認したら保存し、結果を伝える。
    await save.click();
    dialog = page.getByRole('dialog', { name: `${day} まで締めますか` });
    await dialog.getByRole('button', { name: '締める' }).click();
    await expect(page.locator('.toast').filter({ hasText: `${day} まで締めました。` })).toBeVisible();
    await expect(stateBadge).toHaveText(`${day} まで締め済み`);
    expect((await apiCall(page, 'GET', '/settings')).json.locked_until).toBe(day);
    // 締めた日を前へ戻すときは、外れる期間（その間の仕訳は、また直せる）も書く。やめたら変わらない。
    const back = (n) => new Date(opened.getTime() - n * 86400000).toISOString().slice(0, 10);
    const earlier = back(4);
    await input.fill(earlier);
    await save.click();
    dialog = page.getByRole('dialog', { name: `${earlier} まで締めますか` });
    await expect(dialog).toContainText(`${earlier} 以前の、登録・訂正・取消ができなくなります。${back(3)}〜${day} の仕訳は、また直せるようになります。`);
    await dialog.getByRole('button', { name: 'やめる' }).click();
    // 小窓が閉じきる（フォーカスが「締めた日を保存」へ戻る）のを待ってから、欄を空にする（1回目の「やめる」の後と同じ）。
    // 待たずに空にすると、機械が重いときに、空にしている途中でフォーカスが戻され、欄に日付が残ることがあった。
    await expect(dialog).toHaveCount(0);
    expect((await apiCall(page, 'GET', '/settings')).json.locked_until).toBe(day);
    // 空にして保存すると、外す確認。
    await input.fill('');
    await save.click();
    dialog = page.getByRole('dialog', { name: '締めを外しますか' });
    await expect(dialog).toContainText(`締めを外すと、${day} 以前の仕訳を、また直せるようになります。`);
    await dialog.getByRole('button', { name: '外す' }).click();
    await expect(page.locator('.toast').filter({ hasText: '締めを外しました。' })).toBeVisible();
    await expect(stateBadge).toHaveText('締めていません');
    expect((await apiCall(page, 'GET', '/settings')).json.locked_until).toBeNull();
  } finally {
    await apiCall(page, 'PUT', '/settings', before);
  }
});

// SC-10 期首残高: 期首の日付が最初の会計年度の外なら、断りを欄の下に出して欄へ移る（通知だけにしない）。
test('SC-10 設定の期首残高: 期首の日付の断りは欄の下に出て、欄に印とフォーカスが移る', async ({ page }) => {
  await open(page, '#/settings?section=opening');
  const before = (await apiCall(page, 'GET', '/opening')).json;
  const sec = page.locator('#set-opening');
  const date = sec.getByRole('textbox', { name: '期首の日付', exact: true });
  await expect(date).toHaveAttribute('aria-required', 'true');
  await date.fill(`${Number(before.date.slice(0, 4)) - 1}-12-31`);
  await sec.getByRole('button', { name: '期首残高を保存' }).click();
  const err = sec.locator('.field-error').filter({ hasText: '最初の会計年度' });
  await expect(err).toBeVisible();
  await expect(date).toHaveAttribute('aria-invalid', 'true');
  await expect(date).toBeFocused();
  await expect(page.locator('.toast--error')).toHaveCount(0);
  expect((await apiCall(page, 'GET', '/opening')).json).toEqual(before);
  await date.fill(before.date);
  await expect(err).toHaveCount(0);
});

// SC-10 照合し直すときは、前の結果を消す（形の誤りの下に、前の「一致する記録がありました」が残っていた）。
test('SC-10 照合し直すと、前の照合の結果が消える', async ({ page }) => {
  await open(page, '#/settings?section=data');
  const status = (await apiCall(page, 'GET', '/status')).json;
  const input = page.getByLabel('控えたハッシュ');
  const out = page.locator('.settings-data .check-out').last();
  await input.fill(status.ledger.head_hash);
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(out).toContainText('一致する記録がありました');
  await input.fill('0000');
  await expect(input).not.toHaveAttribute('aria-invalid', /./);
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(page.locator('#set-data .field-error').filter({ hasText: '64文字です（いまは4文字）' })).toBeVisible();
  await expect(out).toBeEmpty();
  // 打ち直したら、欄の誤りも消える。
  await input.fill(status.ledger.head_hash);
  await expect(page.locator('#set-data .field-error').filter({ hasText: '64文字です' })).toHaveCount(0);
});

// SC-10 免税・課税の目安を、選択の組に結び付けて出す（数字は書かない）。
test('SC-10 設定の事業者: 免税・課税の目安の1行が、選択の組に結び付いている', async ({ page }) => {
  await open(page, '#/settings');
  const guide = page.locator('#set-business .field-hint', { hasText: '免税事業者かどうかは、前々年の課税売上などで決まります。分からなければ、税務署か税理士に確かめてください。' });
  await expect(guide).toBeVisible();
  const id = await guide.getAttribute('id');
  await expect(page.locator('#set-business').getByRole('group', { name: '消費税の課税の区分' })).toHaveAttribute('aria-describedby', new RegExp(`(^| )${id}( |$)`));
});

// SC-10 事業者名は必須（体験の評価 第2回 E3-6。空のまま保存でき、メニューから名前が消えた。初期設定では必須）。
// 空なら欄の下に誤りを出し、保存せず、欄へフォーカスを移す。打ち直したら誤りは消える。開業日の説明は初期設定と同じ文。
test('SC-10 設定の事業者名は必須: 空なら保存せず、欄の下に誤りを出して欄へ移る', async ({ page }) => {
  await open(page, '#/settings');
  const before = (await apiCall(page, 'GET', '/settings')).json;
  const name = page.getByRole('textbox', { name: '事業者名', exact: true });
  await expect(name).toHaveAttribute('aria-required', 'true');
  await name.fill('');
  await page.getByRole('button', { name: '事業者の設定を保存' }).click();
  const err = page.locator('#set-business .field-error').filter({ hasText: '事業者名を入れてください。' });
  await expect(err).toBeVisible();
  await expect(name).toHaveAttribute('aria-invalid', 'true');
  await expect(name).toBeFocused();
  await expect(page.locator('.toast').filter({ hasText: '設定を保存しました' })).toHaveCount(0);
  expect((await apiCall(page, 'GET', '/settings')).json.business_name).toBe(before.business_name);
  await expect(page.locator('.sidebar-business')).toHaveText(before.business_name);
  await name.fill(before.business_name);
  await expect(err).toHaveCount(0);
  await expect(name).not.toHaveAttribute('aria-invalid', /./);
  // 開業日の欄の名前と説明（初期設定と同じ。改善指示4）。
  await expect(page.getByRole('textbox', { name: '開業日', exact: true })).toBeVisible();
  await expect(page.locator('#set-business .field-hint', { hasText: 'この日より前の日付で取引を登録するときは、確認が出ます。期首残高の日付の初期値にもなります。' })).toContainText('年の途中から記帳を始めるときは、その年の1月1日にすると、年の帳票の「期首」の列に、期首残高が入ります。');
});

// 見た目の採点 第2回（幅1440）: 設定のカードは全幅なのに、入力が左の約560pxに集まって右が空き、ページも長かった。
// 1280px以上は、事業者・期首残高・データ・見た目の節の中を左右2列に並べる（節は縦に1列のまま）。幅768は今までどおり1列。
test('SC-10 幅1440の設定は、事業者・期首残高・データ・見た目の節の中が左右2列に並ぶ（幅768は1列）', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, '#/settings');
  const box = (loc) => loc.boundingBox();
  const biz = page.locator('#set-business');
  const name = biz.getByRole('textbox', { name: '事業者名', exact: true });
  const mode = biz.getByRole('group', { name: '消費税の課税の区分' });
  await expect(name).toBeVisible();
  let [a, b] = await Promise.all([box(name), box(mode)]);
  expect(b.x, '課税の区分は、事業者名の右の列').toBeGreaterThan(a.x + a.width);
  // 保存のボタンは、2列の下で、右の列の右の端にそろう。
  [a, b] = await Promise.all([box(biz.getByRole('button', { name: '事業者の設定を保存' })), box(biz.locator('.settings-col').nth(1))]);
  expect(Math.abs(a.x + a.width - (b.x + b.width))).toBeLessThanOrEqual(1);
  const op = page.locator('#set-opening');
  [a, b] = await Promise.all([box(op.locator('.opening-wrap')), box(op.locator('.opening-totals'))]);
  expect(b.x, '期首残高の合計は、表の右の列').toBeGreaterThan(a.x + a.width);
  const blocks = await page.locator('#set-data .settings-block').evaluateAll((els) => els.map((e) => e.getBoundingClientRect()).map((r) => ({ x: Math.round(r.x), y: Math.round(r.y) })));
  expect(blocks.length).toBe(4);
  expect(blocks[1].x, 'データの2つ目は右の列').toBeGreaterThan(blocks[0].x);
  expect(blocks[1].y, 'データの2つ目は同じ段').toBe(blocks[0].y);
  expect(blocks[2].y, 'データの3つ目は下の段').toBeGreaterThan(blocks[0].y);
  [a, b] = await Promise.all([box(page.getByRole('group', { name: '明るさ' })), box(page.getByRole('group', { name: '表の行の密度' }))]);
  expect(b.x, '表の行の密度は、明るさの右の列').toBeGreaterThan(a.x + a.width - 1);
  await page.setViewportSize({ width: 768, height: 1024 });
  [a, b] = await Promise.all([box(name), box(mode)]);
  expect(b.y, '幅768では、課税の区分は事業者名の下').toBeGreaterThan(a.y + a.height);
});

// 見た目の採点 第2回（幅390）: 期首残高の表で、科目名が狭い左の列で「サンプル銀行／普通預金」と折れ、種類も加わって3段になった。
// 640px未満では、科目名と種類を上の段に、残高の欄を下の段（行の右半分。幅は今までと同じ）に置く。
test('SC-10 幅390の期首残高は、科目名を上の段、残高の欄を下の段に置く', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/settings?section=opening');
  const rows = page.locator('#set-opening .opening-table tbody tr');
  await expect(rows.first()).toBeVisible();
  const shapes = await rows.evaluateAll((trs) =>
    trs.map((tr) => {
      const name = tr.querySelector('.opening-name');
      const r = (el) => el.getBoundingClientRect();
      const row = r(tr);
      return {
        name: name.textContent,
        lines: name.getClientRects().length,
        below: r(tr.querySelector('input')).top >= Math.max(r(name).bottom, r(tr.querySelector('.sub')).bottom) - 1,
        full: Math.abs(row.width - r(tr.closest('table')).width) <= 1,
        half: Math.abs(r(tr.querySelector('td.num')).width - row.width / 2) <= 2,
      };
    }),
  );
  expect(shapes.length, '試験の前提: 口座が2つ以上').toBeGreaterThan(1);
  for (const s of shapes) expect(s, s.name).toEqual({ name: s.name, lines: 1, below: true, full: true, half: true });
});
