//! Vista GPUI de una terminal de `term-core`.

pub mod convert;
pub mod layout;
mod mouse;
mod paint;
mod view;

use gpui::{App, KeyBinding, actions};

pub use view::{TerminalOptions, TerminalView};

actions!(terminal, [Copy, Paste]);

/// Atajos de la terminal: Cmd en macOS, Ctrl+Shift en Linux/Windows.
pub fn bind_keys(cx: &mut App) {
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-c", Copy, Some("Terminal")),
            KeyBinding::new("cmd-v", Paste, Some("Terminal")),
        ]);
    } else {
        cx.bind_keys([
            KeyBinding::new("ctrl-shift-c", Copy, Some("Terminal")),
            KeyBinding::new("ctrl-shift-v", Paste, Some("Terminal")),
        ]);
    }
}
