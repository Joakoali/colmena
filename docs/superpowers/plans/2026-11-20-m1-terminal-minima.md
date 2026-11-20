# M1 — Terminal mínima: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Una ventana nativa (GPUI) con una terminal completamente usable —colores, teclado incl. IME y teclas muertas, pegado, scrollback, selección/copia, mouse, reapertura del shell— que compila y pasa tests en macOS, Linux y Windows.

**Architecture:** `term-core` envuelve `alacritty_terminal` (emulación VT **y** PTY multiplataforma con su `EventLoop`) y expone una API sin UI: `Session` para el proceso, `snapshot()` para leer la grilla ya resuelta a RGB, y codificadores puros de teclado/pegado/mouse. `term-view` es un `TerminalView` de GPUI que, en cada frame, ajusta el tamaño del PTY al área disponible, toma un snapshot y lo pinta con `canvas` + `shape_line(force_width)`. `colmena` es el binario que abre la ventana.

**Tech Stack:** Rust (edition 2024, stable), `gpui =0.2.2` (feature `runtime_shaders`), `alacritty_terminal =0.26.0`, `futures 0.3`, `tracing`.

**Spec:** `docs/superpowers/specs/2026-11-20-colmena-v1-design.md` (secciones 4, 6.3, 8, 9, 12 — hito M1).

**Desvío del spec (aprobar con el usuario):** el spec nombraba `portable-pty` para el PTY. Este plan usa el PTY que trae `alacritty_terminal` 0.26 (`tty::new` + `EventLoop`): soporta macOS/Linux/Windows (ConPTY), acepta variables de entorno por terminal (`tty::Options.env`) y es el mismo camino que usan Alacritty y Zed. Se elimina una dependencia y el hilo lector propio. El Task 1 actualiza el spec.

## Global Constraints

- Edition 2024, `rust-version = "1.85"`, toolchain `stable` con `rustfmt` y `clippy`.
- Versiones exactas: `gpui = "=0.2.2"` con feature `runtime_shaders` (evita necesitar Xcode completo en macOS); `alacritty_terminal = "=0.26.0"`.
- GPUI solo puede usarse en `term-view` y `colmena`. `term-core` no depende de GPUI.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace` deben pasar al cerrar cada task.
- El PTY se lanza con `TERM=xterm-256color` y `COLORTERM=truecolor` (spec 6.3).
- Scrollback por defecto: 10 000 líneas (spec 6.3).
- Al terminar el proceso: mostrar `[proceso terminó, código N]` y reabrir con Enter (spec 6.3 / 8).
- Shift+Enter envía `ESC CR` (`\x1b\r`), que Claude Code interpreta como salto de línea (spec 6.3).
- Atajos de la app: `cmd` en macOS; `ctrl+shift` en Linux/Windows (spec 7.2). En M1: copiar, pegar y (macOS) salir.
- Textos visibles al usuario en español.
- No crear remotos ni hacer push sin que el usuario lo pida.

## Estructura de archivos

```
colmena/
├── Cargo.toml                         # workspace
├── rust-toolchain.toml
├── .gitignore
├── README.md                          # cómo compilar en cada OS
├── .github/workflows/ci.yml           # macOS + Linux + Windows
├── crates/
│   ├── term-core/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs                 # re-exports
│   │   ├── src/color.rs               # Rgb, Palette, resolución de colores
│   │   ├── src/size.rs                # GridSize (implementa Dimensions)
│   │   ├── src/snapshot.rs            # Snapshot/CellView/CursorView + snapshot()
│   │   ├── src/input.rs               # encode_key, encode_paste
│   │   ├── src/mouse.rs               # encode_mouse, alternate_scroll
│   │   ├── src/modes.rs               # Modes (TermMode → modos que usa la UI)
│   │   ├── src/selection.rs           # selección sobre Term
│   │   ├── src/listener.rs            # EventListener → TermEvent (privado)
│   │   ├── src/session.rs             # Session: PTY + EventLoop
│   │   └── tests/session.rs           # tests de integración con PTY real
│   ├── term-view/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs                 # acciones, bind_keys, re-exports
│   │   ├── src/layout.rs              # cálculos puros: grilla, celda bajo el mouse, runs
│   │   ├── src/convert.rs             # Rgb→Hsla, Keystroke→KeyInput, Modifiers→Mods
│   │   ├── src/paint.rs               # pinta un Snapshot
│   │   └── src/view.rs                # TerminalView (Render, input, mouse, IME)
│   └── colmena/
│       ├── Cargo.toml
│       └── src/main.rs
└── docs/
    ├── qa/m1-checklist.md
    └── benchmarks/m1.md
```

---

### Task 1: Toolchain, workspace, CI y ventana vacía

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `README.md`, `.github/workflows/ci.yml`
- Create: `crates/term-core/Cargo.toml`, `crates/term-core/src/lib.rs`
- Create: `crates/term-view/Cargo.toml`, `crates/term-view/src/lib.rs`
- Create: `crates/colmena/Cargo.toml`, `crates/colmena/src/main.rs`
- Modify: `docs/superpowers/specs/2026-11-20-colmena-v1-design.md` (secciones 3 y 4.2: PTY)

**Interfaces:**
- Consumes: nada.
- Produces: workspace con tres crates que compilan; `cargo run -p colmena` abre una ventana.

- [ ] **Step 1: Instalar Rust (si `cargo` no existe)**

Run: `command -v cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`
Luego: `source "$HOME/.cargo/env" && rustc --version`
Expected: `rustc 1.8x.x` o superior (≥ 1.85).

En macOS además: `xcode-select -p` debe imprimir una ruta (si no, `xcode-select --install`). No hace falta Xcode completo gracias a `runtime_shaders`.

- [ ] **Step 2: Crear el workspace**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/term-core", "crates/term-view", "crates/colmena"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
alacritty_terminal = "=0.26.0"
gpui = { version = "=0.2.2", features = ["runtime_shaders"] }
futures = "0.3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
term-core = { path = "crates/term-core" }
term-view = { path = "crates/term-view" }

[profile.release]
lto = "thin"
codegen-units = 1
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`.gitignore`:
```
/target
.DS_Store
```

`crates/term-core/Cargo.toml`:
```toml
[package]
name = "term-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
alacritty_terminal.workspace = true
```

`crates/term-core/src/lib.rs`:
```rust
//! Núcleo de terminal de Colmena: PTY, emulación VT y codificación de entrada, sin UI.
```

`crates/term-view/Cargo.toml`:
```toml
[package]
name = "term-view"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
term-core.workspace = true
gpui.workspace = true
futures.workspace = true
tracing.workspace = true
```

`crates/term-view/src/lib.rs`:
```rust
//! Vista GPUI de una terminal de `term-core`.
```

`crates/colmena/Cargo.toml`:
```toml
[package]
name = "colmena"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
term-core.workspace = true
term-view.workspace = true
gpui.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

`crates/colmena/src/main.rs` (ventana provisoria; el Task 9 la reemplaza):
```rust
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x16181c))
            .text_color(rgb(0xdcdfe4))
            .child("colmena")
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.), px(620.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Empty),
        )
        .expect("no se pudo abrir la ventana");
        cx.activate(true);
    });
}
```

- [ ] **Step 3: Compilar y abrir la ventana**

Run: `cargo build --workspace`
Expected: compila (la primera vez tarda varios minutos por GPUI).

Run: `cargo run -p colmena`
Expected: se abre una ventana oscura con el texto "colmena". Cerrarla con el botón de la ventana (y Ctrl+C en la consola si el proceso sigue vivo; el Task 9 agrega salir al cerrar).

- [ ] **Step 4: CI**

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  check:
    strategy:
      fail-fast: false
      matrix:
        os: [macos-latest, ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Dependencias de sistema (Linux)
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y pkg-config clang libxkbcommon-dev libxkbcommon-x11-dev \
            libwayland-dev libx11-xcb-dev libxcb1-dev libvulkan-dev libfontconfig-dev libfreetype-dev
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
```

Si en Linux el build falla por una biblioteca de sistema faltante, el error de `pkg-config`/linker nombra el paquete: agregar el `-dev` correspondiente a esta lista.

- [ ] **Step 5: README**

`README.md`:
````markdown
# Colmena

Terminal nativa en Rust que observa sesiones de Claude Code. Diseño: `docs/superpowers/specs/2026-11-20-colmena-v1-design.md`.

## Compilar

Requisitos comunes: Rust stable (`rustup`).

- **macOS:** Xcode Command Line Tools (`xcode-select --install`). No hace falta Xcode completo.
- **Linux (Debian/Ubuntu):**
  ```sh
  sudo apt-get install -y pkg-config clang libxkbcommon-dev libxkbcommon-x11-dev \
    libwayland-dev libx11-xcb-dev libxcb1-dev libvulkan-dev libfontconfig-dev libfreetype-dev
  ```
  En tiempo de ejecución necesita un driver con Vulkan.
- **Windows:** Visual Studio Build Tools con "Desarrollo de escritorio con C++".

```sh
cargo run -p colmena          # abrir la app
cargo test --workspace        # tests
COLMENA_LOG=debug cargo run -p colmena   # más logs
```
````

- [ ] **Step 6: Actualizar el spec (desvío del PTY)**

En `docs/superpowers/specs/2026-11-20-colmena-v1-design.md`, sección 3, reemplazar la fila:
```
| PTY | **`portable-pty`** | Multiplataforma, incluye ConPTY en Windows. |
```
por:
```
| PTY | **El de `alacritty_terminal`** (`tty` + `EventLoop`) | Multiplataforma, incluye ConPTY en Windows; acepta variables de entorno por terminal. Mismo camino que Alacritty y Zed. |
```
y en la sección 4.2, fila `term-core`, columna "Depende de", reemplazar `` `portable-pty`, `alacritty_terminal`. Sin UI. `` por `` `alacritty_terminal`. Sin UI. ``.

- [ ] **Step 7: Verificar y commitear**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: todo pasa (0 tests).

```bash
git add -A
git commit -m "chore: workspace de Colmena con GPUI y CI multiplataforma"
```

---

### Task 2: term-core — colores y paleta

**Files:**
- Create: `crates/term-core/src/color.rs`
- Modify: `crates/term-core/src/lib.rs`

**Interfaces:**
- Consumes: `alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb}`, `alacritty_terminal::term::color::Colors`.
- Produces:
  - `pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }` con `Rgb::new(r, g, b)`, `fn dim(self) -> Rgb`, `From<ansi::Rgb>` y `From<Rgb> for ansi::Rgb`.
  - `pub struct Palette { pub ansi: [Rgb; 16], pub foreground: Rgb, pub background: Rgb, pub cursor: Rgb, pub selection: Rgb }` con `Palette::colmena_dark()`, `fn indexed(&self, u8) -> Rgb`, `fn named(&self, NamedColor) -> Rgb`, `fn by_index(&self, usize) -> Rgb`, `fn resolve(&self, Color, &Colors) -> Rgb`.

- [ ] **Step 1: Escribir los tests que fallan**

`crates/term-core/src/color.rs`:
```rust
//! Paleta del tema y resolución de los colores de alacritty a RGB.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{self, Color, NamedColor};

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
        assert_eq!(p.resolve(Color::Named(NamedColor::Red), &overrides), Rgb::new(1, 2, 3));
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
```

`crates/term-core/src/lib.rs`:
```rust
//! Núcleo de terminal de Colmena: PTY, emulación VT y codificación de entrada, sin UI.

pub mod color;

pub use color::{Palette, Rgb};
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-core color`
Expected: FAIL de compilación: `cannot find type Palette` / `Rgb`.

- [ ] **Step 3: Implementar**

Agregar en `crates/term-core/src/color.rs`, entre los `use` y el `mod tests`:
```rust
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
        ansi::Rgb { r: c.r, g: c.g, b: c.b }
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
```

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p term-core color`
Expected: PASS (5 tests).

- [ ] **Step 5: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-core
git commit -m "feat(term-core): paleta y resolución de colores"
```

---

### Task 3: term-core — snapshot de la grilla

**Files:**
- Create: `crates/term-core/src/size.rs`, `crates/term-core/src/snapshot.rs`
- Modify: `crates/term-core/src/lib.rs`

**Interfaces:**
- Consumes: `Palette`, `Rgb` (Task 2).
- Produces:
  - `pub struct GridSize { pub cols: u16, pub rows: u16, pub cell_width: u16, pub cell_height: u16 }`, `GridSize::new(cols, rows)`, `fn window_size(&self) -> WindowSize`; implementa `Dimensions`.
  - `pub struct CellStyle { pub bold, pub italic, pub underline, pub strikeout: bool }`
  - `pub struct CellView { pub ch: char, pub fg: Rgb, pub bg: Rgb, pub style: CellStyle, pub wide: bool, pub spacer: bool }`
  - `pub enum CursorShape { Block, HollowBlock, Beam, Underline }`, `pub struct CursorView { pub col: usize, pub row: usize, pub shape: CursorShape }`
  - `pub struct Snapshot { pub cols, pub rows: usize, pub cells: Vec<CellView>, pub cursor: Option<CursorView>, pub display_offset: usize, pub background: Rgb, pub cursor_color: Rgb }` con `fn row(&self, usize) -> &[CellView]`, `fn row_text(&self, usize) -> String`, `fn text(&self) -> String`.
  - `pub fn snapshot<T: EventListener>(term: &Term<T>, palette: &Palette, focused: bool) -> Snapshot`
  - (solo tests) `crate::test_util::term_with(cols: u16, rows: u16, bytes: &[u8]) -> Term<VoidListener>`

- [ ] **Step 1: Escribir los tests que fallan**

`crates/term-core/src/size.rs`:
```rust
//! Tamaño de la grilla en celdas y en píxeles.

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::grid::Dimensions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl GridSize {
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows, cell_width: 8, cell_height: 16 }
    }

    pub fn window_size(&self) -> WindowSize {
        WindowSize {
            num_lines: self.rows,
            num_cols: self.cols,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}
```

`crates/term-core/src/snapshot.rs` (solo imports y tests por ahora):
```rust
//! Copia del contenido visible de la terminal, con colores ya resueltos, lista para pintar.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{RenderableContent, Term};
use alacritty_terminal::vte::ansi::CursorShape as VteCursorShape;

use crate::color::{Palette, Rgb};

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
            Some(CursorView { col: 2, row: 0, shape: CursorShape::Block })
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
        let shape = snapshot(&term, &Palette::colmena_dark(), true).cursor.map(|c| c.shape);
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
```

`crates/term-core/src/lib.rs`:
```rust
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
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-core snapshot`
Expected: FAIL de compilación: `cannot find function snapshot` / tipos `CursorView`, `Snapshot`.

- [ ] **Step 3: Implementar**

Agregar en `crates/term-core/src/snapshot.rs`, entre los `use` y el `mod tests`:
```rust
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
        let text: String = self.row(row).iter().filter(|c| !c.spacer).map(|c| c.ch).collect();
        text.trim_end().to_string()
    }

    pub fn text(&self) -> String {
        (0..self.rows).map(|r| self.row_text(r)).collect::<Vec<_>>().join("\n")
    }
}

/// Copia lo visible de `term`. `focused` decide si el cursor de bloque va lleno o hueco.
pub fn snapshot<T: EventListener>(term: &Term<T>, palette: &Palette, focused: bool) -> Snapshot {
    let cols = term.columns();
    let rows = term.screen_lines();
    let RenderableContent { display_iter, selection, cursor, display_offset, colors, .. } =
        term.renderable_content();
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
        if selection.as_ref().is_some_and(|s| s.contains(indexed.point)) {
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
        shape.map(|shape| CursorView { col: cursor_col, row: cursor_row as usize, shape })
    } else {
        None
    };

    if let Some(CursorView { col, row, shape: CursorShape::Block }) = cursor_view {
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
```

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p term-core`
Expected: PASS (5 de color + 9 de snapshot).

- [ ] **Step 5: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-core
git commit -m "feat(term-core): snapshot de la grilla con colores, atributos y cursor"
```

---

### Task 4: term-core — teclado y pegado

**Files:**
- Create: `crates/term-core/src/input.rs`
- Modify: `crates/term-core/src/lib.rs`

**Interfaces:**
- Consumes: nada de tasks anteriores.
- Produces:
  - `pub struct Mods { pub ctrl, pub alt, pub shift, pub cmd: bool }` (Default)
  - `pub struct KeyInput<'a> { pub key: &'a str, pub text: Option<&'a str>, pub mods: Mods }`
  - `pub struct InputMode { pub app_cursor: bool, pub bracketed_paste: bool }` (Default)
  - `pub fn encode_key(input: &KeyInput<'_>, mode: InputMode, alt_is_meta: bool) -> Option<Vec<u8>>` — `None` = no mandar al PTY (atajo de la app o texto común que entra por el input handler/IME).
  - `pub fn encode_paste(text: &str, mode: InputMode) -> Vec<u8>`

- [ ] **Step 1: Escribir los tests que fallan**

`crates/term-core/src/input.rs`:
```rust
//! Traducción de teclas y pegado a bytes para el PTY.
//!
//! Los nombres de tecla son los de GPUI: "enter", "tab", "backspace", "escape", "up", "down",
//! "left", "right", "home", "end", "pageup", "pagedown", "insert", "delete", "f1".."f12",
//! "space" o el carácter de la tecla ("a", "[", "5"…).

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: &str) -> KeyInput<'_> {
        KeyInput { key, text: None, mods: Mods::default() }
    }

    fn with(key: &str, mods: Mods) -> KeyInput<'_> {
        KeyInput { key, text: None, mods }
    }

    const CTRL: Mods = Mods { ctrl: true, alt: false, shift: false, cmd: false };
    const ALT: Mods = Mods { ctrl: false, alt: true, shift: false, cmd: false };
    const SHIFT: Mods = Mods { ctrl: false, alt: false, shift: true, cmd: false };
    const CMD: Mods = Mods { ctrl: false, alt: false, shift: false, cmd: true };
    const NORMAL: InputMode = InputMode { app_cursor: false, bracketed_paste: false };
    const APP: InputMode = InputMode { app_cursor: true, bracketed_paste: false };

    fn enc(input: KeyInput<'_>) -> Option<Vec<u8>> {
        encode_key(&input, NORMAL, true)
    }

    #[test]
    fn enter_y_shift_enter() {
        assert_eq!(enc(key("enter")), Some(b"\r".to_vec()));
        assert_eq!(enc(with("enter", SHIFT)), Some(b"\x1b\r".to_vec()));
        assert_eq!(encode_key(&with("enter", ALT), NORMAL, false), Some(b"\x1b\r".to_vec()));
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
        assert_eq!(encode_key(&with("up", CTRL), APP, true), Some(b"\x1b[1;5A".to_vec()));
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
        let alt_b = KeyInput { key: "b", text: Some("b"), mods: ALT };
        assert_eq!(encode_key(&alt_b, NORMAL, true), Some(b"\x1bb".to_vec()));
        // En macOS Alt compone caracteres (ß, ∫…): eso entra como texto por el input handler.
        let opt_b = KeyInput { key: "b", text: Some("∫"), mods: ALT };
        assert_eq!(encode_key(&opt_b, NORMAL, false), None);
    }

    #[test]
    fn texto_comun_no_se_codifica_aca() {
        let a = KeyInput { key: "a", text: Some("a"), mods: Mods::default() };
        assert_eq!(enc(a), None);
        let shift_a = KeyInput { key: "a", text: Some("A"), mods: SHIFT };
        assert_eq!(enc(shift_a), None);
        let space = KeyInput { key: "space", text: Some(" "), mods: Mods::default() };
        assert_eq!(enc(space), None);
    }

    #[test]
    fn atajos_de_la_app_no_llegan_al_pty() {
        assert_eq!(enc(with("c", CMD)), None);
        assert_eq!(enc(with("enter", CMD)), None);
        let ctrl_shift_c = Mods { ctrl: true, shift: true, ..Mods::default() };
        assert_eq!(enc(with("c", ctrl_shift_c)), None);
    }

    #[test]
    fn pegado_normal_convierte_saltos_de_linea() {
        assert_eq!(encode_paste("a\nb\r\nc", NORMAL), b"a\rb\rc".to_vec());
    }

    #[test]
    fn pegado_con_bracketed_paste_envuelve_y_limpia_escapes() {
        let mode = InputMode { app_cursor: false, bracketed_paste: true };
        assert_eq!(
            encode_paste("hola\n\x1b[201~chau", mode),
            b"\x1b[200~hola\r[201~chau\x1b[201~".to_vec()
        );
    }
}
```

En `crates/term-core/src/lib.rs`, agregar `pub mod input;` debajo de `pub mod color;` y el re-export:
```rust
pub use input::{InputMode, KeyInput, Mods, encode_key, encode_paste};
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-core input`
Expected: FAIL de compilación: `cannot find struct KeyInput`.

- [ ] **Step 3: Implementar**

Agregar en `crates/term-core/src/input.rs`, antes del `mod tests`:
```rust
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
        let text = if input.key == "space" { " " } else { input.text.unwrap_or(input.key) };
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
```

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p term-core input`
Expected: PASS (11 tests).

- [ ] **Step 5: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-core
git commit -m "feat(term-core): codificación de teclado y pegado"
```

---

### Task 5: term-core — mouse, scroll alternativo y modos

**Files:**
- Create: `crates/term-core/src/mouse.rs`, `crates/term-core/src/modes.rs`
- Modify: `crates/term-core/src/lib.rs`

**Interfaces:**
- Consumes: `Mods`, `InputMode` (Task 4); `test_util::term_with` (Task 3).
- Produces:
  - `pub enum ReportButton { Left, Middle, Right, WheelUp, WheelDown }`
  - `pub enum MouseAction { Press, Release, Drag }`
  - `pub struct MouseMode { pub report_click, pub report_drag, pub report_motion, pub sgr: bool }` con `fn any(&self) -> bool`
  - `pub fn encode_mouse(button: ReportButton, action: MouseAction, col: usize, row: usize, mods: Mods, mode: MouseMode) -> Option<Vec<u8>>` (col/row 0-based)
  - `pub fn alternate_scroll(lines: i32, app_cursor: bool) -> Vec<u8>` (lines > 0 = hacia arriba)
  - `pub struct Modes { pub input: InputMode, pub mouse: MouseMode, pub alt_screen: bool, pub alternate_scroll: bool }` con `Modes::from_term_mode(TermMode) -> Modes`

- [ ] **Step 1: Escribir los tests que fallan**

`crates/term-core/src/mouse.rs`:
```rust
//! Reporte de mouse a la app que corre en la terminal (vim, htop, TUIs) y scroll alternativo.

use crate::input::Mods;

#[cfg(test)]
mod tests {
    use super::*;

    const CLICK_SGR: MouseMode =
        MouseMode { report_click: true, report_drag: false, report_motion: false, sgr: true };
    const DRAG_SGR: MouseMode =
        MouseMode { report_click: true, report_drag: true, report_motion: false, sgr: true };
    const CLICK_LEGACY: MouseMode =
        MouseMode { report_click: true, report_drag: false, report_motion: false, sgr: false };

    fn none() -> Mods {
        Mods::default()
    }

    #[test]
    fn sin_modo_de_mouse_no_se_reporta() {
        let off = MouseMode::default();
        assert!(!off.any());
        assert_eq!(encode_mouse(ReportButton::Left, MouseAction::Press, 0, 0, none(), off), None);
    }

    #[test]
    fn sgr_click_y_soltar() {
        let press = encode_mouse(ReportButton::Left, MouseAction::Press, 4, 9, none(), CLICK_SGR);
        assert_eq!(press, Some(b"\x1b[<0;5;10M".to_vec()));
        let release =
            encode_mouse(ReportButton::Left, MouseAction::Release, 4, 9, none(), CLICK_SGR);
        assert_eq!(release, Some(b"\x1b[<0;5;10m".to_vec()));
    }

    #[test]
    fn sgr_con_modificadores_y_rueda() {
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        let right = encode_mouse(ReportButton::Right, MouseAction::Press, 0, 0, ctrl, CLICK_SGR);
        assert_eq!(right, Some(b"\x1b[<18;1;1M".to_vec()));
        let up = encode_mouse(ReportButton::WheelUp, MouseAction::Press, 1, 1, none(), CLICK_SGR);
        assert_eq!(up, Some(b"\x1b[<64;2;2M".to_vec()));
    }

    #[test]
    fn arrastre_solo_con_modo_drag() {
        let drag = |mode| encode_mouse(ReportButton::Left, MouseAction::Drag, 2, 3, none(), mode);
        assert_eq!(drag(CLICK_SGR), None);
        assert_eq!(drag(DRAG_SGR), Some(b"\x1b[<32;3;4M".to_vec()));
    }

    #[test]
    fn formato_legacy() {
        let press =
            encode_mouse(ReportButton::Left, MouseAction::Press, 0, 0, none(), CLICK_LEGACY);
        assert_eq!(press, Some(vec![0x1b, b'[', b'M', 32, 33, 33]));
        let release =
            encode_mouse(ReportButton::Left, MouseAction::Release, 0, 0, none(), CLICK_LEGACY);
        assert_eq!(release, Some(vec![0x1b, b'[', b'M', 35, 33, 33]));
        let far =
            encode_mouse(ReportButton::Left, MouseAction::Press, 300, 0, none(), CLICK_LEGACY);
        assert_eq!(far, None, "legacy no puede codificar columnas > 223");
    }

    #[test]
    fn scroll_alternativo_manda_flechas() {
        assert_eq!(alternate_scroll(2, false), b"\x1b[A\x1b[A".to_vec());
        assert_eq!(alternate_scroll(-1, true), b"\x1bOB".to_vec());
    }
}
```

`crates/term-core/src/modes.rs`:
```rust
//! Modos de la terminal que le importan a la UI.

use alacritty_terminal::term::TermMode;

use crate::input::InputMode;
use crate::mouse::MouseMode;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::term_with;

    #[test]
    fn lee_los_modos_que_activa_la_app() {
        let term = term_with(10, 2, b"\x1b[?1h\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?1049h");
        let modes = Modes::from_term_mode(*term.mode());
        assert!(modes.input.app_cursor);
        assert!(modes.input.bracketed_paste);
        assert!(modes.mouse.report_click);
        assert!(modes.mouse.sgr);
        assert!(!modes.mouse.report_drag);
        assert!(modes.alt_screen);
    }

    #[test]
    fn terminal_recien_creada() {
        let term = term_with(10, 2, b"");
        let modes = Modes::from_term_mode(*term.mode());
        assert_eq!(modes.input, InputMode::default());
        assert!(!modes.mouse.any());
        assert!(!modes.alt_screen);
    }
}
```

En `crates/term-core/src/lib.rs` agregar los módulos y re-exports:
```rust
pub mod modes;
pub mod mouse;

pub use modes::Modes;
pub use mouse::{MouseAction, MouseMode, ReportButton, alternate_scroll, encode_mouse};
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-core mouse modes`
Expected: FAIL de compilación: `cannot find type MouseMode` / `Modes`.

(Si `cargo test` no acepta dos filtros, correr `cargo test -p term-core` completo.)

- [ ] **Step 3: Implementar**

En `crates/term-core/src/mouse.rs`, antes del `mod tests`:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    /// Movimiento con un botón apretado.
    Drag,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseMode {
    pub report_click: bool,
    pub report_drag: bool,
    pub report_motion: bool,
    pub sgr: bool,
}

impl MouseMode {
    pub fn any(&self) -> bool {
        self.report_click || self.report_drag || self.report_motion
    }
}

/// Secuencia de reporte de mouse para la app, o `None` si la app no la pidió.
/// `col` y `row` son 0-based dentro de la vista.
pub fn encode_mouse(
    button: ReportButton,
    action: MouseAction,
    col: usize,
    row: usize,
    mods: Mods,
    mode: MouseMode,
) -> Option<Vec<u8>> {
    if !mode.any() {
        return None;
    }
    if action == MouseAction::Drag && !(mode.report_drag || mode.report_motion) {
        return None;
    }
    let mut code: u32 = match button {
        ReportButton::Left => 0,
        ReportButton::Middle => 1,
        ReportButton::Right => 2,
        ReportButton::WheelUp => 64,
        ReportButton::WheelDown => 65,
    };
    if action == MouseAction::Drag {
        code += 32;
    }
    if mods.shift {
        code += 4;
    }
    if mods.alt {
        code += 8;
    }
    if mods.ctrl {
        code += 16;
    }
    let (x, y) = (col + 1, row + 1);

    if mode.sgr {
        let suffix = if action == MouseAction::Release { 'm' } else { 'M' };
        return Some(format!("\x1b[<{code};{x};{y}{suffix}").into_bytes());
    }

    let is_wheel = matches!(button, ReportButton::WheelUp | ReportButton::WheelDown);
    if action == MouseAction::Release && is_wheel {
        return None;
    }
    if x > 223 || y > 223 {
        return None;
    }
    // En el formato legacy, soltar se reporta como "botón 3" conservando los modificadores.
    let code = if action == MouseAction::Release { (code & !0b11) | 3 } else { code };
    Some(vec![0x1b, b'[', b'M', (32 + code) as u8, (32 + x) as u8, (32 + y) as u8])
}

/// Rueda en pantalla alternativa sin modo de mouse (less, man): se traduce a flechas.
/// `lines > 0` = hacia arriba.
pub fn alternate_scroll(lines: i32, app_cursor: bool) -> Vec<u8> {
    let seq: &[u8] = match (lines > 0, app_cursor) {
        (true, true) => b"\x1bOA",
        (true, false) => b"\x1b[A",
        (false, true) => b"\x1bOB",
        (false, false) => b"\x1b[B",
    };
    seq.repeat(lines.unsigned_abs() as usize)
}
```

En `crates/term-core/src/modes.rs`, antes del `mod tests`:
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    pub input: InputMode,
    pub mouse: MouseMode,
    pub alt_screen: bool,
    pub alternate_scroll: bool,
}

impl Modes {
    pub fn from_term_mode(mode: TermMode) -> Self {
        Self {
            input: InputMode {
                app_cursor: mode.contains(TermMode::APP_CURSOR),
                bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            },
            mouse: MouseMode {
                report_click: mode.contains(TermMode::MOUSE_REPORT_CLICK),
                report_drag: mode.contains(TermMode::MOUSE_DRAG),
                report_motion: mode.contains(TermMode::MOUSE_MOTION),
                sgr: mode.contains(TermMode::SGR_MOUSE),
            },
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
        }
    }
}
```

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p term-core`
Expected: PASS (todos, incluidos 6 de mouse y 2 de modos).

- [ ] **Step 5: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-core
git commit -m "feat(term-core): reporte de mouse, scroll alternativo y modos"
```

---

### Task 6: term-core — Session (PTY, eventos, escritura, resize)

**Files:**
- Create: `crates/term-core/src/listener.rs`, `crates/term-core/src/session.rs`, `crates/term-core/tests/session.rs`
- Modify: `crates/term-core/src/lib.rs`

**Interfaces:**
- Consumes: `GridSize`, `Palette`, `Snapshot`/`snapshot()`, `Modes`.
- Produces:
  - `pub enum TermEvent { Wakeup, Title(String), ResetTitle, Bell, ClipboardStore(String), Exited(Option<i32>) }`
  - `pub struct SpawnOptions { pub program: Option<(String, Vec<String>)>, pub cwd: Option<PathBuf>, pub env: HashMap<String, String>, pub size: GridSize, pub scrollback: usize, pub palette: Palette }` (Default: shell del usuario, 80×24, 10 000, `colmena_dark`)
  - `pub struct Session` con:
    - `Session::spawn(opts: SpawnOptions, on_event: impl Fn(TermEvent) + Send + Sync + 'static) -> std::io::Result<Session>`
    - `fn write(&self, bytes: impl Into<Cow<'static, [u8]>>)`
    - `fn resize(&self, size: GridSize)` (no hace nada si no cambió)
    - `fn size(&self) -> GridSize`
    - `fn snapshot(&self, palette: &Palette, focused: bool) -> Snapshot`
    - `fn modes(&self) -> Modes`
    - `fn scroll(&self, lines: i32)` (> 0 = hacia el historial), `fn scroll_to_bottom(&self)`
    - `Drop` apaga el event loop.

- [ ] **Step 1: Escribir los tests de integración que fallan**

`crates/term-core/tests/session.rs`:
```rust
use std::sync::mpsc;
use std::time::{Duration, Instant};

use term_core::{GridSize, Palette, Session, SpawnOptions, TermEvent};

const TIMEOUT: Duration = Duration::from_secs(10);

fn wait_for_text(session: &Session, needle: &str) -> String {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let text = session.snapshot(&Palette::colmena_dark(), true).text();
        if text.contains(needle) {
            return text;
        }
        assert!(Instant::now() < deadline, "no apareció {needle:?}; pantalla:\n{text}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(rx: &mpsc::Receiver<TermEvent>) -> Option<i32> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(TermEvent::Exited(code)) => return code,
            Ok(_) => {}
            Err(_) => assert!(Instant::now() < deadline, "no llegó TermEvent::Exited"),
        }
    }
}

fn program(unix: &str, windows: &str) -> Option<(String, Vec<String>)> {
    if cfg!(windows) {
        Some(("cmd.exe".into(), vec!["/C".into(), windows.into()]))
    } else {
        Some(("/bin/sh".into(), vec!["-c".into(), unix.into()]))
    }
}

#[test]
fn muestra_la_salida_usa_el_entorno_y_avisa_al_terminar() {
    let (tx, rx) = mpsc::channel();
    let mut env = std::collections::HashMap::new();
    env.insert("COLMENA_TEST".to_string(), "123".to_string());
    let opts = SpawnOptions {
        program: program("echo hola-$COLMENA_TEST", "echo hola-%COLMENA_TEST%"),
        env,
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, move |ev| {
        let _ = tx.send(ev);
    })
    .expect("spawn");

    wait_for_text(&session, "hola-123");
    let code = wait_for_exit(&rx);
    if cfg!(unix) {
        assert_eq!(code, Some(0));
    }
}

#[cfg(unix)]
#[test]
fn exporta_term_xterm_256color() {
    let opts = SpawnOptions { program: program("echo term=$TERM", ""), ..SpawnOptions::default() };
    let session = Session::spawn(opts, |_| {}).expect("spawn");
    wait_for_text(&session, "term=xterm-256color");
}

#[cfg(unix)]
#[test]
fn lo_que_se_escribe_llega_al_proceso() {
    let opts = SpawnOptions {
        program: program("read x; echo recibido-$x", ""),
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, |_| {}).expect("spawn");
    session.write(b"abc\r".to_vec());
    wait_for_text(&session, "recibido-abc");
}

#[cfg(unix)]
#[test]
fn el_resize_llega_al_proceso() {
    let opts = SpawnOptions {
        program: program("sleep 0.5; stty size; sleep 5", ""),
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, |_| {}).expect("spawn");
    session.resize(GridSize::new(100, 30));
    assert_eq!(session.size(), GridSize::new(100, 30));
    wait_for_text(&session, "30 100");
}
```

En `crates/term-core/src/lib.rs` agregar:
```rust
mod listener;
pub mod session;

pub use session::{Session, SpawnOptions, TermEvent};
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-core --test session`
Expected: FAIL de compilación: `unresolved import term_core::Session`.

- [ ] **Step 3: Implementar el listener**

`crates/term-core/src/listener.rs`:
```rust
//! Recibe los eventos de alacritty: contesta al PTY lo que es del protocolo (consultas de
//! color, tamaño, respuestas DA/DSR) y reenvía a la UI lo que le importa.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::event_loop::{EventLoopSender, Msg};

use crate::color::Palette;
use crate::session::TermEvent;
use crate::size::GridSize;

type Callback = Arc<dyn Fn(TermEvent) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Listener {
    sender: Arc<OnceLock<EventLoopSender>>,
    size: Arc<Mutex<GridSize>>,
    palette: Arc<Palette>,
    on_event: Callback,
    exit_reported: Arc<AtomicBool>,
}

impl Listener {
    pub(crate) fn new(size: Arc<Mutex<GridSize>>, palette: Palette, on_event: Callback) -> Self {
        Self {
            sender: Arc::new(OnceLock::new()),
            size,
            palette: Arc::new(palette),
            on_event,
            exit_reported: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Se llama una vez, después de crear el event loop y antes de arrancarlo.
    pub(crate) fn attach(&self, sender: EventLoopSender) {
        let _ = self.sender.set(sender);
    }

    fn reply(&self, text: String) {
        if let Some(sender) = self.sender.get() {
            let _ = sender.send(Msg::Input(text.into_bytes().into()));
        }
    }

    fn emit(&self, event: TermEvent) {
        (self.on_event)(event);
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => self.reply(text),
            Event::ColorRequest(index, format) => {
                self.reply(format(self.palette.by_index(index).into()));
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *self.size.lock().expect("size lock");
                self.reply(format(size.window_size()));
            }
            Event::Wakeup => self.emit(TermEvent::Wakeup),
            Event::Title(title) => self.emit(TermEvent::Title(title)),
            Event::ResetTitle => self.emit(TermEvent::ResetTitle),
            Event::Bell => self.emit(TermEvent::Bell),
            Event::ClipboardStore(_, text) => self.emit(TermEvent::ClipboardStore(text)),
            Event::ChildExit(status) => {
                self.exit_reported.store(true, Ordering::SeqCst);
                self.emit(TermEvent::Exited(status.code()));
            }
            Event::Exit => {
                if !self.exit_reported.swap(true, Ordering::SeqCst) {
                    self.emit(TermEvent::Exited(None));
                }
            }
            Event::MouseCursorDirty | Event::CursorBlinkingChange | Event::ClipboardLoad(..) => {}
        }
    }
}
```

- [ ] **Step 4: Implementar Session**

`crates/term-core/src/session.rs`:
```rust
//! Una terminal viva: proceso en un PTY + estado VT de alacritty.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::tty;

use crate::color::Palette;
use crate::listener::Listener;
use crate::modes::Modes;
use crate::size::GridSize;
use crate::snapshot::{Snapshot, snapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermEvent {
    /// Hay contenido nuevo para dibujar.
    Wakeup,
    Title(String),
    ResetTitle,
    Bell,
    /// La app pidió copiar al portapapeles (OSC 52).
    ClipboardStore(String),
    /// El proceso terminó. `None` si el sistema no informó el código.
    Exited(Option<i32>),
}

#[derive(Clone, Debug)]
pub struct SpawnOptions {
    /// Programa y argumentos; `None` = shell por defecto del usuario.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: Option<PathBuf>,
    /// Variables extra para el proceso (por ejemplo `COLMENA_PANE_ID` en M3).
    pub env: HashMap<String, String>,
    pub size: GridSize,
    pub scrollback: usize,
    pub palette: Palette,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            program: None,
            cwd: None,
            env: HashMap::new(),
            size: GridSize::new(80, 24),
            scrollback: 10_000,
            palette: Palette::colmena_dark(),
        }
    }
}

pub struct Session {
    term: Arc<FairMutex<Term<Listener>>>,
    sender: EventLoopSender,
    size: Arc<Mutex<GridSize>>,
}

impl Session {
    /// Lanza el proceso. `on_event` se llama desde el hilo del PTY.
    pub fn spawn(
        opts: SpawnOptions,
        on_event: impl Fn(TermEvent) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let mut env = opts.env;
        env.entry("TERM".into()).or_insert_with(|| "xterm-256color".into());
        env.entry("COLORTERM".into()).or_insert_with(|| "truecolor".into());
        let pty_options = tty::Options {
            shell: opts.program.map(|(program, args)| tty::Shell::new(program, args)),
            working_directory: opts.cwd,
            drain_on_exit: true,
            env,
            ..Default::default()
        };

        let size = Arc::new(Mutex::new(opts.size));
        let listener = Listener::new(size.clone(), opts.palette, Arc::new(on_event));
        let config = Config { scrolling_history: opts.scrollback, ..Config::default() };
        let term = Arc::new(FairMutex::new(Term::new(config, &opts.size, listener.clone())));

        let pty = tty::new(&pty_options, opts.size.window_size(), 0)?;
        let event_loop = EventLoop::new(term.clone(), listener.clone(), pty, true, false)?;
        let sender = event_loop.channel();
        listener.attach(sender.clone());
        // El hilo queda corriendo; se detiene con Msg::Shutdown en Drop.
        let _ = event_loop.spawn();

        Ok(Self { term, sender, size })
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let bytes = bytes.into();
        if !bytes.is_empty() {
            let _ = self.sender.send(Msg::Input(bytes));
        }
    }

    pub fn resize(&self, size: GridSize) {
        {
            let mut current = self.size.lock().expect("size lock");
            if *current == size {
                return;
            }
            *current = size;
        }
        self.term.lock().resize(size);
        let _ = self.sender.send(Msg::Resize(size.window_size()));
    }

    pub fn size(&self) -> GridSize {
        *self.size.lock().expect("size lock")
    }

    pub fn snapshot(&self, palette: &Palette, focused: bool) -> Snapshot {
        snapshot(&self.term.lock(), palette, focused)
    }

    pub fn modes(&self) -> Modes {
        Modes::from_term_mode(*self.term.lock().mode())
    }

    /// `lines > 0` sube hacia el historial.
    pub fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_to_bottom(&self) {
        self.term.lock().scroll_display(Scroll::Bottom);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}
```

- [ ] **Step 5: Correr los tests**

Run: `cargo test -p term-core`
Expected: PASS. En macOS/Linux corren los 4 tests de `tests/session.rs`; en Windows solo el primero.

Si `muestra_la_salida_…` falla esporádicamente porque el proceso termina antes de leer la salida, verificar que `drain_on_exit` esté en `true` tanto en `tty::Options` como en `EventLoop::new(…, true, false)`.

- [ ] **Step 6: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-core
git commit -m "feat(term-core): Session con PTY de alacritty, eventos y resize"
```

---

### Task 7: term-core — selección

**Files:**
- Create: `crates/term-core/src/selection.rs`
- Modify: `crates/term-core/src/session.rs`, `crates/term-core/src/lib.rs`

**Interfaces:**
- Consumes: `test_util::term_with`, `snapshot()`, `Session`.
- Produces:
  - `pub enum SelectKind { Simple, Word, Line }`
  - `pub fn start_selection<T>(term: &mut Term<T>, col: usize, row: usize, right_half: bool, kind: SelectKind)`
  - `pub fn update_selection<T>(term: &mut Term<T>, col: usize, row: usize, right_half: bool)`
  - `pub fn selection_text<T>(term: &Term<T>) -> Option<String>` (`None` si está vacía)
  - En `Session`: `start_selection(&self, col, row, right_half, kind)`, `update_selection(&self, col, row, right_half)`, `clear_selection(&self)`, `selection_text(&self) -> Option<String>`.
  - `col`/`row` son coordenadas de la **vista** (0-based), ya contemplando el scrollback.

- [ ] **Step 1: Escribir los tests que fallan**

`crates/term-core/src/selection.rs`:
```rust
//! Selección de texto con el mouse, en coordenadas de la vista.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Term;

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
        assert_eq!(selection_text(&term).map(|s| s.trim_end().to_string()).as_deref(), Some("hola mundo"));
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
```

En `crates/term-core/src/lib.rs` agregar:
```rust
pub mod selection;

pub use selection::SelectKind;
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-core selection`
Expected: FAIL de compilación: `cannot find function start_selection`.

- [ ] **Step 3: Implementar**

En `crates/term-core/src/selection.rs`, antes del `mod tests`:
```rust
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
    Point { line: Line(row as i32 - offset), column: Column(col) }
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
```

En `crates/term-core/src/session.rs`, agregar `use crate::selection::{self, SelectKind};` a los imports y estos métodos dentro de `impl Session`:
```rust
    pub fn start_selection(&self, col: usize, row: usize, right_half: bool, kind: SelectKind) {
        selection::start_selection(&mut self.term.lock(), col, row, right_half, kind);
    }

    pub fn update_selection(&self, col: usize, row: usize, right_half: bool) {
        selection::update_selection(&mut self.term.lock(), col, row, right_half);
    }

    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        selection::selection_text(&self.term.lock())
    }
```

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p term-core`
Expected: PASS (6 tests nuevos de selección).

Si `click_sin_arrastre_no_selecciona_nada` falla porque alacritty devuelve el carácter bajo el click: en `selection_text`, además del filtro de vacío, devolver `None` cuando `term.selection.as_ref().is_some_and(|s| s.is_empty())` — `Selection::is_empty()` existe en alacritty 0.26.

- [ ] **Step 5: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-core
git commit -m "feat(term-core): selección simple, por palabra y por línea"
```

---

### Task 8: term-view — layout y conversiones puras

**Files:**
- Create: `crates/term-view/src/layout.rs`, `crates/term-view/src/convert.rs`
- Modify: `crates/term-view/src/lib.rs`

**Interfaces:**
- Consumes: `term_core::{CellView, CellStyle, Rgb, KeyInput, Mods}`.
- Produces:
  - `pub struct CellMetrics { pub width: f32, pub height: f32 }`
  - `pub fn grid_dims(width: f32, height: f32, padding: f32, cell: CellMetrics) -> (u16, u16)` (mínimo 2×1)
  - `pub fn cell_at(x: f32, y: f32, padding: f32, cell: CellMetrics, cols: usize, rows: usize) -> (usize, usize, bool)` — `(col, fila, mitad_derecha)`, acotado a la grilla; `x`,`y` relativos al origen del área.
  - `pub struct Run { pub col: usize, pub cols: usize, pub text: String, pub fg: Rgb, pub bg: Rgb, pub style: CellStyle, pub wide: bool }`
  - `pub fn row_runs(cells: &[CellView]) -> Vec<Run>`
  - `pub fn to_hsla(c: Rgb) -> gpui::Hsla`
  - `pub fn mods(m: &gpui::Modifiers) -> Mods`
  - `pub fn key_input(ks: &gpui::Keystroke) -> KeyInput<'_>`
  - `pub const ALT_IS_META: bool` (`false` en macOS)

- [ ] **Step 1: Escribir los tests que fallan**

`crates/term-view/src/layout.rs`:
```rust
//! Cálculos de geometría sin GPUI: cuántas celdas entran, qué celda está bajo el mouse y cómo
//! agrupar celdas en tramos con el mismo estilo para pintar.

use term_core::{CellStyle, CellView, Rgb};

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: CellMetrics = CellMetrics { width: 10.0, height: 20.0 };

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
        assert_eq!(cell_at(8.0 + 16.0, 8.0 + 45.0, 8.0, CELL, 80, 30), (1, 2, true));
        assert_eq!(cell_at(0.0, 0.0, 8.0, CELL, 80, 30), (0, 0, false), "padding → primera celda");
        assert_eq!(cell_at(9999.0, 9999.0, 8.0, CELL, 80, 30), (79, 29, true), "afuera → última");
    }

    #[test]
    fn agrupa_celdas_con_el_mismo_estilo() {
        let white = Rgb::new(255, 255, 255);
        let red = Rgb::new(255, 0, 0);
        let cells = [cell('a', white), cell('b', white), cell('c', red)];
        let runs = row_runs(&cells);
        assert_eq!(runs.len(), 2);
        assert_eq!((runs[0].col, runs[0].cols, runs[0].text.as_str()), (0, 2, "ab"));
        assert_eq!((runs[1].col, runs[1].cols, runs[1].text.as_str()), (2, 1, "c"));
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
        let summary: Vec<_> = runs.iter().map(|r| (r.col, r.cols, r.text.as_str(), r.wide)).collect();
        assert_eq!(summary, vec![(0, 1, "a", false), (1, 2, "日", true), (3, 1, "b", false)]);
    }
}
```

`crates/term-view/src/convert.rs`:
```rust
//! Conversiones entre tipos de GPUI y de term-core.

use gpui::{Hsla, Keystroke, Modifiers, Rgba};
use term_core::{KeyInput, Mods, Rgb};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convierte_teclas_y_modificadores() {
        let ks = Keystroke::parse("ctrl-c").unwrap();
        let input = key_input(&ks);
        assert_eq!(input.key, "c");
        assert_eq!(input.mods, Mods { ctrl: true, ..Mods::default() });

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
```

`crates/term-view/src/lib.rs`:
```rust
//! Vista GPUI de una terminal de `term-core`.

pub mod convert;
pub mod layout;
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test -p term-view`
Expected: FAIL de compilación: `cannot find type CellMetrics` / `fn key_input`.

- [ ] **Step 3: Implementar**

En `crates/term-view/src/layout.rs`, antes del `mod tests`:
```rust
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
    let right_half = if fx >= cols as f32 { true } else { fx.fract() >= 0.5 };
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
```

En `crates/term-view/src/convert.rs`, antes del `mod tests`:
```rust
/// En macOS Alt (Option) compone caracteres; en Linux/Windows actúa como Meta.
pub const ALT_IS_META: bool = !cfg!(target_os = "macos");

pub fn to_hsla(c: Rgb) -> Hsla {
    Rgba { r: c.r as f32 / 255.0, g: c.g as f32 / 255.0, b: c.b as f32 / 255.0, a: 1.0 }.into()
}

pub fn mods(m: &Modifiers) -> Mods {
    Mods { ctrl: m.control, alt: m.alt, shift: m.shift, cmd: m.platform }
}

pub fn key_input(ks: &Keystroke) -> KeyInput<'_> {
    KeyInput { key: ks.key.as_str(), text: ks.key_char.as_deref(), mods: mods(&ks.modifiers) }
}
```

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p term-view`
Expected: PASS (6 tests).

- [ ] **Step 5: Lint y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add crates/term-view
git commit -m "feat(term-view): geometría de la grilla, tramos y conversiones"
```

---

### Task 9: term-view — TerminalView que dibuja y recibe teclado; la app la abre

**Files:**
- Create: `crates/term-view/src/paint.rs`, `crates/term-view/src/view.rs`
- Modify: `crates/term-view/src/lib.rs`, `crates/colmena/src/main.rs`

**Interfaces:**
- Consumes: todo `term-core`; `layout::*`, `convert::*` (Task 8).
- Produces:
  - `pub struct TerminalOptions { pub font_family: String, pub font_size: f32, pub line_height: f32, pub padding: f32, pub palette: Palette, pub program: Option<(String, Vec<String>)>, pub cwd: Option<PathBuf>, pub env: HashMap<String, String>, pub scrollback: usize }` (Default)
  - `pub struct TerminalView` con `TerminalView::new(options, window, cx)`; implementa `Render`, `Focusable`, `EntityInputHandler`.
  - Acciones `term_view::Copy` y `term_view::Paste` (contexto `"Terminal"`) y `pub fn bind_keys(cx: &mut App)`.
  - Campos internos que usan los Tasks 10 y 11: `session: Option<Arc<Session>>`, `layout: Rc<Cell<Option<PaintedLayout>>>`, `exited: Option<Option<i32>>`, `spawn_error: Option<String>`, `title: Option<String>`.

- [ ] **Step 1: Pintado del snapshot**

`crates/term-view/src/paint.rs`:
```rust
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
            let bounds = Bounds::new(point(base.x + cw * run.col as f32, y), size(cw * run.cols as f32, ch));
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
                strikethrough: run
                    .style
                    .strikeout
                    .then_some(StrikethroughStyle { thickness: px(1.0), color: Some(color) }),
            };
            let glyph_width = if run.wide { cw * 2.0 } else { cw };
            let shaped = window.text_system().shape_line(
                run.text.clone().into(),
                style.font_size,
                &[text_run],
                Some(glyph_width),
            );
            if let Err(err) = shaped.paint(point(base.x + cw * run.col as f32, y), ch, window, cx) {
                tracing::warn!("no se pudo pintar una línea: {err}");
            }
        }
    }

    let Some(cursor) = snapshot.cursor else { return };
    let x = base.x + cw * cursor.col as f32;
    let y = base.y + ch * cursor.row as f32;
    let color = to_hsla(snapshot.cursor_color);

    if let Some(text) = marked.filter(|t| !t.is_empty()) {
        let width = cw * text.chars().count() as f32;
        window.paint_quad(fill(Bounds::new(point(x, y), size(width, ch)), to_hsla(snapshot.background)));
        let run = TextRun {
            len: text.len(),
            font: style.font.clone(),
            color: to_hsla(style.foreground),
            background_color: None,
            underline: Some(UnderlineStyle { thickness: px(1.0), color: Some(color), wavy: false }),
            strikethrough: None,
        };
        let shaped = window.text_system().shape_line(text.to_string().into(), style.font_size, &[run], Some(cw));
        let _ = shaped.paint(point(x, y), ch, window, cx);
        return;
    }

    match cursor.shape {
        CursorShape::Block => {}
        CursorShape::HollowBlock => {
            window.paint_quad(outline(Bounds::new(point(x, y), size(cw, ch)), color, BorderStyle::Solid));
        }
        CursorShape::Beam => {
            window.paint_quad(fill(Bounds::new(point(x, y), size(px(2.0), ch)), color));
        }
        CursorShape::Underline => {
            window.paint_quad(fill(Bounds::new(point(x, y + ch - px(2.0)), size(cw, px(2.0))), color));
        }
    }
}
```

- [ ] **Step 2: La vista**

`crates/term-view/src/view.rs`:
```rust
//! `TerminalView`: una terminal de term-core dentro de GPUI.

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    App, Bounds, ClipboardItem, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    Focusable, Font, KeyDownEvent, Pixels, Point, Task, UTF16Selection, Window, canvas, div,
    font, point, prelude::*, px, size,
};
use term_core::{GridSize, Palette, Session, SpawnOptions, TermEvent, encode_key, encode_paste};

use crate::convert::{ALT_IS_META, key_input, to_hsla};
use crate::layout::{CellMetrics, cell_at, grid_dims};
use crate::paint::{PaintStyle, paint_snapshot};
// Con alias para no confundir la acción `Copy` con el trait `std::marker::Copy`.
use crate::{Copy as CopyAction, Paste as PasteAction};

#[derive(Clone, Debug)]
pub struct TerminalOptions {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub padding: f32,
    pub palette: Palette,
    /// `None` = shell del usuario.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub scrollback: usize,
}

/// Monoespaciada que viene con cada sistema. En M2 la elige `config.toml`.
fn default_font_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    }
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            font_family: default_font_family().to_string(),
            font_size: 14.0,
            line_height: 1.2,
            padding: 8.0,
            palette: Palette::colmena_dark(),
            program: None,
            cwd: None,
            env: HashMap::new(),
            scrollback: 10_000,
        }
    }
}

/// Geometría del último frame pintado, para traducir posiciones del mouse a celdas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaintedLayout {
    pub origin: Point<Pixels>,
    pub cell: CellMetrics,
    pub padding: f32,
    pub cols: usize,
    pub rows: usize,
    pub cursor: Option<(usize, usize)>,
}

impl PaintedLayout {
    pub(crate) fn cell_at(&self, position: Point<Pixels>) -> (usize, usize, bool) {
        let x = f32::from(position.x - self.origin.x);
        let y = f32::from(position.y - self.origin.y);
        cell_at(x, y, self.padding, self.cell, self.cols, self.rows)
    }
}

pub struct TerminalView {
    pub(crate) options: TerminalOptions,
    pub(crate) session: Option<Arc<Session>>,
    pub(crate) focus_handle: FocusHandle,
    font: Font,
    metrics: Option<CellMetrics>,
    pub(crate) layout: Rc<Cell<Option<PaintedLayout>>>,
    pub(crate) title: Option<String>,
    applied_title: Option<String>,
    pub(crate) exited: Option<Option<i32>>,
    pub(crate) spawn_error: Option<String>,
    marked_text: Option<String>,
    pub(crate) selecting: bool,
    pub(crate) scroll_accum: f32,
    _events: Option<Task<()>>,
}

impl TerminalView {
    pub fn new(options: TerminalOptions, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let font = font(options.font_family.clone());
        let mut view = Self {
            options,
            session: None,
            focus_handle: cx.focus_handle(),
            font,
            metrics: None,
            layout: Rc::new(Cell::new(None)),
            title: None,
            applied_title: None,
            exited: None,
            spawn_error: None,
            marked_text: None,
            selecting: false,
            scroll_accum: 0.0,
            _events: None,
        };
        view.start_session(cx);
        view
    }

    /// Lanza (o relanza) el proceso y la tarea que escucha sus eventos.
    pub(crate) fn start_session(&mut self, cx: &mut Context<Self>) {
        let (tx, mut rx) = mpsc::unbounded::<TermEvent>();
        let tx = Mutex::new(tx);
        let size = self.session.as_ref().map(|s| s.size()).unwrap_or(GridSize::new(80, 24));
        let spawn = SpawnOptions {
            program: self.options.program.clone(),
            cwd: self.options.cwd.clone(),
            env: self.options.env.clone(),
            size,
            scrollback: self.options.scrollback,
            palette: self.options.palette.clone(),
        };
        let on_event = move |event: TermEvent| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.unbounded_send(event);
            }
        };
        match Session::spawn(spawn, on_event) {
            Ok(session) => {
                self.session = Some(Arc::new(session));
                self.exited = None;
                self.spawn_error = None;
            }
            Err(err) => {
                tracing::error!("no se pudo abrir el shell: {err}");
                self.session = None;
                self.spawn_error = Some(err.to_string());
                return;
            }
        }
        self._events = Some(cx.spawn(async move |this, cx| {
            while let Some(first) = rx.next().await {
                let mut batch = vec![first];
                while let Ok(Some(event)) = rx.try_next() {
                    batch.push(event);
                }
                let alive = this.update(cx, |view, cx| {
                    for event in batch {
                        view.handle_event(event, cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    fn handle_event(&mut self, event: TermEvent, cx: &mut Context<Self>) {
        match event {
            TermEvent::Wakeup | TermEvent::Bell => {}
            TermEvent::Title(title) => self.title = Some(title),
            TermEvent::ResetTitle => self.title = None,
            TermEvent::ClipboardStore(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            TermEvent::Exited(code) => self.exited = Some(code),
        }
    }

    fn measure(&mut self, window: &mut Window) -> CellMetrics {
        if let Some(metrics) = self.metrics {
            return metrics;
        }
        let font_size = px(self.options.font_size);
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&self.font);
        let width = text_system
            .advance(font_id, font_size, 'm')
            .map(|s| f32::from(s.width))
            .unwrap_or(self.options.font_size * 0.6);
        let metrics = CellMetrics {
            width,
            height: (self.options.font_size * self.options.line_height).round(),
        };
        self.metrics = Some(metrics);
        metrics
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let Some(session) = self.session.clone().filter(|_| self.exited.is_none()) else {
            if ks.key == "enter" {
                self.start_session(cx);
                cx.notify();
                cx.stop_propagation();
            }
            return;
        };
        if ks.modifiers.shift && !ks.modifiers.control && (ks.key == "pageup" || ks.key == "pagedown") {
            let rows = session.size().rows as i32;
            session.scroll(if ks.key == "pageup" { rows } else { -rows });
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(bytes) = encode_key(&key_input(ks), session.modes().input, ALT_IS_META) {
            session.scroll_to_bottom();
            session.clear_selection();
            session.write(bytes);
            cx.stop_propagation();
        }
    }

    fn copy(&mut self, _: &CopyAction, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.session.as_ref().and_then(|s| s.selection_text()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, _: &PasteAction, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone().filter(|_| self.exited.is_none()) else { return };
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            session.scroll_to_bottom();
            session.write(encode_paste(&text, session.modes().input));
        }
    }

    fn banner(&self) -> Option<String> {
        if let Some(err) = &self.spawn_error {
            return Some(format!("[no se pudo abrir el shell: {err}] — Enter para reintentar"));
        }
        match self.exited {
            Some(Some(code)) => Some(format!("[proceso terminó, código {code}] — Enter para reabrir")),
            Some(None) => Some("[proceso terminó] — Enter para reabrir".to_string()),
            None => None,
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.title.clone().unwrap_or_else(|| "colmena".to_string());
        if self.applied_title.as_deref() != Some(title.as_str()) {
            window.set_window_title(&title);
            self.applied_title = Some(title);
        }

        let metrics = self.measure(window);
        let focused = self.focus_handle.is_focused(window);
        let session = self.session.clone();
        let palette = self.options.palette.clone();
        let layout = self.layout.clone();
        let padding = self.options.padding;
        let style = PaintStyle {
            font: self.font.clone(),
            font_size: px(self.options.font_size),
            cell: metrics,
            padding,
            foreground: palette.foreground,
        };
        let marked = self.marked_text.clone();
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let banner = self.banner().map(|text| {
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .px_3()
                .py_2()
                .bg(to_hsla(palette.ansi[8]))
                .text_color(to_hsla(palette.foreground))
                .text_sm()
                .child(text)
        });

        div()
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .bg(to_hsla(palette.background))
            .child(
                canvas(
                    move |bounds, _window, _cx| {
                        // Ajustar el PTY al área y tomar el snapshot ya con el tamaño nuevo.
                        let (cols, rows) = grid_dims(
                            f32::from(bounds.size.width),
                            f32::from(bounds.size.height),
                            padding,
                            metrics,
                        );
                        let snapshot = session.map(|session| {
                            session.resize(GridSize {
                                cols,
                                rows,
                                cell_width: metrics.width.round() as u16,
                                cell_height: metrics.height.round() as u16,
                            });
                            session.snapshot(&palette, focused)
                        });
                        layout.set(Some(PaintedLayout {
                            origin: bounds.origin,
                            cell: metrics,
                            padding,
                            cols: cols as usize,
                            rows: rows as usize,
                            cursor: snapshot.as_ref().and_then(|s| s.cursor).map(|c| (c.col, c.row)),
                        }));
                        snapshot
                    },
                    move |bounds, snapshot, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                        if let Some(snapshot) = snapshot {
                            paint_snapshot(&snapshot, &style, bounds.origin, marked.as_deref(), window, cx);
                        }
                    },
                )
                .size_full(),
            )
            .children(banner)
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_text.as_ref().map(|t| 0..t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked_text = None;
        cx.notify();
    }

    /// Texto confirmado (tecla común, tecla muerta resuelta, IME): va directo al PTY.
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = None;
        if let Some(session) = self.session.as_ref().filter(|_| self.exited.is_none()) {
            if !text.is_empty() {
                session.scroll_to_bottom();
                session.clear_selection();
                session.write(text.as_bytes().to_vec());
            }
        }
        cx.notify();
    }

    /// Texto en composición (por ejemplo "´" antes de la vocal): solo se muestra.
    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = (!new_text.is_empty()).then(|| new_text.to_string());
        cx.notify();
    }

    /// Posición del cursor, para ubicar la ventanita del IME.
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.get()?;
        let (col, row) = layout.cursor?;
        let x = layout.origin.x + px(layout.padding + layout.cell.width * col as f32);
        let y = layout.origin.y + px(layout.padding + layout.cell.height * row as f32);
        Some(Bounds::new(point(x, y), size(px(layout.cell.width), px(layout.cell.height))))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
```

`crates/term-view/src/lib.rs`:
```rust
//! Vista GPUI de una terminal de `term-core`.

pub mod convert;
pub mod layout;
mod paint;
mod view;

use gpui::{App, KeyBinding, actions};

pub use view::{TerminalOptions, TerminalView};

actions!(terminal, [Copy, Paste]);

/// Atajos de la terminal: Cmd en macOS, Ctrl+Shift en Linux/Windows.
pub fn bind_keys(cx: &mut App) {
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-c", Copy, Some("Terminal")),
            KeyBinding::new("cmd-v", Paste, Some("Terminal")),
        ]);
    } else {
        cx.bind_keys([
            KeyBinding::new("ctrl-shift-c", Copy, Some("Terminal")),
            KeyBinding::new("ctrl-shift-v", Paste, Some("Terminal")),
        ]);
    }
}
```

- [ ] **Step 3: La app abre la terminal**

Reemplazar `crates/colmena/src/main.rs`:
```rust
use gpui::{
    App, Application, Bounds, Focusable, KeyBinding, WindowBounds, WindowOptions, actions,
    prelude::*, px, size,
};
use term_view::{TerminalOptions, TerminalView};
use tracing_subscriber::EnvFilter;

actions!(colmena, [Quit]);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("COLMENA_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
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
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
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
```

- [ ] **Step 4: Compilar y probar a mano**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: compila sin warnings. Si clippy marca `too_many_lines` o similares no activados por defecto, ignorar; solo deben fallar los lints por defecto.

Run: `cargo run -p colmena`
Verificar a mano:
1. Aparece tu shell con prompt y colores; el título de la ventana es "colmena" o el que ponga el shell.
2. `ls --color` / `ls -G` muestra colores; `printf '\e[1mnegrita\e[0m \e[4msub\e[0m\n'` se ve bien.
3. Escribir con acentos usando teclas muertas: `echo canción ñandú` aparece correcto.
4. Flechas, Ctrl+C, Ctrl+R, Tab completan como en cualquier terminal.
5. Agrandar/achicar la ventana: `tput cols; tput lines` refleja el tamaño nuevo.
6. Cmd+V (o Ctrl+Shift+V) pega; pegar varias líneas en `cat` funciona.
7. `vim` abre y se usa (modo aplicación de flechas).
8. Cmd+Q (macOS) o cerrar la ventana sale.

- [ ] **Step 5: Commit**

```bash
git add crates/term-view crates/colmena
git commit -m "feat(term-view): TerminalView con render GPU, teclado, IME y pegado"
```

---

### Task 10: term-view — mouse, scroll y selección

**Files:**
- Create: `crates/term-view/src/mouse.rs`
- Modify: `crates/term-view/src/view.rs`, `crates/term-view/src/lib.rs`

**Interfaces:**
- Consumes: `TerminalView` (campos `session`, `layout`, `selecting`, `scroll_accum`, `focus_handle`, `exited`), `PaintedLayout::cell_at`, `term_core::{encode_mouse, alternate_scroll, MouseAction, ReportButton, SelectKind}`, `convert::mods`.
- Produces: métodos `on_mouse_down`, `on_mouse_move`, `on_mouse_up`, `on_scroll` en `impl TerminalView` (archivo `mouse.rs`), registrados en `render`.

- [ ] **Step 1: Handlers de mouse**

`crates/term-view/src/mouse.rs`:
```rust
//! Mouse sobre la terminal: selección local o reporte a la app, y rueda.

use gpui::{Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, Window};
use term_core::{MouseAction, ReportButton, SelectKind, alternate_scroll, encode_mouse};

use crate::convert::mods;
use crate::view::TerminalView;

impl TerminalView {
    pub(crate) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else { return };
        if self.exited.is_some() {
            return;
        }
        let (col, row, right_half) = layout.cell_at(event.position);
        let modes = session.modes();
        // Con Shift se selecciona aunque la app haya pedido el mouse (como en xterm).
        if modes.mouse.any() && !event.modifiers.shift {
            let m = mods(&event.modifiers);
            if let Some(bytes) = encode_mouse(ReportButton::Left, MouseAction::Press, col, row, m, modes.mouse) {
                session.write(bytes);
            }
            return;
        }
        let kind = match event.click_count {
            2 => SelectKind::Word,
            n if n >= 3 => SelectKind::Line,
            _ => SelectKind::Simple,
        };
        session.start_selection(col, row, right_half, kind);
        self.selecting = true;
        cx.notify();
    }

    pub(crate) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(gpui::MouseButton::Left) {
            return;
        }
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else { return };
        let (col, row, right_half) = layout.cell_at(event.position);
        if self.selecting {
            session.update_selection(col, row, right_half);
            cx.notify();
            return;
        }
        let modes = session.modes();
        let m = mods(&event.modifiers);
        if let Some(bytes) = encode_mouse(ReportButton::Left, MouseAction::Drag, col, row, m, modes.mouse) {
            session.write(bytes);
        }
    }

    pub(crate) fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else { return };
        if self.selecting {
            self.selecting = false;
            cx.notify();
            return;
        }
        let (col, row, _) = layout.cell_at(event.position);
        let modes = session.modes();
        let m = mods(&event.modifiers);
        if let Some(bytes) = encode_mouse(ReportButton::Left, MouseAction::Release, col, row, m, modes.mouse) {
            session.write(bytes);
        }
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let (Some(session), Some(layout)) = (self.session.clone(), self.layout.get()) else { return };
        let lines = match event.delta {
            ScrollDelta::Lines(delta) => delta.y,
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / layout.cell.height,
        };
        self.scroll_accum += lines;
        let whole = self.scroll_accum.trunc() as i32;
        if whole == 0 {
            return;
        }
        self.scroll_accum -= whole as f32;

        let modes = session.modes();
        if modes.mouse.any() {
            let (col, row, _) = layout.cell_at(event.position);
            let button = if whole > 0 { ReportButton::WheelUp } else { ReportButton::WheelDown };
            let m = mods(&event.modifiers);
            for _ in 0..whole.unsigned_abs() {
                if let Some(bytes) = encode_mouse(button, MouseAction::Press, col, row, m, modes.mouse) {
                    session.write(bytes);
                }
            }
        } else if modes.alt_screen && modes.alternate_scroll {
            session.write(alternate_scroll(whole, modes.input.app_cursor));
        } else {
            session.scroll(whole);
        }
        cx.notify();
    }
}
```

En `crates/term-view/src/lib.rs` agregar `mod mouse;` junto a `mod paint;`.

- [ ] **Step 2: Registrar los handlers en `render`**

En `crates/term-view/src/view.rs`, en `render`, agregar a la cadena del `div()` raíz, después de `.on_key_down(cx.listener(Self::on_key_down))`:
```rust
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
```

- [ ] **Step 3: Compilar, lint y probar a mano**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: todo pasa.

Run: `cargo run -p colmena` y verificar:
1. `seq 1 500` y la rueda sube por el historial; escribir algo vuelve al final. **Verificar la dirección**: rueda hacia arriba debe mostrar líneas anteriores. Si va al revés, invertir el signo de `lines` en `on_scroll` y anotarlo en el commit.
2. Shift+PageUp/PageDown recorre el historial.
3. Arrastrar selecciona (fondo gris); Cmd+C / Ctrl+Shift+C copia; pegarlo en otra app da el texto exacto.
4. Doble click selecciona una palabra; triple click, la línea.
5. `less /etc/services`: la rueda avanza y retrocede (scroll alternativo).
6. `vim` con `:set mouse=a`: click mueve el cursor; con Shift+arrastre se selecciona igual.

- [ ] **Step 4: Commit**

```bash
git add crates/term-view
git commit -m "feat(term-view): mouse, rueda, selección y copia"
```

---

### Task 11: term-view — fin del proceso, reapertura y título

**Files:**
- Modify: `crates/term-view/src/view.rs`

**Interfaces:**
- Consumes: `exited`, `spawn_error`, `banner()`, `start_session()` (Task 9).
- Produces: comportamiento verificado de spec 6.3/8; sin API nueva.

El código de Task 9 ya contiene la lógica (banner, Enter reabre, título). Este task la verifica en uso real y corrige lo que falle.

- [ ] **Step 1: Probar el ciclo de vida a mano**

Run: `cargo run -p colmena` y verificar:
1. `exit` → aparece abajo `[proceso terminó, código 0] — Enter para reabrir`; la pantalla anterior queda visible.
2. Enter → shell nuevo, sin banner, con el mismo tamaño de grilla.
3. `sh -c 'exit 3'` no cierra la terminal (es un hijo); `exec sh -c 'exit 3'` sí → banner con `código 3`.
4. `printf '\e]0;mi título\a'` → el título de la ventana cambia a "mi título".
5. Al cerrar la ventana el proceso de colmena termina (verificar con `pgrep colmena` que no queda nada).

- [ ] **Step 2: Corregir lo que falle**

Correcciones esperables y cómo hacerlas:
- Si tras `exit` el banner no aparece hasta mover el mouse: en `handle_event`, el `cx.notify()` del batch ya debería redibujar; confirmar que `TermEvent::Exited` llega (agregar `tracing::debug!("evento: {event:?}")` al inicio de `handle_event` y correr con `COLMENA_LOG=debug`).
- Si Enter tras el fin no relanza: confirmar que `on_key_down` entra en la rama `else` (`exited.is_some()`); el `filter(|_| self.exited.is_none())` debe devolver `None`.

- [ ] **Step 3: Commit (si hubo cambios)**

```bash
git add crates/term-view
git commit -m "fix(term-view): ciclo de vida del proceso y título"
```

---

### Task 12: Medición, checklist de QA y cierre del M1

**Files:**
- Create: `docs/qa/m1-checklist.md`, `docs/benchmarks/m1.md`

**Interfaces:**
- Consumes: la app completa.
- Produces: evidencia de que M1 cumple el spec en macOS y números de referencia para M2.

- [ ] **Step 1: Checklist de QA**

`docs/qa/m1-checklist.md`:
```markdown
# QA manual — M1 (terminal mínima)

Marcar cada ítem por sistema operativo. macOS es obligatorio para cerrar M1; Linux y Windows se
validan en VM (spec §9) y lo que falle se anota como issue para M2.

| # | Prueba | macOS | Linux | Windows |
|---|---|---|---|---|
| 1 | Abre el shell del usuario con prompt y colores | | | |
| 2 | `TERM` es `xterm-256color` y `COLORTERM` es `truecolor` | | | |
| 3 | Script de 256 colores (`for i in {0..255}; do printf "\e[48;5;${i}m %3d \e[0m" $i; done`) | | | |
| 4 | Negrita, itálica, subrayado, tachado | | | |
| 5 | Caracteres anchos: `echo 日本語 ok` alinea "ok" | | | |
| 6 | Acentos con teclas muertas: `echo canción ñandú` | | | |
| 7 | Flechas, Home/End, Ctrl+A/E/R/C/D | | | |
| 8 | Resize: `tput cols; tput lines` acompaña la ventana | | | |
| 9 | Pegar multilínea (con y sin bracketed paste) | | | |
| 10 | Rueda y Shift+PageUp recorren el scrollback | | | |
| 11 | Selección con arrastre, doble y triple click; copiar | | | |
| 12 | `vim` (flechas en modo aplicación, `:set mouse=a`) | | | |
| 13 | `htop` / `top` se dibuja bien y responde al resize | | | |
| 14 | `less`: rueda = scroll alternativo | | | |
| 15 | **Claude Code**: `claude` abre su UI y se dibuja sin cortes | | | |
| 16 | **Claude Code**: Shift+Enter inserta salto de línea en el prompt | | | |
| 17 | **Claude Code**: pegar un bloque de varias líneas en el prompt | | | |
| 18 | `exit` → banner; Enter reabre | | | |
| 19 | Título de la ventana con `printf '\e]0;titulo\a'` | | | |
| 20 | Cerrar la ventana termina la app | | | |
```

- [ ] **Step 2: Pasar la checklist en macOS**

Run: `cargo build --release -p colmena && ./target/release/colmena`
Completar la columna macOS. Para cada ítem que falle: arreglarlo en el crate correspondiente con un test que lo reproduzca primero (si es lógica de `term-core`) y commitear por separado (`fix(...)`).

Si el ítem 16 falla (Shift+Enter no inserta salto en Claude Code): probar en la terminal `cat -v`, apretar Shift+Enter y confirmar que se ve `^[^M` (= `\x1b\r`). Si se ve eso y Claude igual no lo toma, cambiar la secuencia en `encode_key` (`"enter" if m.shift || m.alt`) por `b"\\\r".to_vec()` (barra invertida + CR, el otro método que acepta Claude Code), actualizar el test `enter_y_shift_enter` y anotar el cambio en el spec §6.3.

- [ ] **Step 3: Medir**

Preparar un archivo de ~55 MB: `seq 1 7000000 > /tmp/colmena-55mb.txt`

Con `./target/release/colmena` abierto:
1. RSS en reposo con una terminal: `ps -o rss= -p $(pgrep -n colmena)` (KB).
2. `time cat /tmp/colmena-55mb.txt` dentro de colmena. La UI no debe congelarse (la ventana responde a mover y redimensionar durante el `cat`).
3. RSS después del `cat` (scrollback lleno).
4. Como referencia, `time cat /tmp/colmena-55mb.txt` en Terminal.app.

`docs/benchmarks/m1.md` (completar con los valores medidos):
```markdown
# Benchmarks M1

Máquina: <modelo, CPU, RAM, macOS x.y> — build `--release`.

| Medida | Valor |
|---|---|
| RSS en reposo, 1 terminal | ___ MB |
| `cat` 55 MB en colmena | ___ s |
| `cat` 55 MB en Terminal.app | ___ s |
| RSS tras el `cat` (scrollback 10 000) | ___ MB |
| ¿La UI se congeló durante el `cat`? | sí / no |

Objetivo del spec (§10, medido en M2/M4): < 150 MB con 5 terminales y 2 sesiones de Claude.
```

Si la UI se congela durante el `cat`: el cuello está en pintar cada `Wakeup`. Medir con `COLMENA_LOG=debug` cuántos batches llegan por segundo y, si son cientos, limitar el redibujo a uno por frame (en la tarea de eventos, después de `rx.next().await`, esperar el próximo frame con `cx.background_executor().timer(Duration::from_millis(8)).await` antes de drenar el lote). Registrar el cambio y la medición nueva.

- [ ] **Step 4: Verificación final y commit**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: todo pasa.

```bash
git add docs/qa docs/benchmarks
git commit -m "docs: checklist de QA y benchmarks del M1"
```

- [ ] **Step 5: Reportar**

Resumir al usuario: estado de la checklist en macOS, números de `docs/benchmarks/m1.md`, ítems pendientes para Linux/Windows y si hubo desvíos (por ejemplo, la secuencia de Shift+Enter o la dirección de la rueda). Preguntar si quiere crear el repo remoto para que corra la CI.
