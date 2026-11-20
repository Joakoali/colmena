//! Recibe los eventos de alacritty: contesta al PTY lo que es del protocolo (consultas de
//! color, tamaño, respuestas DA/DSR) y reenvía a la UI lo que le importa.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::event_loop::{EventLoopSender, Msg};

use crate::color::Palette;
use crate::session::TermEvent;
use crate::size::GridSize;

type Callback = Arc<dyn Fn(TermEvent) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Listener {
    sender: Arc<OnceLock<EventLoopSender>>,
    size: Arc<Mutex<GridSize>>,
    palette: Arc<Palette>,
    on_event: Callback,
    exit_reported: Arc<AtomicBool>,
}

impl Listener {
    pub(crate) fn new(size: Arc<Mutex<GridSize>>, palette: Palette, on_event: Callback) -> Self {
        Self {
            sender: Arc::new(OnceLock::new()),
            size,
            palette: Arc::new(palette),
            on_event,
            exit_reported: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Se llama una vez, después de crear el event loop y antes de arrancarlo.
    pub(crate) fn attach(&self, sender: EventLoopSender) {
        let _ = self.sender.set(sender);
    }

    fn reply(&self, text: String) {
        if let Some(sender) = self.sender.get() {
            let _ = sender.send(Msg::Input(text.into_bytes().into()));
        }
    }

    fn emit(&self, event: TermEvent) {
        (self.on_event)(event);
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => self.reply(text),
            Event::ColorRequest(index, format) => {
                self.reply(format(self.palette.by_index(index).into()));
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *self.size.lock().expect("size lock");
                self.reply(format(size.window_size()));
            }
            Event::Wakeup => self.emit(TermEvent::Wakeup),
            Event::Title(title) => self.emit(TermEvent::Title(title)),
            Event::ResetTitle => self.emit(TermEvent::ResetTitle),
            Event::Bell => self.emit(TermEvent::Bell),
            Event::ClipboardStore(_, text) => self.emit(TermEvent::ClipboardStore(text)),
            Event::ChildExit(status) => {
                self.exit_reported.store(true, Ordering::SeqCst);
                self.emit(TermEvent::Exited(status.code()));
            }
            Event::Exit => {
                if !self.exit_reported.swap(true, Ordering::SeqCst) {
                    self.emit(TermEvent::Exited(None));
                }
            }
            Event::MouseCursorDirty | Event::CursorBlinkingChange | Event::ClipboardLoad(..) => {}
        }
    }
}
