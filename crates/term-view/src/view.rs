//! `TerminalView`: una terminal de term-core dentro de GPUI.

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    App, Bounds, ClipboardItem, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    Focusable, Font, InputHandler, KeyDownEvent, Pixels, Point, Task, UTF16Selection, Window,
    canvas, div, font, point, prelude::*, px, size,
};
use term_core::{GridSize, Palette, Session, SpawnOptions, TermEvent, encode_key, encode_paste};

use crate::convert::{ALT_IS_META, key_input, to_hsla};
use crate::layout::{CellMetrics, cell_at, grid_dims};
use crate::paint::{PaintStyle, paint_snapshot};
// Con alias para no confundir la acción `Copy` con el trait `std::marker::Copy`.
use crate::{Copy as CopyAction, Paste as PasteAction};

#[derive(Clone, Debug)]
pub struct TerminalOptions {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub padding: f32,
    pub palette: Palette,
    /// `None` = shell del usuario.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub scrollback: usize,
}

/// Monoespaciada que viene con cada sistema. En M2 la elige `config.toml`.
fn default_font_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    }
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            font_family: default_font_family().to_string(),
            font_size: 14.0,
            line_height: 1.2,
            padding: 8.0,
            palette: Palette::colmena_dark(),
            program: None,
            cwd: None,
            env: HashMap::new(),
            scrollback: 10_000,
        }
    }
}

/// Geometría del último frame pintado, para traducir posiciones del mouse a celdas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaintedLayout {
    pub origin: Point<Pixels>,
    pub cell: CellMetrics,
    pub padding: f32,
    pub cols: usize,
    pub rows: usize,
    pub cursor: Option<(usize, usize)>,
}

impl PaintedLayout {
    pub(crate) fn cell_at(&self, position: Point<Pixels>) -> (usize, usize, bool) {
        let x = f32::from(position.x - self.origin.x);
        let y = f32::from(position.y - self.origin.y);
        cell_at(x, y, self.padding, self.cell, self.cols, self.rows)
    }
}

pub struct TerminalView {
    pub(crate) options: TerminalOptions,
    pub(crate) session: Option<Arc<Session>>,
    pub(crate) focus_handle: FocusHandle,
    font: Font,
    metrics: Option<CellMetrics>,
    pub(crate) layout: Rc<Cell<Option<PaintedLayout>>>,
    pub(crate) title: Option<String>,
    applied_title: Option<String>,
    pub(crate) exited: Option<Option<i32>>,
    pub(crate) spawn_error: Option<String>,
    marked_text: Option<String>,
    pub(crate) selecting: bool,
    pub(crate) scroll_accum: f32,
    _events: Option<Task<()>>,
}

impl TerminalView {
    pub fn new(options: TerminalOptions, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let font = font(options.font_family.clone());
        let mut view = Self {
            options,
            session: None,
            focus_handle: cx.focus_handle(),
            font,
            metrics: None,
            layout: Rc::new(Cell::new(None)),
            title: None,
            applied_title: None,
            exited: None,
            spawn_error: None,
            marked_text: None,
            selecting: false,
            scroll_accum: 0.0,
            _events: None,
        };
        view.start_session(cx);
        view
    }

    /// Lanza (o relanza) el proceso y la tarea que escucha sus eventos.
    pub(crate) fn start_session(&mut self, cx: &mut Context<Self>) {
        let (tx, mut rx) = mpsc::unbounded::<TermEvent>();
        let tx = Mutex::new(tx);
        let size = self
            .session
            .as_ref()
            .map(|s| s.size())
            .unwrap_or(GridSize::new(80, 24));
        let spawn = SpawnOptions {
            program: self.options.program.clone(),
            cwd: self.options.cwd.clone(),
            env: self.options.env.clone(),
            size,
            scrollback: self.options.scrollback,
            palette: self.options.palette.clone(),
        };
        let on_event = move |event: TermEvent| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.unbounded_send(event);
            }
        };
        match Session::spawn(spawn, on_event) {
            Ok(session) => {
                self.session = Some(Arc::new(session));
                self.exited = None;
                self.spawn_error = None;
                self.title = None;
            }
            Err(err) => {
                tracing::error!("no se pudo abrir el shell: {err}");
                self.session = None;
                self.spawn_error = Some(err.to_string());
                return;
            }
        }
        self._events = Some(cx.spawn(async move |this, cx| {
            while let Some(first) = rx.next().await {
                let mut batch = vec![first];
                while let Ok(event) = rx.try_recv() {
                    batch.push(event);
                }
                let alive = this.update(cx, |view, cx| {
                    for event in batch {
                        view.handle_event(event, cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    fn handle_event(&mut self, event: TermEvent, cx: &mut Context<Self>) {
        tracing::debug!("evento: {event:?}");
        match event {
            TermEvent::Wakeup | TermEvent::Bell => {}
            TermEvent::Title(title) => self.title = Some(title),
            TermEvent::ResetTitle => self.title = None,
            TermEvent::ClipboardStore(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text))
            }
            TermEvent::Exited(code) => self.exited = Some(code),
        }
    }

    fn measure(&mut self, window: &mut Window) -> CellMetrics {
        if let Some(metrics) = self.metrics {
            return metrics;
        }
        let font_size = px(self.options.font_size);
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&self.font);
        let width = text_system
            .advance(font_id, font_size, 'm')
            .map(|s| f32::from(s.width))
            .unwrap_or(self.options.font_size * 0.6);
        let metrics = CellMetrics {
            width,
            height: (self.options.font_size * self.options.line_height).round(),
        };
        self.metrics = Some(metrics);
        metrics
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let Some(session) = self.session.clone().filter(|_| self.exited.is_none()) else {
            if ks.key == "enter" {
                self.start_session(cx);
                cx.notify();
                cx.stop_propagation();
            }
            return;
        };
        if ks.modifiers.shift
            && !ks.modifiers.control
            && (ks.key == "pageup" || ks.key == "pagedown")
        {
            let rows = session.size().rows as i32;
            session.scroll(if ks.key == "pageup" { rows } else { -rows });
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(bytes) = encode_key(&key_input(ks), session.modes().input, ALT_IS_META) {
            session.scroll_to_bottom();
            session.clear_selection();
            session.write(bytes);
            cx.stop_propagation();
        }
    }

    fn copy(&mut self, _: &CopyAction, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.session.as_ref().and_then(|s| s.selection_text()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, _: &PasteAction, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone().filter(|_| self.exited.is_none()) else {
            return;
        };
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            session.scroll_to_bottom();
            session.write(encode_paste(&text, session.modes().input));
        }
    }

    fn banner(&self) -> Option<String> {
        if let Some(err) = &self.spawn_error {
            return Some(format!(
                "[no se pudo abrir el shell: {err}] — Enter para reintentar"
            ));
        }
        match self.exited {
            Some(Some(code)) => Some(format!(
                "[proceso terminó, código {code}] — Enter para reabrir"
            )),
            Some(None) => Some("[proceso terminó] — Enter para reabrir".to_string()),
            None => None,
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.title.clone().unwrap_or_else(|| "colmena".to_string());
        if self.applied_title.as_deref() != Some(title.as_str()) {
            window.set_window_title(&title);
            self.applied_title = Some(title);
        }

        let metrics = self.measure(window);
        let focused = self.focus_handle.is_focused(window);
        let session = self.session.clone();
        let palette = self.options.palette.clone();
        let layout = self.layout.clone();
        let padding = self.options.padding;
        let style = PaintStyle {
            font: self.font.clone(),
            font_size: px(self.options.font_size),
            cell: metrics,
            padding,
            foreground: palette.foreground,
        };
        let marked = self.marked_text.clone();
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let banner = self.banner().map(|text| {
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .px_3()
                .py_2()
                .bg(to_hsla(palette.ansi[8]))
                .text_color(to_hsla(palette.foreground))
                .text_sm()
                .child(text)
        });

        div()
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .relative()
            .size_full()
            .bg(to_hsla(palette.background))
            .child(
                canvas(
                    move |bounds, _window, _cx| {
                        // Ajustar el PTY al área y tomar el snapshot ya con el tamaño nuevo.
                        let (cols, rows) = grid_dims(
                            f32::from(bounds.size.width),
                            f32::from(bounds.size.height),
                            padding,
                            metrics,
                        );
                        let snapshot = session.map(|session| {
                            session.resize(GridSize {
                                cols,
                                rows,
                                cell_width: metrics.width.round() as u16,
                                cell_height: metrics.height.round() as u16,
                            });
                            session.snapshot(&palette, focused)
                        });
                        layout.set(Some(PaintedLayout {
                            origin: bounds.origin,
                            cell: metrics,
                            padding,
                            cols: cols as usize,
                            rows: rows as usize,
                            cursor: snapshot
                                .as_ref()
                                .and_then(|s| s.cursor)
                                .map(|c| (c.col, c.row)),
                        }));
                        snapshot
                    },
                    move |bounds, snapshot, window, cx| {
                        window.handle_input(
                            &focus,
                            TerminalInputHandler(ElementInputHandler::new(bounds, entity)),
                            cx,
                        );
                        if let Some(snapshot) = snapshot {
                            paint_snapshot(
                                &snapshot,
                                &style,
                                bounds.origin,
                                marked.as_deref(),
                                window,
                                cx,
                            );
                        }
                    },
                )
                .size_full(),
            )
            .children(banner)
    }
}

/// `ElementInputHandler` con el "mantener apretado" de macOS apagado: una tecla sostenida se
/// repite (`j` en `vim`) en vez de abrir el popup de acentos. Lo demás se delega tal cual.
struct TerminalInputHandler(ElementInputHandler<TerminalView>);

impl InputHandler for TerminalInputHandler {
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.0
            .selected_text_range(ignore_disabled_input, window, cx)
    }

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.0.marked_text_range(window, cx)
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.0
            .text_for_range(range_utf16, adjusted_range, window, cx)
    }

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0
            .replace_text_in_range(replacement_range, text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.replace_and_mark_text_in_range(
            range_utf16,
            new_text,
            new_selected_range,
            window,
            cx,
        );
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.0.unmark_text(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.0.bounds_for_range(range_utf16, window, cx)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.0.character_index_for_point(point, window, cx)
    }

    fn apple_press_and_hold_enabled(&mut self) -> bool {
        false
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_text
            .as_ref()
            .map(|t| 0..t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked_text = None;
        cx.notify();
    }

    /// Texto confirmado (tecla común, tecla muerta resuelta, IME): va directo al PTY.
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = None;
        if let Some(session) = self.session.as_ref().filter(|_| self.exited.is_none()) {
            if !text.is_empty() {
                session.scroll_to_bottom();
                session.clear_selection();
                session.write(text.as_bytes().to_vec());
            }
        }
        cx.notify();
    }

    /// Texto en composición (por ejemplo "´" antes de la vocal): solo se muestra.
    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = (!new_text.is_empty()).then(|| new_text.to_string());
        cx.notify();
    }

    /// Posición del cursor, para ubicar la ventanita del IME.
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.get()?;
        let (col, row) = layout.cursor?;
        let x = layout.origin.x + px(layout.padding + layout.cell.width * col as f32);
        let y = layout.origin.y + px(layout.padding + layout.cell.height * row as f32);
        Some(Bounds::new(
            point(x, y),
            size(px(layout.cell.width), px(layout.cell.height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
