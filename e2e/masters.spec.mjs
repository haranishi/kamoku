// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 登録簿（SC-07 口座、SC-08 勘定科目、SC-09 取引先）: 一覧、追加・編集のダイアログ、型の選択、名前の検索、
// 無効の切り替え、名前を変えられない科目、登録番号の形式の誤りを欄の下に出す。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText } from './helpers.mjs';

test('SC-07 口座: 残高（カードは未払い額）と、元帳・未確認へのリンク。種類を選んで追加し、使わない口座にできる', async ({ page }) => {
  await open(page, '#/wallets');
  const list = page.locator('.wallet-list').first();
  const card = list.locator('.wallet-item', { has: page.locator('.wallet-balance', { hasText: '未払い額' }) }).first();
  await expect(card).toBeVisible();
  const href = await card.getByRole('link', { name: /未確認/ }).getAttribute('href');
  expect(href).toMatch(/^#\/inbox\?wallet=/);
  await expect(card.getByRole('link', { name: 'この口座の元帳' })).toHaveAttribute('href', /^#\/reports\/ledger\?account=/);

  const name = uniqueText('デモ電子マネー');
  await page.getByRole('button', { name: '口座を追加' }).click();
  const dialog = page.getByRole('dialog', { name: '口座を追加' });
  await dialog.locator('.seg-option', { hasText: '電子マネー' }).click();
  await dialog.getByRole('textbox', { name: /^名前/ }).fill(name);
  await page.keyboard.press('Enter');
  await expect(page.locator('.toast').filter({ hasText: `口座「${name}」を追加しました` })).toBeVisible();
  const item = page.locator('.wallet-item', { hasText: name });
  await expect(item).toContainText('電子マネー');
  const saved = (await apiCall(page, 'GET', '/accounts')).json.items.find((a) => a.name === name);
  expect([saved.wallet.type, saved.kind, saved.parent]).toEqual(['ewallet', 'asset', 'cash']);

  await item.getByRole('button', { name: `「${name}」を直す` }).click();
  const edit = page.getByRole('dialog', { name: `「${name}」を直す` });
  await edit.getByRole('checkbox', { name: 'この口座を使う' }).uncheck();
  await edit.getByRole('button', { name: '保存する' }).click();
  await expect(page.locator('.masters-off')).toContainText(name);
});

test('SC-08 勘定科目: 分類ごとに並び、読みで探せる。既定の割合を直せる。名前を変えられない科目と、外せない科目', async ({ page }) => {
  await open(page, '#/accounts');
  for (const k of ['資産', '負債', '資本', '収益', '費用']) await expect(page.getByRole('heading', { name: new RegExp(`^${k}`) })).toBeVisible();
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('つうしん');
  const rows = page.locator('.accounts-table tbody tr');
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('通信費');
  // 行で Enter を押すと、直すダイアログ。
  await rows.first().focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: '「通信費」を直す' });
  const pct = dialog.getByRole('textbox', { name: '既定の事業割合' });
  const before = await pct.inputValue();
  await pct.fill('60');
  await dialog.getByRole('button', { name: '保存する' }).click();
  await expect(page.locator('.toast').filter({ hasText: '「通信費」を保存しました' })).toBeVisible();
  expect((await apiCall(page, 'GET', '/accounts')).json.items.find((a) => a.code === 'exp_comm').default_pct).toBe(60);
  await apiCall(page, 'PATCH', '/accounts/exp_comm', { default_pct: Number(before) });

  // 事業主貸は名前を変えられない（欄が無効）。売上高は外せない。
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('じぎょうぬしかし');
  await page.locator('.accounts-table tbody tr').first().click();
  const owner = page.getByRole('dialog', { name: '「事業主貸」を直す' });
  await expect(owner.getByRole('textbox', { name: /^名前/ })).toBeDisabled();
  await expect(owner).toContainText('名前を変えられません');
  await owner.getByRole('button', { name: 'やめる' }).click();
  // 小窓が閉じきる（フォーカスが行へ戻る）のを待ってから探す欄に打つ（途中でフォーカスが戻ると、打った文字が欄に入らない）。
  await expect(owner).toHaveCount(0);
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('うりあげ');
  await page.locator('.accounts-table tbody tr').first().click();
  const sales = page.getByRole('dialog', { name: '「売上高」を直す' });
  await expect(sales.getByRole('checkbox', { name: 'この科目を使う' })).toBeDisabled();
  await sales.getByRole('button', { name: 'やめる' }).click();
  await expect(sales).toHaveCount(0);

  // 使っていない科目は、切り替えで出る。
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('とうざ');
  await expect(page.locator('.masters')).toContainText('当てはまる科目はありません');
  await page.getByRole('checkbox', { name: '使っていない科目も出す' }).check();
  await expect(page.locator('.accounts-table tbody tr').first()).toContainText('当座預金');
  await expect(page.locator('.accounts-table tbody tr').first()).toContainText('使っていない');
});

test('SC-09 取引先: 登録番号の形式の誤りを欄の下に出し、全角や小文字でもそろえて保存する。同じ名前は断る', async ({ page }) => {
  await open(page, '#/partners');
  const name = uniqueText('デモ取引先');
  await page.getByRole('button', { name: '取引先を足す' }).click();
  const dialog = page.getByRole('dialog', { name: '取引先を足す' });
  await dialog.getByRole('textbox', { name: /^名前/ }).fill(name);
  const inv = dialog.getByRole('textbox', { name: /登録番号/ });
  await inv.fill('T123');
  await dialog.getByRole('button', { name: '足す' }).click();
  const err = dialog.locator('.field-error').filter({ hasText: 'T と13桁' });
  await expect(err).toBeVisible();
  await expect(inv).toHaveAttribute('aria-invalid', 'true');
  await expect(inv).toBeFocused();
  await inv.fill('ｔ１２３４５６７８９０１２３');
  await dialog.getByRole('button', { name: '足す' }).click();
  await expect(page.locator('.toast').filter({ hasText: `取引先「${name}」を足しました` })).toBeVisible();
  const saved = (await apiCall(page, 'GET', '/partners')).json.items.find((p) => p.name === name);
  expect(saved.invoice_number).toBe('T1234567890123');
  await expect(page.locator('.partners-table')).toContainText(name);

  // 同じ名前は、サーバーが断る。理由は名前の欄の下に出る。
  await page.getByRole('button', { name: '取引先を足す' }).click();
  const again = page.getByRole('dialog', { name: '取引先を足す' });
  await again.getByRole('textbox', { name: /^名前/ }).fill(name);
  await again.getByRole('button', { name: '足す' }).click();
  await expect(again.locator('.field-error').filter({ hasText: 'すでにあります' })).toBeVisible();
  await again.getByRole('button', { name: 'やめる' }).click();
});

// SC-07 口座を追加する小窓は、言葉が「種類」「追加」。名前・読み・金融機関の例は、選んだ種類に合わせて変わる
// （カードを選んでも、銀行の例のままだった）。
test('SC-07 口座を追加する小窓: 「種類」を選ぶと、名前などの例が種類に合わせて変わる', async ({ page }) => {
  await open(page, '#/wallets');
  await page.getByRole('button', { name: '口座を追加' }).click();
  const dialog = page.getByRole('dialog', { name: '口座を追加' });
  await expect(dialog.getByRole('group', { name: '口座の種類' })).toBeVisible();
  await expect(dialog).not.toContainText('型');
  const nameIn = dialog.getByRole('textbox', { name: /^名前/ });
  const readingIn = dialog.getByRole('textbox', { name: /^読み/ });
  for (const [type, example, reading] of [['クレジットカード', 'サンプルカード', 'さんぷるかーど'], ['現金', '現金', 'げんきん'], ['電子マネー', 'サンプル電子マネー', 'さんぷるでんしまねー'], ['預金', 'サンプル銀行 普通預金', 'さんぷるぎんこう']]) {
    await dialog.locator('.seg-option', { hasText: type }).click();
    await expect(nameIn, type).toHaveAttribute('placeholder', `例: ${example}`);
    await expect(readingIn, type).toHaveAttribute('placeholder', `例: ${reading}`);
  }
  await expect(dialog.getByRole('button', { name: '追加', exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'やめる' }).click();
  await expect(dialog).toHaveCount(0);
});

// SC-07 口座ごとに並ぶ「この口座の元帳」と「未確認 0件」のリンクは、見える文字はそのままで、読み上げの名前に口座の名前を入れる
// （同じ名前のリンクが並び、どの口座のものか分からなかった）。幅640未満の短い「元帳」でも同じ。
for (const width of [1440, 390]) {
  test(`SC-07 口座のリンクは、幅${width}pxでも、読み上げの名前が口座ごとに違う（見える文字を含む）`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await open(page, '#/wallets');
    const items = page.locator('.wallet-list').first().locator('.wallet-item');
    await expect(items.first()).toBeVisible();
    const names = [];
    for (let i = 0; i < (await items.count()); i += 1) {
      const item = items.nth(i);
      const wallet = (await item.locator('.wallet-name .keep').innerText()).trim();
      const visible = width < 640 ? '元帳' : 'この口座の元帳';
      const esc = wallet.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const ledger = item.getByRole('link', { name: new RegExp(`^${visible} ?（${esc}）$`) });
      await expect(ledger, wallet).toHaveCount(1);
      const drafts = item.getByRole('link', { name: new RegExp(`^未確認 ?[0-9,]+件 ?（${esc}）$`) });
      await expect(drafts, wallet).toHaveCount(1);
      names.push(wallet);
    }
    expect(names.length, '口座が2つ以上ある（試験の前提）').toBeGreaterThan(1);
    expect(new Set(names).size, '口座の名前が重ならない（試験の前提）').toBe(names.length);
  });
}

// 見た目の採点 第2回: 勘定科目の一覧で、毎行の「既定の税区分」「既定の割合」が太字で反復され、科目名より強く見えた（幅390）。
// 名前の下の読みとコードが、小さく近接していた（幅768）。見出しは細字の補助の色にし、読みとコードの間は間隔と区切りの線を足す。
test('SC-08 勘定科目: 幅390の税区分と割合の見出しは太字にせず、幅768の読みとコードの間に区切りを置く', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/accounts');
  const td = page.locator('section[aria-labelledby="kind-expense"] td.acct-tax').first();
  await expect(td).toBeVisible();
  const [label, name] = await td.evaluate((el) => [getComputedStyle(el, '::before').fontWeight, getComputedStyle(el.closest('tr').querySelector('.acct-name')).fontWeight]);
  expect(Number(label), '見出しは細字').toBeLessThan(600);
  expect(Number(name), '科目名は太字のまま').toBeGreaterThanOrEqual(600);
  await page.setViewportSize({ width: 768, height: 1024 });
  const sub = page.locator('.accounts-table .acct-sub').first();
  await expect(sub).toBeVisible();
  const s = await sub.evaluate((el) => {
    const [reading, code] = el.children;
    return { gap: code.getBoundingClientRect().left - reading.getBoundingClientRect().right, border: getComputedStyle(code).borderLeftWidth };
  });
  expect(s.gap, '読みとコードの間').toBeGreaterThanOrEqual(12);
  expect(s.border).toBe('1px');
});
