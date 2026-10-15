// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::model::{EntryPage, Filters, PAGE_SIZE};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCredentials, RequestInit, RequestMode, Response};

use super::{dom, navigation};

pub(super) async fn entries(filters: &Filters) -> Result<EntryPage, String> {
    let params = navigation::params(filters).map_err(|_| network_message())?;
    // The API defaults to active entries, while the journal shows all by default.
    params.set("state", &filters.state);
    params.delete("page");
    params.set("limit", &PAGE_SIZE.to_string());
    params.set("offset", &filters.offset().to_string());
    let url = format!("/api/v1/entries?{}", params.to_string());
    let init = RequestInit::new();
    init.set_method("GET");
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_mode(RequestMode::SameOrigin);
    let request = Request::new_with_str_and_init(&url, &init).map_err(|_| network_message())?;
    let window = dom::window().map_err(|_| network_message())?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| network_message())?;
    let response: Response = response.dyn_into().map_err(|_| network_message())?;
    if !response.ok() {
        return Err(match response.status() {
            401 => {
                "合言葉の期限が切れました。サーバー起動時のアドレスから開き直してください。".into()
            }
            400 | 422 => {
                "検索条件を確認してください。期間の始めと終わりも確かめてください。".into()
            }
            _ => "仕訳を読み込めませんでした。もう一度読み込んでください。".into(),
        });
    }
    let text = response.text().map_err(|_| network_message())?;
    let text = JsFuture::from(text).await.map_err(|_| network_message())?;
    let text = text.as_string().ok_or_else(network_message)?;
    // Integer yen values stay as i64; JSON numbers never pass through JS Number.
    serde_json::from_str(&text).map_err(|_| {
        "応答の形式を確認できませんでした。通常の仕訳帳から開き直してください。".into()
    })
}

fn network_message() -> String {
    "サーバーにつながりません。kamokuが起動しているか確かめて、もう一度読み込んでください。".into()
}
