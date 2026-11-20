//! Núcleo de terminal de Colmena: PTY, emulación VT y codificación de entrada, sin UI.

pub mod color;
pub mod size;
pub mod snapshot;

pub use color::{Palette, Rgb};
pub use size::GridSize;
pub use snapshot::{CellStyle, CellView, CursorShape, CursorView, Snapshot, snapshot};

#[cfg(test)]
pub(crate) mod test_util {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::ansi::Processor;

    use crate::size::GridSize;

    /// Terminal en memoria (sin PTY) alimentada con `bytes`.
    pub(crate) fn term_with(cols: u16, rows: u16, bytes: &[u8]) -> Term<VoidListener> {
        let mut term = Term::new(Config::default(), &GridSize::new(cols, rows), VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, bytes);
        term
    }
}
