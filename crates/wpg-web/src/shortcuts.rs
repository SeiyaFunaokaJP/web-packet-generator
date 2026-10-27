//! Global keyboard shortcuts (spec §2.5 "全操作にキーボードショートカット"):
//! `g` + letter chords for navigation, `?` for help, `l` for the logs drawer,
//! `Escape` to close overlays. The parser is pure; the listener lives in `app`.

/// Screens reachable with `g` + key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    Dashboard,
    Builder,
    Jobs,
    Templates,
    Rx,
    Terminal,
    Snmp,
    Settings,
}

impl Screen {
    /// Router path.
    pub(crate) const fn path(self) -> &'static str {
        match self {
            Screen::Dashboard => "/",
            Screen::Builder => "/builder",
            Screen::Jobs => "/jobs",
            Screen::Templates => "/templates",
            Screen::Rx => "/rx",
            Screen::Terminal => "/terminal",
            Screen::Snmp => "/snmp",
            Screen::Settings => "/settings",
        }
    }
}

/// A recognised shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Go(Screen),
    Help,
    ToggleLogs,
    NewJob,
    Escape,
}

/// How long after `g` the second key is accepted.
pub(crate) const CHORD_TIMEOUT_MS: f64 = 1500.0;

/// Shortcut list for the help overlay: (keys, i18n key of the description).
pub(crate) const HELP: &[(&str, &str)] = &[
    ("g d", "ui-shortcut-dashboard"),
    ("g b", "ui-shortcut-builder"),
    ("g j", "ui-shortcut-jobs"),
    ("g t", "ui-shortcut-templates"),
    ("g r", "ui-shortcut-rx"),
    ("g c", "ui-shortcut-terminal"),
    ("g m", "ui-shortcut-snmp"),
    ("g s", "ui-shortcut-settings"),
    ("n", "ui-shortcut-new-job"),
    ("l", "ui-shortcut-logs"),
    ("?", "ui-shortcut-help"),
    ("Esc", "ui-shortcut-escape"),
];

/// Two-key chord state machine.
#[derive(Debug, Default)]
pub(crate) struct Parser {
    /// Time `g` was pressed.
    pending_g: Option<f64>,
}

impl Parser {
    /// Feeds one `keydown` (`KeyboardEvent.key`). `modifier` = Ctrl / Meta / Alt
    /// held (those combinations belong to the browser). `now_ms` is a monotonic
    /// clock.
    pub(crate) fn feed(&mut self, key: &str, modifier: bool, now_ms: f64) -> Option<Action> {
        if key == "Escape" {
            self.pending_g = None;
            return Some(Action::Escape);
        }
        if modifier {
            self.pending_g = None;
            return None;
        }
        // Lone modifier keys (Shift for `?`) do not break a chord.
        if matches!(key, "Shift" | "Control" | "Alt" | "Meta" | "CapsLock") {
            return None;
        }
        let pending = self
            .pending_g
            .take()
            .is_some_and(|t| now_ms - t <= CHORD_TIMEOUT_MS);
        if pending {
            let screen = match key {
                "d" | "D" => Screen::Dashboard,
                "b" | "B" => Screen::Builder,
                "j" | "J" => Screen::Jobs,
                "t" | "T" => Screen::Templates,
                "r" | "R" => Screen::Rx,
                "c" | "C" => Screen::Terminal,
                "m" | "M" => Screen::Snmp,
                "s" | "S" => Screen::Settings,
                _ => return None,
            };
            return Some(Action::Go(screen));
        }
        match key {
            "g" | "G" => {
                self.pending_g = Some(now_ms);
                None
            }
            "?" => Some(Action::Help),
            "l" | "L" => Some(Action::ToggleLogs),
            "n" | "N" => Some(Action::NewJob),
            _ => None,
        }
    }
}

/// Whether a key event from an element with this tag / `contenteditable` should be
/// left to the element (typing in a form).
pub(crate) fn is_typing_target(tag: &str, content_editable: bool) -> bool {
    content_editable
        || matches!(
            tag.to_ascii_uppercase().as_str(),
            "INPUT" | "TEXTAREA" | "SELECT"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords() {
        let mut p = Parser::default();
        assert_eq!(p.feed("g", false, 0.0), None);
        assert_eq!(p.feed("j", false, 100.0), Some(Action::Go(Screen::Jobs)));
        // The chord is consumed: a lone `j` does nothing.
        assert_eq!(p.feed("j", false, 200.0), None);
        for (k, s) in [
            ("d", Screen::Dashboard),
            ("b", Screen::Builder),
            ("t", Screen::Templates),
            ("r", Screen::Rx),
            ("c", Screen::Terminal),
            ("m", Screen::Snmp),
            ("s", Screen::Settings),
        ] {
            assert_eq!(p.feed("g", false, 1000.0), None);
            assert_eq!(p.feed(k, false, 1001.0), Some(Action::Go(s)));
        }
    }

    #[test]
    fn chord_timeout_and_breaks() {
        let mut p = Parser::default();
        p.feed("g", false, 0.0);
        assert_eq!(p.feed("d", false, CHORD_TIMEOUT_MS + 1.0), None);
        p.feed("g", false, 0.0);
        assert_eq!(p.feed("x", false, 1.0), None);
        assert_eq!(p.feed("d", false, 2.0), None);
        p.feed("g", false, 0.0);
        assert_eq!(p.feed("d", true, 1.0), None, "Ctrl+d is the browser's");
        p.feed("g", false, 0.0);
        assert_eq!(p.feed("Shift", false, 1.0), None);
        assert_eq!(p.feed("D", false, 2.0), Some(Action::Go(Screen::Dashboard)));
    }

    #[test]
    fn single_keys() {
        let mut p = Parser::default();
        assert_eq!(p.feed("?", false, 0.0), Some(Action::Help));
        assert_eq!(p.feed("l", false, 0.0), Some(Action::ToggleLogs));
        assert_eq!(p.feed("n", false, 0.0), Some(Action::NewJob));
        assert_eq!(p.feed("Escape", true, 0.0), Some(Action::Escape));
        assert_eq!(p.feed("q", false, 0.0), None);
        assert_eq!(Screen::Templates.path(), "/templates");
    }

    #[test]
    fn typing_targets() {
        assert!(is_typing_target("input", false));
        assert!(is_typing_target("TEXTAREA", false));
        assert!(is_typing_target("select", false));
        assert!(is_typing_target("div", true));
        assert!(!is_typing_target("BUTTON", false));
    }
}
