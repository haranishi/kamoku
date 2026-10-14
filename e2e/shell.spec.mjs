// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 共通の枠: コマンド一覧、ショートカットの一覧、ライトとダーク、フォーカスの輪、動きを減らす設定。

import { test, expect } from '@playwright/test';
import { open, ensureEntryWith } from './helpers.mjs';

// 色の文字（rgb(...)）を [r, g, b] にして、WCAGのコントラスト比を出す。
function ratio(a, b) {
  const lum = (rgb) => {
    const [r, g, bl] = rgb.match(/\d+(\.\d+)?/g).slice(0, 3).map(Number).map((v) => {
      const c = v / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

const bodyBg = (page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test('AC-10.1 Ctrl/Cmd+K のコマンド一覧から、画面を移り、仕訳を文字で探せる', async ({ page }) => {
  await open(page, '#/');
  // 本物のサーバーの合成データには「家賃」の仕訳が無いことがあるので、無ければ作る。
  await ensureEntryWith(page, '家賃');
  await page.keyboard.press('ControlOrMeta+K');
  const dialog = page.getByRole('dialog', { name: 'コマンド' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('combobox')).toBeFocused();
  await page.keyboard.insertText('しわけちょう');
  await expect(dialog.getByRole('option').first()).toContainText('仕訳帳');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(dialog).toHaveCount(0);

  await page.keyboard.press('ControlOrMeta+K');
  await page.keyboard.insertText('家賃');
  const search = page.getByRole('option', { name: /仕訳を「家賃」で探す/ });
  await expect(search).toBeVisible();
  await search.click();
  await expect(page).toHaveURL(/#\/journal\?q=/);
  await expect(page.locator('.journal-table tbody tr').first()).toContainText('家賃');

  // U2 の画面（帳票）へも移れる。「帳票」は月次推移のタブに着く。
  await page.keyboard.press('ControlOrMeta+K');
  await page.keyboard.insertText('帳票');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/reports\/monthly/);
  await expect(page.locator('main h1')).toHaveText('帳票');
  await expect(page.getByRole('tab', { name: '月次推移' })).toHaveAttribute('aria-selected', 'true');

  // 帳票のタブへ直に移るコマンドもある。
  await page.keyboard.press('ControlOrMeta+K');
  await page.keyboard.insertText('しさんひょう');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/reports\/trial-balance/);

  // どの画面にも当たらないアドレスは「見つかりません」の画面に着く。
  await page.goto('/#/nowhere');
  await expect(page.locator('main')).toContainText('このアドレスの画面はありません');
});

test('AC-10.2 ? でショートカットの一覧が出て、Escで閉じる', async ({ page }) => {
  await open(page, '#/');
  await page.keyboard.press('?');
  const dialog = page.getByRole('dialog', { name: 'ショートカット' });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('仕訳帳へ');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('AC-11.3 OSの設定に従い、手動でもライトとダークを変えられる', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await open(page, '#/');
  expect(await bodyBg(page)).toBe('rgb(16, 21, 27)');
  await page.emulateMedia({ colorScheme: 'light' });
  expect(await bodyBg(page)).toBe('rgb(246, 245, 241)');

  // 手動の切り替え（OS → ライト → ダーク）。OSがライトでも、ダークを選べばダークになる。
  const toggle = page.locator('.sidebar [data-action="theme"]');
  await toggle.click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await toggle.click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await bodyBg(page)).toBe('rgb(16, 21, 27)');
  // 再読込しても、選んだ明暗が残る。
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await toggle.click();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /./);
});

for (const scheme of ['light', 'dark']) {
  test(`AC-10.3 AC-11.3 フォーカスの輪が見える（${scheme}、背景との差が3:1以上）`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page, '#/entry');
    await page.locator('main h1').focus();
    await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => {
      const el = document.activeElement;
      const s = getComputedStyle(el);
      let bg = 'rgba(0, 0, 0, 0)';
      for (let n = el.parentElement; n; n = n.parentElement) {
        const c = getComputedStyle(n).backgroundColor;
        if (c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent') {
          bg = c;
          break;
        }
      }
      return { style: s.outlineStyle, width: s.outlineWidth, color: s.outlineColor, bg, tag: el.tagName };
    });
    expect(ring.style).toBe('solid');
    expect(parseFloat(ring.width)).toBeGreaterThanOrEqual(2);
    expect(ratio(ring.color, ring.bg)).toBeGreaterThanOrEqual(3);
  });
}

test('AC-11.4 動きを減らす設定では、ダイアログの動きが止まる', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '#/');
  await page.keyboard.press('?');
  const dur = await page.getByRole('dialog').evaluate((el) => getComputedStyle(el).animationDuration);
  expect(dur).toBe('0s');
  await page.keyboard.press('Escape');

  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.keyboard.press('?');
  const moving = await page.getByRole('dialog').evaluate((el) => getComputedStyle(el).animationDuration);
  expect(moving).not.toBe('0s');
});

test('768px未満では、下のバーの「メニュー」から全部の画面へ行ける', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/');
  await page.locator('.bottomnav').getByRole('button', { name: 'メニュー' }).click();
  const menu = page.getByRole('dialog', { name: 'メニュー' });
  await expect(menu).toBeVisible();
  await menu.getByRole('link', { name: '勘定科目' }).click();
  await expect(menu).toHaveCount(0);
  await expect(page).toHaveURL(/#\/accounts$/);
  await expect(page.locator('.bottomnav [aria-current="page"]')).toHaveCount(0);
  await page.locator('.bottomnav').getByRole('link', { name: '仕訳帳' }).click();
  await expect(page.locator('.bottomnav [aria-current="page"]')).toHaveText('仕訳帳');
});

test('768〜1023pxでは、細いサイドバーを広げて名前を全部出せる', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 900 });
  await open(page, '#/');
  const app = page.locator('#app');
  const toggle = page.locator('.rail-toggle');
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await expect(page.locator('.sidebar').getByText('取引を登録')).toBeHidden();
  await toggle.click();
  await expect(app).toHaveAttribute('data-rail', 'open');
  await expect(page.locator('.sidebar').getByText('取引を登録')).toBeVisible();
  await toggle.click();
  await expect(app).toHaveAttribute('data-rail', 'closed');
});

test('読み込みが300ms以上かかるときだけ骨組みを出す', async ({ page }) => {
  await page.route('**/api/v1/reports/dashboard*', async (route) => {
    await new Promise((r) => setTimeout(r, 900));
    await route.continue();
  });
  await open(page, '#/');
  await expect(page.locator('.skel-block').first()).toBeVisible();
  await expect(page.locator('.kpis')).toBeVisible({ timeout: 5000 });
  await expect(page.locator('.skel-block')).toHaveCount(0);
  await page.unroute('**/api/v1/reports/dashboard*');
  // すぐ終わる読み込みでは、骨組みは一度も出ない。
  await page.evaluate(() => {
    window.skelSeen = false;
    new MutationObserver(() => {
      if (document.querySelector('.skel-block')) window.skelSeen = true;
    }).observe(document.body, { childList: true, subtree: true });
  });
  await page.locator('.month-picker').getByRole('button', { name: '前の月' }).click();
  await expect(page.locator('.kpis')).toBeVisible();
  expect(await page.evaluate(() => window.skelSeen)).toBe(false);
});

test('デモのときは、画面の上に「デモのデータです」と常に出す', async ({ page }) => {
  await page.route('**/api/v1/status', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    body.mode = 'demo';
    await route.fulfill({ response: res, json: body });
  });
  await open(page, '#/journal');
  await expect(page.locator('#banners')).toContainText('デモのデータです');
});

test('本文へ移動のリンクが文書の先頭にあり、押すと本文の見出しへフォーカスが移る', async ({ page }) => {
  await open(page, '#/journal');
  const first = await page.evaluate(() => {
    const el = document.querySelector('a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])');
    return el && el.textContent.trim();
  });
  expect(first).toBe('本文へ移動');
  const skip = page.getByRole('link', { name: '本文へ移動' });
  await skip.focus();
  await expect(skip).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.locator('main h1')).toBeFocused();
  await expect(page).toHaveURL(/#\/journal$/);
});
