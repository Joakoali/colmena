use gpui::{
    App, Application, Bounds, Focusable, KeyBinding, WindowBounds, WindowOptions, actions,
    prelude::*, px, size,
};
use term_view::{TerminalOptions, TerminalView};
use tracing_subscriber::EnvFilter;

actions!(colmena, [Quit]);

fn main() {
    // Lo primero: que el shell no herede la identidad de la terminal o de la sesión de Claude Code
    // desde la que se lanzó Colmena (`TERM_PROGRAM`, `CLAUDECODE`, …). alacritty solo puede
    // agregar variables al hijo, así que se borran del entorno del proceso.
    for name in term_core::inherited_terminal_vars() {
        // SAFETY: todavía no existe ningún otro hilo (ni el logger, ni GPUI, ni el PTY); los
        // constructores previos a `main` de GPUI solo registran clases de Objective-C. Nadie lee
        // el entorno mientras se modifica.
        unsafe { std::env::remove_var(name) };
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("COLMENA_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    Application::new().run(|cx: &mut App| {
        term_view::bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        if cfg!(target_os = "macos") {
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        }
        cx.on_window_closed(|cx| cx.quit()).detach();

        let bounds = Bounds::centered(None, size(px(960.), px(620.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TerminalView::new(TerminalOptions::default(), window, cx)),
            )
            .expect("no se pudo abrir la ventana");
        window
            .update(cx, |view, window, cx| {
                window.focus(&view.focus_handle(cx));
                cx.activate(true);
            })
            .expect("no se pudo enfocar la terminal");
    });
}
