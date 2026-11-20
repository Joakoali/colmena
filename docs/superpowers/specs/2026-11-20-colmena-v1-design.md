# Colmena v1 — Diseño

- **Fecha:** 2026-11-20
- **Estado:** aprobado en brainstorming, pendiente de revisión del spec escrito
- **Alcance de este documento:** la v1 completa. Cada hito (sección 12) tiene su propio plan de implementación.

## 1. Qué es

Colmena es una terminal de escritorio nativa, escrita en Rust, para macOS, Linux y Windows. Está pensada para trabajar con varias sesiones de **Claude Code** en paralelo y ver qué hace cada una sin entrar en ella.

Colmena **no es un agente** ni llama a ningún modelo. El agente es Claude Code, que el usuario invoca a mano dentro de una terminal de Colmena. Colmena observa esas sesiones y muestra:

- una barra izquierda con todas las terminales abiertas y un resumen del agente de cada una;
- un panel derecho con el **árbol de agentes** de la terminal enfocada: el agente principal y los subagentes que despachó, cada uno con su encargo y una línea de **contexto · modelo · tokens · tiempo**;
- notificaciones del sistema cuando una sesión necesita al usuario.

## 2. Objetivos y no-objetivos

### Objetivos de la v1

1. Terminal funcional y rápida en los tres sistemas operativos, con bajo consumo de memoria.
2. Abrir terminales nuevas desde la barra lateral y alternar entre ellas.
3. Ver, para cada sesión de Claude Code que corre en Colmena:
   - el árbol de agentes: quién despachó a quién y el estado de cada uno;
   - el encargo de cada subagente: descripción y prompt completo;
   - del agente padre: cuántos subagentes lanzó, cuántos sigue esperando y su actividad actual;
   - por agente: % de contexto, modelo, tokens acumulados y tiempo trabajando.
4. Avisos: notificación del sistema cuando una sesión espera permiso, hace una pregunta o termina su turno, y navegación rápida a esa terminal.
5. Personalización por `config.toml` con recarga en vivo: apariencia, atajos, campos de info y layout.

### No-objetivos de la v1

- Integrar un LLM o un agente propio.
- Controlar sesiones desde la UI (aprobar permisos, cancelar subagentes, mandar mensajes). Queda para la v2.
- Splits, restaurar sesiones, búsqueda en el scrollback, URLs clickeables. Quedan para la v1.1.
- Plugins o scripting, perfiles de terminal, pantalla de ajustes con UI. Quedan para la v2.
- Mostrar sesiones de Claude Code que corren fuera de Colmena. Queda para la v2.
- "Bloques" estilo Warp.

## 3. Decisiones tomadas

| Decisión | Elección | Motivo |
|---|---|---|
| UI | **GPUI** + **gpui-component** | Render por GPU. Zed usa la misma combinación (GPUI + `alacritty_terminal`) en los 3 OS. `gpui-component` trae árbol, paneles redimensionables, listas y theming. |
| Emulación VT | **`alacritty_terminal`** | Motor probado, usado por Alacritty y Zed. |
| PTY | **El de `alacritty_terminal`** (`tty` + `EventLoop`) | Multiplataforma, incluye ConPTY en Windows; acepta variables de entorno por terminal. Mismo camino que Alacritty y Zed. |
| Integración con Claude Code | **Hooks + transcripts** | Los hooks dan eventos en tiempo real y vinculan cada sesión con su terminal. Los transcripts dan el detalle: prompts, modelo, tokens y árbol. |
| IPC hook → app | Socket local con **`interprocess`** | Unix domain socket en macOS/Linux y named pipe en Windows, con la misma API. |
| Vigilancia de archivos | **`notify`**, con polling de respaldo | Lectura incremental de transcripts. |
| Notificaciones | **`notify-rust`** | Funciona en macOS, Linux (D-Bus) y Windows (toast). |
| Config | TOML con recarga en vivo | Mismo modelo que Alacritty, WezTerm y Ghostty. |

Descartados: Tauri/Electron con xterm.js (consumo de memoria del webview), fork de WezTerm (base enorme y una UI no pensada para paneles), TUI dentro de otra terminal (no es una app propia), iced + iced_term (ecosistema de terminal chico) y renderer propio sobre wgpu (habría que construir todos los widgets).

## 4. Arquitectura

### 4.1 Diagrama

```
                    ┌──────────────────── colmena (app, GPUI) ───────────────────┐
                    │  sidebar        term-view (centro)        agents-panel     │
                    │     ▲                 ▲  │ teclas              ▲           │
                    └─────┼─────────────────┼──┼─────────────────────┼───────────┘
                          │ snapshots       │  ▼                     │ snapshots
  ~/.claude/projects ─┐   │          ┌──────┴─────┐          ┌───────┴───────┐
   (.jsonl, .meta)    ├─► ingest ───►│ agent-model│          │   term-core   │◄─► shell / claude
  hooks ─► colmena-hook ┘  (eventos)  │ (estado)   │          │ PTY+alacritty │    (PTY)
         (socket local)              └────────────┘          └───────────────┘
                                         config ──► todos (hot reload)
```

### 4.2 Crates

Es un workspace de Cargo en `crates/`, con Rust edition 2024 y GPUI fijado a una versión exacta.

| Crate | Responsabilidad | Depende de |
|---|---|---|
| `term-core` | Abre el PTY, lanza el shell del usuario con `COLMENA_PANE_ID` y `COLMENA_SOCKET` en el entorno, alimenta `alacritty_terminal` y expone la grilla, el resize, la escritura y la salida del proceso. | `alacritty_terminal`. Sin UI. |
| `term-view` | Elemento GPUI que dibuja la grilla (glifos, colores, cursor, selección) y traduce teclado y mouse a bytes. | `term-core`, GPUI |
| `agent-model` | Estado puro `Pane → Sesión → árbol de Agentes`. La función `apply(&mut State, Event)` no hace IO. | Ninguna |
| `ingest` | Servidor de socket que recibe hooks y lector incremental de transcripts. Los dos producen `Event`s. | `agent-model`, `interprocess`, `notify`, `serde_json` |
| `colmena-hook` | Binario mínimo que Claude Code ejecuta en cada hook. Reenvía el payload al socket. | `serde_json`, `interprocess`. **Sin GPUI.** |
| `config` | Esquema, valores por defecto, validación con errores ubicados por línea y recarga en vivo. | `serde`, `toml`, `notify` |
| `colmena` | La app: ventana, barra lateral, panel de agentes, notificaciones, atajos e instalador de hooks. | Todos |

El uso de GPUI queda confinado a `term-view` y `colmena`, para que los cambios de API de GPUI no se propaguen al resto.

### 4.3 Hilos

- Un hilo lector por PTY.
- `ingest` corre en un executor de fondo.
- La UI corre en el hilo principal y recibe snapshots del modelo. La UI nunca espera por IO.
- Solo se redibuja cuando cambia algo; no hay loop de render constante.

## 5. Integración con Claude Code

### 5.1 Vinculación terminal ↔ sesión

1. Al arrancar, cada instancia de Colmena abre su socket:
   - macOS/Linux: `$XDG_RUNTIME_DIR/colmena/<pid>.sock`, o `<temp_dir>/colmena-<uid>/<pid>.sock` si no está definido. El directorio tiene permisos `0700`.
   - Windows: named pipe `colmena-<pid>`.
2. Cada PTY se lanza con:
   - `COLMENA_PANE_ID` = identificador único de la terminal (UUID v4);
   - `COLMENA_SOCKET` = la ruta del socket de esa instancia.
3. Claude Code hereda el entorno del shell, y sus hooks heredan el de Claude Code. `colmena-hook` lee ambas variables.
4. El hook `SessionStart` trae `session_id`, `transcript_path` y `cwd`. Con eso `ingest` asocia la sesión a la terminal y empieza a leer el transcript.

### 5.2 `colmena-hook`

Contrato **estricto**, porque Claude Code inyecta en su contexto el stdout de algunos hooks:

- **Nunca escribe en stdout ni en stderr.**
- **Siempre sale con código 0.**
- Si falta `COLMENA_PANE_ID` o `COLMENA_SOCKET`, sale de inmediato sin leer nada. Esto pasa cuando Claude corre fuera de Colmena.
- Lee stdin completo (el JSON del hook). Se conecta al socket con un **timeout de 50 ms**, escribe una línea y cierra. No espera respuesta.
- Si cualquier paso falla (socket inexistente, timeout, JSON inválido), sale en silencio.

Formato en el socket, una línea JSON por conexión:

```json
{"v":1,"pane_id":"<COLMENA_PANE_ID>","payload":{ ...JSON del hook sin modificar... }}
```

### 5.3 Hooks registrados

Todos ejecutan `colmena-hook` (ruta absoluta), con `"timeout": 5` en segundos como red de seguridad:

| Hook | Matcher | Uso |
|---|---|---|
| `SessionStart` | — | Vincular la sesión y empezar a leer el transcript. |
| `UserPromptSubmit` | — | Inicio del turno: el agente pasa a trabajando y arranca el reloj. |
| `PreToolUse` | `*` | Actividad actual. Detecta `Agent` (despacho de subagente) y `AskUserQuestion` (pregunta al usuario). |
| `PostToolUse` | `*` | Fin de una tool; sale de los estados de espera. |
| `Notification` | — | Espera de permiso. Solo las notificaciones de tipo permiso generan `WaitingPermission` (el campo que lo indica se verifica en M3); las demás se ignoran. |
| `Stop` | — | Fin del turno. |
| `SubagentStop` | — | Fin de un subagente. |
| `SessionEnd` | — | Cierre de la sesión. |

**Instalación:**

- La app detecta si los hooks están instalados y si la ruta del binario sigue vigente. Si no, muestra el banner "Instalar integración con Claude".
- Archivo: `~/.claude/settings.json`, o `$CLAUDE_CONFIG_DIR/settings.json` si esa variable está definida.
- Antes de escribir, la app muestra el diff y crea un backup `settings.json.colmena-bak-<timestamp>`.
- Las entradas se **agregan** sin tocar los hooks existentes. Las de Colmena se reconocen porque su `command` contiene `colmena-hook`, lo que hace que instalar y desinstalar sean idempotentes.
- La desinstalación quita solo esas entradas.

### 5.4 Transcripts

Estructura observada en Claude Code 2.1.x:

```
~/.claude/projects/<proyecto>/<session_id>.jsonl                  # agente principal
~/.claude/projects/<proyecto>/<session_id>/subagents/agent-<id>.jsonl
~/.claude/projects/<proyecto>/<session_id>/subagents/agent-<id>.meta.json
```

**Lectura incremental:**

- Se guarda un offset de bytes por archivo. Ante cada evento de `notify`, se lee desde el offset hasta el final del archivo y se separa por `\n`. Una línea final incompleta queda en buffer.
- Si el archivo es más chico que el offset (se truncó o se reemplazó), se relee desde 0 y se reconstruye el estado derivado de ese archivo.
- Hay polling de respaldo cada 2 s para cubrir eventos perdidos de `notify`.
- El directorio `subagents/` se vigila para detectar archivos nuevos.

**Campos que se usan.** Todo lo demás se ignora.

- Línea: `type`, `uuid`, `timestamp`, `isSidechain`, `agentId`, `version`.
- `type == "assistant"`:
  - `message.id`, `message.model`;
  - `message.usage.{input_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens}`;
  - `message.content[]` con `type == "tool_use"`: `id`, `name`, `input`.
- `type == "user"`: `message.content[]` con `type == "tool_result"`: `tool_use_id`, `is_error`.
- `type == "ai-title"`: `aiTitle`.
- `.meta.json`: `agentType`, `description`, `toolUseId`, `spawnDepth`, `model`, `finished`.

**Deduplicación de uso (verificado):** Claude Code parte una misma respuesta en varias líneas que comparten `message.id` y repiten el mismo `usage`. En una muestra real hubo 1344 líneas y 626 mensajes únicos. El `usage` se cuenta **una sola vez por `message.id`**.

### 5.5 Eventos

```rust
enum AgentKey { Main, Sub(AgentId) }

enum Event {
    SessionStarted  { pane: PaneId, session: SessionId, transcript: PathBuf, cwd: PathBuf, at: Timestamp },
    SessionEnded    { session: SessionId, at: Timestamp },
    TitleChanged    { session: SessionId, title: String },
    PromptSubmitted { session: SessionId, at: Timestamp },
    TurnStopped     { session: SessionId, at: Timestamp },
    WaitingPermission { session: SessionId, at: Timestamp },
    ToolStarted     { session: SessionId, agent: AgentKey, tool_use_id: String, tool: String, summary: String, at: Timestamp },
    ToolFinished    { session: SessionId, agent: AgentKey, tool_use_id: String, is_error: bool, at: Timestamp },
    SubagentSpawned { session: SessionId, parent: AgentKey, tool_use_id: String,
                      agent_type: String, description: String, prompt: String, at: Timestamp },
    SubagentLinked  { session: SessionId, agent_id: AgentId, tool_use_id: String, model_hint: Option<String> },
    SubagentFinished{ session: SessionId, tool_use_id: String, is_error: bool, at: Timestamp },
    Usage           { session: SessionId, agent: AgentKey, message_id: String, model: String,
                      input: u64, cache_creation: u64, cache_read: u64, output: u64, at: Timestamp },
}
```

- `ToolStarted.summary` es una descripción corta de la tool en curso, por ejemplo `Bash: cargo test` o `Edit: src/main.rs`, recortada a 80 caracteres.
- `AskUserQuestion` es un `ToolStarted` con `tool == "AskUserQuestion"`. El modelo lo interpreta como un estado de espera (sección 5.7).
- **Idempotencia:** aplicar el mismo evento dos veces deja el mismo estado. Se deduplica por `message_id` en `Usage` y por `tool_use_id` en tools y subagentes, porque el mismo hecho puede llegar por hook y por transcript.

### 5.6 Árbol de agentes

- Una `tool_use` llamada `Agent` (o `Task`, el nombre anterior) en el transcript de un agente X genera `SubagentSpawned { parent: X, tool_use_id, … }` con `subagent_type`, `description` y `prompt` de su `input`.
- Un `.meta.json` con `toolUseId` genera `SubagentLinked`, que asocia el `agentId` (y con eso su `.jsonl`) a ese nodo.
- El `tool_result` con ese `tool_use_id` en el transcript del padre, o `meta.finished == true`, genera `SubagentFinished`. Si `is_error` es verdadero, el estado es ✗.
- El anidamiento sale solo: si el `tool_use` aparece en el transcript de un subagente, el padre es `Sub(id)`.
- **Padre (resumen):** "lanzó N · esperando M", donde N es la cantidad de hijos y M la de hijos sin terminar. Se muestra además su **actividad actual**, que es el `summary` de su última tool en curso.

### 5.7 Estados

Agente principal:

| Estado | Ícono | Entra con | Sale con |
|---|---|---|---|
| Inactivo | ○ | `SessionStarted`, `TurnStopped` | `PromptSubmitted` |
| Trabajando | ● | `PromptSubmitted`, `ToolStarted`, `ToolFinished`, `Usage` | `TurnStopped`, estados de espera |
| Esperando permiso | ⚠ | `WaitingPermission` | Siguiente `ToolStarted`, `ToolFinished` o `TurnStopped` |
| Te hizo una pregunta | ? | `ToolStarted` con `AskUserQuestion` | `ToolFinished` de esa tool |
| Terminada | — | `SessionEnded` | — |

Subagente: ● trabajando, ✓ terminado, ✗ falló.

### 5.8 La línea de stats

| Campo (`config`) | Cálculo |
|---|---|
| `model` | `message.model` del último `Usage` del agente, abreviado: `claude-opus-5-5` → `opus 5.5`. La regla general es quitar el prefijo `claude-`, separar familia y versión, y unir los números de versión con punto. Si el id no sigue el patrón, se muestra completo. Antes del primer `Usage`, se usa `model_hint` del meta. |
| `ctx` | Del último `Usage` del agente: `(input + cache_creation + cache_read) / ventana_del_modelo`, en %. La ventana sale de una tabla interna que se puede sobrescribir en `[context_windows]`. Si el modelo no está en la tabla, se muestra el número absoluto (`184k`) en lugar del %. |
| `tokens` | Suma de `input + cache_creation + output` de todos los `Usage` del agente, deduplicados por `message_id`. `cache_read` se excluye porque repite el contexto en cada request. |
| `tokens_out` | Suma de `output`. |
| `cache_read` | Suma de `cache_read`. |
| `elapsed` | Principal: desde `PromptSubmitted` hasta `TurnStopped` del turno actual; mientras está inactivo, queda congelado en el último turno. Subagente: desde `SubagentSpawned` hasta `SubagentFinished`. |
| `tool` | `summary` de la tool en curso. |

- Formato de números: `1234` → `1.2k`, `1_234_567` → `1.2M`. Tiempo: `40s`, `3m`, `1h12m`.
- Al pasar el mouse sobre la línea se ve el desglose completo (input, cache_creation, cache_read y output).
- Si falta un dato, el campo muestra `—`.

### 5.9 Tolerancia a versiones

- Los tipos de línea y los campos desconocidos se ignoran.
- Una línea que no parsea se saltea y se cuenta en el log a nivel `debug`.
- El campo `version` de las líneas se guarda para diagnóstico.
- Cuando el formato cambia, se degrada un campo; nunca falla la app.

## 6. UI

### 6.1 Layout

```
┌Terminales─────┬───────────────────────────┬Agentes───────┐
│● flexora  12m │$ claude                   │flexora        │
│ 62%·opus·184k │> implementá contact-reach │62%·opus·184k  │
│ 3 subagentes  │                           │ esperando 2/4 │
│⚠ api-server   │                           │├● Task 1      │
│ espera permiso│                           ││ haiku·31%    │
│○ zsh ~/Proyec │                           │├✓ Review      │
│[+ nueva]      │                           │└● Task 2      │
└───────────────┴───────────────────────────┴───────────────┘
```

### 6.2 Barra izquierda (terminales)

- Una entrada por terminal con los campos de `sidebar.entry_fields`: estado, título, línea de stats y cantidad de subagentes.
- El título se elige en este orden: nombre puesto por el usuario (doble click para renombrar), luego `aiTitle` de la sesión, luego la carpeta actual y el proceso en primer plano. En Windows, donde el proceso en primer plano no se detecta de forma confiable, solo la carpeta.
- Cuando la terminal está en ⚠ o ?, la entrada muestra un aviso.
- **"+ nueva"** abre un shell en la carpeta actual de la terminal enfocada, o en el home si no hay ninguna.
- Cerrar una terminal pide confirmación si tiene una sesión de Claude activa o, en macOS/Linux, un proceso en primer plano distinto del shell.

### 6.3 Centro (terminal)

- Una terminal visible a la vez: la seleccionada en la barra.
- Funciones de la v1: selección con mouse, copiar y pegar, scrollback (10 000 líneas por defecto, configurable), true color, reporte de mouse, bracketed paste y `TERM=xterm-256color`.
- **Shift+Enter** envía la secuencia que Claude Code interpreta como salto de línea, para escribir prompts multilínea.
- Cuando el proceso termina, se muestra `[proceso terminó, código N]` y Enter reabre el shell.

### 6.4 Panel derecho (agentes)

- Muestra el árbol de la terminal enfocada, con nodos que se pliegan y despliegan.
- Cada nodo tiene una primera línea con estado, descripción y tipo de agente, y una segunda con la línea de stats (`agent_line.fields`).
- El nodo raíz (agente principal) muestra además "lanzó N · esperando M" y su actividad actual.
- Click en un subagente despliega el **encargo completo**: descripción, tipo y prompt, con scroll y botón de copiar.
- Si la terminal no tiene una sesión de Claude, el panel dice "Sin agente en esta terminal".
- El panel se puede ocultar o mover a la izquierda (`layout.agents_panel`).

### 6.5 Avisos y navegación

- Se dispara una notificación del sistema cuando una sesión entra en ⚠ (`waiting_permission`), en ? (`question`) o en ○ después de un turno (`turn_finished`), filtrado por `notifications.on`.
- Con `only_when_unfocused = true`, el aviso solo se envía si esa terminal no está enfocada o la ventana no tiene el foco.
- Click en la notificación enfoca la ventana y esa terminal.
- Atajo `next_waiting`: salta a la próxima terminal en ⚠ o ?; si no hay ninguna, a la próxima en ○ con turno terminado y sin ver.

## 7. Config

### 7.1 Ubicación y recarga

- macOS/Linux: `$XDG_CONFIG_HOME/colmena/config.toml` (por defecto `~/.config/colmena/config.toml`).
- Windows: `%APPDATA%\colmena\config.toml`.
- Si el archivo no existe, se usan los valores por defecto y no se crea nada.
- La recarga en vivo se dispara al guardar. Si la config nueva es inválida, se **conserva la anterior** y se muestra un banner con el error y la línea.
- Una clave desconocida produce una advertencia en el banner, no un error.

### 7.2 Esquema (valores por defecto)

```toml
[appearance]
theme = "colmena-dark"        # "colmena-dark" | "colmena-light" | ruta a un .toml propio
font_family = "JetBrains Mono" # si no está instalada, se usa la monoespaciada del sistema
font_size = 14
line_height = 1.2
opacity = 1.0
padding = 8

[terminal]
shell = ""                    # vacío = shell del usuario ($SHELL / COMSPEC)
scrollback = 10000

[layout]
sidebar_width = 240
sidebar_visible = true
agents_panel = "right"        # "right" | "left" | "hidden"
agents_panel_width = 300

[agent_line]
fields = ["ctx", "model", "tokens", "elapsed"]  # disponibles: ctx, model, tokens, tokens_out, cache_read, elapsed, tool

[sidebar]
entry_fields = ["status", "title", "agent_line", "subagents"]

[notifications]
on = ["waiting_permission", "question", "turn_finished"]
only_when_unfocused = true

[keys]                        # "mod" = Cmd en macOS, Ctrl+Shift en Linux/Windows
new_terminal = "mod-t"
close_terminal = "mod-w"
next_terminal = "mod-]"
prev_terminal = "mod-["
next_waiting = "mod-j"
toggle_agents_panel = "mod-shift-a"
toggle_sidebar = "mod-b"
copy = "mod-c"
paste = "mod-v"
# "mod-1" … "mod-9" van a la terminal N (no configurable en v1)

[context_windows]             # sobrescribe o agrega entradas a la tabla interna, en tokens
# "claude-opus-5-5" = 1_000_000
```

### 7.3 Temas

Un tema es un archivo TOML:

- `[terminal]`: `foreground`, `background`, `cursor`, `selection` y `ansi` (16 colores).
- `[ui]`: `sidebar_bg`, `panel_bg`, `text`, `muted`, `accent`, `border`, `status_working`, `status_waiting`, `status_done`, `status_error`.

Vienen dos temas incluidos: `colmena-dark` y `colmena-light`.

## 8. Manejo de errores

| Situación | Comportamiento |
|---|---|
| Falla `colmena-hook` (sin socket, timeout, JSON inválido) | Sale con código 0 en silencio. Claude no se entera. |
| El socket no se puede crear al arrancar | La app funciona como terminal sin integración y muestra un banner con el error. |
| Hooks no instalados o ruta del binario desactualizada | Banner "Instalar integración con Claude". |
| Error al escribir `settings.json` | No se toca nada (escritura atómica: archivo temporal + rename). Se muestra el error. |
| El PTY no arranca | La entrada de la barra muestra ✗ con el mensaje; se puede reintentar. |
| El proceso del shell termina | `[proceso terminó, código N]`; Enter reabre. |
| Transcript inexistente o ilegible | Se conserva el último estado y el árbol se marca "datos parciales". |
| Config inválida | Se conserva la anterior y se muestra un banner con el error y la línea. |
| Panic en un hilo de `ingest` | Se registra en el log y el lector se reinicia. La terminal no se ve afectada. |

**Logs:** `tracing` escribe a un archivo rotativo diario en `<data_local_dir>/colmena/logs/` y conserva 7 archivos. El nivel por defecto es `info`; la variable `COLMENA_LOG` lo cambia.

## 9. Testing

- **`agent-model`:** secuencias de eventos → árbol, estados y stats esperados. Incluye tests de idempotencia (el mismo evento por hook y por transcript) y de deduplicación de `Usage` por `message_id`.
- **`ingest`:**
  - fixtures **sintéticas** con la forma real de los JSONL, los `.meta.json` y los payloads de hooks, nunca transcripts reales del usuario;
  - tests golden que verifican qué eventos salen de cada archivo;
  - lectura incremental: líneas partidas, archivo truncado, archivos nuevos en `subagents/`.
- **`colmena-hook`:** se ejecuta el binario real:
  - contra un socket de test, verificando que el mensaje llega bien formado;
  - sin socket, verificando que sale en menos de 100 ms con código 0;
  - verificando siempre que stdout y stderr quedan vacíos;
  - sin las variables de entorno, verificando que sale sin leer stdin.
- **`term-core`:** se lanzan comandos reales en un PTY (`echo`, `printf` con secuencias de color) y se verifican la grilla y el resize.
- **`config`:** valores por defecto, errores con línea, claves desconocidas y recarga en vivo.
- **Instalador de hooks:** merge sobre `settings.json` con hooks preexistentes, idempotencia y desinstalación, todo sobre archivos temporales.
- **UI:** lógica de vistas con el soporte de tests de GPUI, y una checklist de QA manual por OS para el render.
- **CI:** GitHub Actions con macOS, Linux y Windows, corriendo build, `cargo test`, `clippy -D warnings` y `fmt --check`. El render visual en Linux y Windows se valida a mano en una VM.

## 10. Rendimiento

Son metas que se miden al final de M2 y de M4, no garantías:

- RSS menor a **150 MB** con 5 terminales abiertas y 2 sesiones de Claude con subagentes activos.
- Latencia tecla → píxel menor a **10 ms**.
- **0 %** de CPU de render con todo inactivo.
- `cat` de un archivo de 50 MB sin que la UI se congele.

## 11. Alcance por versión

| v1 | v1.1 | v2 |
|---|---|---|
| Todo lo de este documento | Splits, restaurar terminales al reabrir, búsqueda en el scrollback, URLs clickeables | Control desde la UI (permisos, cancelar subagentes, mensajes), plugins/scripting, perfiles de terminal, sesiones fuera de Colmena, pantalla de ajustes, costo estimado en USD |

## 12. Hitos de la v1

Cada hito tiene su propio plan de implementación y termina con algo usable y testeado.

1. **M1 — Terminal mínima:** `term-core` + `term-view` + ventana GPUI con una terminal funcional. Benchmark de `cat` grande. Build en los 3 OS en CI.
2. **M2 — Varias terminales:** barra lateral (sin datos de agentes), crear, cerrar y renombrar terminales, `config` con recarga en vivo, temas y atajos. Medición de memoria.
3. **M3 — Integración con Claude (sin UI):**
   - primero, capturar payloads reales de cada hook en Claude Code 2.1.329 con un hook de registro, para verificar los campos de las secciones 5.3 y 5.5 y fijar las fixtures;
   - después, `colmena-hook`, el socket de `ingest`, el lector de transcripts, `agent-model` y el instalador de hooks;
   - salida de depuración: un volcado del estado a log.
4. **M4 — Panel de agentes y avisos:** panel derecho, línea de stats en la barra, encargo desplegable, notificaciones y `next_waiting`. Medición de memoria final.
5. **M5 — Empaquetado:** `.app`/`.dmg` en macOS, AppImage en Linux y `.zip` con los dos binarios en Windows. La firma y notarización quedan para la v1.1.

## 13. Riesgos

| Riesgo | Mitigación |
|---|---|
| GPUI está antes de la 1.0, con API cambiante y poca documentación. | Versión exacta fijada. GPUI confinado a `term-view` y `colmena`. Zed como referencia de lectura (es GPL: no se copia código). |
| El formato de los transcripts es interno y puede cambiar. | Parsing tolerante, fixtures por versión y degradación por campo (sección 5.9). |
| Los payloads de hooks pueden diferir de lo esperado. | M3 arranca capturando payloads reales antes de escribir el parser. |
| La tabla de ventanas de contexto puede quedar desactualizada. | Override en `[context_windows]` y fallback a número absoluto. |
| Linux necesita Vulkan; ConPTY en Windows tiene particularidades (resize, mouse). | QA manual en VM antes de cerrar M1 y M4. |
| El render de la grilla con GPUI puede ser lento con mucha salida. | Benchmark en M1 antes de seguir. |

## 14. Criterios de aceptación de la v1

1. Con 3 terminales abiertas, 2 de ellas con Claude Code despachando subagentes, el panel derecho muestra el árbol correcto con el encargo de cada subagente, y la línea de stats se actualiza menos de 1 s después de cada evento.
2. Los tokens que muestra Colmena para un agente coinciden con la suma de `input + cache_creation + output` del transcript, deduplicada por `message.id`.
3. Una sesión que pide permiso o hace una pregunta genera una notificación del sistema, y el click lleva a esa terminal.
4. Cambiar `agent_line.fields`, el tema o un atajo en `config.toml` se refleja sin reiniciar la app.
5. Con Colmena cerrada, Claude Code funciona igual que sin hooks: sin demoras perceptibles y sin salida extra.
6. Los tres OS compilan y pasan los tests en CI. macOS pasa la checklist de QA manual completa, y Linux y Windows pasan la de render.
