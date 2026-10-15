// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 初期設定（SC-00、AC-01.1 AC-01.2 AC-01.4）。空の帳簿のサーバー（8768、--fresh）で、最後まで通る。
// 途中でリロードしても、決めた分が残る（各段階でサーバーに保存するため）。初期設定が済むまでは、どの画面に入っても初期設定へ移る。

import { test, expect } from '@playwright/test';
import { apiCall, uniqueText, openSetup, SETUP_URL, entryFields } from './helpers.mjs';

test.use({ baseURL: SETUP_URL });

test('AC-01.1 AC-01.2 AC-01.4 初期設定を最後まで通る。途中でリロードしても決めた分が残り、免税なら税区分の欄が隠れる', async ({ page }) => {
  await openSetup(page, '#/');
  // もう一度流すときのために、前の実行の分を戻す（済んでいたら「済んでいない」に。口座は残ったまま）。
  const s = (await apiCall(page, 'GET', '/settings')).json;
  if (s.setup_done) expect((await apiCall(page, 'PUT', '/settings', { ...s, setup_done: false })).status).toBe(200);
  // 前の実行で入れた期首残高を空にする（元入金の確かめを、この実行の数字だけにするため）。配置の試験（layout.spec）が
  // 先に「済んでいない」に戻すことがあるので、済んだかどうかに関係なく見る（本物のサーバーは帳簿が残り続ける）。
  const op = (await apiCall(page, 'GET', '/opening')).json;
  if (op.balances.length) {
    const cleared = await apiCall(page, 'PUT', '/opening', { date: op.date, balances: [] });
    expect(cleared.status, JSON.stringify(cleared.json)).toBe(200);
  }
  // アドレスの # だけを変えても読み込み直さないので、画面が覚えている状態を捨てる。
  await page.reload();
  // どの画面に入っても、初期設定へ移る。
  await openSetup(page, '#/journal');
  await expect(page).toHaveURL(/#\/setup/);
  await expect(page.locator('main h1')).toHaveText('初期設定');
  await expect(page.locator('.sidebar')).toBeHidden();

  // (1) 事業者名と課税の区分。名前が空なら進まない。
  const name = page.getByLabel('事業者名');
  await name.fill('');
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page.locator('.setup-card .field-error').filter({ hasText: '事業者名を入れてください' })).toBeVisible();
  await name.fill('デモ商店（試験）');
  await page.locator('.seg-option', { hasText: '免税事業者' }).click();
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=2/);
  await expect(page.getByRole('heading', { name: '2. 口座' })).toBeVisible();

  // 途中でリロードしても、決めた分は残っている（AC-01.1）。
  await page.reload();
  await expect(page.getByRole('heading', { name: '2. 口座' })).toBeVisible();
  const saved = (await apiCall(page, 'GET', '/settings')).json;
  expect([saved.business_name, saved.tax_mode, saved.setup_done]).toEqual(['デモ商店（試験）', 'exempt', false]);
  await expect(page.locator('.setup-wallets')).toContainText('現金');

  // (2) 口座は種類を選んで追加し、名前は自由に付けられる（AC-01.2）。
  const bank = uniqueText('デモ銀行');
  const card = uniqueText('デモカード');
  for (const [type, label] of [['預金', bank], ['クレジットカード', card]]) {
    await page.getByRole('button', { name: '口座を追加' }).click();
    const dialog = page.getByRole('dialog', { name: '口座を追加' });
    await dialog.locator('.seg-option', { hasText: type }).click();
    await dialog.getByRole('textbox', { name: /^名前/ }).fill(label);
    await dialog.getByRole('button', { name: '追加', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator('.setup-wallets')).toContainText(label);
  }
  const wallets = (await apiCall(page, 'GET', '/accounts')).json.items.filter((a) => a.wallet);
  expect(wallets.find((w) => w.name === bank).wallet.type).toBe('bank');
  expect(wallets.find((w) => w.name === card).wallet.type).toBe('card');
  await page.getByRole('button', { name: '次へ' }).click();

  // (3) 期首残高。カードは未払い額（負債）。元入金は差額。
  await expect(page.getByRole('heading', { name: '3. 期首残高' })).toBeVisible();
  await page.getByLabel(`${bank}の残高`).fill('500000');
  await page.getByLabel(`${card}の残高`).fill('12000');
  await page.getByLabel('現金の残高').fill('30000');
  await page.getByLabel('現金の残高').press('Tab');
  await expect(page.locator('.opening-capital')).toContainText('518,000');
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=4/);

  // (4) 確認して始める。リロードしても、保存した内容が並ぶ。
  await page.reload();
  const summary = page.locator('.setup-summary');
  await expect(summary).toContainText('デモ商店（試験）');
  await expect(summary).toContainText('免税事業者');
  await expect(summary).toContainText(bank);
  await expect(summary).toContainText('元入金 518,000円');
  await page.getByRole('button', { name: '記帳を始める' }).click();
  await expect(page).toHaveURL(/#\/$/);
  // 完了の知らせは、次にすることまで読めるよう長めに残す（数秒で消えていた。体験の評価 第2回 E3-13）。
  const done = page.locator('.toast').filter({ hasText: '初期設定が済みました。' });
  await expect(done).toBeVisible();
  await page.waitForTimeout(5000);
  await expect(done).toBeVisible();
  await expect(page.locator('.next-card')).toBeVisible();
  await expect(page.locator('.sidebar')).toBeVisible();
  expect((await apiCall(page, 'GET', '/status')).json.setup_done).toBe(true);

  // 免税なので、取引の登録で税区分の欄が隠れている（AC-01.4）。
  await page.goto(`${SETUP_URL}/#/entry`);
  const f = entryFields(page);
  await expect(f.amount).toBeVisible();
  await expect(f.tax).toBeHidden();
  await expect(page.getByRole('button', { name: '詳細: 税区分を出す' })).toBeVisible();
});

// SC-00 誤りの出し方（初期設定）: 打ち直したら欄の誤りが消える。期首の日付の断り（最初の会計年度の外）は、通知ではなく
// 欄の下に出して、欄へフォーカスを移す。押し直しても増えない。保存の失敗の通知は重ならず、保存に成功したら消える。
test('SC-00 初期設定の誤りは欄に結び付き、直したら消える。期首の日付の断りは欄の下に出て、成功したら失敗の通知も残らない', async ({ page }) => {
  await openSetup(page, '#/');
  const s = (await apiCall(page, 'GET', '/settings')).json;
  if (s.setup_done) expect((await apiCall(page, 'PUT', '/settings', { ...s, setup_done: false })).status).toBe(200);
  await page.reload();
  await openSetup(page, '#/setup?step=1');
  const year = new Date().getFullYear();

  // (1) 名前が空のまま押すと誤り。打ち直したら、押す前でも誤りの文と印が消える。
  const name = page.getByRole('textbox', { name: '事業者名', exact: true });
  await expect(name).toHaveAttribute('aria-required', 'true');
  await name.fill('');
  await page.getByRole('button', { name: '保存して次へ' }).click();
  const nameErr = page.locator('.setup-card .field-error').filter({ hasText: '事業者名を入れてください' });
  await expect(nameErr).toBeVisible();
  await expect(name).toHaveAttribute('aria-invalid', 'true');
  await name.fill('デモ商店（誤りの試験）');
  await expect(nameErr).toHaveCount(0);
  await expect(name).not.toHaveAttribute('aria-invalid', /./);
  // 免税・課税の目安（数字は書かない）。選択の組に結び付いている。次の必須の欄まで遠くならないよう、「選び方をくわしく」に
  // 畳んでおき、開くと出る（見た目の採点 第2回 幅390。畳んでいても、組の説明として読まれる）。
  const guide = page.locator('.setup-card .field-hint', { hasText: '免税事業者かどうかは、前々年の課税売上などで決まります。分からなければ、税務署か税理士に確かめてください。' });
  await expect(guide).toBeHidden();
  const guideId = await guide.getAttribute('id');
  await expect(page.getByRole('group', { name: '消費税の課税の区分' })).toHaveAttribute('aria-describedby', new RegExp(`(^| )${guideId}( |$)`));
  await page.locator('.setup-card summary', { hasText: '選び方をくわしく' }).click();
  await expect(guide).toBeVisible();
  await page.locator('.seg-option', { hasText: '免税事業者' }).click();
  await page.getByRole('textbox', { name: /^開業日/ }).fill(`${year}-01-01`);
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=2/);
  await page.getByRole('button', { name: '次へ' }).click();
  await expect(page.getByRole('heading', { name: '3. 期首残高' })).toBeVisible();
  // 期首の日付と開業日の違いを、補足の1文で言う。
  await expect(page.locator('.setup-card')).toContainText('期首の日付は、残高を入れる日です。ふつうは開業日と同じです。');

  // (3) 保存の失敗（欄の無い断り）は通知で知らせる。押し直しても、同じ文の通知は増えない。
  await page.route('**/api/v1/opening', (route) =>
    route.request().method() === 'PUT'
      ? route.fulfill({ status: 500, json: { error: { code: 'internal', message: 'E2E 一時的に保存できませんでした。', field: null } } })
      : route.continue(),
  );
  const next = page.getByRole('button', { name: '保存して次へ' });
  await next.click();
  const failed = page.locator('.toast--error').filter({ hasText: 'E2E 一時的に保存できませんでした。' });
  await expect(failed).toHaveCount(1);
  await next.click();
  await page.waitForTimeout(200);
  await expect(failed).toHaveCount(1);
  await page.unroute('**/api/v1/opening');

  // 期首の日付が最初の会計年度の外。通知ではなく、欄の下に理由を出し、欄に印を付けてフォーカスを移す。
  const date = page.getByRole('textbox', { name: '期首の日付', exact: true });
  await date.fill(`${year - 1}-12-31`);
  await next.click();
  const dateErr = page.locator('.setup-card .field-error').filter({ hasText: '最初の会計年度' });
  await expect(dateErr).toBeVisible();
  await expect(date).toHaveAttribute('aria-invalid', 'true');
  await expect(date).toBeFocused();
  await expect(page.locator('.toast--error').filter({ hasText: '最初の会計年度' })).toHaveCount(0);
  // 押し直した応えが届くのを待ってから確かめる（待たないと、直した後に古い応えが届いて誤りが出直すことがあり、
  // 「増えない」の確かめも、前から出ている誤りだけで通ってしまう）。
  const again = page.waitForResponse((r) => r.url().includes('/api/v1/opening') && r.request().method() === 'PUT');
  await next.click();
  await again;
  await expect(next).toBeEnabled(); // 押している間は押せない。応えの処理（誤りの表示）が終わると、また押せる。
  await expect(dateErr).toHaveCount(1);
  // 直すと、押す前に誤りが消える。保存に成功したら、前の失敗の通知も消える。
  await date.fill(`${year}-01-01`);
  await expect(dateErr).toHaveCount(0);
  await next.click();
  await expect(page).toHaveURL(/step=4/);
  await expect(page.locator('.toast--error')).toHaveCount(0);
});

/** 初期設定を「済んでいない」に戻して、画面が覚えている状態を捨てる（setup_done は起動時の状態に入るので、読み込み直す）。 */
async function notDone(page, patch = {}) {
  await openSetup(page, '#/');
  const s = (await apiCall(page, 'GET', '/settings')).json;
  expect((await apiCall(page, 'PUT', '/settings', { ...s, setup_done: false, ...patch })).status).toBe(200);
  await page.reload();
}

const steps = (page) => page.locator('.stepper .step');

// SC-00 AC-01.1 閉じて開き直したら、最後に保存した段階の次から始まり、済んだ段階には印が残る（体験の評価 第2回 E3-2。
// 画面に「途中で閉じても続きから再開できます」と書いてあるのに、1段階目から始まり、済みの印も消えていた）。
test('SC-00 初期設定を閉じて開き直すと、最後に保存した段階の次から始まり、済んだ段階に印が残る', async ({ page }) => {
  const year = new Date().getFullYear();
  await notDone(page);
  // 前の実行で入れた期首残高を空にする（3段階目が済んでいない状態から始める）。
  const op = (await apiCall(page, 'GET', '/opening')).json;
  if (op.balances.length) expect((await apiCall(page, 'PUT', '/opening', { date: op.date, balances: [] })).status).toBe(200);
  // (1) 1段階目を保存する。
  await openSetup(page, '#/setup?step=1');
  await page.getByRole('textbox', { name: '事業者名', exact: true }).fill('デモ商店（再開の試験）');
  await page.getByRole('textbox', { name: '開業日', exact: true }).fill(`${year}-01-01`);
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=2/);
  // 閉じて開き直す（段階の無いアドレス）。2段階目から始まり、1段階目に済みの印。
  await page.goto(`${SETUP_URL}/`);
  await expect(page.getByRole('heading', { name: '2. 口座' })).toBeVisible();
  await expect(page).toHaveURL(/#\/setup$/);
  await expect(steps(page).nth(0)).toHaveClass(/is-done/);
  await expect(steps(page).nth(0)).toContainText('（済み）');
  await expect(steps(page).nth(1)).toHaveAttribute('aria-current', 'step');
  await expect(steps(page).nth(2)).not.toHaveClass(/is-done/);
  // (2)(3) 期首残高まで保存する。
  await page.getByRole('button', { name: '次へ' }).click();
  await expect(page.getByRole('heading', { name: '3. 期首残高' })).toBeVisible();
  await page.getByLabel('現金の残高').fill('45000');
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=4/);
  // 閉じて開き直すと、4段階目から。1〜3段階目に済みの印。
  await page.goto(`${SETUP_URL}/`);
  await expect(page.getByRole('heading', { name: '4. 確認して始める' })).toBeVisible();
  for (const i of [0, 1, 2]) await expect(steps(page).nth(i)).toHaveClass(/is-done/);
  await expect(steps(page).nth(3)).toHaveAttribute('aria-current', 'step');
  // 確認の段階に、科目ごとの期首残高が並ぶ（体験の評価 第2回 E3-11）。
  await expect(page.locator('.setup-balances li', { hasText: '現金' }).first()).toContainText('45,000円');
  // 戻るで前の段階を開いても、済んだ段階の印は残る。
  await page.getByRole('button', { name: '戻る' }).click();
  await expect(page.getByRole('heading', { name: '3. 期首残高' })).toBeVisible();
  await page.getByRole('button', { name: '戻る' }).click();
  await page.getByRole('button', { name: '戻る' }).click();
  await expect(page.getByRole('heading', { name: '1. 事業者と課税の区分' })).toBeVisible();
  await expect(steps(page).nth(0)).toHaveAttribute('aria-current', 'step');
  for (const i of [1, 2]) await expect(steps(page).nth(i)).toHaveClass(/is-done/);
  await expect(steps(page).nth(3)).not.toHaveClass(/is-done/);
});

// SC-00 キー操作（体験の評価 第2回 E3-4）: 文字の欄の Enter で次の欄へ進み、最後の欄の Enter は「保存して次へ」と同じ
// （取引の登録と同じ決まり）。変換中の Enter では進まない。期首残高は、口座でない科目を足した行も含めて、Enter で次の
// 残高の欄へ進み、最後の残高の欄の Enter で保存して次へ。開業日の欄の名前と説明（改善指示4）も確かめる。
test('SC-00 初期設定の文字の欄は Enter で次の欄へ進み、最後の欄の Enter で保存して次へ進む（変換中は進まない）', async ({ page }) => {
  const year = new Date().getFullYear();
  await notDone(page, { tax_mode: 'exempt', invoice_number: null });
  await openSetup(page, '#/setup?step=1');
  const name = page.getByRole('textbox', { name: '事業者名', exact: true });
  const opened = page.getByRole('textbox', { name: '開業日', exact: true });
  await expect(page.locator('.setup-card .field-hint', { hasText: 'この日より前の日付で取引を登録するときは、確認が出ます。期首残高の日付の初期値にもなります。' })).toContainText('年の途中から記帳を始めるときは、その年の1月1日にすると、年の帳票の「期首」の列に、期首残高が入ります。');
  // 変換中の Enter では進まない（Chromium の変換の模擬）。
  await name.fill('');
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.imeSetComposition', { text: 'でもしょうてん', selectionStart: 7, selectionEnd: 7 });
  await page.keyboard.press('Enter');
  await expect(name).toBeFocused();
  await cdp.send('Input.insertText', { text: 'デモ商店（キーの試験）' });
  await page.waitForTimeout(120);
  await expect(name).toHaveValue('デモ商店（キーの試験）');
  // 事業者名 → 課税の区分（選んである選択肢） → 開業日 → 最後の欄の Enter で保存して次へ。
  await page.keyboard.press('Enter');
  await expect(page.getByRole('radio', { name: '免税事業者' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(opened).toBeFocused();
  await opened.fill(`${year}-01-01`);
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/step=2/);
  expect((await apiCall(page, 'GET', '/settings')).json.business_name).toBe('デモ商店（キーの試験）');
  // 期首残高: 口座でない科目（前払金）を足すと、表の最後の行になり、Enter の順にも入る。
  await page.getByRole('button', { name: '次へ' }).click();
  await expect(page.getByRole('heading', { name: '3. 期首残高' })).toBeVisible();
  const others = page.getByRole('combobox', { name: /口座でない科目を足す/ });
  await others.click();
  await page.keyboard.insertText('まえばらい');
  await page.keyboard.press('Enter');
  await expect(page.getByLabel('前払金の残高')).toBeFocused();
  const date = page.getByRole('textbox', { name: '期首の日付', exact: true });
  await date.focus();
  await page.keyboard.press('Enter');
  const inputs = page.locator('.opening-table input[aria-label$="の残高"]');
  const n = await inputs.count();
  expect(n, '試験の前提: 残高の欄が2つ以上').toBeGreaterThan(1);
  await expect(inputs.first()).toBeFocused();
  for (let i = 1; i < n; i += 1) {
    await page.keyboard.press('Enter');
    await expect(inputs.nth(i)).toBeFocused();
  }
  await expect(inputs.last()).toHaveAttribute('aria-label', '前払金の残高');
  await page.keyboard.type('2000');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/step=4/);
  expect((await apiCall(page, 'GET', '/opening')).json.balances).toContainEqual({ account: 'prepaid', amount: 2000 });
});
