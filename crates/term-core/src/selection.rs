//! Selección de texto con el mouse, en coordenadas de la vista.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Term;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectKind {
    /// Click y arrastre.
    Simple,
    /// Doble click: palabra.
    Word,
    /// Triple click: línea.
    Line,
}

fn grid_point<T>(term: &Term<T>, col: usize, row: usize) -> Point {
    let offset = term.grid().display_offset() as i32;
    let col = col.min(term.columns().saturating_sub(1));
    Point {
        line: Line(row as i32 - offset),
        column: Column(col),
    }
}

fn side(right_half: bool) -> Side {
    if right_half { Side::Right } else { Side::Left }
}

pub fn start_selection<T>(
    term: &mut Term<T>,
    col: usize,
    row: usize,
    right_half: bool,
    kind: SelectKind,
) {
    let ty = match kind {
        SelectKind::Simple => SelectionType::Simple,
        SelectKind::Word => SelectionType::Semantic,
        SelectKind::Line => SelectionType::Lines,
    };
    let point = grid_point(term, col, row);
    term.selection = Some(Selection::new(ty, point, side(right_half)));
}

pub fn update_selection<T>(term: &mut Term<T>, col: usize, row: usize, right_half: bool) {
    let point = grid_point(term, col, row);
    if let Some(selection) = term.selection.as_mut() {
        selection.update(point, side(right_half));
    }
}

pub fn selection_text<T>(term: &Term<T>) -> Option<String> {
    term.selection_to_string().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Palette;
    use crate::snapshot::snapshot;
    use crate::test_util::term_with;
    use alacritty_terminal::grid::Scroll;

    #[test]
    fn seleccion_simple() {
        let mut term = term_with(20, 2, b"hola mundo");
        start_selection(&mut term, 0, 0, false, SelectKind::Simple);
        update_selection(&mut term, 3, 0, true);
        assert_eq!(selection_text(&term).as_deref(), Some("hola"));
    }

    #[test]
    fn doble_click_selecciona_la_palabra() {
        let mut term = term_with(20, 2, b"hola mundo");
        start_selection(&mut term, 7, 0, false, SelectKind::Word);
        assert_eq!(selection_text(&term).as_deref(), Some("mundo"));
    }

    #[test]
    fn triple_click_selecciona_la_linea() {
        let mut term = term_with(20, 2, b"hola mundo\r\nchau");
        start_selection(&mut term, 2, 0, false, SelectKind::Line);
        assert_eq!(
            selection_text(&term)
                .map(|s| s.trim_end().to_string())
                .as_deref(),
            Some("hola mundo")
        );
    }

    #[test]
    fn click_sin_arrastre_no_selecciona_nada() {
        let mut term = term_with(20, 2, b"hola");
        start_selection(&mut term, 1, 0, false, SelectKind::Simple);
        assert_eq!(selection_text(&term), None);
    }

    #[test]
    fn la_seleccion_se_ve_en_el_snapshot() {
        let p = Palette::colmena_dark();
        let mut term = term_with(20, 2, b"hola mundo");
        start_selection(&mut term, 0, 0, false, SelectKind::Simple);
        update_selection(&mut term, 3, 0, true);
        let snap = snapshot(&term, &p, false);
        assert!(snap.row(0)[..4].iter().all(|c| c.bg == p.selection));
        assert_eq!(snap.row(0)[5].bg, p.background);
    }

    #[test]
    fn respeta_el_scrollback() {
        let mut term = term_with(10, 2, b"uno\r\ndos\r\ntres\r\ncuatro");
        term.scroll_display(Scroll::Delta(2));
        // La fila 0 de la vista ahora es "uno".
        start_selection(&mut term, 0, 0, false, SelectKind::Simple);
        update_selection(&mut term, 2, 0, true);
        assert_eq!(selection_text(&term).as_deref(), Some("uno"));
    }
}
