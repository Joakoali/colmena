//! Reporte de mouse a la app que corre en la terminal (vim, htop, TUIs) y scroll alternativo.

use crate::input::Mods;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    /// Movimiento con un botón apretado.
    Drag,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseMode {
    pub report_click: bool,
    pub report_drag: bool,
    pub report_motion: bool,
    pub sgr: bool,
}

impl MouseMode {
    pub fn any(&self) -> bool {
        self.report_click || self.report_drag || self.report_motion
    }
}

/// Secuencia de reporte de mouse para la app, o `None` si la app no la pidió.
/// `col` y `row` son 0-based dentro de la vista.
pub fn encode_mouse(
    button: ReportButton,
    action: MouseAction,
    col: usize,
    row: usize,
    mods: Mods,
    mode: MouseMode,
) -> Option<Vec<u8>> {
    if !mode.any() {
        return None;
    }
    if action == MouseAction::Drag && !(mode.report_drag || mode.report_motion) {
        return None;
    }
    let mut code: u32 = match button {
        ReportButton::Left => 0,
        ReportButton::Middle => 1,
        ReportButton::Right => 2,
        ReportButton::WheelUp => 64,
        ReportButton::WheelDown => 65,
    };
    if action == MouseAction::Drag {
        code += 32;
    }
    if mods.shift {
        code += 4;
    }
    if mods.alt {
        code += 8;
    }
    if mods.ctrl {
        code += 16;
    }
    let (x, y) = (col + 1, row + 1);

    if mode.sgr {
        let suffix = if action == MouseAction::Release {
            'm'
        } else {
            'M'
        };
        return Some(format!("\x1b[<{code};{x};{y}{suffix}").into_bytes());
    }

    let is_wheel = matches!(button, ReportButton::WheelUp | ReportButton::WheelDown);
    if action == MouseAction::Release && is_wheel {
        return None;
    }
    if x > 223 || y > 223 {
        return None;
    }
    // En el formato legacy, soltar se reporta como "botón 3" conservando los modificadores.
    let code = if action == MouseAction::Release {
        (code & !0b11) | 3
    } else {
        code
    };
    Some(vec![
        0x1b,
        b'[',
        b'M',
        (32 + code) as u8,
        (32 + x) as u8,
        (32 + y) as u8,
    ])
}

/// Rueda en pantalla alternativa sin modo de mouse (less, man): se traduce a flechas.
/// `lines > 0` = hacia arriba.
pub fn alternate_scroll(lines: i32, app_cursor: bool) -> Vec<u8> {
    let seq: &[u8] = match (lines > 0, app_cursor) {
        (true, true) => b"\x1bOA",
        (true, false) => b"\x1b[A",
        (false, true) => b"\x1bOB",
        (false, false) => b"\x1b[B",
    };
    seq.repeat(lines.unsigned_abs() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLICK_SGR: MouseMode = MouseMode {
        report_click: true,
        report_drag: false,
        report_motion: false,
        sgr: true,
    };
    const DRAG_SGR: MouseMode = MouseMode {
        report_click: true,
        report_drag: true,
        report_motion: false,
        sgr: true,
    };
    const CLICK_LEGACY: MouseMode = MouseMode {
        report_click: true,
        report_drag: false,
        report_motion: false,
        sgr: false,
    };

    fn none() -> Mods {
        Mods::default()
    }

    #[test]
    fn sin_modo_de_mouse_no_se_reporta() {
        let off = MouseMode::default();
        assert!(!off.any());
        assert_eq!(
            encode_mouse(ReportButton::Left, MouseAction::Press, 0, 0, none(), off),
            None
        );
    }

    #[test]
    fn sgr_click_y_soltar() {
        let press = encode_mouse(
            ReportButton::Left,
            MouseAction::Press,
            4,
            9,
            none(),
            CLICK_SGR,
        );
        assert_eq!(press, Some(b"\x1b[<0;5;10M".to_vec()));
        let release = encode_mouse(
            ReportButton::Left,
            MouseAction::Release,
            4,
            9,
            none(),
            CLICK_SGR,
        );
        assert_eq!(release, Some(b"\x1b[<0;5;10m".to_vec()));
    }

    #[test]
    fn sgr_con_modificadores_y_rueda() {
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        let right = encode_mouse(
            ReportButton::Right,
            MouseAction::Press,
            0,
            0,
            ctrl,
            CLICK_SGR,
        );
        assert_eq!(right, Some(b"\x1b[<18;1;1M".to_vec()));
        let up = encode_mouse(
            ReportButton::WheelUp,
            MouseAction::Press,
            1,
            1,
            none(),
            CLICK_SGR,
        );
        assert_eq!(up, Some(b"\x1b[<64;2;2M".to_vec()));
    }

    #[test]
    fn arrastre_solo_con_modo_drag() {
        let drag = |mode| encode_mouse(ReportButton::Left, MouseAction::Drag, 2, 3, none(), mode);
        assert_eq!(drag(CLICK_SGR), None);
        assert_eq!(drag(DRAG_SGR), Some(b"\x1b[<32;3;4M".to_vec()));
    }

    #[test]
    fn formato_legacy() {
        let press = encode_mouse(
            ReportButton::Left,
            MouseAction::Press,
            0,
            0,
            none(),
            CLICK_LEGACY,
        );
        assert_eq!(press, Some(vec![0x1b, b'[', b'M', 32, 33, 33]));
        let release = encode_mouse(
            ReportButton::Left,
            MouseAction::Release,
            0,
            0,
            none(),
            CLICK_LEGACY,
        );
        assert_eq!(release, Some(vec![0x1b, b'[', b'M', 35, 33, 33]));
        let far = encode_mouse(
            ReportButton::Left,
            MouseAction::Press,
            300,
            0,
            none(),
            CLICK_LEGACY,
        );
        assert_eq!(far, None, "legacy no puede codificar columnas > 223");
    }

    #[test]
    fn scroll_alternativo_manda_flechas() {
        assert_eq!(alternate_scroll(2, false), b"\x1b[A\x1b[A".to_vec());
        assert_eq!(alternate_scroll(-1, true), b"\x1bOB".to_vec());
    }
}
