// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Browser-independent IME decisions shared by every application key handler.

pub(crate) const IME_ENTER_WINDOW_MS: f64 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyKind {
    /// COMPOSING: leave the browser's composition behavior alone.
    Composing,
    /// SWALLOW: prevent a late IME confirmation from activating the application.
    Swallow,
    /// REPEAT: prevent held Enter from saving or confirming repeatedly.
    Repeat,
    Normal,
}

impl KeyKind {
    pub(crate) fn should_ignore(self) -> bool {
        self != Self::Normal
    }

    pub(crate) fn prevents_default(self) -> bool {
        matches!(self, Self::Swallow | Self::Repeat)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct KeyInput<'a> {
    pub key: &'a str,
    pub key_code: u32,
    pub is_composing: bool,
    pub repeat: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ImeEvent {
    Start,
    End,
    FocusOut,
}

pub(crate) struct ImeState {
    installed: bool,
    composing: bool,
    last_end_at: Option<f64>,
}

impl ImeState {
    pub(crate) const fn new() -> Self {
        Self {
            installed: false,
            composing: false,
            last_end_at: None,
        }
    }

    pub(crate) fn install(&mut self) {
        self.installed = true;
        self.composing = false;
        self.last_end_at = None;
    }

    pub(crate) fn uninstall(&mut self) {
        self.installed = false;
        self.composing = false;
    }

    pub(crate) fn observe(&mut self, event: ImeEvent, now: f64) {
        if !self.installed {
            return;
        }
        match event {
            ImeEvent::Start => self.composing = true,
            ImeEvent::End => {
                self.composing = false;
                self.last_end_at = Some(now);
            }
            ImeEvent::FocusOut => self.composing = false,
        }
    }

    pub(crate) fn is_composing(&self) -> bool {
        self.composing
    }

    pub(crate) fn classify(&self, input: Option<KeyInput<'_>>, now: f64) -> KeyKind {
        let Some(input) = input else {
            return KeyKind::Normal;
        };
        if input.is_composing || self.is_composing() {
            return KeyKind::Composing;
        }
        if input.key_code == 229 {
            return KeyKind::Swallow;
        }
        if input.key == "Enter"
            && self
                .last_end_at
                .is_some_and(|ended| now - ended <= IME_ENTER_WINDOW_MS)
        {
            return KeyKind::Swallow;
        }
        if input.key == "Enter" && input.repeat {
            return KeyKind::Repeat;
        }
        KeyKind::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::{IME_ENTER_WINDOW_MS, ImeEvent, ImeState, KeyInput, KeyKind};

    fn setup() -> ImeState {
        let mut state = ImeState::new();
        state.install();
        state
    }

    fn enter() -> KeyInput<'static> {
        KeyInput {
            key: "Enter",
            key_code: 13,
            is_composing: false,
            repeat: false,
        }
    }

    #[test]
    fn event_composition_is_ignored_without_preventing_the_browser() {
        let state = setup();
        let kind = state.classify(
            Some(KeyInput {
                is_composing: true,
                ..enter()
            }),
            1000.0,
        );
        assert_eq!(kind, KeyKind::Composing);
        assert!(kind.should_ignore());
        assert!(!kind.prevents_default());
    }

    #[test]
    fn tracking_start_and_end_cover_keys_without_the_event_composition_flag() {
        let mut state = setup();
        state.observe(ImeEvent::Start, 1000.0);
        assert!(state.is_composing());
        let key = KeyInput {
            key: "ArrowDown",
            key_code: 40,
            ..enter()
        };
        let kind = state.classify(Some(key), 1000.0);
        assert_eq!(kind, KeyKind::Composing);
        assert!(kind.should_ignore());
        assert!(!kind.prevents_default());
        state.observe(ImeEvent::End, 2000.0);
        assert!(!state.is_composing());
    }

    #[test]
    fn process_key_229_is_swallowed_and_prevents_default_activation() {
        let state = setup();
        let kind = state.classify(
            Some(KeyInput {
                key: "Process",
                key_code: 229,
                ..enter()
            }),
            1000.0,
        );
        assert_eq!(kind, KeyKind::Swallow);
        assert!(kind.should_ignore());
        assert!(kind.prevents_default());
    }

    #[test]
    fn safari_post_composition_window_includes_fifty_ms_and_only_enter() {
        let mut state = setup();
        state.observe(ImeEvent::Start, 1000.0);
        state.observe(ImeEvent::End, 5000.0);
        for elapsed in [10.0, IME_ENTER_WINDOW_MS] {
            let kind = state.classify(Some(enter()), 5000.0 + elapsed);
            assert_eq!(kind, KeyKind::Swallow);
            assert!(kind.should_ignore());
            assert!(kind.prevents_default());
        }
        let down = KeyInput {
            key: "ArrowDown",
            key_code: 40,
            ..enter()
        };
        assert_eq!(state.classify(Some(down), 5010.0), KeyKind::Normal);
        let later = state.classify(Some(enter()), 5000.0 + IME_ENTER_WINDOW_MS + 1.0);
        assert_eq!(later, KeyKind::Normal);
        assert!(!later.should_ignore());
        assert!(!later.prevents_default());
    }

    #[test]
    fn held_enter_is_suppressed_but_held_arrow_keys_still_work() {
        let state = setup();
        let held = state.classify(
            Some(KeyInput {
                repeat: true,
                ..enter()
            }),
            1000.0,
        );
        assert_eq!(held, KeyKind::Repeat);
        assert!(held.should_ignore());
        assert!(held.prevents_default());
        let down = KeyInput {
            key: "ArrowDown",
            key_code: 40,
            repeat: true,
            ..enter()
        };
        assert_eq!(state.classify(Some(down), 1000.0), KeyKind::Normal);
    }

    #[test]
    fn composition_precedes_process_and_repeat_while_swallow_precedes_repeat() {
        let mut state = setup();
        let composing = KeyInput {
            is_composing: true,
            key_code: 229,
            repeat: true,
            ..enter()
        };
        assert_eq!(state.classify(Some(composing), 1000.0), KeyKind::Composing);
        state.observe(ImeEvent::Start, 1000.0);
        assert_eq!(
            state.classify(
                Some(KeyInput {
                    is_composing: false,
                    ..composing
                }),
                1000.0
            ),
            KeyKind::Composing
        );
        state.observe(ImeEvent::End, 9000.0);
        assert_eq!(
            state.classify(
                Some(KeyInput {
                    repeat: true,
                    ..enter()
                }),
                9001.0
            ),
            KeyKind::Swallow
        );
    }

    #[test]
    fn text_keys_and_missing_events_are_normal() {
        let mut state = setup();
        let normal = state.classify(
            Some(KeyInput {
                key: "n",
                key_code: 78,
                ..enter()
            }),
            1000.0,
        );
        assert_eq!(normal, KeyKind::Normal);
        assert!(!normal.should_ignore());
        assert!(!normal.prevents_default());
        assert_eq!(state.classify(None, 1000.0), KeyKind::Normal);
        state.observe(ImeEvent::Start, 1000.0);
        assert_eq!(state.classify(None, 1000.0), KeyKind::Normal);
    }

    #[test]
    fn focusout_clears_composition_when_the_browser_omits_compositionend() {
        let mut state = setup();
        state.observe(ImeEvent::Start, 1000.0);
        state.observe(ImeEvent::FocusOut, 1000.0);
        assert!(!state.is_composing());
        assert_eq!(state.classify(Some(enter()), 1000.0), KeyKind::Normal);
    }

    #[test]
    fn uninstall_stops_tracking_events_and_clears_stuck_composition() {
        let mut state = setup();
        state.observe(ImeEvent::Start, 1000.0);
        state.uninstall();
        state.observe(ImeEvent::Start, 2000.0);
        assert!(!state.is_composing());
        assert_eq!(state.classify(Some(enter()), 2000.0), KeyKind::Normal);
    }

    #[test]
    fn reinstall_resets_a_previous_sessions_confirmation_window() {
        let mut state = setup();
        state.observe(ImeEvent::End, 5000.0);
        state.uninstall();
        state.install();
        assert_eq!(state.classify(Some(enter()), 5001.0), KeyKind::Normal);
    }
}
