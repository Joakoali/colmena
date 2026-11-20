//! Conversiones entre tipos de GPUI y de term-core.

use gpui::{Hsla, Keystroke, Modifiers, Rgba};
use term_core::{KeyInput, Mods, Rgb};

/// En macOS Alt (Option) compone caracteres; en Linux/Windows actúa como Meta.
pub const ALT_IS_META: bool = !cfg!(target_os = "macos");

pub fn to_hsla(c: Rgb) -> Hsla {
    Rgba {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

pub fn mods(m: &Modifiers) -> Mods {
    Mods {
        ctrl: m.control,
        alt: m.alt,
        shift: m.shift,
        cmd: m.platform,
    }
}

pub fn key_input(ks: &Keystroke) -> KeyInput<'_> {
    KeyInput {
        key: ks.key.as_str(),
        text: ks.key_char.as_deref(),
        mods: mods(&ks.modifiers),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convierte_teclas_y_modificadores() {
        let ks = Keystroke::parse("ctrl-c").unwrap();
        let input = key_input(&ks);
        assert_eq!(input.key, "c");
        assert_eq!(
            input.mods,
            Mods {
                ctrl: true,
                ..Mods::default()
            }
        );

        let ks = Keystroke::parse("cmd-v").unwrap();
        assert!(key_input(&ks).mods.cmd);

        let ks = Keystroke::parse("shift-enter").unwrap();
        assert_eq!(key_input(&ks).key, "enter");
        assert!(key_input(&ks).mods.shift);
    }

    #[test]
    fn convierte_colores() {
        let hsla = to_hsla(Rgb::new(255, 255, 255));
        assert!((hsla.l - 1.0).abs() < 1e-3);
        assert!((hsla.a - 1.0).abs() < 1e-6);
    }
}
