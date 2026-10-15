// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// Standalone runner for tools/bench-ui.py: no test reporter, trace, screenshot,
// video, console forwarding, or credential-bearing file/argument is produced.
import { chromium } from '@playwright/test';
import { performance } from 'node:perf_hooks';

const input = [];
for await (const chunk of process.stdin) input.push(chunk);
const config = JSON.parse(Buffer.concat(input).toString('utf8'));
const names = ['baseline', 'head'];
const cases = ['cold_home', 'cold_journal', 'home_to_journal', 'journal_to_home', 'inbox_input'];
const raw = Object.fromEntries(names.map((name) => [name, Object.fromEntries(cases.map((key) => [key, []]))]));
let browser;
let stage = 'browser_launch';

// A readiness timestamp is taken inside the page after the target DOM exists,
// requestAnimationFrame and a zero-delay task. It is a rendering opportunity,
// not a guarantee that pixels reached a display or a Core Web Vitals metric.
function instrument({ target, journalIds }) {
  const state = { target, started: 0, ms: null, pending: false, clickHash: null };
  const ready = () => {
    const heading = document.querySelector('main h1')?.textContent;
    if (state.target === 'home') return heading === 'ホーム' && document.querySelectorAll('.kpis .kpi-value').length === 4;
    if (heading !== '仕訳帳') return false;
    // The legacy table stores IDs in data-key; Rust rows also expose data-entry-id.
    const ids = [...document.querySelectorAll('.journal-table tbody tr[data-entry-id], .journal-table tbody tr[data-key]')]
      .map((row) => row.dataset.entryId ?? row.dataset.key);
    return ids.length === journalIds.length && ids.every((id, i) => id === journalIds[i]);
  };
  const check = () => {
    if (state.ms !== null || state.pending || state.started === null || !ready()) return;
    state.pending = true;
    requestAnimationFrame(() => setTimeout(() => {
      state.pending = false;
      if (state.ms === null && state.started !== null && ready()) state.ms = performance.now() - state.started;
    }, 0));
  };
  new MutationObserver(check).observe(document, { childList: true, subtree: true, attributes: true });
  document.addEventListener('click', (event) => {
    const link = event.target.closest?.('a');
    if (state.clickHash && link?.getAttribute('href') === state.clickHash) {
      state.started = performance.now();
      state.clickHash = null;
    }
  }, true);
  window.__uiPerformance = {
    state,
    arm(target, clickHash) {
      Object.assign(state, { target, started: null, ms: null, pending: false, clickHash });
    },
  };
  check();
}

async function contextFor(name, target) {
  const context = await browser.newContext({
    locale: 'ja-JP', timezoneId: 'Asia/Tokyo', viewport: { width: 1440, height: 900 },
  });
  const { origin, cookie } = config.variants[name];
  const split = cookie.indexOf('=');
  await context.addCookies([{ name: cookie.slice(0, split), value: cookie.slice(split + 1), url: origin }]);
  const page = await context.newPage();
  page.setDefaultTimeout(config.timeoutMs);
  await page.addInitScript(instrument, { target, journalIds: config.journalIds });
  return { context, page, origin };
}

async function resourceSnapshot(page, from = 0) {
  return page.evaluate((from) => {
    const entries = performance.getEntriesByType('resource').slice(from);
    const sum = (key) => entries.reduce((total, entry) => total + (entry[key] || 0), 0);
    const byType = {};
    for (const entry of entries) {
      const path = new URL(entry.name).pathname;
      const type = path.endsWith('.wasm') ? 'wasm' : path.endsWith('.js') ? 'javascript'
        : path.endsWith('.css') ? 'css' : path.startsWith('/api/') ? 'api' : 'other';
      const group = byType[type] ||= { requests: 0, transfer_bytes: 0, encoded_bytes: 0, decoded_bytes: 0 };
      group.requests += 1;
      group.transfer_bytes += entry.transferSize || 0;
      group.encoded_bytes += entry.encodedBodySize || 0;
      group.decoded_bytes += entry.decodedBodySize || 0;
    }
    const nav = performance.getEntriesByType('navigation')[0];
    const document = from === 0 && nav ? {
      transfer_bytes: nav.transferSize, encoded_bytes: nav.encodedBodySize, decoded_bytes: nav.decodedBodySize,
    } : { transfer_bytes: 0, encoded_bytes: 0, decoded_bytes: 0 };
    return {
      resource_entries: performance.getEntriesByType('resource').length,
      requests: entries.length + (from === 0 ? 1 : 0),
      transfer_bytes: sum('transferSize') + document.transfer_bytes,
      encoded_bytes: sum('encodedBodySize') + document.encoded_bytes,
      decoded_bytes: sum('decodedBodySize') + document.decoded_bytes,
      document, by_type: byType,
    };
  }, from);
}

async function finish(page, started, from = 0) {
  await page.waitForFunction(() => Number.isFinite(window.__uiPerformance?.state.ms));
  const observed = performance.now() - started;
  return { ms: await page.evaluate(() => window.__uiPerformance.state.ms), observed_ms: observed,
    resources: await resourceSnapshot(page, from) };
}

async function cold(page, origin, kind) {
  const started = performance.now();
  await page.goto(origin + (kind === 'home' ? '/#/' : '/#/journal'), { waitUntil: 'domcontentloaded' });
  return finish(page, started);
}

async function transition(page, kind, hash) {
  const from = await page.evaluate(() => performance.getEntriesByType('resource').length);
  await page.evaluate(({ kind, hash }) => window.__uiPerformance.arm(kind, hash), { kind, hash });
  const started = performance.now();
  await page.locator(`a[data-nav][href="${hash}"]`).first().click();
  return finish(page, started, from);
}

async function navigationSample(name, keep) {
  stage = name + '_cold_home';
  const home = await contextFor(name, 'home');
  try {
    const initial = await cold(home.page, home.origin, 'home');
    stage = name + '_home_to_journal';
    const toJournal = await transition(home.page, 'journal', '#/journal');
    stage = name + '_journal_to_home';
    const toHome = await transition(home.page, 'home', '#/');
    if (keep) {
      raw[name].cold_home.push(initial);
      raw[name].home_to_journal.push(toJournal);
      raw[name].journal_to_home.push(toHome);
    }
  } finally { await home.context.close(); }
  stage = name + '_cold_journal';
  const journal = await contextFor(name, 'journal');
  try {
    const initial = await cold(journal.page, journal.origin, 'journal');
    if (keep) raw[name].cold_journal.push(initial);
  } finally { await journal.context.close(); }
}

async function inboxContext(name) {
  stage = name + '_inbox_setup';
  const side = await contextFor(name, 'home');
  await side.page.goto(`${side.origin}/#/inbox?q=${encodeURIComponent(config.marker)}`, { waitUntil: 'domcontentloaded' });
  await side.page.waitForFunction(() => document.querySelectorAll('tr.inbox-row').length === 200);
  const field = side.page.locator('tr.inbox-row input[placeholder="科目（必須）"]').first();
  await field.focus();
  return { ...side, field };
}

async function accountInput(side) {
  await side.page.bringToFront();
  await side.field.focus();
  await side.page.evaluate(() => {
    window.__uiInputMs = null;
    const field = document.activeElement;
    let start = null;
    const input = (event) => {
      if (event.target !== field) return;
      start = performance.now();
      document.removeEventListener('input', input, true);
    };
    document.addEventListener('input', input, true);
    const observer = new MutationObserver(() => {
      const list = document.getElementById(field.getAttribute('aria-controls') || '');
      const selected = list?.querySelector('[role="option"][aria-selected="true"]');
      if (start === null || !list || list.hidden || field.getAttribute('aria-expanded') !== 'true'
        || !selected?.textContent.includes('通信費')) return;
      observer.disconnect();
      requestAnimationFrame(() => setTimeout(() => { window.__uiInputMs = performance.now() - start; }, 0));
    });
    observer.observe(document, { childList: true, subtree: true, attributes: true });
  });
  const started = performance.now();
  await side.page.keyboard.insertText('つうし');
  await side.page.waitForFunction(() => window.__uiInputMs !== null);
  const observed = performance.now() - started;
  const ms = await side.page.evaluate(() => window.__uiInputMs);
  await side.page.keyboard.press('ControlOrMeta+a');
  await side.page.keyboard.press('Backspace');
  await side.page.keyboard.press('Escape');
  await side.page.waitForFunction(() => document.activeElement?.getAttribute('aria-expanded') === 'false');
  return { ms, observed_ms: observed };
}

function stats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return { p50: sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2,
    p95: sorted[Math.ceil(sorted.length * 0.95) - 1] };
}

function summarize(samples) {
  const primary = stats(samples.map((x) => x.ms));
  const observed = stats(samples.map((x) => x.observed_ms));
  const resourceSamples = samples.map((x) => x.resources).filter(Boolean);
  return {
    samples: samples.length, raw_ms: samples.map((x) => x.ms), p50_ms: primary.p50, p95_ms: primary.p95,
    observed_raw_ms: samples.map((x) => x.observed_ms), observed_p50_ms: observed.p50, observed_p95_ms: observed.p95,
    ...(resourceSamples.length ? { resource_samples: resourceSamples,
      bytes: Object.fromEntries(['transfer_bytes', 'encoded_bytes', 'decoded_bytes'].map((key) => [key, stats(resourceSamples.map((x) => x[key]))])) } : {}),
  };
}

process.once('SIGTERM', () => {
  Promise.resolve(browser?.close()).finally(() => process.exit(1));
});

try {
  browser = await chromium.launch({ headless: true, ...(config.channel === 'chrome' ? { channel: 'chrome' } : {}) });
  for (let index = 0; index < config.samples + 1; index += 1) {
    for (const name of index % 2 ? [...names].reverse() : names) await navigationSample(name, index > 0);
  }
  const inboxes = {};
  try {
    for (const name of names) inboxes[name] = await inboxContext(name);
    for (let index = 0; index < config.inputSamples + 3; index += 1) {
      for (const name of index % 2 ? [...names].reverse() : names) {
        stage = name + '_inbox_input';
        const sample = await accountInput(inboxes[name]);
        if (index >= 3) raw[name].inbox_input.push(sample);
      }
    }
  } finally {
    for (const side of Object.values(inboxes)) await side.context.close();
  }
  const variants = Object.fromEntries(names.map((name) => [name, Object.fromEntries(cases.map((key) => [key, summarize(raw[name][key])]))]));
  const comparison = Object.fromEntries(cases.map((key) => [key, {
    p50_ratio_head_over_baseline: variants.baseline[key].p50_ms > 0 ? variants.head[key].p50_ms / variants.baseline[key].p50_ms : null,
    p95_ratio_head_over_baseline: variants.baseline[key].p95_ms > 0 ? variants.head[key].p95_ms / variants.baseline[key].p95_ms : null,
  }]));
  console.log(JSON.stringify({ schema: 1, complete: true,
    browser: { version: browser.version(), channel: config.channel, viewport: { width: 1440, height: 900 } },
    measurement: {
      ordering: 'alternating baseline/head; pair order reversed on each iteration',
      navigation_warmups: 1, input_warmups: 3, input_text: 'つうし',
      primary_clock: 'page performance.now; target DOM or input event through requestAnimationFrame then zero-delay task',
      observed_clock: 'Node performance.now; includes Playwright IPC, actionability and polling',
      cache: 'one browser process; new HTTP context per cold navigation; warm OS/files/server; browser-wide WASM/code caches may persist',
      transition: 'real navigation-link click in an already loaded context',
      bytes: 'completed ResourceTiming entries at readiness plus navigation document; transfer/encoded/decoded bytes; late/background requests may be omitted; no URLs',
      limits: 'no CPU/network throttling; Chromium only; rendering opportunity is not actual pixel presentation or INP; insertText does not exercise IME conversion; no traces/videos/screenshots',
    }, variants, comparison }));
} catch {
  console.log(JSON.stringify({ schema: 1, complete: false, failed_stage: stage,
    error: 'UI measurement failed; private browser diagnostics withheld' }));
  process.exitCode = 1;
} finally {
  await browser?.close().catch(() => {});
}
