//! Cálculos de geometría sin GPUI: cuántas celdas entran, qué celda está bajo el mouse y cómo
//! agrupar celdas en tramos con el mismo estilo para pintar.

use term_core::{CellStyle, CellView, Rgb};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellMetrics {
    pub width: f32,
    pub height: f32,
}

/// Columnas y filas que entran en `width` × `height` px con `padding` en cada borde. Mínimo 2×1.
pub fn grid_dims(width: f32, height: f32, padding: f32, cell: CellMetrics) -> (u16, u16) {
    let usable_w = (width - 2.0 * padding).max(0.0);
    let usable_h = (height - 2.0 * padding).max(0.0);
    let cols = (usable_w / cell.width).floor() as u16;
    let rows = (usable_h / cell.height).floor() as u16;
    (cols.max(2), rows.max(1))
}

/// Celda bajo (x, y) —relativos al origen del área— como (col, fila, mitad_derecha).
pub fn cell_at(
    x: f32,
    y: f32,
    padding: f32,
    cell: CellMetrics,
    cols: usize,
    rows: usize,
) -> (usize, usize, bool) {
    let fx = ((x - padding) / cell.width).max(0.0);
    let fy = ((y - padding) / cell.height).max(0.0);
    let col = (fx.floor() as usize).min(cols.saturating_sub(1));
    let row = (fy.floor() as usize).min(rows.saturating_sub(1));
    let right_half = if fx >= cols as f32 {
        true
    } else {
        fx.fract() >= 0.5
    };
    (col, row, right_half)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub col: usize,
    pub cols: usize,
    pub text: String,
    pub fg: Rgb,
    pub bg: Rgb,
    pub style: CellStyle,
    /// Un único carácter ancho (2 columnas).
    pub wide: bool,
}

/// Agrupa celdas consecutivas con el mismo estilo. Cada carácter ancho va en su propio tramo
/// de 2 columnas y las celdas espaciadoras se saltean.
pub fn row_runs(cells: &[CellView]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (col, cell) in cells.iter().enumerate() {
        if cell.spacer {
            continue;
        }
        if !cell.wide {
            if let Some(run) = runs.last_mut() {
                let same = !run.wide
                    && run.fg == cell.fg
                    && run.bg == cell.bg
                    && run.style == cell.style
                    && run.col + run.cols == col;
                if same {
                    run.text.push(cell.ch);
                    run.cols += 1;
                    continue;
                }
            }
        }
        runs.push(Run {
            col,
            cols: if cell.wide { 2 } else { 1 },
            text: cell.ch.to_string(),
            fg: cell.fg,
            bg: cell.bg,
            style: cell.style,
            wide: cell.wide,
        });
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: CellMetrics = CellMetrics {
        width: 10.0,
        height: 20.0,
    };

    fn cell(ch: char, fg: Rgb) -> CellView {
        CellView {
            ch,
            fg,
            bg: Rgb::new(0, 0, 0),
            style: CellStyle::default(),
            wide: false,
            spacer: false,
        }
    }

    #[test]
    fn dimensiones_de_la_grilla() {
        assert_eq!(grid_dims(816.0, 616.0, 8.0, CELL), (80, 30));
        assert_eq!(grid_dims(5.0, 5.0, 8.0, CELL), (2, 1), "nunca menos de 2×1");
    }

    #[test]
    fn celda_bajo_el_mouse() {
        assert_eq!(cell_at(8.0, 8.0, 8.0, CELL, 80, 30), (0, 0, false));
        assert_eq!(
            cell_at(8.0 + 16.0, 8.0 + 45.0, 8.0, CELL, 80, 30),
            (1, 2, true)
        );
        assert_eq!(
            cell_at(0.0, 0.0, 8.0, CELL, 80, 30),
            (0, 0, false),
            "padding → primera celda"
        );
        assert_eq!(
            cell_at(9999.0, 9999.0, 8.0, CELL, 80, 30),
            (79, 29, true),
            "afuera → última"
        );
    }

    #[test]
    fn agrupa_celdas_con_el_mismo_estilo() {
        let white = Rgb::new(255, 255, 255);
        let red = Rgb::new(255, 0, 0);
        let cells = [cell('a', white), cell('b', white), cell('c', red)];
        let runs = row_runs(&cells);
        assert_eq!(runs.len(), 2);
        assert_eq!(
            (runs[0].col, runs[0].cols, runs[0].text.as_str()),
            (0, 2, "ab")
        );
        assert_eq!(
            (runs[1].col, runs[1].cols, runs[1].text.as_str()),
            (2, 1, "c")
        );
    }

    #[test]
    fn los_anchos_van_solos_y_se_saltean_los_espaciadores() {
        let white = Rgb::new(255, 255, 255);
        let mut wide = cell('日', white);
        wide.wide = true;
        let mut spacer = cell(' ', white);
        spacer.spacer = true;
        let cells = [cell('a', white), wide, spacer, cell('b', white)];
        let runs = row_runs(&cells);
        let summary: Vec<_> = runs
            .iter()
            .map(|r| (r.col, r.cols, r.text.as_str(), r.wide))
            .collect();
        assert_eq!(
            summary,
            vec![(0, 1, "a", false), (1, 2, "日", true), (3, 1, "b", false)]
        );
    }
}
