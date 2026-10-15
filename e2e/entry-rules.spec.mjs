// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録の決まり: 再送防止、プレビューの失敗、免税、口座が相手科目、経過措置の案内、仕訳で入力、テンプレート、読み取り専用、
// 取引先の候補と新しい取引先、候補が0件のときの一言、候補を選んだら誤りを消す、課税で「対象外」のままの一言。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, withSettings, withAccountDefaults, walletOf } from './helpers.mjs';

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
  // 何も入れていないうちは、「合っています」と言わない（体験の評価 第3回 E4）。
  await expect(form).toContainText('金額を入れると、貸借の差を確かめます');
  await expect(form).not.toContainText('貸借が合っています');
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
  // 保存できなくても、入力があるので、離れる前に確認が出る（SC-02。捨てて仕訳帳へ移る）。
  await open(page, '#/journal');
  await page.getByRole('dialog', { name: '入力中の内容を捨てますか' }).getByRole('button', { name: '捨てて離れる' }).click();
  await page.locator('.journal-table tbody tr').first().click();
  await expect(page.getByRole('dialog').getByRole('button', { name: /取り消す/ })).toBeDisabled();
});

test('SC-02 取引先の欄は、Enterで先頭の候補を選ぶ。打った名前は、一覧の最後の行を選んだときだけ新しい取引先になる', async ({ page }) => {
  const tag = Date.now().toString(36);
  const known = `E2E商店${tag}`;
  expect((await apiCall(page, 'POST', '/partners', { name: known, reading: '', invoice_number: null, note: '' })).status).toBe(201);
  const desc = uniqueText('E2E 新しい取引先');
  await open(page, '#/entry');
  const f = await fillSupplies(page, desc);
  const list = page.getByRole('listbox', { name: '取引先の候補' });

  // 名前の一部でEnter: 先頭の候補（印あり）を選び、次の欄へ。末尾には「新しい取引先として足す」の行がある。
  await f.partner.click();
  await page.keyboard.insertText(`商店${tag}`);
  await expect(list.getByRole('option').first()).toHaveText(known);
  await expect(list.getByRole('option').first()).toHaveAttribute('aria-selected', 'true');
  await expect(list.getByRole('option').last()).toHaveText(`「商店${tag}」を新しい取引先として足す`);
  await page.keyboard.press('Enter');
  await expect(f.partner).toHaveValue(known);
  await expect(f.description).toBeFocused();
  await expect(f.form).not.toContainText('新しい取引先:');

  // 候補に無い名前を、選ばずに欄を離れたら、保存の前に止める（黙って新しい取引先にしない）。
  const fresh = `E2E新しい店${tag}`;
  await f.partner.click();
  await page.keyboard.insertText(fresh);
  await expect(list.getByRole('option')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await f.description.click();
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  await expect(f.partner).toBeFocused();
  await expect(f.partner).toHaveAttribute('aria-invalid', 'true');
  await expect(f.form).toContainText(`「${fresh}」は候補にありません。`);
  expect((await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.total).toBe(0);

  // 一覧の最後の行を選ぶと、新しい取引先として使い、欄の下に名前を出す。誤りの表示は消える。
  await page.keyboard.press('ArrowDown');
  await expect(list.getByRole('option')).toHaveText(`「${fresh}」を新しい取引先として足す`);
  await page.keyboard.press('Enter');
  await expect(f.partner).toHaveValue(fresh);
  await expect(f.partner).not.toHaveAttribute('aria-invalid', /./);
  await expect(f.form).toContainText(`新しい取引先: ${fresh}`);
  await expect(f.partner).toHaveAttribute('aria-describedby', /note/);
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  const saved = (await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}`)).json.items[0];
  expect(saved.partner).toBe(fresh);
});

test('SC-02 科目に合う候補が無いときは、空の一覧を開かず、探し方の一言を出す。Esc は一言を閉じるだけ', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.account.click();
  await page.keyboard.insertText('ぞぞぞ');
  // 一言は、科目の欄の入れ物（.combo）の中にある。
  const msg = f.account.locator('xpath=..').locator('.combo-msg');
  await expect(f.account).toHaveAttribute('aria-expanded', 'false');
  await expect(msg).toBeVisible();
  await expect(msg).toHaveText('合う科目がありません。ひらがなの読みか、科目の名前で探せます。');
  await page.keyboard.press('Escape');
  await expect(msg).toBeHidden();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(f.account).toBeFocused();
  // 打ち直して合えば、一覧が開き、一言は消える。
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('しょうもう');
  await expect(f.account).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('option', { name: /消耗品費/ })).toBeVisible();
  await expect(msg).toBeHidden();
});

test('SC-02 候補を選んだ瞬間に、その欄の誤り（赤枠・文・aria-invalid）を消す', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('1100');
  // 口座を空にしてから保存を押し、科目と口座の両方に誤りを出す。
  await f.wallet.click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('Backspace');
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  await expect(f.account).toBeFocused();
  await expect(f.account).toHaveAttribute('aria-invalid', 'true');
  await expect(f.form).toContainText('科目を選んでください。');
  await expect(f.wallet).toHaveAttribute('aria-invalid', 'true');
  await expect(f.form).toContainText('口座を選んでください。');
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  await expect(f.account).not.toHaveAttribute('aria-invalid', /./);
  await expect(f.form).not.toContainText('科目を選んでください。');
  const cash = await walletOf(page, 'cash');
  await f.wallet.click();
  await page.keyboard.insertText(cash.name);
  await page.getByRole('option', { name: new RegExp(cash.name) }).first().click();
  await expect(f.wallet).not.toHaveAttribute('aria-invalid', /./);
  await expect(f.form).not.toContainText('口座を選んでください。');
});

test('AC-01.4 課税事業者で、既定が「対象外」の費用の科目を選ぶと、税区分の欄の下に一言出す。税区分を選ぶと消え、免税では出さない', async ({ page }) => {
  const note = 'この科目の既定は「対象外」です。課税の取引なら、税区分を選んでください。';
  await open(page, '#/');
  // 標準の科目表の既定（旅費交通費は対象外）は変えない。試験の間だけ、合成データの値を同じ形にそろえる。
  await withAccountDefaults(page, 'exp_travel', { default_tax: null, default_pct: 100 }, async () => {
    await withSettings(page, { tax_mode: 'taxable' }, async () => {
      await open(page, '#/entry');
      const f = entryFields(page);
      await f.account.click();
      await page.keyboard.insertText('旅費交通費');
      await page.keyboard.press('Enter');
      await expect(f.tax).toHaveValue('');
      await expect(f.form).toContainText(note);
      await expect(f.tax).toHaveAttribute('aria-describedby', /note/);
      await f.tax.selectOption('P10');
      await expect(f.form).not.toContainText(note);
    });
    await withSettings(page, { tax_mode: 'exempt', invoice_number: null }, async () => {
      // 同じアドレスのままなので、読み込み直して、免税の設定で開き直す。
      await page.reload();
      await expect(page.locator('main h1')).toBeVisible();
      const f = entryFields(page);
      await f.account.click();
      await page.keyboard.insertText('旅費交通費');
      await page.keyboard.press('Enter');
      await page.getByRole('button', { name: '詳細: 税区分を出す' }).click();
      await expect(f.tax).toBeVisible();
      await expect(f.form).not.toContainText(note);
    });
  });
});

// 締めた期間（2026-10-15 の体験の評価 E3 の9）: 断りの文に締めた日が無く、保存して初めて断られた。
test('AC-06.3 締めた日以前の日付は、欄を確定したときに締めた日つきで出す。保存で断られた文にも締めた日を入れる。締めていなければ出さない', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  const dateError = page.locator('form.entry-form .field').filter({ has: page.getByLabel('日付') }).locator('.field-error');
  // 締めていないときは何も出ない。
  await f.date.fill('2026-09-15');
  await f.date.blur();
  await expect(dateError).toHaveText('');
  await withSettings(page, { locked_until: '2026-09-30' }, async () => {
    await page.reload();
    await expect(page.locator('main h1')).toBeVisible();
    await f.date.fill('2026-09-15');
    await f.date.blur();
    await expect(dateError).toHaveText('2026年9月30日まで締めてあるため、この日付には登録できません。');
    // 締めた日その日も締めの中。
    await f.date.fill('2026-09-30');
    await f.date.blur();
    await expect(dateError).toHaveText('2026年9月30日まで締めてあるため、この日付には登録できません。');
    // 保存すると、サーバーが断る。その文にも締めた日を入れ、日付の欄へ移る。
    await f.amount.fill('1000');
    await f.account.click();
    await page.keyboard.insertText('消耗品費');
    await page.keyboard.press('Enter');
    await f.description.fill(uniqueText('E2E 締めた日'));
    await page.keyboard.press('ControlOrMeta+Enter');
    await expect(dateError).toHaveText('2026年9月30日まで締めてあるため、この日付には登録できません。日付を直すか、設定の「締め」で締めた日を見直してください。');
    await expect(f.date).toBeFocused();
    // 締めた日より後にすると消える。
    await f.date.fill('2026-10-01');
    await f.date.blur();
    await expect(dateError).toHaveText('');
  });
});

test('AC-06.3 仕訳で入力でも、締めた日以前の日付は、欄を確定したときに締めた日つきで出す', async ({ page }) => {
  await open(page, '#/entry');
  await withSettings(page, { locked_until: '2026-09-30' }, async () => {
    await page.reload();
    await page.getByRole('tab', { name: '仕訳で入力' }).click();
    const date = page.locator('form.entry-form').getByLabel('日付');
    const dateError = page.locator('form.entry-form .field').filter({ has: page.getByLabel('日付') }).locator('.field-error');
    await date.fill('2026-09-01');
    await date.blur();
    await expect(dateError).toHaveText('2026年9月30日まで締めてあるため、この日付には登録できません。');
    await date.fill('2026-10-02');
    await date.blur();
    await expect(dateError).toHaveText('');
  });
});
