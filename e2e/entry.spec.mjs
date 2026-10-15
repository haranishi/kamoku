// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録（SC-02）。キーボードだけの登録、続けて入力、按分とプレビュー、金額の式と桁区切り、日付の形、割合の出どころ。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, withSettings, withAccountDefaults, todayTokyo } from './helpers.mjs';

test('AC-02.1 AC-02.5 キーボードだけで1件登録し、種別・日付・口座を残して金額へ戻る', async ({ page }) => {
  const desc = uniqueText('E2E 携帯電話');
  // 日付は今日の月日で打つ（31日より先の日付は、保存の前に確認が出るため。その確認は entry-guard.spec.mjs）。
  const [, mm, dd] = todayTokyo().split('-');
  await open(page, '#/entry');
  // 通信費の既定値（課税仕入10%・割合70%）を、この試験の間だけ入れる（本物の合成データは割合100%のため）。
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 70 }, async () => {
    // 画面は科目を開いたときに読むので、読み込み直して新しい既定値を使わせる。
    await page.reload();
    await expect(page.locator('main h1')).toBeVisible();
    const f = entryFields(page);
    // 開いたときは、日付の欄にいる（すぐ打ち始められる）。種別の欄は、その1つ前。
    await expect(f.date).toBeFocused();
    await page.keyboard.press('Shift+Tab');

    // 種別の欄で 1 を押すと支出。ここからはEnterだけで欄を進める。
    await page.keyboard.press('1');
    await expect(f.kind('支出')).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(f.date).toBeFocused();
    await page.keyboard.type(`${mm}${dd}`);
    await page.keyboard.press('Enter');
    await expect(f.amount).toBeFocused();
    await page.keyboard.type('11000');
    await page.keyboard.press('Enter');
    await expect(f.account).toBeFocused();
    await page.keyboard.insertText('つうしん');
    await expect(page.getByRole('option', { name: /通信費/ })).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(f.account).toHaveValue('通信費');
    await expect(f.wallet).toBeFocused();
    await page.keyboard.insertText('サンプルカード');
    await page.keyboard.press('Enter');
    await expect(f.wallet).toHaveValue('サンプルカード');
    await expect(f.tax).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(f.partner).toBeFocused();
    await page.keyboard.insertText('サンプル通信');
    await page.keyboard.press('Enter');
    await expect(f.description).toBeFocused();
    await page.keyboard.insertText(desc);
    await page.keyboard.press('Enter');
    await expect(f.pct).toBeFocused();
    // 通信費の既定の割合（70%）が入っていて、プレビューに事業主貸 3,300円が出ている。
    await expect(f.pct).toHaveValue('70');
    await expect(f.preview).toContainText('3,300');
    await page.keyboard.press('Enter');

    await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
    // 続けて入力: 金額へ戻り、種別・日付・口座は残る（AC-02.5）。
    await expect(f.amount).toBeFocused();
    await expect(f.amount).toHaveValue('');
    await expect(f.kind('支出')).toBeChecked();
    await expect(f.date).toHaveValue(new RegExp(`-${mm}-${dd}`));
    await expect(f.wallet).toHaveValue('サンプルカード');
    await expect(f.account).toHaveValue('');

    // 保存された仕訳は、プレビューと同じ明細（AC-02.2 の画面側）。
    const res = await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}&state=all`);
    expect(res.json.total).toBe(1);
    const lines = res.json.items[0].lines.map((l) => [l.account, l.side, l.amount, l.tax_amount]);
    expect(lines).toEqual([
      ['exp_comm', 'debit', 7700, 700],
      ['owner_draw', 'debit', 3300, 0],
      ['w03', 'credit', 11000, 0],
    ]);
  });
});

test('AC-03.4 割合を変えると、仕訳のプレビューも変わる', async ({ page }) => {
  await open(page, '#/entry');
  // 通信費の既定値（課税仕入10%・割合70%）を、この試験の間だけ入れる（本物の合成データは割合100%のため）。
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 70 }, async () => {
    // 画面は科目を開いたときに読むので、読み込み直して新しい既定値を使わせる。
    await page.reload();
    await expect(page.locator('main h1')).toBeVisible();
    const f = entryFields(page);
    await f.amount.fill('11000');
    await f.account.click();
    await page.keyboard.insertText('通信費');
    await page.keyboard.press('Enter');
    await expect(f.pct).toHaveValue('70');
    await expect(f.preview).toContainText('7,700');
    await expect(f.preview).toContainText('3,300');
    await f.pct.fill('50');
    await expect(f.preview).toContainText('5,500');
    await expect(f.preview).not.toContainText('7,700');
    await expect(page.locator('.pct-out')).toHaveText('→ 事業主貸 5,500円');
    await f.pct.fill('100');
    await expect(f.preview).not.toContainText('事業主貸');
  });
});

test('AC-02.3 金額に「1000+650」を入れると、離れたときに 1,650 になる', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('1000+650');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('1,650');
  await expect(f.form).toContainText('1000+650 = 1,650円');
  await f.amount.fill('１,６５０');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('1,650');
  await f.amount.fill('100/3');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('33');
  await expect(f.form).toContainText('切り捨てました');
});

test('AC-02.4 日付に「1005」を入れると、その年の10月5日になる。t と上下の矢印も効く', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.date.click();
  await f.date.fill('1005');
  await f.date.press('Tab');
  await expect(f.date).toHaveValue(/^\d{4}-10-05 \(.\)$/);
  await f.date.focus();
  await page.keyboard.press('ArrowUp');
  await expect(f.date).toHaveValue(/-10-06$/);
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(f.date).toHaveValue(/-10-04$/);
  await page.keyboard.press('t');
  const today = await page.evaluate(() => {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  });
  await expect(f.date).toHaveValue(today);
  await f.date.fill('0230');
  await f.date.press('Tab');
  await expect(f.form).toContainText('2月30日はありません');
});

test('AC-02.3 桁区切りの位置が違う金額（1,2,3）は断る。正しい区切り（1,234、12,345,678）と全角は通す', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('1,2,3');
  await f.amount.press('Tab');
  await expect(f.form).toContainText('桁区切りの位置が違います。');
  await expect(f.amount).toHaveAttribute('aria-invalid', 'true');
  for (const [text, shown] of [['1,234', '1,234'], ['12,345,678', '12,345,678'], ['１，２３４', '1,234']]) {
    await f.amount.fill(text);
    await f.amount.press('Tab');
    await expect(f.amount).toHaveValue(shown);
    await expect(f.amount).not.toHaveAttribute('aria-invalid', /./);
  }
  await expect(f.form).not.toContainText('桁区切りの位置が違います。');
});

test('AC-02.3 値の入った金額の欄をマウスで押すと、中身を全部選ぶ（打つと前の値につながらず置き換わる）', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('1100');
  await f.description.click();
  await expect(f.amount).toHaveValue('1,100');
  await f.amount.click();
  await page.keyboard.type('500');
  await expect(f.amount).toHaveValue('500');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('500');
});

test('AC-03.4 事業の割合が範囲外（150、-10、abc）の間は、プレビューに100%の仕訳を出さず、誤りを出す', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('11000');
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await expect(f.preview.locator('table')).toBeVisible();
  for (const bad of ['150', '-10', 'abc']) {
    await f.pct.fill(bad);
    await expect(f.preview).toContainText('事業の割合は、0から100の整数で入れてください。');
    await expect(f.preview.locator('table')).toHaveCount(0);
  }
  // 欄を離れると、欄の下にも誤りが出る。科目を変えて割合が既定の値に入り直したら、その誤りの表示は残さない。
  await f.description.click();
  await expect(f.pct).toHaveAttribute('aria-invalid', 'true');
  await f.account.click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('通信費');
  await page.keyboard.press('Enter');
  await expect(f.pct).not.toHaveAttribute('aria-invalid', /./);
  await expect(f.form).not.toContainText('事業の割合は、0から100の整数で入れてください。');
  await f.pct.fill('50');
  await expect(f.preview.locator('table')).toBeVisible();
  await expect(f.preview).toContainText('5,500');
  await expect(f.preview).not.toContainText('事業の割合は、0から100の整数で入れてください。');
});

test('AC-03.4 事業の割合の欄は、Enterで入っても全体が選ばれ、打つと置き換わる。範囲外になった時点で、欄の下に誤りが出る', async ({ page }) => {
  // Enterで入ると文字が選ばれず、既定の割合に打った数字が足されて「3070」になった（体験の評価 第2回 E1）。
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('6000');
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await expect(f.wallet).toBeFocused();
  await f.description.fill('ガソリン代');
  await page.keyboard.press('Enter');
  await expect(f.pct).toBeFocused();
  // 摘要に文字があっても、割合の中身は全部選ばれている。
  expect(await f.pct.evaluate((el) => [el.selectionStart, el.selectionEnd, el.value.length])).toEqual([0, 3, 3]);
  await page.keyboard.type('70');
  await expect(f.pct).toHaveValue('70');
  // 範囲外になった時点で、欄のすぐ下に誤りが出る（保存やプレビューまで待たない）。直したら消える。
  await page.keyboard.type('1');
  await expect(f.pct).toHaveValue('701');
  await expect(f.pct).toHaveAttribute('aria-invalid', 'true');
  const pctField = f.form.locator('.field:has(.pct-input)');
  await expect(pctField.locator('.field-error')).toContainText('0から100の整数で入れてください。');
  await page.keyboard.press('Backspace');
  await expect(f.pct).not.toHaveAttribute('aria-invalid', /./);
  await expect(pctField.locator('.field-error')).toHaveText('');
});

test('AC-02.9 科目の候補に、口座の親の「現金」は出ない。口座の「現金」だけが出る（かんたん入力も、仕訳で入力も）', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.account.click();
  await page.keyboard.insertText('げんきん');
  const cash = page.getByRole('option', { name: /現金/ });
  await expect(cash).toHaveCount(1);
  await expect(cash.first()).toContainText('口座・現金');
  await page.keyboard.press('Escape');
  // 仕訳で入力の行の科目も同じ。
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const row = f.form.locator('.jl-table tbody tr').nth(0);
  await row.getByRole('combobox', { name: '科目' }).click();
  await page.keyboard.insertText('げんきん');
  await expect(page.getByRole('option', { name: /現金/ })).toHaveCount(1);
  await expect(page.getByRole('option', { name: /現金/ }).first()).toContainText('口座・現金');
  // 絞り込みの欄（仕訳帳）では、親の科目も探せる。
  await page.keyboard.press('Escape');
});

test('SC-02 打った文字に合う科目が無いまま Enter を押しても、次の欄へ進まず、「合う科目がありません」を残す', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('1200');
  await f.account.click();
  await page.keyboard.insertText('ぞぞぞぞ');
  const note = f.form.locator('.combo-msg').filter({ hasText: '合う科目がありません。ひらがなの読みか、科目の名前で探せます。' });
  await expect(note).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(f.account).toBeFocused();
  await expect(f.account).toHaveValue('ぞぞぞぞ');
  await expect(note).toBeVisible();
  // 一言を閉じた後でも、Enterで出し直す（黙って進まない）。
  await page.keyboard.press('Escape');
  await expect(note).toBeHidden();
  await page.keyboard.press('Enter');
  await expect(f.account).toBeFocused();
  await expect(note).toBeVisible();
  // 合う候補があれば、いつもどおり選んで次へ進む。
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('しょうもう');
  await page.keyboard.press('Enter');
  await expect(f.account).toHaveValue('消耗品費');
  await expect(f.wallet).toBeFocused();
});

test('SC-02 保存して続けたら、プレビューはすぐに空になる（前の仕訳を、空のフォームの横に残さない）', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('2200');
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await f.description.fill(uniqueText('E2E プレビューを空に'));
  await expect(f.preview.locator('table')).toBeVisible();
  // 通知が出た直後（同じ処理の続き）に、プレビューの表が残っていないかを、ページの中で記録する。
  await page.evaluate(() => {
    window.__pvLeft = null;
    const box = document.getElementById('toasts');
    new MutationObserver((_, obs) => {
      if (!box.querySelector('.toast')) return;
      window.__pvLeft = Boolean(document.querySelector('.pv-card table'));
      obs.disconnect();
    }).observe(box, { childList: true, subtree: true });
  });
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  expect(await page.evaluate(() => window.__pvLeft)).toBe(false);
  await expect(f.preview).toContainText('日付・金額・科目・口座を入れると');
});

test('AC-03.1 科目の既定の割合が入ったら、出どころを欄の下に出す。プレビューの事業主貸は「私用の分」と書く', async ({ page }) => {
  await open(page, '#/entry');
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 70 }, async () => {
    await withAccountDefaults(page, 'exp_supplies', { default_tax: 'P10', default_pct: 100 }, async () => {
      await page.reload();
      await expect(page.locator('main h1')).toBeVisible();
      const f = entryFields(page);
      await f.amount.fill('11000');
      await f.account.click();
      await page.keyboard.insertText('通信費');
      await page.keyboard.press('Enter');
      await expect(f.pct).toHaveValue('70');
      await expect(f.form).toContainText('この科目の既定の割合は、事業70%です。「勘定科目」の画面で変えられます。');
      await expect(f.pct).toHaveAttribute('aria-describedby', /note/);
      await expect(f.preview.locator('table')).toContainText('私用の分 30%');
      await expect(f.preview).not.toContainText('家事按分');
      // 既定が100%の科目に変えると、一言は消える。
      await f.account.click();
      await page.keyboard.press('ControlOrMeta+a');
      await page.keyboard.insertText('消耗品費');
      await page.keyboard.press('Enter');
      await expect(f.pct).toHaveValue('100');
      await expect(f.form).not.toContainText('この科目の既定の割合は');
    });
  });
});

// ---- 読み上げと Tab（2026-10-15 の体験の評価 E4 の4・7・8） ----

test('SC-02 保存の誤りは、保存の帯の一言（role=alert）に、数と欄の名前で出る。誤りの欄にいるまま保存し直しても、もう一度出る', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  const alert = page.locator('.entry-actions-card [role="alert"]');
  // 金額と科目が空のまま保存する。最初の誤りの欄（金額）へ移る。
  await f.description.fill('E2E 誤りの一言');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(f.amount).toBeFocused();
  await expect(alert).toHaveText('2か所を直してください：金額、科目');
  // 金額の欄にいるまま、もう一度保存する。同じ文でも、一度空になってから入り直す（読み上げに変化として届く）。
  await page.evaluate(() => {
    const el = document.querySelector('.entry-actions-card [role="alert"]');
    window.alertTexts = [];
    new MutationObserver(() => window.alertTexts.push(el.textContent)).observe(el, { childList: true, characterData: true, subtree: true });
  });
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect.poll(() => page.evaluate(() => window.alertTexts)).toEqual(['', '2か所を直してください：金額、科目']);
  await expect(f.amount).toBeFocused();
  // 1か所だけなら「1か所を直してください：…」。
  await f.amount.fill('500');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(alert).toHaveText('1か所を直してください：科目');
  // 直して保存すると消える。
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await f.description.focus();
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  await expect(alert).toHaveText('');
});

// 保存の帯の一言は、保存を試みたときの結果。欄を直したり、入力の形を切り替えたりしたら、古い一言は消す
// （直した欄を指したまま残る・切り替えた先に無い欄を指したまま残る。2026-10-15 の体験の評価 E1）。
test('SC-02 保存の帯の「Nか所を直してください」は、欄を直すか、入力の形を切り替えると消える（古い一言を残さない）', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  const alert = page.locator('.entry-actions-card [role="alert"]');
  await f.description.fill('E2E 古い一言');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(alert).toHaveText('2か所を直してください：金額、科目');
  // 金額を直すと、一言は消える（直した欄を指したまま残らない）。出す順番を待っていた一言も出てこない。
  await f.amount.fill('1200');
  await expect(alert).toHaveText('');
  await page.waitForTimeout(300);
  await expect(alert).toHaveText('');
  // もう一度保存すると、残りの誤りだけが出る。
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(alert).toHaveText('1か所を直してください：科目');
  // 入力の形を切り替えても残らない（切り替えた先に無い欄を指さない）。
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  await expect(alert).toHaveText('');
  await page.getByRole('tab', { name: 'かんたん入力' }).click();
  await expect(alert).toHaveText('');
});

test('SC-02 仕訳で入力: 保存の誤りの一言に、行の番号つきの欄の名前が出る', async ({ page }) => {
  await open(page, '#/entry');
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const rows = page.locator('form.entry-form .jl-table tbody tr');
  await rows.nth(0).getByLabel('金額').fill('1000');
  await rows.nth(0).getByLabel('金額').blur();
  await rows.nth(1).getByLabel('金額').fill('1000');
  await rows.nth(1).getByLabel('金額').focus();
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.locator('.entry-actions-card [role="alert"]')).toHaveText('2か所を直してください：1行目の科目、2行目の科目');
  await expect(rows.nth(0).getByRole('combobox', { name: '1行目 科目' })).toBeFocused();
});

// 押せないボタンに、マウスを乗せたときの見た目（不透明度が0.92に戻る）を効かせない。押せないのに、押せるときとほぼ同じ濃さに
// 見えた（体験の評価 第3回 E2、取込の「0件を取り込む」。ここでは、貸借が合わず止まっている保存のボタンで確かめる）。
test('SC-02 押せない主ボタンは、マウスを乗せても薄いまま（押せるときと同じ濃さに戻らない）', async ({ page }) => {
  await open(page, '#/entry');
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const rows = page.locator('form.entry-form .jl-table tbody tr');
  await rows.nth(0).getByLabel('金額').fill('1000');
  await rows.nth(0).getByLabel('金額').blur();
  const save = page.getByRole('button', { name: /^保存して続ける/ });
  await expect(save).toBeDisabled();
  const opacity = () => save.evaluate((el) => Number(getComputedStyle(el).opacity));
  // 濃さは、短い動き（transition）で変わるので、落ち着くのを待ってから測る。
  await expect.poll(opacity).toBeLessThan(0.7);
  await save.hover({ force: true });
  await page.waitForTimeout(500);
  expect(await opacity(), '乗せても、押せるときの濃さ（0.92）に戻らない').toBeLessThan(0.7);
});

test('SC-02 仕訳で入力: 明細の欄の名前に行の番号が入り、行を足す・消すと振り直す', async ({ page }) => {
  await open(page, '#/entry');
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const form = page.locator('form.entry-form');
  const names = () =>
    form.locator('.jl-table tbody tr').evaluateAll((trs) =>
      trs.map((tr) => [...tr.querySelectorAll('select, input, button')].map((el) => el.getAttribute('aria-label')).filter(Boolean)),
    );
  expect(await names()).toEqual([
    ['1行目 借方か貸方', '1行目 科目', '1行目 金額', '1行目 税区分', '1行目 メモ', '1行目を消す'],
    ['2行目 借方か貸方', '2行目 科目', '2行目 金額', '2行目 税区分', '2行目 メモ', '2行目を消す'],
  ]);
  await form.getByRole('button', { name: '行を足す' }).click();
  await expect(form.getByRole('combobox', { name: '3行目 借方か貸方' })).toBeFocused();
  await form.getByRole('button', { name: '1行目を消す' }).click();
  expect((await names()).map((r) => r[0])).toEqual(['1行目 借方か貸方', '2行目 借方か貸方']);
  expect((await names())[1]).toContain('2行目を消す');
});

test('SC-02 「入力の形」のタブを ← → で選ぶと、フォーカスはタブに残る。マウスで押したときは、最初の欄へ移る', async ({ page }) => {
  await open(page, '#/entry');
  const simpleTab = page.getByRole('tab', { name: 'かんたん入力' });
  const journalTab = page.getByRole('tab', { name: '仕訳で入力' });
  await simpleTab.focus();
  await page.keyboard.press('ArrowRight');
  await expect(journalTab).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('form.entry-form .jl-table')).toBeVisible();
  await expect(journalTab).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await expect(simpleTab).toHaveAttribute('aria-selected', 'true');
  await expect(simpleTab).toBeFocused();
  // マウスで押したときは、今までどおり最初の欄（日付）へ。
  await journalTab.click();
  await expect(page.locator('form.entry-form').getByLabel('日付')).toBeFocused();
});

test('SC-02 仕訳で入力: 表のすぐ上に、借方と貸方の1行の説明があり、表（明細の領域）に結び付く', async ({ page }) => {
  await open(page, '#/entry');
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const help = page.locator('form.entry-form .jl-help');
  await expect(help).toContainText('借方の合計と貸方の合計を同じにします');
  const region = page.getByRole('region', { name: '仕訳の明細' });
  await expect(region).toHaveAttribute('aria-describedby', await help.getAttribute('id'));
  const [h, r] = await Promise.all([help.boundingBox(), region.boundingBox()]);
  expect(r.y - (h.y + h.height)).toBeLessThanOrEqual(12);
  expect(r.y).toBeGreaterThanOrEqual(h.y + h.height);
});
