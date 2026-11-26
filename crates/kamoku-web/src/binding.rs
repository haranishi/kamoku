// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `DemoBook` を JavaScript に出す。日付は画面の meta と同じ 2026-09-30。

use kamoku_app::web::Book;
use wasm_bindgen::prelude::*;

/// 架空の帳簿。`new` で開き、`handle` で `/api/v1` に応える。
#[wasm_bindgen]
pub struct DemoBook {
    book: Book,
}

#[wasm_bindgen]
impl DemoBook {
    /// 2026年9月30日を「今日」にして、架空の帳簿をメモリに作る。
    ///
    /// # Errors
    /// 帳簿を開けないとき。
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<DemoBook, JsValue> {
        let today = time::macros::date!(2026 - 09 - 30);
        let book = Book::demo(today).map_err(|error| JsValue::from_str(&error))?;
        Ok(Self { book })
    }

    /// 1つのAPI要求。`content_type` が無いときは `null`。本文はバイト列。
    ///
    /// # Errors
    /// 応答のオブジェクトを作れないとき。
    #[allow(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen は Option<&str> を受け取れないので、所有する文字列で受ける"
    )]
    pub fn handle(
        &mut self,
        method: &str,
        path: &str,
        query: &str,
        content_type: Option<String>,
        body: &[u8],
    ) -> Result<JsValue, JsValue> {
        let reply = self
            .book
            .call(method, path, query, content_type.as_deref(), body);
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("status"),
            &JsValue::from_f64(f64::from(reply.status)),
        )?;
        let headers = js_sys::Object::new();
        for (name, value) in &reply.headers {
            let _ = js_sys::Reflect::set(
                &headers,
                &JsValue::from_str(name),
                &JsValue::from_str(value),
            )?;
        }
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("headers"), &headers)?;
        let bytes = js_sys::Uint8Array::from(reply.body.as_slice());
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("body"), &bytes)?;
        Ok(obj.into())
    }
}
