// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 入力から仕訳の明細を組み立てる（docs/spec/api-entries.md と data-model.md の「端数処理の決まり」を簡単に再現）。
// 按分の事業分は四捨五入、残りは事業主貸。税額は税込の事業分から、設定の端数処理で出す。
// 金額は最大でも 9,999,999,999,999 × 100 なので、Numberの整数の範囲（2^53）に収まる。

export const MAX_AMOUNT = 9999999999999;
export const TAX_CODES = ['S10', 'S8', 'P10', 'P8', 'P10X', 'P8X', 'NT'];
const KINDS = ['expense', 'income', 'transfer', 'journal'];

/** 応答の失敗（{ error: { code, message, field } } になる）。 */
export class ApiFail extends Error {
  constructor(status, code, message, field) {
    super(message);
    this.status = status;
    this.code = code;
    this.field = field || null;
  }
}

const yen = (n) => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');

function taxRate(code) {
  if (!code || code === 'NT') return 0;
  return code === 'S8' || code === 'P8' || code === 'P8X' ? 8 : 10;
}

/** 税込の金額に含まれる税額。rounding は floor / round / ceil。 */
export function taxIncluded(code, gross, rounding) {
  const rate = taxRate(code);
  if (!rate) return 0;
  const num = gross * rate;
  const den = 100 + rate;
  const q = Math.floor(num / den);
  const r = num - q * den;
  if (rounding === 'ceil') return r > 0 ? q + 1 : q;
  if (rounding === 'round') return r * 2 >= den ? q + 1 : q;
  return q;
}

function isRealDate(iso) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''));
  if (!m) return false;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const t = new Date(Date.UTC(y, mo - 1, d));
  return t.getUTCFullYear() === y && t.getUTCMonth() === mo - 1 && t.getUTCDate() === d;
}

function checkAmount(amount, field = 'amount') {
  if (!Number.isInteger(amount) || amount < 1 || amount > MAX_AMOUNT) {
    throw new ApiFail(422, 'amount_range', '金額は1円以上、9,999,999,999,999円以下の整数にしてください。', field);
  }
}

function account(ctx, code, field, label) {
  if (!code) throw new ApiFail(422, 'missing_account', `${label}を選んでください。`, field);
  const a = ctx.accounts.get(code);
  if (!a) throw new ApiFail(422, 'unknown_account', `${label}「${code}」が見つかりません。候補から選び直してください。`, field);
  if (!a.enabled) {
    throw new ApiFail(422, 'disabled_account', `「${a.name}」は使わない設定です。勘定科目の画面で有効にするか、別の${label}を選んでください。`, field);
  }
  return a;
}

function wallet(ctx, code, field, label) {
  const a = account(ctx, code, field, label);
  if (!a.wallet) throw new ApiFail(422, 'unknown_account', `「${a.name}」は口座ではありません。口座を選んでください。`, field);
  return a;
}

function taxCode(code, field = 'tax_code') {
  if (code === null || code === undefined || code === '') return null;
  if (!TAX_CODES.includes(code)) throw new ApiFail(422, 'bad_tax_code', `税区分「${code}」は使えません。一覧から選んでください。`, field);
  return code;
}

const line = (acc, side, amount, tax_code = null, tax_amount = 0, memo = '') => ({
  account: acc.code,
  account_name: acc.name,
  side,
  amount,
  tax_code,
  tax_amount,
  memo,
});

/**
 * 入力を検査して、明細・付随情報・注意を返す。保存とプレビューで同じ関数を使う。
 * @returns {{ kind, date, description, partner, lines, total, meta, warnings, input }}
 */
export function compose(input, ctx) {
  if (!input || typeof input !== 'object') throw new ApiFail(400, 'bad_request', '本文がJSONの形ではありません。', null);
  const kind = input.kind;
  if (!KINDS.includes(kind)) {
    throw new ApiFail(400, 'bad_request', '種別は expense・income・transfer・journal のどれかにしてください。', 'kind');
  }
  if (!isRealDate(input.date)) throw new ApiFail(400, 'bad_request', '日付は YYYY-MM-DD の形の、実在する日にしてください。', 'date');
  const locked = ctx.settings.locked_until;
  if (locked && input.date <= locked) {
    throw new ApiFail(422, 'locked_period', `${locked} 以前は締めてあるため、書き込めません。日付を確かめるか、設定の「締め」を見直してください。`, 'date');
  }
  const rounding = ctx.settings.rounding || 'floor';
  const description = String(input.description ?? '');
  const partner = String(input.partner ?? '').trim();
  const warnings = [];
  const meta = { kind, source: 'manual' };
  if (partner) meta.partner = partner;
  let lines;

  if (kind === 'journal') {
    lines = composeJournal(input, ctx, rounding);
  } else {
    const amount = input.amount;
    checkAmount(amount);
    if (kind === 'transfer') {
      const from = wallet(ctx, input.wallet, 'wallet', '移動元の口座');
      const to = wallet(ctx, input.to_wallet, 'to_wallet', '移動先の口座');
      if (from.code === to.code) throw new ApiFail(422, 'bad_request', '移動元と移動先が同じ口座です。別の口座を選んでください。', 'to_wallet');
      lines = [line(to, 'debit', amount), line(from, 'credit', amount)];
    } else {
      const acc = account(ctx, input.account, 'account', '科目');
      const pay = wallet(ctx, input.wallet, 'wallet', '口座');
      if (acc.code === pay.code) throw new ApiFail(422, 'bad_request', '科目と口座が同じです。別の科目を選んでください。', 'account');
      const tax = taxCode(input.tax_code);
      // 相手科目が口座（カードの引き落とし、現金の引き出しなど）なら、税区分と割合は使えない（AC-02.8）。
      if (acc.wallet) {
        if (tax) throw new ApiFail(422, 'bad_request', '相手が口座の取引には、税区分を付けられません。税区分を「対象外」にしてください。', 'tax_code');
        if (input.business_pct !== undefined && input.business_pct !== null && input.business_pct !== 100) {
          throw new ApiFail(422, 'bad_request', '相手が口座の取引には、事業の割合を付けられません。100のままにしてください。', 'business_pct');
        }
      }
      if (kind === 'expense') {
        const pct = input.business_pct ?? acc.default_pct;
        if (!Number.isInteger(pct) || pct < 0 || pct > 100) {
          throw new ApiFail(400, 'bad_request', '事業の割合は0から100の整数にしてください。', 'business_pct');
        }
        const business = Math.floor((amount * pct + 50) / 100);
        const own = amount - business;
        // 0円の明細は作らない。
        lines = [];
        if (business > 0) lines.push(line(acc, 'debit', business, tax, taxIncluded(tax, business, rounding)));
        if (own > 0) lines.push(line(ctx.accounts.get('owner_draw'), 'debit', own, null, 0, `家事按分 ${100 - pct}%`));
        lines.push(line(pay, 'credit', amount));
        meta.business_pct = String(pct);
        if (business === 0) warnings.push('事業分が0円になるため、全額を事業主貸に入れます。');
        if (acc.kind === 'revenue') warnings.push(`支出に収益の科目（${acc.name}）が選ばれています。種別を確かめてください。`);
      } else {
        if (input.business_pct !== undefined && input.business_pct !== null && input.business_pct !== 100) {
          throw new ApiFail(400, 'bad_request', '収入には事業の割合を付けられません。100のままにしてください。', 'business_pct');
        }
        lines = [line(pay, 'debit', amount), line(acc, 'credit', amount, tax, taxIncluded(tax, amount, rounding))];
        if (acc.kind === 'expense') warnings.push(`収入に費用の科目（${acc.name}）が選ばれています。種別を確かめてください。`);
      }
      const plAccount = acc.kind === 'revenue' || acc.kind === 'expense';
      if (ctx.settings.tax_mode === 'taxable' && plAccount && !tax) {
        warnings.push('税区分が空です。消費税のかかる取引なら、税区分を選んでください。');
      }
    }
  }
  const total = lines.filter((l) => l.side === 'debit').reduce((a, l) => a + l.amount, 0);
  return { kind, date: input.date, description, partner, lines, total, meta, warnings };
}

function composeJournal(input, ctx, rounding) {
  const rows = Array.isArray(input.lines) ? input.lines : [];
  if (rows.length < 2) throw new ApiFail(400, 'bad_request', '明細は2行以上にしてください。', 'lines');
  const lines = rows.map((row, i) => {
    const acc = account(ctx, row.account, `lines.${i}.account`, `${i + 1}行目の科目`);
    if (row.side !== 'debit' && row.side !== 'credit') {
      throw new ApiFail(400, 'bad_request', `${i + 1}行目の借方・貸方を選んでください。`, `lines.${i}.side`);
    }
    checkAmount(row.amount, `lines.${i}.amount`);
    const tax = taxCode(row.tax_code, `lines.${i}.tax_code`);
    // 税額を画面が省いたときは、その明細の金額から出す（記帳上の目安）。非課税と対象外は0。
    const taxAmount = !tax || tax === 'NT' ? 0 : Number.isInteger(row.tax_amount) ? row.tax_amount : taxIncluded(tax, row.amount, rounding);
    return line(acc, row.side, row.amount, tax, taxAmount, String(row.memo ?? ''));
  });
  const debit = lines.filter((l) => l.side === 'debit').reduce((a, l) => a + l.amount, 0);
  const credit = lines.filter((l) => l.side === 'credit').reduce((a, l) => a + l.amount, 0);
  if (debit !== credit) {
    const diff = Math.abs(debit - credit);
    throw new ApiFail(
      422,
      'unbalanced',
      `貸借が一致しません。借方 ${yen(debit)}円、貸方 ${yen(credit)}円です。差の${yen(diff)}円を、どちらかの行に足してください。`,
      'lines',
    );
  }
  return lines;
}
