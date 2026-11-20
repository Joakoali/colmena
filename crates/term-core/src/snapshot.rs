//! Copia del contenido visible de la terminal, con colores ya resueltos, lista para pintar.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{RenderableContent, Term};
use alacritty_terminal::vte::ansi::CursorShape as VteCursorShape;

use crate::color::{Palette, Rgb};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellView {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub style: CellStyle,
    /// Ocupa dos columnas (CJK, algunos emoji).
    pub wide: bool,
    /// Segunda mitad de un carácter ancho: no se dibuja.
    pub spacer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    /// Bloque lleno: ya viene pintado en la celda (colores invertidos).
    Block,
    HollowBlock,
    Beam,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorView {
    pub col: usize,
    pub row: usize,
    pub shape: CursorShape,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cols: usize,
    pub rows: usize,
    /// `rows * cols` celdas, fila por fila.
    pub cells: Vec<CellView>,
    pub cursor: Option<CursorView>,
    pub display_offset: usize,
    pub background: Rgb,
    pub cursor_color: Rgb,
}

impl Snapshot {
    pub fn row(&self, row: usize) -> &[CellView] {
        &self.cells[row * self.cols..(row + 1) * self.cols]
    }

    /// Texto de la fila sin espacios finales (útil para tests y diagnóstico).
    pub fn row_text(&self, row: usize) -> String {
        let text: String = self
            .row(row)
            .iter()
            .filter(|c| !c.spacer)
            .map(|c| c.ch)
            .collect();
        text.trim_end().to_string()
    }

    pub fn text(&self) -> String {
        (0..self.rows)
            .map(|r| self.row_text(r))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Copia lo visible de `term`. `focused` decide si el cursor de bloque va lleno o hueco.
pub fn snapshot<T: EventListener>(term: &Term<T>, palette: &Palette, focused: bool) -> Snapshot {
    let cols = term.columns();
    let rows = term.screen_lines();
    let RenderableContent {
        display_iter,
        selection,
        cursor,
        display_offset,
        colors,
        ..
    } = term.renderable_content();
    let offset = display_offset as i32;

    let blank = CellView {
        ch: ' ',
        fg: palette.foreground,
        bg: palette.background,
        style: CellStyle::default(),
        wide: false,
        spacer: false,
    };
    let mut cells = vec![blank; cols * rows];

    for indexed in display_iter {
        let row = indexed.point.line.0 + offset;
        let col = indexed.point.column.0;
        if row < 0 || row as usize >= rows || col >= cols {
            continue;
        }
        let cell = indexed.cell;
        let flags = cell.flags;
        let mut fg = palette.resolve(cell.fg, colors);
        let mut bg = palette.resolve(cell.bg, colors);
        if flags.contains(Flags::DIM) {
            fg = fg.dim();
        }
        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if flags.contains(Flags::HIDDEN) {
            fg = bg;
        }
        if selection
            .as_ref()
            .is_some_and(|s| s.contains(indexed.point))
        {
            bg = palette.selection;
        }
        let spacer = flags.contains(Flags::WIDE_CHAR_SPACER);
        let blank_char = spacer || flags.contains(Flags::LEADING_WIDE_CHAR_SPACER);
        cells[row as usize * cols + col] = CellView {
            ch: if blank_char { ' ' } else { cell.c },
            fg,
            bg,
            style: CellStyle {
                bold: flags.contains(Flags::BOLD),
                italic: flags.contains(Flags::ITALIC),
                underline: flags.intersects(Flags::ALL_UNDERLINES),
                strikeout: flags.contains(Flags::STRIKEOUT),
            },
            wide: flags.contains(Flags::WIDE_CHAR),
            spacer,
        };
    }

    let cursor_row = cursor.point.line.0 + offset;
    let cursor_col = cursor.point.column.0;
    let cursor_view = if cursor_row >= 0 && (cursor_row as usize) < rows && cursor_col < cols {
        let shape = match cursor.shape {
            VteCursorShape::Hidden => None,
            VteCursorShape::Block if focused => Some(CursorShape::Block),
            VteCursorShape::Block | VteCursorShape::HollowBlock => Some(CursorShape::HollowBlock),
            VteCursorShape::Beam => Some(CursorShape::Beam),
            VteCursorShape::Underline => Some(CursorShape::Underline),
        };
        shape.map(|shape| CursorView {
            col: cursor_col,
            row: cursor_row as usize,
            shape,
        })
    } else {
        None
    };

    if let Some(CursorView {
        col,
        row,
        shape: CursorShape::Block,
    }) = cursor_view
    {
        let cell = &mut cells[row * cols + col];
        cell.fg = palette.background;
        cell.bg = palette.cursor;
    }

    Snapshot {
        cols,
        rows,
        cells,
        cursor: cursor_view,
        display_offset,
        background: palette.background,
        cursor_color: palette.cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::term_with;
    use alacritty_terminal::grid::Scroll;

    #[test]
    fn texto_plano_en_filas() {
        let term = term_with(10, 3, b"hola\r\nmundo");
        let snap = snapshot(&term, &Palette::colmena_dark(), true);
        assert_eq!((snap.cols, snap.rows), (10, 3));
        assert_eq!(snap.row_text(0), "hola");
        assert_eq!(snap.row_text(1), "mundo");
        assert_eq!(snap.text(), "hola\nmundo\n");
    }

    #[test]
    fn colores_sgr_y_truecolor() {
        let p = Palette::colmena_dark();
        let term = term_with(10, 1, b"\x1b[31;44mA\x1b[0m\x1b[38;2;10;20;30mB");
        let snap = snapshot(&term, &p, true);
        assert_eq!(snap.row(0)[0].fg, p.ansi[1]);
        assert_eq!(snap.row(0)[0].bg, p.ansi[4]);
        assert_eq!(snap.row(0)[1].fg, Rgb::new(10, 20, 30));
        assert_eq!(snap.row(0)[1].bg, p.background);
    }

    #[test]
    fn inverso_negrita_y_subrayado() {
        let p = Palette::colmena_dark();
        let term = term_with(10, 1, b"\x1b[7mI\x1b[0m\x1b[1;4mN");
        let snap = snapshot(&term, &p, true);
        assert_eq!(snap.row(0)[0].fg, p.background);
        assert_eq!(snap.row(0)[0].bg, p.foreground);
        assert!(snap.row(0)[1].style.bold);
        assert!(snap.row(0)[1].style.underline);
        assert!(!snap.row(0)[1].style.italic);
    }

    #[test]
    fn caracteres_anchos_ocupan_dos_columnas() {
        let term = term_with(10, 1, "日x".as_bytes());
        let snap = snapshot(&term, &Palette::colmena_dark(), true);
        assert!(snap.row(0)[0].wide);
        assert!(snap.row(0)[1].spacer);
        assert_eq!(snap.row(0)[2].ch, 'x');
        assert_eq!(snap.row_text(0), "日x");
    }

    #[test]
    fn cursor_de_bloque_con_foco_invierte_la_celda() {
        let p = Palette::colmena_dark();
        let term = term_with(10, 2, b"ab");
        let snap = snapshot(&term, &p, true);
        assert_eq!(
            snap.cursor,
            Some(CursorView {
                col: 2,
                row: 0,
                shape: CursorShape::Block
            })
        );
        assert_eq!(snap.row(0)[2].bg, p.cursor);
        assert_eq!(snap.row(0)[2].fg, p.background);
        assert_eq!(snap.cursor_color, p.cursor);
    }

    #[test]
    fn cursor_sin_foco_es_hueco_y_no_invierte() {
        let p = Palette::colmena_dark();
        let term = term_with(10, 2, b"ab");
        let snap = snapshot(&term, &p, false);
        assert_eq!(snap.cursor.map(|c| c.shape), Some(CursorShape::HollowBlock));
        assert_eq!(snap.row(0)[2].bg, p.background);
    }

    #[test]
    fn cursor_oculto() {
        let term = term_with(10, 2, b"\x1b[?25l");
        assert_eq!(snapshot(&term, &Palette::colmena_dark(), true).cursor, None);
    }

    #[test]
    fn cursor_beam() {
        let term = term_with(10, 2, b"\x1b[6 q");
        let shape = snapshot(&term, &Palette::colmena_dark(), true)
            .cursor
            .map(|c| c.shape);
        assert_eq!(shape, Some(CursorShape::Beam));
    }

    #[test]
    fn scrollback_desplaza_las_filas_visibles() {
        let mut term = term_with(10, 2, b"1\r\n2\r\n3\r\n4");
        term.scroll_display(Scroll::Delta(2));
        let snap = snapshot(&term, &Palette::colmena_dark(), true);
        assert_eq!(snap.display_offset, 2);
        assert_eq!(snap.row_text(0), "1");
        assert_eq!(snap.row_text(1), "2");
        assert_eq!(snap.cursor, None, "el cursor quedó fuera de la vista");
    }
}
