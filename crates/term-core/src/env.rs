//! Variables de entorno heredadas que no deben llegar al shell de Colmena.
//!
//! Si Colmena se lanza desde otra terminal (Warp, iTerm, VS Code…) o desde una sesión de
//! Claude Code, el proceso hereda variables que identifican a ESA terminal o sesión
//! (`TERM_PROGRAM=WarpTerminal`, `CLAUDECODE=1`, …). Pasadas al shell, las apps adentro creen
//! que corren en otro lado: Claude Code, por ejemplo, ajusta su comportamiento según
//! `TERM_PROGRAM` y trata `CLAUDECODE` como sesión anidada. `tty::Options.env` de alacritty solo
//! agrega variables, así que el binario las borra del entorno del proceso al arrancar.

/// Prefijos de variables que identifican a la terminal o al agente que lanzó Colmena.
const INHERITED_PREFIXES: &[&str] = &[
    "CLAUDE_CODE_",
    "TERM_PROGRAM",
    "ITERM_",
    "KITTY_",
    "GHOSTTY_",
    "WEZTERM_",
    "VSCODE_",
    "ALACRITTY_",
    "WARP_",
];

/// Nombres exactos con el mismo papel.
const INHERITED_NAMES: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "AI_AGENT",
    "VTE_VERSION",
    "WINDOWID",
    "TERM_SESSION_ID",
    "LC_TERMINAL",
    "LC_TERMINAL_VERSION",
];

/// `true` si `name` describe a la terminal o sesión de agente desde la que se lanzó Colmena y no
/// debe heredarla el shell. Las variables del usuario (`PATH`, `HOME`, `LANG`, `SHELL`…) quedan.
pub fn is_inherited_terminal_var(name: &str) -> bool {
    INHERITED_NAMES.contains(&name) || INHERITED_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// Nombres de las variables del entorno actual que cumplen [`is_inherited_terminal_var`].
pub fn inherited_terminal_vars() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| is_inherited_terminal_var(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtra_variables_de_otras_terminales_y_de_claude_code() {
        for name in [
            "CLAUDECODE",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDE_CODE_MESSAGING_SOCKET",
            "CLAUDE_CODE_MESSAGING_TOKEN",
            "CLAUDE_PID",
            "CLAUDE_EFFORT",
            "AI_AGENT",
            "TERM_PROGRAM",
            "TERM_PROGRAM_VERSION",
            "TERM_SESSION_ID",
            "ITERM_SESSION_ID",
            "ITERM_PROFILE",
            "LC_TERMINAL",
            "LC_TERMINAL_VERSION",
            "KITTY_WINDOW_ID",
            "GHOSTTY_RESOURCES_DIR",
            "WEZTERM_PANE",
            "VSCODE_GIT_IPC_HANDLE",
            "ALACRITTY_WINDOW_ID",
            "WARP_IS_LOCAL_SHELL_SESSION",
            "VTE_VERSION",
            "WINDOWID",
        ] {
            assert!(is_inherited_terminal_var(name), "{name} debería filtrarse");
        }
    }

    #[test]
    fn conserva_las_variables_del_usuario() {
        for name in [
            "PATH",
            "HOME",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "SHELL",
            "USER",
            "TERM",
            "COLORTERM",
            "TMPDIR",
            "SSH_AUTH_SOCK",
            "EDITOR",
            "CLAUDE_CONFIG_DIR",
            "ANTHROPIC_API_KEY",
            "COLMENA_LOG",
        ] {
            assert!(
                !is_inherited_terminal_var(name),
                "{name} no debería filtrarse"
            );
        }
    }
}
