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
