//! Pinta un `Snapshot` con primitivas de GPUI.

use gpui::{
    App, BorderStyle, Bounds, Font, FontStyle, FontWeight, Pixels, Point, StrikethroughStyle,
    TextRun, UnderlineStyle, Window, fill, outline, point, px, size,
};
use term_core::{CellStyle, CursorShape, Rgb, Snapshot};

use crate::convert::to_hsla;
use crate::layout::{CellMetrics, row_runs};

/// Fuente, tamaño y geometría con que se pinta la grilla.
#[derive(Clone)]
pub(crate) struct PaintStyle {
    pub font: Font,
    pub font_size: Pixels,
    pub cell: CellMetrics,
    pub padding: f32,
    /// Color del texto en composición del IME.
    pub foreground: Rgb,
}

fn styled_font(base: &Font, style: CellStyle) -> Font {
    let mut font = base.clone();
    if style.bold {
        font.weight = FontWeight::BOLD;
    }
    if style.italic {
        font.style = FontStyle::Italic;
    }
    font
}

/// `origin` es la esquina superior izquierda del área de la terminal (antes del padding).
/// `marked` es el texto en composición del IME, que se dibuja sobre el cursor.
pub(crate) fn paint_snapshot(
    snapshot: &Snapshot,
    style: &PaintStyle,
    origin: Point<Pixels>,
    marked: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let base = point(origin.x + px(style.padding), origin.y + px(style.padding));
    let cw = px(style.cell.width);
    let ch = px(style.cell.height);

    for row in 0..snapshot.rows {
        let y = base.y + ch * row as f32;
        let runs = row_runs(snapshot.row(row));

        for run in runs.iter().filter(|r| r.bg != snapshot.background) {
            let bounds = Bounds::new(
                point(base.x + cw * run.col as f32, y),
                size(cw * run.cols as f32, ch),
            );
            window.paint_quad(fill(bounds, to_hsla(run.bg)));
        }

        for run in runs.iter().filter(|r| r.text.chars().any(|c| c != ' ')) {
            let color = to_hsla(run.fg);
            let text_run = TextRun {
                len: run.text.len(),
                font: styled_font(&style.font, run.style),
                color,
                background_color: None,
                underline: run.style.underline.then_some(UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(color),
                    wavy: false,
                }),
                strikethrough: run.style.strikeout.then_some(StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(color),
                }),
            };
            // Avance fijo: cada glifo cae en su columna aunque la fuente no sea exacta. Un racimo
            // (base + marcas de ancho cero) va solo en su tramo y se deja que el shaper ponga la
            // marca sobre la base; con avance fijo la marca se iría a la columna siguiente.
            let force_width = if run.cluster {
                None
            } else if run.wide {
                Some(cw * 2.0)
            } else {
                Some(cw)
            };
            let shaped = window.text_system().shape_line(
                run.text.clone().into(),
                style.font_size,
                &[text_run],
                force_width,
            );
            if let Err(err) = shaped.paint(point(base.x + cw * run.col as f32, y), ch, window, cx) {
                tracing::warn!("no se pudo pintar una línea: {err}");
            }
        }
    }

    let Some(cursor) = snapshot.cursor else {
        return;
    };
    let x = base.x + cw * cursor.col as f32;
    let y = base.y + ch * cursor.row as f32;
    let color = to_hsla(snapshot.cursor_color);

    if let Some(text) = marked.filter(|t| !t.is_empty()) {
        let width = cw * text.chars().count() as f32;
        window.paint_quad(fill(
            Bounds::new(point(x, y), size(width, ch)),
            to_hsla(snapshot.background),
        ));
        let run = TextRun {
            len: text.len(),
            font: style.font.clone(),
            color: to_hsla(style.foreground),
            background_color: None,
            underline: Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(color),
                wavy: false,
            }),
            strikethrough: None,
        };
        let shaped = window.text_system().shape_line(
            text.to_string().into(),
            style.font_size,
            &[run],
            Some(cw),
        );
        let _ = shaped.paint(point(x, y), ch, window, cx);
        return;
    }

    match cursor.shape {
        CursorShape::Block => {}
        CursorShape::HollowBlock => {
            window.paint_quad(outline(
                Bounds::new(point(x, y), size(cw, ch)),
                color,
                BorderStyle::Solid,
            ));
        }
        CursorShape::Beam => {
            window.paint_quad(fill(Bounds::new(point(x, y), size(px(2.0), ch)), color));
        }
        CursorShape::Underline => {
            window.paint_quad(fill(
                Bounds::new(point(x, y + ch - px(2.0)), size(cw, px(2.0))),
                color,
            ));
        }
    }
}
