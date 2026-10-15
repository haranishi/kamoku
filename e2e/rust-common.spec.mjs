// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, walletOf, importRows } from './helpers.mjs';

for (const storage of ['throw-getter', 'throw-methods', 'missing']) {
  test(`AC-10.5 localStorage ${storage}: keys and theme still work for the session`, async ({ page }) => {
    await page.addInitScript((mode) => {
      if (mode === 'throw-getter') Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new DOMException('synthetic denied', 'SecurityError'); } });
      else Object.defineProperty(window, 'localStorage', { configurable: true, value: mode === 'missing' ? undefined : {
        getItem() { throw new DOMException('synthetic denied', 'SecurityError'); },
        setItem() { throw new DOMException('synthetic denied', 'SecurityError'); },
      } });
    }, storage);
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await open(page);
    await page.locator('.sidebar [data-action="theme"]').click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await page.keyboard.press('?');
    const help = page.getByRole('dialog', { name: 'ショートカット' });
    await help.getByRole('checkbox', { name: '1文字のショートカットを使う' }).uncheck();
    await page.keyboard.press('Escape');
    await expect(help).toHaveCount(0);
    await page.locator('main h1').focus();
    await page.keyboard.press('n');
    await expect(page).toHaveURL(/#\/$/);
    await page.keyboard.press('ControlOrMeta+K');
    const palette = page.getByRole('dialog', { name: 'コマンド' });
    await palette.getByRole('combobox').fill('いちもじ');
    await palette.getByRole('option').first().click();
    await expect(palette).toHaveCount(0);
    await page.locator('main h1').focus();
    await page.keyboard.press('n');
    await expect(page.locator('form.entry-form')).toBeVisible();
    expect(errors).toEqual([]);
  });
}

test('SC-03 a focused notification survives mouseleave and receives its full lifetime on focusout', async ({ page }) => {
  await open(page);
  await page.clock.install();
  await page.locator('.sidebar [data-action="theme"]').click();
  const toast = page.locator('.toast').filter({ hasText: '表示を「ライト」にしました' });
  await expect(toast).toBeVisible();
  const close = toast.getByRole('button', { name: 'この通知を閉じる' });
  await close.focus();
  await toast.hover();
  await page.mouse.move(0, 0);
  await page.clock.runFor(30_000);
  await expect(close).toBeFocused();
  await expect(toast).toBeVisible();
  await page.locator('main h1').focus();
  await page.clock.runFor(3999);
  await expect(toast).toBeVisible();
  await page.clock.runFor(300);
  await expect(toast).toHaveCount(0);
});

test('SC-03 undo after leaving inbox restores the record and keeps the current screen', async ({ page }) => {
  await open(page);
  const name = uniqueText('E2E 離脱後Undo').replace(/\s+/g, '-');
  const wallet = await walletOf(page, 'card');
  await importRows(page, wallet.code, [['2026-01-02', name, '1230', '']]);
  const drafts = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(name)}`)).json.items;
  expect(drafts).toHaveLength(1);
  await open(page, `#/inbox?q=${encodeURIComponent(name)}`);
  await page.locator('tr.inbox-row').getByRole('button', { name: /^無視/ }).click();
  const toast = page.locator('.toast').filter({ hasText: 'を無視しました' });
  await expect(toast).toBeVisible();
  await page.locator('.sidebar').getByRole('link', { name: 'ホーム', exact: true }).click();
  await expect(page.locator('main h1')).toHaveText('ホーム');
  await toast.getByRole('button', { name: '元に戻す' }).click();
  await expect.poll(async () => (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(name)}`)).json.items.map((row) => row.id)).toEqual([drafts[0].id]);
  await expect(page).toHaveURL(/#\/$/);
  await expect(page.locator('main h1')).toHaveText('ホーム');
});

test('SC-02 notifications follow save-bar size, visibility and replacement, then release observers', async ({ page }) => {
  await page.addInitScript(() => {
    const watches = { ResizeObserver: [], MutationObserver: [] };
    window.__kamokuNoticeWatches = watches;
    for (const kind of Object.keys(watches)) {
      const Native = window[kind];
      window[kind] = class extends Native {
        constructor(callback) {
          super(callback);
          this.watch = { targets: new Set(), options: null };
          watches[kind].push(this.watch);
        }
        observe(target, options) {
          super.observe(target, options);
          this.watch.targets.add(target);
          this.watch.options = options;
        }
        disconnect() {
          super.disconnect();
          this.watch.targets.clear();
        }
      };
    }
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/entry');
  const bar = page.locator('.entry-actions-card[data-bottom-bar]');
  await expect(bar).toBeVisible();
  // Pin the real save bar for a controlled geometry change; its usual layout is covered by shell/entry-layout.
  // No scroll, focus or resize event is sent to wake the notifier after the notification appears.
  await bar.evaluate((el) => {
    el.style.position = 'fixed';
    el.style.bottom = 'var(--bar-bottom)';
    el.style.left = '0';
    el.style.right = '0';
    el.style.height = '120px';
    el.style.minHeight = '0';
    el.id = 'notice-bar-fixture';
    const sheet = [...document.styleSheets].find((sheet) => sheet.href && new URL(sheet.href).origin === location.origin);
    sheet.insertRule('#notice-bar-fixture.notice-bar-hidden { visibility: hidden; }', sheet.cssRules.length);
  });
  await page.locator('.topbar [data-action="theme"]').click();
  const toast = page.locator('.toast').filter({ hasText: '表示を「' });
  await expect(toast).toBeVisible();
  await toast.getByRole('button', { name: 'この通知を閉じる', exact: true }).focus();
  const lift = () => page.locator('#toasts').evaluate((el) => Number.parseFloat(el.style.getPropertyValue('--toast-lift')));
  await expect.poll(lift).toBe(120);
  await bar.evaluate((el) => { el.style.height = '200px'; });
  await expect.poll(lift).toBe(200);
  await bar.evaluate((el) => { el.hidden = true; });
  await expect.poll(lift).toBe(0);
  await bar.evaluate((el) => { el.hidden = false; });
  await expect.poll(lift).toBe(200);
  await bar.evaluate((el) => el.classList.add('notice-bar-hidden'));
  await expect.poll(lift).toBe(0);
  await bar.evaluate((el) => el.classList.remove('notice-bar-hidden'));
  await expect.poll(lift).toBe(200);
  await bar.evaluate((el) => {
    const replacement = el.cloneNode(true);
    replacement.style.height = '160px';
    el.replaceWith(replacement);
  });
  await expect.poll(lift).toBe(160);
  await bar.evaluate((el) => { el.style.height = '180px'; });
  await expect.poll(lift).toBe(180);
  await toast.getByRole('button', { name: 'この通知を閉じる', exact: true }).click();
  await expect(page.locator('#toasts .toast')).toHaveCount(0);
  await expect.poll(lift).toBe(0);
  expect(await page.evaluate(() => {
    const watches = window.__kamokuNoticeWatches;
    return {
      bars: watches.ResizeObserver.filter((watch) => [...watch.targets].some((el) => el.matches('[data-bottom-bar]'))).length,
      body: watches.MutationObserver.filter((watch) => watch.targets.has(document.body) && watch.options?.attributeFilter?.includes('open')).length,
    };
  })).toEqual({ bars: 0, body: 0 });
});
