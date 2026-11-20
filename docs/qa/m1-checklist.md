# QA manual — M1 (terminal mínima)

Marcar cada ítem por sistema operativo. macOS es obligatorio para cerrar M1; Linux y Windows se
validan en VM (spec §9) y lo que falle se anota como issue para M2.

| # | Prueba | macOS | Linux | Windows |
|---|---|---|---|---|
| 1 | Abre el shell del usuario con prompt y colores | | | |
| 2 | `TERM` es `xterm-256color` y `COLORTERM` es `truecolor` | ✓ (auto) | | |
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
| 21 | Mantener apretada una letra (p. ej. `j` en `vim`) repite la tecla, sin popup de acentos de macOS | | | |
| 22 | Arrastrar una selección y soltar el mouse fuera de la ventana: la selección termina bien | | | |
| 23 | Option+Backspace borra una palabra en el prompt (macOS) | | n/a | n/a |
| 24 | Con la ventana sin foco, el cursor se ve hueco | | | |
| 25 | AltGr compone caracteres (`@`, `{`) en teclados que lo usan | n/a | | |
| 26 | `printf '\e]0;x\a'`, luego `exit` y Enter: el título vuelve a "colmena" | | | |

`(auto)` marca ítems verificados por script (ítem 2: se lanzó el binario release con `SHELL=<script>`
y el script registró `TERM=xterm-256color COLORTERM=truecolor` dentro del PTY); el resto lo corre
una persona a mano.
