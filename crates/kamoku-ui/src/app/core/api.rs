// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::dom::window;
use serde_json::Value;
use std::fmt::Write as _;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{RequestCredentials, RequestInit, RequestMode, Response, UrlSearchParams};

#[derive(Debug, Clone)]
pub(crate) struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}
impl ApiError {
    pub(crate) fn message(text: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: "ui".into(),
            message: text.into(),
            field: None,
        }
    }
    pub(crate) fn is_stale(&self) -> bool {
        matches!(self.code.as_str(), "stale" | "stale_revision")
    }
    pub(crate) fn is_read_only(&self) -> bool {
        self.code == "read_only"
    }
}
impl From<JsValue> for ApiError {
    fn from(_: JsValue) -> Self {
        Self::message("画面の操作を完了できませんでした。もう一度試してください。")
    }
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
fn network() -> ApiError {
    ApiError{status:0,code:"network".into(),message:"サーバーにつながりません。kamokuのサーバーが動いているかを確かめて、もう一度試してください。".into(),field:None}
}
pub(crate) fn url(path: &str, query: &[(&str, &str)]) -> Result<String, ApiError> {
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains(['\\', '#'])
        || path.contains("://")
    {
        return Err(ApiError::message("送信先を確認できませんでした。"));
    }
    let params = UrlSearchParams::new().map_err(|_| network())?;
    for (key, value) in query {
        if !value.is_empty() {
            params.append(key, value);
        }
    }
    let encoded: String = params.to_string().into();
    Ok(format!(
        "/api/v1{path}{}{}",
        if encoded.is_empty() { "" } else { "?" },
        encoded
    ))
}
pub(crate) async fn response(
    method: &str,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
    bytes: Option<&[u8]>,
) -> Result<Response, ApiError> {
    response_with_signal(method, path, query, body, bytes, None).await
}
async fn response_with_signal(
    method: &str,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
    bytes: Option<&[u8]>,
    signal: Option<&web_sys::AbortSignal>,
) -> Result<Response, ApiError> {
    let init = RequestInit::new();
    init.set_method(method);
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_mode(RequestMode::SameOrigin);
    let headers = web_sys::Headers::new()?;
    headers.set("Accept", "application/json")?;
    if let Some(bytes) = bytes {
        headers.set("Content-Type", "application/octet-stream")?;
        let array = js_sys::Uint8Array::from(bytes);
        init.set_body(&array.into());
    } else if let Some(body) = body {
        headers.set("Content-Type", "application/json")?;
        init.set_body(&JsValue::from_str(
            &serde_json::to_string(body)
                .map_err(|_| ApiError::message("入力の形式を確認してください。"))?,
        ));
    }
    init.set_headers(&headers);
    init.set_signal(signal);
    let response = JsFuture::from(window().fetch_with_str_and_init(&url(path, query)?, &init))
        .await
        .map_err(|_| {
            if signal.is_some_and(web_sys::AbortSignal::aborted) {
                ApiError {
                    status: 0,
                    code: "aborted".into(),
                    message: "要求を取り消しました。".into(),
                    field: None,
                }
            } else {
                network()
            }
        })?;
    let response: Response = response.dyn_into().map_err(|_| network())?;
    if response.ok() {
        return Ok(response);
    }
    let status = response.status();
    let text = JsFuture::from(response.text()?)
        .await
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    let data: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let err = &data["error"];
    let fallback = match status {
        401 => {
            "合言葉の期限が切れました。サーバーを起動したときに出たアドレスから、開き直してください。"
        }
        403 => "ほかのページからの操作として断られました。kamokuの画面から操作してください。",
        404 => "見つかりませんでした。一覧から選び直してください。",
        409 => "いまの状態と合わないため、断られました。画面を読み込み直してください。",
        422 => "入力が決まりに合いません。入力を確かめてください。",
        _ => "要求を完了できませんでした。もう一度試してください。",
    };
    Err(ApiError {
        status,
        code: err["code"]
            .as_str()
            .map_or_else(|| format!("http_{status}"), str::to_owned),
        message: err["message"].as_str().unwrap_or(fallback).into(),
        field: err["field"].as_str().map(str::to_owned),
    })
}
pub(crate) async fn post_abort(
    path: &str,
    body: &Value,
    signal: &web_sys::AbortSignal,
) -> Result<Value, ApiError> {
    let response = response_with_signal("POST", path, &[], Some(body), None, Some(signal)).await?;
    let text = JsFuture::from(response.text()?)
        .await
        .map_err(|_| network())?
        .as_string()
        .ok_or_else(network)?;
    serde_json::from_str(&text).map_err(|_| ApiError::message("応答の形式を確認できませんでした。"))
}
pub(crate) async fn request(
    method: &str,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
    bytes: Option<&[u8]>,
) -> Result<Value, ApiError> {
    let response = response(method, path, query, body, bytes).await?;
    if response.status() == 204 {
        return Ok(Value::Null);
    }
    let text = JsFuture::from(response.text()?)
        .await
        .map_err(|_| network())?
        .as_string()
        .ok_or_else(network)?;
    serde_json::from_str(&text).map_err(|_| {
        ApiError::message("応答の形式を確認できませんでした。もう一度読み込んでください。")
    })
}
pub(crate) fn new_client_id() -> String {
    let mut bytes = [0u8; 12];
    window()
        .crypto()
        .expect("browser crypto")
        .get_random_values_with_u8_array(&mut bytes)
        .expect("secure random client id");
    let mut suffix = String::with_capacity(24);
    for n in bytes {
        write!(&mut suffix, "{n:02x}").expect("writing to a string");
    }
    format!("c_{suffix}")
}
