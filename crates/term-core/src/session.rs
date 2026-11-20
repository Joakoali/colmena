//! Una terminal viva: proceso en un PTY + estado VT de alacritty.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::tty;

use crate::color::Palette;
use crate::listener::Listener;
use crate::modes::Modes;
use crate::selection::{self, SelectKind};
use crate::size::GridSize;
use crate::snapshot::{Snapshot, snapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermEvent {
    /// Hay contenido nuevo para dibujar.
    Wakeup,
    Title(String),
    ResetTitle,
    Bell,
    /// La app pidió copiar al portapapeles (OSC 52).
    ClipboardStore(String),
    /// El proceso terminó. `None` si el sistema no informó el código.
    Exited(Option<i32>),
}

#[derive(Clone, Debug)]
pub struct SpawnOptions {
    /// Programa y argumentos; `None` = shell por defecto del usuario.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: Option<PathBuf>,
    /// Variables extra para el proceso (por ejemplo `COLMENA_PANE_ID` en M3).
    pub env: HashMap<String, String>,
    pub size: GridSize,
    pub scrollback: usize,
    pub palette: Palette,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            program: None,
            cwd: None,
            env: HashMap::new(),
            size: GridSize::new(80, 24),
            scrollback: 10_000,
            palette: Palette::colmena_dark(),
        }
    }
}

pub struct Session {
    term: Arc<FairMutex<Term<Listener>>>,
    sender: EventLoopSender,
    size: Arc<Mutex<GridSize>>,
}

impl Session {
    /// Lanza el proceso. `on_event` se llama desde el hilo del PTY.
    pub fn spawn(
        opts: SpawnOptions,
        on_event: impl Fn(TermEvent) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let mut env = opts.env;
        env.entry("TERM".into())
            .or_insert_with(|| "xterm-256color".into());
        env.entry("COLORTERM".into())
            .or_insert_with(|| "truecolor".into());
        // En Windows `Options` tiene campos extra, por eso `..Default::default()`.
        #[allow(clippy::needless_update)]
        let pty_options = tty::Options {
            shell: opts
                .program
                .map(|(program, args)| tty::Shell::new(program, args)),
            working_directory: opts.cwd,
            drain_on_exit: true,
            env,
            ..Default::default()
        };

        let size = Arc::new(Mutex::new(opts.size));
        let listener = Listener::new(size.clone(), opts.palette, Arc::new(on_event));
        let config = Config {
            scrolling_history: opts.scrollback,
            ..Config::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(
            config,
            &opts.size,
            listener.clone(),
        )));

        let pty = tty::new(&pty_options, opts.size.window_size(), 0)?;
        let event_loop = EventLoop::new(term.clone(), listener.clone(), pty, true, false)?;
        let sender = event_loop.channel();
        listener.attach(sender.clone());
        // El hilo queda corriendo; se detiene con Msg::Shutdown en Drop.
        let _ = event_loop.spawn();

        Ok(Self { term, sender, size })
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let bytes = bytes.into();
        if !bytes.is_empty() {
            let _ = self.sender.send(Msg::Input(bytes));
        }
    }

    pub fn resize(&self, size: GridSize) {
        {
            let mut current = self.size.lock().expect("size lock");
            if *current == size {
                return;
            }
            *current = size;
        }
        self.term.lock().resize(size);
        let _ = self.sender.send(Msg::Resize(size.window_size()));
    }

    pub fn size(&self) -> GridSize {
        *self.size.lock().expect("size lock")
    }

    pub fn snapshot(&self, palette: &Palette, focused: bool) -> Snapshot {
        snapshot(&self.term.lock(), palette, focused)
    }

    pub fn modes(&self) -> Modes {
        Modes::from_term_mode(*self.term.lock().mode())
    }

    /// `lines > 0` sube hacia el historial.
    pub fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_to_bottom(&self) {
        self.term.lock().scroll_display(Scroll::Bottom);
    }

    pub fn start_selection(&self, col: usize, row: usize, right_half: bool, kind: SelectKind) {
        selection::start_selection(&mut self.term.lock(), col, row, right_half, kind);
    }

    pub fn update_selection(&self, col: usize, row: usize, right_half: bool) {
        selection::update_selection(&mut self.term.lock(), col, row, right_half);
    }

    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        selection::selection_text(&self.term.lock())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}
