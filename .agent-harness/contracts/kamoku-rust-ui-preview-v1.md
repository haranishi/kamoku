# Rust journal preview contract

## Purpose and scope

Move authored journal UI logic to Rust without changing accounting or the default
UI. A bookkeeper searches a transaction, changes filters/pages, opens its existing
detail, and returns to the same query and keyboard position.

Allowed: a minimal Rust/WASM UI crate, an isolated `/rust/index.html` preview,
same-origin read-only requests, narrow WASM CSP/MIME support, build tooling,
browser checks, and this decision record. No accounting/storage/schema changes.
No commit, push, or merge by the implementing agent; the main agent owns those.

## UX decisions before implementation

- Strategy: predictable, quick local bookkeeping; language is not speed evidence.
- Requirements: search, date/state filters, exact totals, bounded 50-row pages,
  keyboard navigation, Japanese composition protection, history/focus restoration.
- Structure: journal preview with a visible route back to the established UI;
  entry links explicitly open the established detail screen.
- Skeleton: labelled filters, live summary, pager above a semantic table, retry.
- Surface: existing color/typography tokens; responsive rows at 390/768/1440px,
  visible focus, reduced-motion support, no external assets.

Authored UI/business logic stays in Rust. Only a minimal module loader and the
generated wasm-bindgen JavaScript bindings are allowed. HTML/CSS remain web
assets. This is the first vertical slice, not full UI parity or full migration.

## Acceptance and evidence

- Native model tests cover malformed filters, pagination, yen formatting, and IME.
- CI checks Rust 1.88 and current Rust, clippy, actual WASM build and bindings.
- Real-server Playwright checks search/filter/page/history/detail return, errors,
  response races, Japanese composition, safe text, keyboard and three widths.
- Capture synthetic screenshots and cold-start/query/asset-size evidence.
  Upload screenshots/metrics only; do not upload auth URLs, logs, or traces.
- Keep original CI passing. Independent read-only review before merge.
- At most three fix rounds; same unresolved failure twice returns to maintainer.
- Remote CI performs heavy builds while the external SSD is unavailable.

## Why this approach

WASM plus DOM retains native input, semantic tables, and browser testability.
Start with web-sys rather than adding a reactive framework before its value is
demonstrated. Generated JS remains necessary for browser API bindings.
Narrow `'wasm-unsafe-eval'` permits WASM compilation without JavaScript eval.
New generated assets are built before the server embeds the UI; they are not
checked in. The preview's build requirement does not change default-UI editing.
