//! Paleta del tema y resolución de los colores de alacritty a RGB.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{self, Color, NamedColor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Oscurece el color a 2/3, como el atributo DIM de alacritty.
    pub fn dim(self) -> Self {
        let f = |c: u8| ((c as u16 * 2) / 3) as u8;
        Self::new(f(self.r), f(self.g), f(self.b))
    }
}

impl From<ansi::Rgb> for Rgb {
    fn from(c: ansi::Rgb) -> Self {
        Self::new(c.r, c.g, c.b)
    }
}

impl From<Rgb> for ansi::Rgb {
    fn from(c: Rgb) -> Self {
        ansi::Rgb {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub ansi: [Rgb; 16],
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub selection: Rgb,
}

impl Palette {
    /// Tema oscuro por defecto (`colmena-dark`). En M2 los temas salen de `config.toml`.
    pub fn colmena_dark() -> Self {
        Self {
            ansi: [
                Rgb::new(0x1d, 0x1f, 0x21),
                Rgb::new(0xe0, 0x6c, 0x75),
                Rgb::new(0x98, 0xc3, 0x79),
                Rgb::new(0xe5, 0xc0, 0x7b),
                Rgb::new(0x61, 0xaf, 0xef),
                Rgb::new(0xc6, 0x78, 0xdd),
                Rgb::new(0x56, 0xb6, 0xc2),
                Rgb::new(0xdc, 0xdf, 0xe4),
                Rgb::new(0x5c, 0x63, 0x70),
                Rgb::new(0xff, 0x7b, 0x86),
                Rgb::new(0xb5, 0xe8, 0x90),
                Rgb::new(0xff, 0xd6, 0x8a),
                Rgb::new(0x82, 0xc4, 0xff),
                Rgb::new(0xdd, 0x95, 0xf0),
                Rgb::new(0x7a, 0xd3, 0xde),
                Rgb::new(0xff, 0xff, 0xff),
            ],
            foreground: Rgb::new(0xdc, 0xdf, 0xe4),
            background: Rgb::new(0x16, 0x18, 0x1c),
            cursor: Rgb::new(0xf5, 0xa9, 0x7f),
            selection: Rgb::new(0x3a, 0x40, 0x4b),
        }
    }

    /// Paleta de 256 colores de xterm: 16 del tema, cubo 6×6×6 y 24 grises.
    pub fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                Rgb::new(level(i / 36), level((i / 6) % 6), level(i % 6))
            }
            232..=255 => {
                let v = 8 + (index - 232) * 10;
                Rgb::new(v, v, v)
            }
        }
    }

    pub fn named(&self, color: NamedColor) -> Rgb {
        match color {
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimForeground => self.foreground.dim(),
            NamedColor::DimBlack => self.ansi[0].dim(),
            NamedColor::DimRed => self.ansi[1].dim(),
            NamedColor::DimGreen => self.ansi[2].dim(),
            NamedColor::DimYellow => self.ansi[3].dim(),
            NamedColor::DimBlue => self.ansi[4].dim(),
            NamedColor::DimMagenta => self.ansi[5].dim(),
            NamedColor::DimCyan => self.ansi[6].dim(),
            NamedColor::DimWhite => self.ansi[7].dim(),
            other => self.ansi[(other as usize).min(15)],
        }
    }

    /// Índice usado por las consultas de color OSC (0..=255, 256 = fg, 257 = bg, 258 = cursor…).
    pub fn by_index(&self, index: usize) -> Rgb {
        match index {
            0..=255 => self.indexed(index as u8),
            256 | 267 => self.foreground,
            257 => self.background,
            258 => self.cursor,
            259..=266 => self.ansi[index - 259].dim(),
            _ => self.foreground.dim(),
        }
    }

    /// Resuelve el color de una celda. Los colores redefinidos por la app (OSC 4/10/11) ganan.
    pub fn resolve(&self, color: Color, overrides: &Colors) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb.into(),
            Color::Indexed(i) => overrides[i as usize]
                .map(Rgb::from)
                .unwrap_or_else(|| self.indexed(i)),
            Color::Named(n) => overrides[n as usize]
                .map(Rgb::from)
                .unwrap_or_else(|| self.named(n)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubo_y_grises_de_256_colores() {
        let p = Palette::colmena_dark();
        assert_eq!(p.indexed(16), Rgb::new(0, 0, 0));
        assert_eq!(p.indexed(196), Rgb::new(255, 0, 0));
        assert_eq!(p.indexed(231), Rgb::new(255, 255, 255));
        assert_eq!(p.indexed(232), Rgb::new(8, 8, 8));
        assert_eq!(p.indexed(255), Rgb::new(238, 238, 238));
    }

    #[test]
    fn ansi_y_colores_con_nombre() {
        let p = Palette::colmena_dark();
        assert_eq!(p.indexed(1), p.ansi[1]);
        assert_eq!(p.named(NamedColor::Foreground), p.foreground);
        assert_eq!(p.named(NamedColor::Background), p.background);
        assert_eq!(p.named(NamedColor::BrightRed), p.ansi[9]);
        assert_eq!(p.named(NamedColor::DimGreen), p.ansi[2].dim());
    }

    #[test]
    fn por_indice_de_consulta_osc() {
        let p = Palette::colmena_dark();
        assert_eq!(p.by_index(4), p.ansi[4]);
        assert_eq!(p.by_index(256), p.foreground);
        assert_eq!(p.by_index(257), p.background);
        assert_eq!(p.by_index(258), p.cursor);
    }

    #[test]
    fn los_colores_redefinidos_por_la_app_ganan() {
        let p = Palette::colmena_dark();
        let mut overrides = Colors::default();
        overrides[1] = Some(ansi::Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(p.resolve(Color::Indexed(1), &overrides), Rgb::new(1, 2, 3));
        assert_eq!(
            p.resolve(Color::Named(NamedColor::Red), &overrides),
            Rgb::new(1, 2, 3)
        );
        assert_eq!(p.resolve(Color::Indexed(2), &overrides), p.ansi[2]);
        assert_eq!(
            p.resolve(Color::Spec(ansi::Rgb { r: 9, g: 8, b: 7 }), &overrides),
            Rgb::new(9, 8, 7)
        );
    }

    #[test]
    fn dim_oscurece_a_dos_tercios() {
        assert_eq!(Rgb::new(255, 90, 0).dim(), Rgb::new(170, 60, 0));
    }
}
