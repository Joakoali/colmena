//! Modos de la terminal que le importan a la UI.

use alacritty_terminal::term::TermMode;

use crate::input::InputMode;
use crate::mouse::MouseMode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    pub input: InputMode,
    pub mouse: MouseMode,
    pub alt_screen: bool,
    pub alternate_scroll: bool,
}

impl Modes {
    pub fn from_term_mode(mode: TermMode) -> Self {
        Self {
            input: InputMode {
                app_cursor: mode.contains(TermMode::APP_CURSOR),
                bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            },
            mouse: MouseMode {
                report_click: mode.contains(TermMode::MOUSE_REPORT_CLICK),
                report_drag: mode.contains(TermMode::MOUSE_DRAG),
                report_motion: mode.contains(TermMode::MOUSE_MOTION),
                sgr: mode.contains(TermMode::SGR_MOUSE),
            },
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::term_with;

    #[test]
    fn lee_los_modos_que_activa_la_app() {
        let term = term_with(
            10,
            2,
            b"\x1b[?1h\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?1049h",
        );
        let modes = Modes::from_term_mode(*term.mode());
        assert!(modes.input.app_cursor);
        assert!(modes.input.bracketed_paste);
        assert!(modes.mouse.report_click);
        assert!(modes.mouse.sgr);
        assert!(!modes.mouse.report_drag);
        assert!(modes.alt_screen);
    }

    #[test]
    fn terminal_recien_creada() {
        let term = term_with(10, 2, b"");
        let modes = Modes::from_term_mode(*term.mode());
        assert_eq!(modes.input, InputMode::default());
        assert!(!modes.mouse.any());
        assert!(!modes.alt_screen);
    }
}
