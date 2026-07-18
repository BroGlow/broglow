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
cargo test               # unit tests + end-to-end tests (tests/) of the built binary
cargo clippy             # lint (CI enforces -D warnings)
cargo fmt                # format (CI enforces --check)
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
| *(none)* | Launch the TUI dashboard (`run_tui`); first launch on an unconfigured machine auto-runs the install (see gotchas) |
| `install` / `uninstall` | Hooks + platform autostart (launchd / registry Run key / XDG autostart) |
| `menubar` | Run the tray daemon in the foreground |
| `led [--port]` | Foreground ESP32 LED mirror (debugging) |
| `update <firmware> [--port]` | Serial-OTA push of new board firmware |
| `hook` *(hidden)* | Claude Code hook backend; reads one event as JSON on stdin |
| `event` *(hidden)* | Normalized-event backend for non-Claude harnesses (opencode); reads one event as JSON on stdin |
| `name <id> <transcript>` *(hidden)* | Detached auto-namer; titles a session via the `claude` CLI |

## Module map (src/)

- **main.rs** — CLI parsing, TUI setup/teardown (panic hook restores the terminal),
  and all install/uninstall + per-platform autostart logic.
- **hook.rs** — the `hook`, `event`, and `name` backends. Maps Claude hook events (and
  the harness-agnostic normalized verbs from `event`) → `Status`, does the locked
  read-modify-write of `state.json` via the shared `update_state` helper, spawns the
  detached auto-namer on first `Stop`. `run_event` is the multi-harness ingestion path
  (opencode today) — see "Multi-harness adapters" below.
- **state.rs** — `HookState`/`SessionStatus`/`Status` types (incl. the optional
  `harness` tag: absent = Claude, `"opencode"` = opencode), `state.json` read/write
  (atomic temp-file + rename), the shared `.state.lock` (`acquire_state_lock`), the
  `reap_ended_sessions` downgrade to `Done` (dead-process reap via `terminal::is_alive`,
  plus the 24h staleness backstop), and `Aggregate` (Red/Yellow/Green/None) health rollup.
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

## Multi-harness adapters (opencode, and future ones)

Sessions from other coding agents flow into the *same* `state.json` and so light up the
TUI, tray, aggregate, and LED with no reader changes — `merge_sessions` already displays
hook-state sessions that have no Claude sessions-index entry. The adapter is deliberately
thin:

- **`clawlight event`** (`hook::run_event`) is the ingestion path. It reads one *normalized*
  event as JSON on stdin: `{ harness, event, session_id, title?, directory? }`. The status
  verbs are harness-agnostic — a future Codex/Copilot adapter emits the same shape and needs
  no new Rust:

  | verb | `Status` |
  |---|---|
  | `working` / `resumed` | `Active` (chatty `working` is write-suppressed when already active, unless a title rides along) |
  | `idle` | `Inactive` |
  | `needs_input` | `NeedsHelp` |
  | `ended` | `Done` |
  | `title` | name-only update, never changes status (an idle session must not flip green on a rename) |
  | `reconnected` | restart sweep: this harness's `Active`/`Inactive` sessions whose owner process is gone (or was never captured) → `Done` |

  Unknown verbs, a missing `harness`, and (except `reconnected`) missing session ids are
  dropped, never errors. `harness` is **required** — never defaulted to a specific agent, or a
  mislabeled event would masquerade as it. `directory` is optional and, like `name`/`terminal`,
  preserved when an event omits it.
- **`SessionStatus.harness`** tags the origin (`#[serde(default, skip_serializing_if)]`;
  absent = Claude). Backward/forward compatible in both directions.
- **`update_state`** in hook.rs is the one locked RMW helper shared by `run` (Claude hooks),
  `run_event`, and `run_namer` — never reimplement the lock / atomic-write / never-write-on-
  unreadable rules.
- **`harness.rs` is the registry.** Everything agent-specific — detection, plugin/hook
  install + uninstall, and the UI badge — lives in one `const ADAPTERS: &[Adapter]` table.
  `install_all`/`uninstall_all` (called from `register_hooks`/`uninstall_hooks`) and
  `badge(name)` (used by the TUI and popover) iterate it, so **adding Codex/Copilot is one
  `Adapter` entry + its embedded asset**, not edits across main.rs/session.rs/ui.rs. Each new
  harness needs a **unique** `badge` (a test enforces it — the `codex`/`copilot` → `"co"`
  collision is the trap the two-char fallback would otherwise hide).
- **`assets/opencode-plugin.js`** is embedded via `include_str!` and written to opencode's
  global `~/.config/opencode/plugins/clawlight.js` at install time, with this binary's absolute
  path and version baked in (so it runs even when clawlight isn't on opencode's PATH; the
  unconditional rewrite each install is the version-skew fix). It is **logic-free by design**:
  it only normalizes opencode's bus events to the verbs above and fire-and-forgets `clawlight
  event` — every mapping decision lives in Rust where it's tested. Install is detection-gated
  (opencode config dir exists *or* `opencode` on PATH); uninstall removes the file only if it
  still carries the `managed by clawlight` header. opencode loads plugins at startup, so
  already-running sessions need a restart to appear.
- **Shutdown** (opencode has no per-session exit event) is handled in three layers: the
  plugin's `process.on("exit")` emits `ended`; the `reconnected` sweep is the backstop; and,
  because `run_event` captures the host terminal's `owner_pid` on first sighting, the existing
  `state::reap_ended_sessions` dead-process reap already clears a terminal-hosted opencode
  session whose process exits. `opencode serve` (no controlling tty) has no `owner_pid` and
  leans on the sweep / 24h staleness backstop.

## Conventions & gotchas

- **First-run auto-setup is asymmetric on purpose** (main.rs). Both entry points check
  `hooks_registered()` (a clawlight hook command present in `settings.json`) and act only
  when it's false. The **TUI** (`first_run_setup_tui`) runs the *full* `install_hooks`
  (hooks + autostart), so `brew install clawlight && clawlight` is a complete setup. The
  **tray daemon** (`first_run_setup_daemon`) runs `register_hooks` **only** — never
  autostart: the daemon is already running, and bootstrapping/kickstarting its own
  LaunchAgent (or spawning a detached `menubar` on Linux/Windows) would create a duplicate
  tray. Unparseable `settings.json` counts as "registered" so we stay hands-off.
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
- **LEDs are opt-in, but a clean first install auto-enables them when a board is already
  plugged in** (`maybe_autoenable_led` in main.rs, called from `install_hooks`). It's gated
  on there being no `config.json` yet, so re-running install — or an existing user — never
  has the LEDs flipped back on against a deliberate opt-out. Otherwise the daemon never
  opens a serial port until `led_enabled` is set — from the tray popover's footer lamp
  control (macOS/Windows) or `l` in the TUI. Board detection matches only the XIAO's native
  USB-Serial-JTAG vendor ID (`0x303A`), so a positive hit is unambiguously our board and
  clawlight never writes to an unrelated serial device.
- **The popover footer shows lamp connection status** (`renderLamp` in assets/popover.html,
  fed by `LampPayload` in popover.rs). Four states from `{present, enabled}`: connected
  (click disconnects), detected-but-off (click connects → `SetLed`), owned-but-unplugged,
  and none (a "Get a lamp" link → `GetLamp` opens clawlight.dev). Presence is read via
  `led::board_present_cached` (short-TTL cache, since the page asks on every state push).
- **Usage/spend tracking is strictly opt-in** (`usage_enabled`, set in the popover's
  Settings view; off by default). While off, `usage::spawn_refresher` does *no* work —
  it never scans the transcript JSONLs, reads Claude Code's credentials, or contacts the
  OAuth usage endpoint — and both the tray readout (`apply_readout`) and the popover's
  usage section stay empty regardless of any cached snapshot. Enabling it is what
  authorizes that work. The foreground `clawlight usage` subcommand is the exception:
  running it is itself the opt-in for that one invocation, like `clawlight led`.
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

`.github/workflows/ci.yml` runs on PRs and pushes to main: `cargo fmt --check`,
then clippy (`-D warnings`) + `cargo test` on ubuntu/macos/windows. The tests/
suite drives the *compiled binary* end-to-end: Claude hook events on stdin →
`state.json` assertions, the auto-namer against a fake `claude` on PATH
(hook_lifecycle.rs, unix-only — `$HOME` can't be redirected on Windows),
normalized harness events → `state.json` (event_lifecycle.rs, unix-only, same
reason), install/uninstall round-trip incl. the opencode plugin (install.rs,
Linux-only — on macOS it would `launchctl bootout` the dev machine's real
daemon), and CLI smoke tests incl. a `node --check` parse of the embedded
opencode plugin when node is present (cli.rs, all platforms). Prefer extending
these over unit tests for new behavior. A `Stop` event with a `transcript_path` spawns the real detached
namer — tests must omit it or pre-seed a name.

`tests/js/` holds the opencode plugin **integration** tests (`node --test`,
Linux CI): they drive the real `assets/opencode-plugin.js` with opencode's
actual event shapes and assert the `state.json` the built binary writes — the
one boundary the Rust suite (which feeds `clawlight event` directly) can't cover.
`exit-child.mjs` is a helper process, not a test, so the plugin's
`process.on("exit")` shutdown path can be exercised for real.

`.github/workflows/release.yml` builds and packages release binaries per platform.
