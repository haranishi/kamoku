// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use serde_json::Value;
pub(crate) fn saved_up_to(settings: &Value, opening: &Value) -> usize {
    if settings["business_name"].as_str().unwrap_or("").is_empty()
        || settings["opened_on"].as_str().unwrap_or("").is_empty()
    {
        0
    } else if opening["entry_id"].as_str().is_some_and(|s| !s.is_empty()) {
        3
    } else {
        1
    }
}
pub(crate) fn requested_step(text: &str) -> Option<usize> {
    if text.is_empty() {
        None
    } else {
        usize::try_from(text.parse::<i64>().unwrap_or(1).clamp(1, 4)).ok()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn resume_uses_server_saved_data_only() {
        assert_eq!(saved_up_to(&json!({}), &json!({})), 0);
        let s = json!({"business_name":"試験","opened_on":"2026-01-01"});
        assert_eq!(saved_up_to(&s, &json!({"entry_id":null})), 1);
        assert_eq!(saved_up_to(&s, &json!({"entry_id":"abc"})), 3);
    }
    #[test]
    fn step_history_is_bounded() {
        assert_eq!(requested_step(""), None);
        assert_eq!(requested_step("9"), Some(4));
        assert_eq!(requested_step("0"), Some(1));
        assert_eq!(requested_step("garbage"), Some(1));
    }
}
