# Rust UI implementation interface

All new screens live under `crates/kamoku-ui/src/app/screens/` and import
`use crate::app::prelude::*;`. Root owns `app/{mod,prelude,core,components,...}`,
Cargo, module registration, static assets, tests/CI and documentation.
Each screen exports `pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue>`.
Use `Screen::new(&ctx, root_element)`; asynchronous work uses `ctx.task(future)`.

## Context and lifecycle

`Context: Clone`; public `route: Route`, `scope: Scope` (both cloneable).
Route has `path`, `hash`, `previous_hash: Option<String>`;
`param(name)` / `query(name)` return String (empty when absent).
`build_hash(path, &[(&str,&str)]) -> String` encodes parameters.
`ctx.alive()` rejects stale responses; check after every awaited load before DOM.
`ctx.on(&Element, event, Fn(Event))` attaches a page-scoped listener.
`ctx.key(Fn(KeyboardEvent)->bool)` installs page shortcuts after IME filtering.
`ctx.dirty(Fn()->bool)` installs the unsaved-input guard. `ctx.allow_leave()`
permits the next explicit navigation after a successful save/discard.
`ctx.on_stay(Fn())` restores the last edited field after a cancelled guard;
do not override a subsequent deliberate focus change. `ctx.child()` owns a
shorter-lived listener/cleanup scope, disposed with its parent.
`ctx.navigate(hash, replace: bool)`, `ctx.back(fallback)` preserve hash history.
`ctx.task(Future<Output=Result<(),ApiError>>)` reports errors only if still alive.
`ctx.success(message)`, `ctx.info(message)`, `ctx.error(message)` show notices.
`ctx.notice(kind,message,Option<(&str,&str)>)` supports action label/href.
`notice_action(kind,message,label,Fn())` / `dismiss_notice_action(label)` support
Undo that survives screen disposal; guard screen DOM work with `ctx.alive()`.
`ctx.is_read_only()` / `ctx.read_only_reason()` preserve server write restrictions.
`Screen` exposes `manages_focus: bool` and optional
`update: Option<Rc<dyn Fn(Context)->bool>>` (true means handled same-page update).

## API and shared state

Async Context methods return `Result<Value,ApiError>`:
`get(path,&[(&str,&str)])`, `post/put/patch(path,&Value)`,
`upload(path,&[(&str,&str)],&[u8])`, `load(name,force:bool)`.
`post_abort(path,&Value,&AbortSignal)` cancels obsolete read-only previews.
`ctx.invalidate(&[name,...])`; `ctx.peek(name)->Option<Value>`.
`ctx.download(path,query)->Result<(),ApiError>` saves and announces warnings.
ApiError has `status:u16`, `code/message:String`, `field:Option<String>`,
`is_stale()` and `is_read_only()`. `ApiError::message(text)` creates a UI error.
`new_client_id()->String` uses browser crypto. JSON is serde_json::Value,
serialized and parsed in Rust; yen integers never pass through JS Number.

## DOM and components

`h(tag,class,text)->Element`, `attrs(&Element,&[(&str,&str)])`,
`append/replace(&Element,&[Element])`, `set_text(&Element,text)`, `focus(&Element)`,
`button(label,variant)->Element`, `link(label,href,class)->Element`,
`input(kind,value)->HtmlInputElement`,
`select(&[(&str,&str)],value)->HtmlSelectElement`,
`disabled(&Element,bool)`, `busy(&Element,bool)`, `icon(name)->Element`.
`field(label,&control,hint,required)->Field` exposes `el`, `control`,
`set_error(text)`, `set_label(text)`; links live errors to input while retaining
their live Text node. `select_all_on_focus(&ctx,&input)` preserves later caret
placement; `focus_when_scrollable(&ctx,&wrapper,&table)` owns ResizeObserver.

`ComboItem {value,label,reading:String,meta:Value}`; `Combo::new(&ctx,items,value)`
exposes `el:Element`, `input:HtmlInputElement`, `list:Element` and methods
`value()->String`, `item()->Option<ComboItem>`, `is_open()->bool`,
`set_items(Vec<ComboItem>)`, `set_value(value,notify:bool)`, `open/close/focus()`,
`on_change(Fn(String,Option<ComboItem>,bool))`, `allow_free(bool)`.
The bool callback argument means keyboard selection. Set placeholder on input.
`on_select(Fn(Option<ComboItem>,&str,Option<Event>)->bool)` may handle selection;
`create_label(Fn(&str)->String)` enables explicit new-partner selection.

`Dialog::new(&ctx,title)` exposes `el:HtmlDialogElement`, `body/footer:Element`,
`scope:Scope` (scoped `on`), `show/close()`, `set_busy(bool)`, `set_error(text)`,
`set_title(text)`. `confirm(&ctx,title,body,label,danger).await -> bool`.
Use native inputs/semantic DOM; never innerHTML/eval or embedded legacy JS.

## Shared pure helpers

`money(i64)->String`, `number(i64)->String`, `today()->String`,
`parse_amount(&str)->Result<i64,String>`, `parse_date(&str,today:&str)->Result<String,String>`,
`normalize_search(&str)->String`, `year_month(&str)->String`.
Additional helpers may stay in the owning feature's module. Screen code owns
its feature-specific pure models and regression tests. Root owns shared tests.
