//! Traducción de teclas y pegado a bytes para el PTY.
//!
//! Los nombres de tecla son los de GPUI: "enter", "tab", "backspace", "escape", "up", "down",
//! "left", "right", "home", "end", "pageup", "pagedown", "insert", "delete", "f1".."f12",
//! "space" o el carácter de la tecla ("a", "[", "5"…).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Cmd en macOS, Super/Windows en Linux/Windows.
    pub cmd: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyInput<'a> {
    pub key: &'a str,
    /// Texto que produce la tecla según el layout (`key_char` de GPUI).
    pub text: Option<&'a str>,
    pub mods: Mods,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputMode {
    /// DECCKM: las flechas mandan `ESC O x` en vez de `ESC [ x`.
    pub app_cursor: bool,
    /// La app pidió bracketed paste (`ESC [ ? 2004 h`).
    pub bracketed_paste: bool,
}

/// Bytes que hay que mandar al PTY por esta tecla, o `None` si no corresponde:
/// atajos de la app (Cmd, Ctrl+Shift+letra) o texto común, que entra por el input handler (IME).
/// `alt_is_meta`: Alt manda `ESC` + tecla (Linux/Windows). En macOS va `false` y Alt compone.
pub fn encode_key(input: &KeyInput<'_>, mode: InputMode, alt_is_meta: bool) -> Option<Vec<u8>> {
    let m = input.mods;
    if m.cmd {
        return None;
    }
    let meta = m.alt && alt_is_meta;
    let param = 1 + (m.shift as u8) + ((m.alt as u8) << 1) + ((m.ctrl as u8) << 2);
    let has_mods = param > 1;

    let cursor_key = |letter: char| -> Vec<u8> {
        if has_mods {
            format!("\x1b[1;{param}{letter}").into_bytes()
        } else if mode.app_cursor {
            format!("\x1bO{letter}").into_bytes()
        } else {
            format!("\x1b[{letter}").into_bytes()
        }
    };
    let tilde_key = |code: u8| -> Vec<u8> {
        if has_mods {
            format!("\x1b[{code};{param}~").into_bytes()
        } else {
            format!("\x1b[{code}~").into_bytes()
        }
    };
    let ss3_key = |letter: char| -> Vec<u8> {
        if has_mods {
            format!("\x1b[1;{param}{letter}").into_bytes()
        } else {
            format!("\x1bO{letter}").into_bytes()
        }
    };
    let with_meta = |bytes: &[u8]| -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len() + 1);
        if meta {
            out.push(0x1b);
        }
        out.extend_from_slice(bytes);
        out
    };

    let bytes = match input.key {
        "enter" if m.shift || m.alt => b"\x1b\r".to_vec(),
        "enter" => b"\r".to_vec(),
        "tab" if m.shift => b"\x1b[Z".to_vec(),
        "tab" => with_meta(b"\t"),
        "backspace" if m.ctrl => vec![0x08],
        "backspace" => with_meta(&[0x7f]),
        "escape" => vec![0x1b],
        "up" => cursor_key('A'),
        "down" => cursor_key('B'),
        "right" => cursor_key('C'),
        "left" => cursor_key('D'),
        "home" => cursor_key('H'),
        "end" => cursor_key('F'),
        "insert" => tilde_key(2),
        "delete" => tilde_key(3),
        "pageup" => tilde_key(5),
        "pagedown" => tilde_key(6),
        "f1" => ss3_key('P'),
        "f2" => ss3_key('Q'),
        "f3" => ss3_key('R'),
        "f4" => ss3_key('S'),
        "f5" => tilde_key(15),
        "f6" => tilde_key(17),
        "f7" => tilde_key(18),
        "f8" => tilde_key(19),
        "f9" => tilde_key(20),
        "f10" => tilde_key(21),
        "f11" => tilde_key(23),
        "f12" => tilde_key(24),
        _ => return encode_char(input, meta),
    };
    Some(bytes)
}

fn encode_char(input: &KeyInput<'_>, meta: bool) -> Option<Vec<u8>> {
    let m = input.mods;
    let single_letter = input.key.len() == 1 && input.key.as_bytes()[0].is_ascii_alphabetic();
    if m.ctrl {
        // Ctrl+Shift+letra queda para atajos de la app (copiar/pegar en Linux/Windows).
        if m.shift && single_letter {
            return None;
        }
        let byte = ctrl_byte(input.key)?;
        let mut out = Vec::with_capacity(2);
        if meta {
            out.push(0x1b);
        }
        out.push(byte);
        return Some(out);
    }
    if meta {
        let text = if input.key == "space" {
            " "
        } else {
            input.text.unwrap_or(input.key)
        };
        if text.chars().count() != 1 {
            return None;
        }
        let mut out = vec![0x1b];
        out.extend_from_slice(text.as_bytes());
        return Some(out);
    }
    None
}

fn ctrl_byte(key: &str) -> Option<u8> {
    let byte = match key {
        "space" | "2" | "@" => 0x00,
        "[" | "3" => 0x1b,
        "\\" | "4" => 0x1c,
        "]" | "5" => 0x1d,
        "^" | "6" => 0x1e,
        "_" | "-" | "/" | "7" => 0x1f,
        "8" | "?" => 0x7f,
        k if k.len() == 1 && k.as_bytes()[0].is_ascii_alphabetic() => {
            k.as_bytes()[0].to_ascii_lowercase() & 0x1f
        }
        _ => return None,
    };
    Some(byte)
}

/// Texto pegado → bytes. Normaliza saltos de línea a `\r` y, con bracketed paste,
/// lo envuelve en `ESC[200~ … ESC[201~` quitando cualquier ESC del texto (evita inyección).
pub fn encode_paste(text: &str, mode: InputMode) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    if !mode.bracketed_paste {
        return normalized.into_bytes();
    }
    let clean: String = normalized.chars().filter(|&c| c != '\x1b').collect();
    let mut out = b"\x1b[200~".to_vec();
    out.extend_from_slice(clean.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: &str) -> KeyInput<'_> {
        KeyInput {
            key,
            text: None,
            mods: Mods::default(),
        }
    }

    fn with(key: &str, mods: Mods) -> KeyInput<'_> {
        KeyInput {
            key,
            text: None,
            mods,
        }
    }

    const CTRL: Mods = Mods {
        ctrl: true,
        alt: false,
        shift: false,
        cmd: false,
    };
    const ALT: Mods = Mods {
        ctrl: false,
        alt: true,
        shift: false,
        cmd: false,
    };
    const SHIFT: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: true,
        cmd: false,
    };
    const CMD: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: true,
    };
    const NORMAL: InputMode = InputMode {
        app_cursor: false,
        bracketed_paste: false,
    };
    const APP: InputMode = InputMode {
        app_cursor: true,
        bracketed_paste: false,
    };

    fn enc(input: KeyInput<'_>) -> Option<Vec<u8>> {
        encode_key(&input, NORMAL, true)
    }

    #[test]
    fn enter_y_shift_enter() {
        assert_eq!(enc(key("enter")), Some(b"\r".to_vec()));
        assert_eq!(enc(with("enter", SHIFT)), Some(b"\x1b\r".to_vec()));
        assert_eq!(
            encode_key(&with("enter", ALT), NORMAL, false),
            Some(b"\x1b\r".to_vec())
        );
    }

    #[test]
    fn teclas_de_edicion() {
        assert_eq!(enc(key("backspace")), Some(vec![0x7f]));
        assert_eq!(enc(with("backspace", CTRL)), Some(vec![0x08]));
        assert_eq!(enc(with("backspace", ALT)), Some(b"\x1b\x7f".to_vec()));
        assert_eq!(enc(key("tab")), Some(b"\t".to_vec()));
        assert_eq!(enc(with("tab", SHIFT)), Some(b"\x1b[Z".to_vec()));
        assert_eq!(enc(key("escape")), Some(b"\x1b".to_vec()));
        assert_eq!(enc(key("delete")), Some(b"\x1b[3~".to_vec()));
        assert_eq!(enc(key("pageup")), Some(b"\x1b[5~".to_vec()));
    }

    #[test]
    fn flechas_en_modo_normal_y_aplicacion() {
        assert_eq!(enc(key("up")), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode_key(&key("up"), APP, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(enc(key("left")), Some(b"\x1b[D".to_vec()));
        assert_eq!(enc(key("home")), Some(b"\x1b[H".to_vec()));
        assert_eq!(encode_key(&key("end"), APP, true), Some(b"\x1bOF".to_vec()));
    }

    #[test]
    fn flechas_con_modificadores() {
        assert_eq!(enc(with("right", CTRL)), Some(b"\x1b[1;5C".to_vec()));
        assert_eq!(enc(with("up", SHIFT)), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(
            encode_key(&with("up", CTRL), APP, true),
            Some(b"\x1b[1;5A".to_vec())
        );
        assert_eq!(enc(with("delete", CTRL)), Some(b"\x1b[3;5~".to_vec()));
    }

    #[test]
    fn teclas_de_funcion() {
        assert_eq!(enc(key("f1")), Some(b"\x1bOP".to_vec()));
        assert_eq!(enc(key("f5")), Some(b"\x1b[15~".to_vec()));
        assert_eq!(enc(key("f12")), Some(b"\x1b[24~".to_vec()));
        assert_eq!(enc(with("f1", SHIFT)), Some(b"\x1b[1;2P".to_vec()));
    }

    #[test]
    fn ctrl_mas_letra_y_simbolos() {
        assert_eq!(enc(with("c", CTRL)), Some(vec![0x03]));
        assert_eq!(enc(with("a", CTRL)), Some(vec![0x01]));
        assert_eq!(enc(with("z", CTRL)), Some(vec![0x1a]));
        assert_eq!(enc(with("[", CTRL)), Some(vec![0x1b]));
        assert_eq!(enc(with("space", CTRL)), Some(vec![0x00]));
        assert_eq!(enc(with("/", CTRL)), Some(vec![0x1f]));
    }

    #[test]
    fn alt_como_meta_solo_si_se_pide() {
        let alt_b = KeyInput {
            key: "b",
            text: Some("b"),
            mods: ALT,
        };
        assert_eq!(encode_key(&alt_b, NORMAL, true), Some(b"\x1bb".to_vec()));
        // En macOS Alt compone caracteres (ß, ∫…): eso entra como texto por el input handler.
        let opt_b = KeyInput {
            key: "b",
            text: Some("∫"),
            mods: ALT,
        };
        assert_eq!(encode_key(&opt_b, NORMAL, false), None);
    }

    #[test]
    fn texto_comun_no_se_codifica_aca() {
        let a = KeyInput {
            key: "a",
            text: Some("a"),
            mods: Mods::default(),
        };
        assert_eq!(enc(a), None);
        let shift_a = KeyInput {
            key: "a",
            text: Some("A"),
            mods: SHIFT,
        };
        assert_eq!(enc(shift_a), None);
        let space = KeyInput {
            key: "space",
            text: Some(" "),
            mods: Mods::default(),
        };
        assert_eq!(enc(space), None);
    }

    #[test]
    fn atajos_de_la_app_no_llegan_al_pty() {
        assert_eq!(enc(with("c", CMD)), None);
        assert_eq!(enc(with("enter", CMD)), None);
        let ctrl_shift_c = Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        };
        assert_eq!(enc(with("c", ctrl_shift_c)), None);
    }

    #[test]
    fn pegado_normal_convierte_saltos_de_linea() {
        assert_eq!(encode_paste("a\nb\r\nc", NORMAL), b"a\rb\rc".to_vec());
    }

    #[test]
    fn pegado_con_bracketed_paste_envuelve_y_limpia_escapes() {
        let mode = InputMode {
            app_cursor: false,
            bracketed_paste: true,
        };
        assert_eq!(
            encode_paste("hola\n\x1b[201~chau", mode),
            b"\x1b[200~hola\r[201~chau\x1b[201~".to_vec()
        );
    }
}
