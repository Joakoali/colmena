//! Mouse sobre la terminal: selección local o reporte a la app, y rueda.

use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta,
    ScrollWheelEvent, Window,
};
use term_core::{MouseAction, ReportButton, SelectKind, alternate_scroll, encode_mouse};

use crate::convert::mods;
use crate::view::TerminalView;

impl TerminalView {
    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else {
            return;
        };
        if self.exited.is_some() {
            return;
        }
        let (col, row, right_half) = layout.cell_at(event.position);
        let modes = session.modes();
        // Con Shift se selecciona aunque la app haya pedido el mouse (como en xterm).
        if modes.mouse.any() && !event.modifiers.shift {
            // Un clic que va a la app no continúa ninguna selección local.
            self.selecting = false;
            let m = mods(&event.modifiers);
            if let Some(bytes) = encode_mouse(
                ReportButton::Left,
                MouseAction::Press,
                col,
                row,
                m,
                modes.mouse,
            ) {
                session.write(bytes);
                self.reported_press = Some((col, row));
            }
            return;
        }
        let kind = match event.click_count {
            2 => SelectKind::Word,
            n if n >= 3 => SelectKind::Line,
            _ => SelectKind::Simple,
        };
        session.start_selection(col, row, right_half, kind);
        self.selecting = true;
        self.reported_press = None;
        cx.notify();
    }

    /// Se escucha a nivel ventana: llega aunque el mouse esté fuera de la vista, y `cell_at` lo
    /// lleva a la celda del borde más cercana. Solo sigue arrastres que empezaron en esta vista.
    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left)
            || !(self.selecting || self.reported_press.is_some())
        {
            return;
        }
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else {
            return;
        };
        let (col, row, right_half) = layout.cell_at(event.position);
        if self.selecting {
            session.update_selection(col, row, right_half);
            cx.notify();
            return;
        }
        // macOS repite el último arrastre mientras el mouse está quieto: no reportar la misma celda.
        if self.reported_press == Some((col, row)) {
            return;
        }
        self.reported_press = Some((col, row));
        let modes = session.modes();
        let m = mods(&event.modifiers);
        if let Some(bytes) = encode_mouse(
            ReportButton::Left,
            MouseAction::Drag,
            col,
            row,
            m,
            modes.mouse,
        ) {
            session.write(bytes);
        }
    }

    /// Registrado con `on_mouse_up` y `on_mouse_up_out`: también llega si se suelta fuera de la
    /// vista o de la ventana.
    pub(crate) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selecting {
            self.selecting = false;
            cx.notify();
            return;
        }
        // Solo se reporta la liberación de un clic que se reportó.
        if self.reported_press.take().is_none() {
            return;
        }
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else {
            return;
        };
        let (col, row, _) = layout.cell_at(event.position);
        let modes = session.modes();
        let m = mods(&event.modifiers);
        if let Some(bytes) = encode_mouse(
            ReportButton::Left,
            MouseAction::Release,
            col,
            row,
            m,
            modes.mouse,
        ) {
            session.write(bytes);
        }
    }

    pub(crate) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else {
            return;
        };
        let lines = match event.delta {
            ScrollDelta::Lines(delta) => delta.y,
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / layout.cell.height,
        };
        self.scroll_accum += lines;
        let whole = self.scroll_accum.trunc() as i32;
        if whole == 0 {
            return;
        }
        self.scroll_accum -= whole as f32;

        let modes = session.modes();
        if modes.mouse.any() {
            let (col, row, _) = layout.cell_at(event.position);
            let button = if whole > 0 {
                ReportButton::WheelUp
            } else {
                ReportButton::WheelDown
            };
            let m = mods(&event.modifiers);
            for _ in 0..whole.unsigned_abs() {
                if let Some(bytes) =
                    encode_mouse(button, MouseAction::Press, col, row, m, modes.mouse)
                {
                    session.write(bytes);
                }
            }
        } else if modes.alt_screen && modes.alternate_scroll {
            session.write(alternate_scroll(whole, modes.input.app_cursor));
        } else {
            session.scroll(whole);
        }
        cx.notify();
    }
}
