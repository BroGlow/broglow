# clawlight

A single Rust binary that monitors [Claude Code](https://claude.ai/code) sessions
in real time. It ships three faces of the same executable:

- **TUI dashboard** (`clawlight`) — a ratatui table of all sessions with live status.
- **Menu bar / system tray daemon** (`clawlight menubar`) — a color-coded Clawd icon
  reflecting aggregate session health; auto-starts at login.
- **Hook backend** (`clawlight hook`) — invoked by Claude Code's lifecycle hooks to
  record per-session status. Replaces the old bash+jq hook script; no shell/`jq` dep.

Optionally mirrors aggregate state to a Seeed XIAO ESP32-C6 status board over USB serial.

Cross-platform: macOS, Windows, Linux. MIT licensed.

## Build / test / install

```bash
cargo build              # debug build
cargo test               # unit tests (config, ota crc, session truncate)
cargo clippy             # lint
cargo install --path .   # build + install to ~/.cargo/bin/clawlight
clawlight install        # register hooks in ~/.claude/settings.json + login autostart
clawlight uninstall      # reverse install and delete ~/.claude/clawlight
```

`clawlight install` is idempotent and also (re)starts the tray daemon, so re-run it
after `cargo install` to make the running daemon pick up a new binary. On macOS it
rewrites and reloads the LaunchAgent via `launchctl bootout`/`bootstrap`/`kickstart`.

**Always redeploy after code changes.** Whenever you change code in this repo, finish
by updating the app that is already running on this machine so it can be tested live:

```bash
cargo install --path . && clawlight install
```

Never test by launching a second daemon (e.g. `cargo run -- menubar`) — macOS has no
single-instance lock, and two daemons mean two tray icons fighting over the same
serial port. Redeploy the installed one instead.

## Subcommands (src/main.rs)

| Command | Purpose |
|---|---|
| *(none)* | Launch the TUI dashboard (`run_tui`) |
| `install` / `uninstall` | Hooks + platform autostart (launchd / registry Run key / XDG autostart) |
| `menubar` | Run the tray daemon in the foreground |
| `led [--port]` | Foreground ESP32 LED mirror (debugging) |
| `update <firmware> [--port]` | Serial-OTA push of new board firmware |
| `hook` *(hidden)* | Hook backend; reads one event as JSON on stdin |
| `name <id> <transcript>` *(hidden)* | Detached auto-namer; titles a session via the `claude` CLI |

## Module map (src/)

- **main.rs** — CLI parsing, TUI setup/teardown (panic hook restores the terminal),
  and all install/uninstall + per-platform autostart logic.
- **hook.rs** — the `hook` and `name` backends. Maps hook events → `Status`, does the
  locked read-modify-write of `state.json`, spawns the detached auto-namer on first `Stop`.
- **state.rs** — `HookState`/`SessionStatus`/`Status` types, `state.json` read/write
  (atomic temp-file + rename), the shared `.state.lock` (`acquire_state_lock`), the
  24h staleness downgrade to `Done`, and `Aggregate` (Red/Yellow/Green/None) health rollup.
- **session.rs** — discovers `~/.claude/projects/*/sessions-index.json`, merges those
  entries with hook state into `DisplaySession`s, resolves display names and sort order.
- **config.rs** — `~/.claude/clawlight/config.json` (LED opt-in, optional `led_port`,
  and `yellow_mode` — how idle sessions color the aggregate).
- **app.rs** — TUI event loop: file watcher + 5s timer refresh, key handling, notifications.
- **ui.rs** — ratatui rendering of the table and status bar.
- **menubar.rs** — tray icon + menu (tao/tray-icon event loop), spawns the LED daemon thread.
- **led.rs** — ESP32 detection by USB VID and the serial LED driver (`run` / `run_daemon`).
- **ota.rs** — host side of the stop-and-wait serial firmware update protocol.
- **notification.rs** — desktop notifications (macOS `osascript`; else `notify-rust`).
- **spawn.rs** — Windows-only detached/windowless child-process flags (no-op elsewhere).
- **terminal.rs** — click-to-focus: `capture` records which terminal window/app hosts a
  session (env vars + process-tree walk, stored per session in `state.json`); `focus`
  raises that window later (AppleScript tab match by tty on macOS, ancestor-window
  `SetForegroundWindow` on Windows, xdotool on Linux). Used by the popover session
  rows, the Linux tray menu, and the TUI's `↵`.

## Data flow

Claude Code fires a hook → `clawlight hook` reads the event JSON on stdin → updates the
session's entry in `~/.claude/clawlight/state.json`. The TUI and the tray daemon both
watch that file (via `notify`) and re-render on change. Status mapping:

- `SessionStart` / `UserPromptSubmit` / `PreToolUse` → `Active`
- `Stop` → `Inactive`
- `Notification` → `NeedsHelp` (except `idle_prompt`, which is ignored)
- `SessionEnd` → `Done`

Aggregate health for the icon/LED: any `NeedsHelp` → Red; then the `yellow_mode` config
(set in the popover's Settings view) decides mixed states — `any_inactive` (default):
any `Inactive` → Yellow, else any `Active` → Green; `active_wins`: any `Active` → Green,
Yellow only when every live session is idle. No live sessions → None (gray).
See `state::aggregate`.

## Conventions & gotchas

- **Never byte-slice user/LLM strings.** Session names come from prompts, transcript
  summaries, and LLM-generated titles — all arbitrary UTF-8. Slice by chars (see
  `truncate` / `char_prefix` in session.rs); a byte cut inside a multibyte char panics
  and takes the TUI down on refresh.
- **`state.json` writes are atomic** (temp file + rename) and **guarded by `.state.lock`.**
  Any read-modify-write of the state file (hooks *and* the TUI's `x`/clear) must hold
  `state::acquire_state_lock` first, or it can clobber a concurrent writer's update.
- **Never write to state on a failed read.** `hook.rs` returns `Unreadable` when
  `state.json` can't be parsed (a mid-write snapshot, unknown future schema); the caller
  must not write, or it would wipe every other session's status.
- **The auto-namer runs detached** and sets `CLAWLIGHT_NAMING=1` so the nested `claude`
  CLI call's own hooks no-op instead of recursing.
- **LEDs are strictly opt-in.** The daemon never opens a serial port until `led_enabled`
  is set (TUI: `l`). Board detection matches only the XIAO's native USB-Serial-JTAG
  vendor ID (`0x303A`), so it never writes to an unrelated serial device.
- **Escape before shelling out.** The macOS notification path builds an AppleScript
  string — escape `\` before `"`.
- Platform-specific code is `#[cfg(...)]`-gated in place (autostart in main.rs, console
  hiding / single-instance mutex in menubar.rs, detached-spawn flags in spawn.rs).

## ESP32 board (optional)

Firmware lives in a separate repo, `clawlight/clawlight-firmware`. `scripts/flash.sh`
does the first USB flash; afterward `clawlight update <image>` pushes new firmware over
the same serial link. The OTA CRC-32 must match the firmware's implementation — pinned
by the test in `ota.rs`.

## CI

`.github/workflows/release.yml` builds and packages release binaries per platform.
