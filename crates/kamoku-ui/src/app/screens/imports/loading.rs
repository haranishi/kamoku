// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Cell, Context, Element, JsCast, Rc, RefCell, append, attrs, h, window};

pub(super) struct Loading {
    stopped: Rc<Cell<bool>>,
    shown: Rc<RefCell<Option<Element>>>,
}
impl Loading {
    pub fn new(ctx: &Context, container: &Element) -> Self {
        let stopped = Rc::new(Cell::new(false));
        let shown = Rc::new(RefCell::new(None));
        let waiting = stopped.clone();
        let node = shown.clone();
        let container = container.clone();
        let context = ctx.clone();
        ctx.task(async move {
            let promise = js_sys::Promise::new(&mut |resolve, _reject| {
                let _ =
                    window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 300);
            });
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            if waiting.get() || !context.alive() {
                return Ok(());
            }
            let block = h("div", "skel-block", "");
            attrs(
                &block,
                &[("role", "status"), ("aria-label", "口座を読み込んでいます")],
            );
            for width in ["40%", "90%", "70%", "85%"] {
                let bar = h("span", "skel", "");
                if let Some(el) = bar.dyn_ref::<web_sys::HtmlElement>() {
                    let _ = el.style().set_property("width", width);
                }
                append(&block, &[bar]);
            }
            append(&container, std::slice::from_ref(&block));
            *node.borrow_mut() = Some(block);
            Ok(())
        });
        Self { stopped, shown }
    }
}
impl Drop for Loading {
    fn drop(&mut self) {
        self.stopped.set(true);
        if let Some(node) = self.shown.borrow_mut().take() {
            node.remove();
        }
    }
}
