// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Notification lifetime decisions shared with the browser timer.

pub(crate) struct Lifetime {
    duration: f64,
    deadline: f64,
    hovered: bool,
}
impl Lifetime {
    pub(crate) fn new(kind: &str, action: bool, duration: Option<f64>, now: f64) -> Option<Self> {
        (kind != "error").then(|| {
            let duration = duration.unwrap_or(if action { 10_000.0 } else { 4_000.0 });
            Self {
                duration,
                deadline: now + duration,
                hovered: false,
            }
        })
    }
    pub(crate) fn hover(&mut self, hovered: bool, now: f64) {
        self.hovered = hovered;
        self.restart(now);
    }
    pub(crate) fn restart(&mut self, now: f64) {
        self.deadline = now + self.duration;
    }
    pub(crate) fn expired(&self, now: f64, focused: bool) -> bool {
        !self.hovered && !focused && now >= self.deadline
    }
}

#[cfg(test)]
mod tests {
    use super::Lifetime;

    #[test]
    fn normal_action_custom_and_persistent_error_deadlines() {
        let normal = Lifetime::new("success", false, None, 0.0).unwrap();
        assert!(!normal.expired(3999.0, false));
        assert!(normal.expired(4000.0, false));
        let action = Lifetime::new("success", true, None, 0.0).unwrap();
        assert!(!action.expired(9999.0, false));
        assert!(action.expired(10_000.0, false));
        let custom = Lifetime::new("info", true, Some(6000.0), 0.0).unwrap();
        assert!(!custom.expired(5999.0, false));
        assert!(custom.expired(6000.0, false));
        assert!(Lifetime::new("error", false, Some(0.0), 0.0).is_none());
    }
    #[test]
    fn leaving_hover_cannot_expire_a_focused_notification() {
        let mut lifetime = Lifetime::new("success", true, None, 0.0).unwrap();
        lifetime.hover(true, 500.0);
        assert!(!lifetime.expired(30_000.0, false));
        lifetime.hover(false, 30_000.0);
        assert!(!lifetime.expired(60_000.0, true));
        lifetime.restart(60_000.0);
        assert!(!lifetime.expired(69_999.0, false));
        assert!(lifetime.expired(70_000.0, false));
    }
    #[test]
    fn moving_focus_cannot_expire_a_hovered_notification() {
        let mut lifetime = Lifetime::new("info", false, None, 0.0).unwrap();
        lifetime.hover(true, 500.0);
        lifetime.restart(10_000.0);
        assert!(!lifetime.expired(60_000.0, false));
        lifetime.hover(false, 60_000.0);
        assert!(!lifetime.expired(63_999.0, false));
        assert!(lifetime.expired(64_000.0, false));
    }
}
