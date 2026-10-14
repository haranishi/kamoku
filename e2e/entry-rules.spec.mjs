// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録の決まり: 再送防止、プレビューの失敗、免税、口座が相手科目、経過措置の案内、仕訳で入力、テンプレート、読み取り専用。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, withSettings, walletOf } from './helpers.mjs';

async function fillSupplies(page, desc, amount = '1100') {
  const f = entryFields(page);
  await f.amount.fill(amount);
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await f.description.fill(desc);
  await expect(f.preview.locator('table')).toBeVisible();
  return f;
}

test('AC-02.7 保存の応えが届かずに押し直しても、同じ client_id で送るので二重に登録しない', async ({ page }) => {
  const desc = uniqueText('E2E 再送');
  await open(page, '#/entry');
  await fillSupplies(page, desc);
  const ids = [];
  let first = true;
  await page.route('**/api/v1/entries', async (route) => {
    if (route.request().method() !== 'POST') return route.continue();
    ids.push(route.request().postDataJSON().client_id);
    if (first) {
      // サーバーには届いて登録されたが、応えが画面に届かなかった、という場面を作る。
      first = false;
      await route.fetch();
      return route.abort('failed');
    }
    return route.continue();
  });
  const save = page.getByRole('button', { name: /保存して続ける/ });
  await save.click();
  await expect(page.locator('.form-error')).toContainText('サーバーにつながりません');
  await save.click();
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  expect(ids).toHaveLength(2);
  expect(ids[0]).toMatch(/^c_[0-9a-f]+$/);
  expect(ids[1]).toBe(ids[0]);
  expect((await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.total).toBe(1);
});

test('プレビューの通信が失敗したら、理由を出して保存を止め、直れば保存できる', async ({ page }) => {
  await open(page, '#/entry');
  await page.route('**/api/v1/entries/preview', (route) => route.abort('failed'));
  const f = entryFields(page);
  await f.amount.fill('2200');
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await expect(f.preview).toContainText('仕訳のプレビューを作れませんでした');
  const save = page.getByRole('button', { name: /保存して続ける/ });
  await expect(save).toBeDisabled();
  await expect(page.locator('.block-note')).toHaveText('プレビューを作れないため、保存を止めています。');
  await page.unroute('**/api/v1/entries/preview');
  await f.preview.getByRole('button', { name: 'もう一度試す' }).click();
  await expect(f.preview.locator('table')).toBeVisible();
  await expect(save).toBeEnabled();
});

test('AC-01.4 の画面側: 免税の設定では税区分の欄を隠し、「詳細」で出せる', async ({ page }) => {
  await open(page, '#/');
  await withSettings(page, { tax_mode: 'exempt' }, async () => {
    await open(page, '#/entry');
    const f = entryFields(page);
    await expect(f.tax).toBeHidden();
    const detail = page.getByRole('button', { name: '詳細: 税区分を出す' });
    await expect(detail).toHaveAttribute('aria-expanded', 'false');
    await detail.click();
    await expect(f.tax).toBeVisible();
    await expect(f.tax).toBeFocused();
  });
});

test('AC-01.4 免税で税区分の欄を隠している間は、科目の既定の税区分を入れず空で送る。「詳細」で出すと既定値が入る（2026-10-14 20:10 の決定）', async ({ page }) => {
  await open(page, '#/');
  await withSettings(page, { tax_mode: 'exempt', invoice_number: null }, async () => {
    const desc = uniqueText('E2E 免税の税区分');
    await open(page, '#/entry');
    const f = entryFields(page);
    await f.amount.fill('11000');
    await f.account.click();
    await page.keyboard.insertText('通信費');
    await page.keyboard.press('Enter');
    await f.description.fill(desc);
    await expect(f.preview.locator('table')).toBeVisible();
    await expect(f.preview).not.toContainText('（税');
    await page.getByRole('button', { name: /保存して続ける/ }).click();
    await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
    const saved = (await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.items[0];
    expect(saved.lines.find((l) => l.account === 'exp_comm').tax_code).toBeNull();
    // 「詳細」で欄を出したときだけ、科目の既定の税区分が入る。
    await f.amount.fill('2200');
    await f.account.click();
    await page.keyboard.insertText('通信費');
    await page.keyboard.press('Enter');
    await page.getByRole('button', { name: '詳細: 税区分を出す' }).click();
    await expect(f.tax).toHaveValue('P10');
    await expect(f.preview).toContainText('（税');
  });
});

test('AC-02.8 の画面側: 相手科目が口座なら、税区分と割合の欄を使わない', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('16940');
  await f.account.click();
  await page.keyboard.insertText('サンプルカード');
  await page.keyboard.press('Enter');
  await expect(f.tax).toBeHidden();
  await expect(f.pct).toBeHidden();
  await expect(f.form).toContainText('相手が口座の取引');
  // 預金の口座の名前は、サーバーから引く（模擬サーバーと本物の合成データで名前が違う）。
  const bank = await walletOf(page, 'bank');
  await f.wallet.click();
  await page.keyboard.insertText(bank.name);
  await page.keyboard.press('Enter');
  await expect(f.preview.locator('table')).toBeVisible();
  await expect(f.preview).not.toContainText('（税');
});

test('税区分が経過措置（P10X）のとき、日付の欄の下に課税仕入れの日の案内を出す', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.account.click();
  await page.keyboard.insertText('外注費');
  await page.keyboard.press('Enter');
  const note = page.getByText('課税仕入れの日（商品の引渡し・役務の提供が完了した日）にしてください', { exact: false });
  await expect(note).toBeHidden();
  await f.tax.selectOption('P10X');
  await expect(note).toBeVisible();
  await expect(f.date).toHaveAttribute('aria-describedby', /note/);
  await f.tax.selectOption('P10');
  await expect(note).toBeHidden();
});

test('SC-02 仕訳で入力: 貸借の差を出し、0のときだけ保存できる', async ({ page }) => {
  const desc = uniqueText('E2E 仕訳の形');
  await open(page, '#/entry');
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const form = page.locator('form.entry-form');
  await form.getByLabel('摘要').fill(desc);
  const rows = form.locator('.jl-table tbody tr');
  await expect(rows).toHaveCount(2);
  await rows.nth(0).getByRole('combobox', { name: '科目' }).click();
  await page.keyboard.insertText('工具器具備品');
  await page.keyboard.press('Enter');
  await rows.nth(0).getByLabel('金額').fill('55000');
  const bank = await walletOf(page, 'bank');
  await rows.nth(1).getByRole('combobox', { name: '科目' }).click();
  await page.keyboard.insertText(bank.name);
  await page.keyboard.press('Enter');
  await rows.nth(1).getByLabel('金額').fill('50000');
  await expect(form).toContainText('差 5,000円（貸方が足りません）');
  const save = page.getByRole('button', { name: /保存して続ける/ });
  await expect(save).toBeDisabled();
  await rows.nth(1).getByLabel('金額').fill('55000');
  await expect(form).toContainText('貸借が合っています');
  await expect(save).toBeEnabled();
  await save.click();
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  const saved = (await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.items[0];
  expect(saved.kind).toBe('journal');
  expect(saved.total).toBe(55000);
});

test('SC-02 テンプレートを選ぶと入力欄に入り、いまの入力をテンプレートに登録できる', async ({ page }) => {
  await open(page, '#/entry');
  // 選ぶテンプレートは、試験の中で作る（合成データにあるとは限らない）。
  const card = await walletOf(page, 'card');
  const source = uniqueText('E2E 携帯電話の利用料');
  const made = await apiCall(page, 'POST', '/templates', { name: source, body: { kind: 'expense', amount: 11000, account: 'exp_comm', wallet: card.code, tax_code: 'P10', partner: 'サンプル通信', description: '携帯電話の利用料', business_pct: 70, memo: '' } });
  expect(made.status).toBe(201);
  await page.reload();
  const f = entryFields(page);
  await page.getByRole('button', { name: 'テンプレートから入力' }).click();
  await page.getByRole('button', { name: new RegExp(source) }).click();
  await expect(f.amount).toHaveValue('11,000');
  await expect(f.account).toHaveValue('通信費');
  await expect(f.pct).toHaveValue('70');
  await expect(f.date).toHaveValue(/\(.\)$/);

  const name = uniqueText('E2E テンプレート');
  await page.getByRole('button', { name: 'テンプレートに登録' }).click();
  const dialog = page.getByRole('dialog', { name: 'テンプレートに登録' });
  await dialog.getByLabel('テンプレートの名前').fill(name);
  await page.keyboard.press('Enter');
  await expect(page.locator('.toast').filter({ hasText: name })).toBeVisible();
  const list = (await apiCall(page, 'GET', '/templates')).json.items;
  const t = list.find((x) => x.name === name);
  expect(t.body.account).toBe('exp_comm');
  expect(t.body.date).toBeUndefined();
  await apiCall(page, 'DELETE', `/templates/${t.id}`);
  await apiCall(page, 'DELETE', `/templates/${made.json.id}`);
});

test('読み取り専用（台帳の検証に失敗）のときは、上に帯を出し、保存と取消を止めて理由を出す', async ({ page }) => {
  await page.route('**/api/v1/status', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    body.ledger.verified = false;
    // 本物のサーバーの値（保存の層の broken_link）でも、英字でなく日本語で理由を出す。
    body.ledger.error = { seq: 55, kind: 'broken_link', rule: null };
    body.ledger.events_loaded = 54;
    await route.fulfill({ response: res, json: body });
  });
  await open(page, '#/entry');
  const bannerEl = page.locator('#banner-readonly');
  await expect(bannerEl).toContainText('読み取り専用で開いています');
  await expect(bannerEl).toContainText('55番目の記録に前の記録とのつながりの切れ');
  await expect(bannerEl).not.toContainText('broken_link');
  await fillSupplies(page, uniqueText('E2E 読み取り専用'));
  const save = page.getByRole('button', { name: /保存して続ける/ });
  await expect(save).toBeDisabled();
  await expect(save).toHaveAttribute('aria-describedby', /banner-readonly/);
  await open(page, '#/journal');
  await page.locator('.journal-table tbody tr').first().click();
  await expect(page.getByRole('dialog').getByRole('button', { name: /取り消す/ })).toBeDisabled();
});
