// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 日本語入力（keyboard.md の「IMEの決めごと」）。Chromium の変換の模擬（Input.imeSetComposition）で確かめる。
// Safari と Firefox、Windows の変換は、公開の前に人が確かめる（自動の試験は Chromium の範囲）。

import { test, expect } from '@playwright/test';
import { open, entryFields, uniqueText, apiCall } from './helpers.mjs';

test('AC-05.2 科目の欄で、変換中のEnterでは候補が選ばれず、変換を確定してからのEnterで選ばれる', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.account.click();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.imeSetComposition', { text: 'つうしん', selectionStart: 4, selectionEnd: 4 });
  await expect(page.getByRole('option', { name: /通信費/ })).toBeVisible();

  // 変換中のEnter: 候補は選ばれず、次の欄へも進まない。
  await page.keyboard.press('Enter');
  await expect(f.account).toBeFocused();
  await expect(f.account).toHaveValue('つうしん');
  await expect(f.account).toHaveAttribute('aria-expanded', 'true');

  // 変換を確定する。確定のEnterと見分けるため、50msの窓より後に押す。
  await cdp.send('Input.insertText', { text: 'つうしん' });
  await page.waitForTimeout(120);
  await page.keyboard.press('Enter');
  await expect(f.account).toHaveValue('通信費');
  await expect(f.wallet).toBeFocused();
});

test('AC-05.2 変換中の文字（ひらがなの読み）でも候補が絞られる', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.account.click();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.imeSetComposition', { text: 'しょうもう', selectionStart: 5, selectionEnd: 5 });
  const options = page.getByRole('option');
  await expect(options.first()).toContainText('消耗品費');
  await expect(page.getByRole('option', { name: /通信費/ })).toHaveCount(0);
  await cdp.send('Input.insertText', { text: 'しょうもう' });
});

test('AC-05.2 compositionend の直後（50ms以内）のEnterは、変換の確定として捨てる（Safariの順序）', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.account.click();
  await page.keyboard.insertText('通信');
  await expect(page.getByRole('option', { name: /通信費/ })).toBeVisible();
  // Safari は、変換を決めるEnterの keydown を、compositionend の後に isComposing 偽で送る。同じ順で、続けて送る。
  const prevented = await f.account.evaluate((el) => {
    el.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '通信' }));
    const ev = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
    el.dispatchEvent(ev);
    return ev.defaultPrevented;
  });
  expect(prevented).toBe(true);
  await expect(f.account).toHaveValue('通信');
  await expect(f.account).toBeFocused();
  // 窓を過ぎてからのEnterは、ふつうに候補を選ぶ。
  await page.waitForTimeout(120);
  await page.keyboard.press('Enter');
  await expect(f.account).toHaveValue('通信費');
});

test('AC-05.2 押しっぱなしのEnter（repeat）では保存しない', async ({ page }) => {
  const desc = uniqueText('E2E 押しっぱなし');
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('500');
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await f.description.fill(desc);
  await f.pct.focus();
  const prevented = await f.pct.evaluate((el) => {
    const ev = new KeyboardEvent('keydown', { key: 'Enter', repeat: true, bubbles: true, cancelable: true });
    el.dispatchEvent(ev);
    return ev.defaultPrevented;
  });
  expect(prevented).toBe(true);
  await page.waitForTimeout(400);
  expect((await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.total).toBe(0);
  // 押しっぱなしでない1回のEnterなら、保存する。
  await page.keyboard.press('Enter');
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  expect((await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.total).toBe(1);
});

test('AC-10.4 文字入力の欄にいるとき（変換中を含む）は、n や g の1文字のキーで画面が移らない', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.description.click();
  await page.keyboard.type('ngj?');
  await expect(f.description).toHaveValue('ngj?');
  await expect(page).toHaveURL(/#\/entry$/);
  await expect(page.getByRole('dialog')).toHaveCount(0);

  // 変換中のキーも働かない。
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.imeSetComposition', { text: 'ん', selectionStart: 1, selectionEnd: 1 });
  await page.keyboard.press('n');
  await expect(page).toHaveURL(/#\/entry$/);
  await cdp.send('Input.insertText', { text: 'ん' });

  // 文字入力の欄の外なら、g j で仕訳帳へ移る（同じキーが働くことの確かめ）。
  // 入力が残っていると、離れる前の確認（entry-guard.spec）が出て移らないので、入力を消してから試す。
  await f.description.fill('');
  await page.locator('main h1').focus();
  await page.keyboard.press('g');
  await page.keyboard.press('j');
  await expect(page).toHaveURL(/#\/journal$/);
  // アドレスは、離れる前の確認で戻される前の一瞬だけ #/journal になることがある（それを拾って緑になった）。画面が移ったことまで確かめる。
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
