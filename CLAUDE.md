[English](CLAUDE.md) | [Русский](CLAUDE.ru.md)

# broglow

Follow the standalone product scope in [AGENTS.md](AGENTS.md).

For installation from a package or source, connecting a board, and checking a
running Mac service, start with the [agent runbook](site/guide/en/hardware.md#agent-runbook).

A single Rust binary that monitors [Claude Code](https://claude.ai/code) sessions
in real time. It ships three faces of the same executable:

- **TUI dashboard** (`broglow`) — a ratatui table of all sessions with live status.
- **Menu bar / system tray daemon** (`broglow menubar`) — a color-coded status-light icon
  reflecting aggregate session health; auto-starts at login.
- **Hook backend** (`broglow hook`) — invoked by Claude Code's lifecycle hooks to
  record per-session status. Replaces the old bash+jq hook script; no shell/`jq` dep.

Optionally mirrors aggregate state over USB serial, or from macOS over BLE to the
ESP32-C3 SuperMini firmware in this repository. For hardware changes, flashing,
Bluetooth setup, deployment, and acceptance results, read
[site/guide/en/hardware.md](site/guide/en/hardware.md).

Cross-platform: macOS, Windows, Linux. MIT licensed.

## Build / test / install

For source changes, follow [CONTRIBUTING.md](CONTRIBUTING.md) for development
setup, formatting, hooks and checks. The [build and test commands](CONTRIBUTING.md#build-and-test)
are shared by human and agent contributors.

`broglow install` is idempotent and also (re)starts the tray daemon, so re-run it
after `cargo install` to make the running daemon pick up a new binary. On macOS it
installs `~/Applications/BroGlow.app` and reloads the existing LaunchAgent
(`io.github.broglow.menubar`) via `launchctl bootout`/`bootstrap`/`kickstart`.
Inspect the installed service and preserve unrelated hooks/settings before deployment.

**Redeploy only for live checks.** Builds and tests do not need an installed app.
When a change must be verified in the app already running on this machine, and
replacing that installation is intended, update it:

```bash
cargo install --path . && broglow install
```

A process-lifetime lock rejects a second tray instance (a lock file on
macOS/Linux, a named mutex on Windows); a separate BLE lock prevents discovery
and the worker from using the radio concurrently. Builds do not prove permission
grant or board operation: record those acceptance checks separately.

## Subcommands (src/main.rs)

| Command | Purpose |
|---|---|
| *(none)* | Launch the TUI dashboard (`run_tui`); first launch on an unconfigured machine auto-runs the install (see gotchas) |
| `install` / `uninstall` | Hooks + platform autostart (launchd / registry Run key / XDG autostart) |
| `menubar` | Run the tray daemon in the foreground |
| `led [--port]` | Foreground ESP32 LED mirror (debugging) |
| `ble pair [--port]` | Identify BroGlow firmware over USB, open pairing and remember its matching BLE ID; actual tray handles encryption/macOS approval |
| `ble scan/add/remove/rename/brightness/list/on/off/usb/status` | Discover, remember/forget CoreBluetooth IDs, name a lamp, save per-lamp or global brightness, list per-lamp state, enable/disable the selected transport, restore USB, or inspect worker status |
| `hook` *(hidden)* | Claude Code hook backend; reads one event as JSON on stdin |
| `event` *(hidden)* | Normalized-event backend for non-Claude harnesses (opencode); reads one event as JSON on stdin |
| `codex-hook` *(hidden)* | Codex shim: reads one Claude-dialect hook payload on stdin, maps it to a normalized event |
| `copilot-hook <event>` *(hidden)* | Copilot shim: reads one per-event payload on stdin (the event name rides on argv), maps it to a normalized event |
| `name <id> <transcript>` *(hidden)* | Detached auto-namer; titles an untitled session via the `claude` CLI, falling back to the first prompt |

## Module map (src/)

- **main.rs** — CLI parsing, TUI setup/teardown (panic hook restores the terminal),
  and all install/uninstall + per-platform autostart logic.
- **hook.rs** — the `hook`, `event`, `codex-hook`, `copilot-hook`, and `name` backends.
  Maps Claude hook events (and the harness-agnostic normalized verbs from `event`) →
  `Status`, does the locked read-modify-write of `state.json` via the shared
  `update_state` helper, names Claude sessions from Claude's own `custom-title`
  transcript record, spawns the detached auto-namer on the first untitled `Stop`. `run_event` is
  the multi-harness ingestion path; `run_codex_hook` and `run_copilot_hook` translate
  Codex's Claude-dialect / Copilot's per-event payloads onto the same verbs in-process —
  see "Multi-harness adapters" below.
- **codex.rs** — everything Codex-specific behind the `codex` adapter: `$CODEX_HOME`
  paths, the position-preserving hooks.json merge (Codex trusts hooks by content hash +
  position — never reorder foreign groups, never write Codex's config.toml), thread
  names from `session_index.jsonl`, exec-vs-interactive from rollout `session_meta`,
  and first-typed-prompt extraction for fallback naming.
- **copilot.rs** — everything Copilot-specific behind the `copilot` adapter:
  `$COPILOT_HOME` paths and the wholly-owned `hooks/broglow.json` registration file
  (one command group per lifecycle event, the event name baked into argv because
  Copilot payloads don't carry it; ownership proven by the `copilot-hook` command
  string — JSON has no comments for the managed-by marker).
- **state.rs** — `HookState`/`SessionStatus`/`Status` types (incl. the optional
  `harness` tag: absent = Claude, `"opencode"` = opencode), `state.json` read/write
  (atomic temp-file + rename), the shared `.state.lock` (`acquire_state_lock`), the
  `reap_ended_sessions` downgrade to `Done` (dead-process reap via `terminal::is_alive`,
  plus the 24h staleness backstop), and `Aggregate` (Red/Yellow/Green/None) health rollup.
- **session.rs** — turns hook state into the TUI's `DisplaySession`s: display names,
  sort order and the five most recent finished sessions. Every name comes from the hooks.
  Also the one home of project labelling and grouping shared by every surface:
  `project_labels` (the folder name, extended with parent folders only where two shown
  folders share a name; `NO_PROJECT` for an unknown directory) and `group_by_project`
  (the order behind the `group_by_project` setting: projects by their most urgent
  session, then recency, then path; "No project" last).
- **config.rs** — `~/.claude/broglow/config.json` (LED opt-in, `led_transport`
  defaulting to USB, optional `led_port`, remembered `ble_peripherals`, `yellow_mode`,
  `show_agent_names`, on by default, and `group_by_project`, off by default).
  Use `ble_ids()` to deduplicate remembered `ble_peripherals`.
  Preserve unrelated and unknown settings when changing the lamp selection.
- **app.rs** — TUI event loop: file watcher + 5s timer refresh, key handling, notifications.
- **ui.rs** — ratatui rendering of the table and status bar. Colors are 24-bit only when
  `COLORTERM` is `truecolor`/`24bit`; otherwise each frame maps them to the xterm 256-color
  palette, because Terminal.app before macOS 26 misreads RGB escapes as separate SGR codes.
- **menubar.rs** — tray icon + menu (tao/tray-icon event loop), spawns the LED daemon thread.
- **led.rs** — USB detection/driver, aggregate-to-byte mapping, transport dispatch,
  and fresh cross-process worker status in `lamp-status.json`.
- **ble.rs** — USB-assisted setup, discovery/selection and the macOS BLE worker: service-filtered
  scan, independent connections to every saved nearby lamp, acknowledged writes,
  heartbeat and reconnect. An absent lamp must not block another lamp's updates.
  The worker follows saved-lamp edits itself (one task per lamp): `same_target` and
  `status_for` must not compare the lamp list, or Forget reconnects every lamp.
  USB-assisted `pair` requires exact INFO identity before PAIR and matches the
  service plus full six-byte base MAC in manufacturer data (`0xffff`). INFO runs before
  the worker pause unless USB mirroring holds the port, so a known board never pauses
  the other lamps. A new lamp is found by the running BLE worker without a pause: `pair`
  writes `.ble-setup-request`, the worker's `discover` records service + `0xffff`
  advertisements to `ble-sightings.json` only while asked, and exactly one fresh
  exact match is saved; `supervise` then starts its task. No worker answer, no
  sighting, disabled/USB mirroring or a lamp connected to this Mac (it stops
  advertising) fall back to the pause and own scan under `radio_lock`. The automatic
  `remember_usb` guard expects the enable state setup left (`paused`). The tray checks
  USB every five seconds and attempts setup once per new attachment; `usb_lamps`
  records verified boards so Disable/Forget survive restart. Retain exact identity
  checks, serial/worker locks, failure rollback and macOS consent.
  Keep btleplug 0.13.1 locked: its macOS write-error propagation fixes dropped
  CoreBluetooth NSError failures during pairing. A longer host timeout cannot
  extend the macOS system pairing alert (about 30 seconds observed).
- **notification.rs** — desktop notifications (macOS `osascript`; else `notify-rust`).
- **spawn.rs** — Windows-only detached/windowless child-process flags (no-op elsewhere).
- **terminal.rs** — click-to-focus: `capture` records which terminal window/app hosts a
  session (env vars + process-tree walk, stored per session in `state.json`), plus the
  hosting agent's `owner_pid` for the dead-process reap (the tty owner on unix, the first
  ancestor above the wrapper shells on Windows); `focus`
  raises that window later (AppleScript tab match by tty on macOS, ancestor-window
  `SetForegroundWindow` on Windows, xdotool on Linux); a Claude desktop session opens
  by its own `CLAUDE_CODE_HOST_SESSION_ID` via `claude://code/continue`. Used by the
  popover session rows, the Linux tray menu, and the TUI's `↵`. `host` classifies the
  same identity as an agent's desktop app, a terminal or an IDE with a short name
  ("Claude app", "Ghostty", "VS Code") for the popover's host glyph and the TUI's HOST
  column; a desktop bundle wins over stale terminal variables, and an identity-less
  Codex session is the Codex app, as in `focus`.

## Data flow

Claude Code fires a hook → `broglow hook` reads the event JSON on stdin → updates the
session's entry in `~/.claude/broglow/state.json`. The TUI and the tray daemon both
watch that file (via `notify`) and re-render on change. Status mapping:

- `SessionStart` / `UserPromptSubmit` / `PreToolUse` → `Active`
- `PreToolUse` of a tool that hands the turn to the user → `NeedsHelp`, because no
  `Notification` fires for it: `AskUserQuestion` (marker `ask_user_question`; the tool
  blocks the turn, so the next event clears it), and the desktop app's
  `mcp__ccd_turn__wrap_up` when it carries an `ask` or `blocked: true` (marker
  `question_box`). A recap-only wrap-up is a normal turn end.
- `Stop` → `Inactive`, except right after a `question_box` wrap-up: the app ends the
  turn the moment the box is shown, so that `Stop` keeps `NeedsHelp` until the user's
  reply (`UserPromptSubmit`) lands.
- `Notification` → `NeedsHelp` (except `idle_prompt`, which is ignored)
- `SessionEnd` → `Done`

Aggregate health for the icon/LED: any `NeedsHelp` → Red; then the `yellow_mode` config
(set in the popover's Settings view) decides mixed states — `any_inactive` (default):
any `Inactive` → Yellow, else any `Active` → Green; `active_wins`: any `Active` → Green,
Yellow only when every live session is idle. No live sessions → None (gray).
See `state::aggregate`.

## Multi-harness adapters (opencode, and future ones)

Sessions from other coding agents flow into the *same* `state.json` and so light up the
TUI, tray, aggregate, and LED with no reader changes — every reader lists hook-state
sessions, whatever agent wrote them. The adapter is deliberately
thin:

- **`broglow event`** (`hook::run_event`) is the ingestion path. It reads one *normalized*
  event as JSON on stdin: `{ harness, event, session_id, title?, directory? }`. The status
  verbs are harness-agnostic — adapters emit the same shape (opencode via its JS plugin,
  Codex and Copilot via the in-binary `codex-hook` / `copilot-hook` shims):

  | verb | `Status` |
  |---|---|
  | `working` / `resumed` | `Active` (chatty `working`/`resumed` are write-suppressed when already active, unless a title rides along) |
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
  absent = Claude).
- **`update_state`** in hook.rs is the one locked RMW helper shared by `run` (Claude hooks),
  `run_event`, and `run_namer` — never reimplement the lock / atomic-write / never-write-on-
  unreadable rules.
- **`harness.rs` is the registry.** Everything agent-specific — detection, plugin/hook
  install + uninstall, and the UI badge, label and icon — lives in one `const ADAPTERS:
  &[Adapter]` table. `install_all`/`uninstall_all` (called from `register_hooks`/
  `uninstall_hooks`) and `agent(harness)` iterate it. `agent` gives the popover's icon and
  name, the TUI's AGENT column (the name, or the tag with names off) and the Linux tray
  menu's `[cx] Codex · recco`; Claude comes from `CLAUDE_LABEL`/`CLAUDE_ICON`/`CLAUDE_BADGE` (`cc`). So **adding
  Codex/Copilot is one `Adapter` entry + its embedded asset**, not edits across
  main.rs/session.rs/ui.rs. Each new harness needs a **unique** `badge` (a test enforces
  it — the `codex`/`copilot` → `"co"` collision is the trap the two-char fallback would
  otherwise hide), plus a unique `label` and `icon`. Icons are our own geometric glyphs,
  never vendor logos: plain SVG path data for a 16×16 stroked `currentColor` icon, which
  the popover places only in a `d` attribute. An unknown harness shows its badge instead.
- **`assets/opencode-plugin.js`** is embedded via `include_str!` and written to opencode's
  global `~/.config/opencode/plugins/broglow.js` at install time, with this binary's absolute
  path and version baked in (so it runs even when broglow isn't on opencode's PATH; the
  unconditional rewrite each install is the version-skew fix). It is **logic-free by design**:
  it only normalizes opencode's bus events to the verbs above and fire-and-forgets `broglow
  event` — every mapping decision lives in Rust where it's tested. Install is detection-gated
  (opencode config dir exists *or* `opencode` on PATH); uninstall removes the file only if it
  still carries the `managed by broglow` header. opencode loads plugins at startup, so
  already-running sessions need a restart to appear.
- **Codex needs no plugin.** Codex (>= 0.144) fires Claude-dialect hooks itself, so its
  adapter registers `broglow codex-hook` matcher groups in `$CODEX_HOME/hooks.json` and
  the shim maps events in-process: `PermissionRequest` → `needs_input` (Codex has no
  `Notification`), `PostToolUse`/`UserPromptSubmit` → `resumed` (what clears the red after
  an approval), `Stop` → `idle` — or `ended` for one-shot `codex exec` rollouts (detected
  from the rollout's `session_meta`; Codex has no `SessionEnd`, so interactive quits are
  left to the owner-pid reap). Titles come from Codex's own `session_index.jsonl` thread
  names at turn boundaries, with the first typed prompt as a one-time fallback — never an
  LLM call. Two Codex-side rules: its hooks.json merge must preserve foreign matcher
  groups *in place* (Codex trusts hooks by content hash + position; the user approves new
  broglow entries once via `/hooks` in Codex), and broglow must never write Codex's
  config.toml (Codex rewrites it while running).
- **Copilot needs no plugin.** GitHub Copilot CLI fires user-level lifecycle hooks
  itself: any `*.json` in `$COPILOT_HOME/hooks/` (default `~/.copilot/hooks/`) may
  register commands per event, so the adapter writes a file broglow wholly owns —
  `hooks/broglow.json` — and never merges into anyone else's configuration. Copilot
  payloads don't name their event, so each registration is `broglow copilot-hook
  <event>` and the shim maps the argv/payload pair in-process: `permissionRequest` →
  `needs_input` (its `notification` types are an undocumented vocabulary — left
  unregistered rather than risk false reds), `postToolUse`/`userPromptSubmitted` →
  `resumed` (what clears the red after an approval), `agentStop` → `idle`, and — unlike
  Codex — a real `sessionEnd` → `ended`, covering one-shot `copilot -p` runs and
  interactive quits with no rollout sniffing. Titles: Copilot's own AI-generated
  session names live in its SQLite session store (not worth a dependency), so the
  first typed prompt — carried in the hook payload itself (`prompt`/`initialPrompt`) —
  names the session once; never an LLM call, never a file read. Ownership of
  `broglow.json` is the `copilot-hook` command string inside it (JSON has no
  comments), guarding both the install overwrite and the uninstall delete.
- **Shutdown** (opencode has no per-session exit event) is handled in three layers: the
  plugin's `process.on("exit")` emits `ended`; the `reconnected` sweep is the backstop; and,
  because `run_event` captures the host terminal's `owner_pid` on first sighting, the existing
  `state::reap_ended_sessions` dead-process reap already clears a terminal-hosted opencode
  session whose process exits. `opencode serve` (no controlling tty) has no `owner_pid` and
  leans on the sweep / 24h staleness backstop.

## Conventions & gotchas

- **First-run auto-setup is asymmetric on purpose** (main.rs). Both entry points check
  `hooks_registered()` (a broglow hook command present in `settings.json`) and act only
  when it's false. The **TUI** (`first_run_setup_tui`) runs the *full* `install_hooks`
  (hooks + autostart), so first launch can finish setup after installing our binary. The
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
- **Claude owns its session titles.** At `SessionStart`/`UserPromptSubmit`/`Stop`, and on
  every event while a session is still unnamed, the hook reads the latest `custom-title`
  record (desktop auto-title or `/rename`) from the last 1 MiB of the transcript; it
  replaces any earlier name. The desktop app titles a session a few records into its
  first turn, so a long first turn is named mid-turn. Only an untitled first `Stop`
  spawns the auto-namer. At startup the tray runs `backfill_claude_titles` once, so idle
  sessions get their title without waiting for a new turn; it never overwrites a name.
- **The auto-namer runs detached** and sets `BROGLOW_NAMING=1` so the nested `claude`
  CLI call's own hooks no-op instead of recursing. It passes `--no-session-persistence`
  so naming calls don't appear in the user's history. When the CLI fails (for example,
  not logged in while only the desktop app is), it names the session from the first
  prompt, without `<system-reminder>` blocks. It never overwrites an existing name.
- **LEDs are opt-in, but a clean first install auto-enables USB when a board is already
  plugged in** (`maybe_autoenable_led` in main.rs, called from `install_hooks`). It's gated
  on there being no `config.json` yet, so re-running install — or an existing user — never
  has the LEDs flipped back on against a deliberate opt-out. The daemon mirrors status
  only while `led_enabled` is set — from Lamp details or TUI `l`.
  On macOS, the tray also identifies new USB candidates while mirroring is disabled
  and enables successfully added new lamps over BLE. Rendering stays read-only.
  USB VID `0x303A` identifies Espressif,
  not an exact board model: identify the actual hardware before flashing or issuing
  serial commands. Preserve the existing USB default and explicit-port behavior.
- **The popover footer shows lamp connection status** (`renderLamp` in assets/popover.html,
  fed by `LampPayload` in popover.rs). The footer opens Lamp details; it does not
  toggle the lamp. Enable/disable is an explicit action in that scrollable view,
  with Back/Escape returning to sessions. The TUI uses `d` for lamp details,
  arrows or `j`/`k` to scroll, and Escape to return. Both main session views keep
  status compact; full worker status and per-device diagnostics live in details.
  Advertisement or USB presence never means connected. USB presence offers USB
  enabling and triggers macOS setup of a new lamp. BLE targets persist
  across restarts and enable without USB. A healthy `1/2` is normal for home/office
  lamps: one successful recent write makes the aggregate connected, with explicit
  per-device states/count. Expiry is per device; another lamp cannot refresh it.
- **macOS Bluetooth belongs to the installed app identity.** The LaunchAgent runs
  `BroGlow.app/Contents/MacOS/broglow menubar`; the app's Info.plist supplies
  `NSBluetoothAlwaysUsageDescription`. Explicit `ble scan` re-execs the bundled
  binary. Local installs use ad-hoc signing with a stable designated identifier;
  `BROGLOW_SIGNING_IDENTITY` supports a real signing identity. Terminal permission
  alone does not establish that the login-started service has permission.
- **Usage tracking is strictly opt-in** (`usage_enabled`, set in the popover's
  Settings view; off by default). It shows the plan's 5-hour/weekly percentages and
  today's token total; BroGlow never prices tokens. It covers both harness families:
  Claude transcripts (per-message `usage` blocks) and Codex rollouts (cumulative `token_count` events,
  folded as per-event deltas with reset detection). While off, `usage::spawn_refresher`
  does *no* work —
  it never scans the transcript JSONLs, reads Claude Code's credentials, or contacts the
  OAuth usage endpoint — and both the tray readout (`apply_readout`) and the popover's
  usage section stay empty regardless of any cached snapshot. Enabling it is what
  authorizes that work. The foreground `broglow usage` subcommand is the exception:
  running it is itself the opt-in for that one invocation, like `broglow led`.
- **Project grouping is one order and one label rule** (`group_by_project`, set in the
  popover's Settings view; off by default). `session::project_labels` and
  `session::group_by_project` are the only implementations: the TUI and the Linux menu
  take them through `load_all_sessions`, the popover's payload carries the label plus
  `projectPath` and its JS applies the same keys (most urgent session, then that row's
  recency, then path; "No project" last). Labels are computed over the sessions a
  surface shows, so a hidden done session never changes a visible label. Never add a
  per-surface sort or a basename-only label.
- **Escape before shelling out.** The macOS notification path builds an AppleScript
  string — escape `\` before `"`.
- Platform-specific code is `#[cfg(...)]`-gated in place (autostart in main.rs, console
  hiding / single-instance mutex in menubar.rs, detached-spawn flags in spawn.rs).

## ESP32 board (optional)

The ESP32-C3 SuperMini firmware is in `firmware/esp32-c3-supermini/` (Arduino /
PlatformIO). Red/yellow/green use GPIO0/1/3 on the GND side, each through its own
resistor (330 Ω red/yellow, 220 Ω green); cathodes share GND. Skip GPIO2, a boot-strapping pin. BLE
uses exactly one `R`, `Y`, `G`, or `N` byte with response; the host sends immediately,
on changes, and every two seconds. The board clears session status ten seconds
after its last valid update; a still-connected stale link stays dark, while a
disconnected board resumes its setup/waiting indication. A connected BLE client owns LED control; serial commands work
when BLE is disconnected. Flash with PlatformIO or the documented prebuilt
esptool procedure; firmware updates over the application protocol are unsupported.
BLE writes require encryption and LE Secure Connections Just Works bonding.
After normal boot, BOOT/GPIO9 held three seconds opens a 60-second pairing window;
ten seconds requests deletion of all board bonds. No bonds at startup also opens
the window. Just Works is not MITM-authenticated. Indications use 2 kHz/eight-bit
PWM: pairing chases red/yellow/green; unpaired blinks all three; bonded waiting
pulses dimly every 30 seconds. The first valid encrypted write per connection
causes one 250 ms confirmation flash, then fresh status (including `N`) wins;
disconnect, lost authentication or stale expiry cancels the flash.
GPIO8 defaults to an active-low single-color LED, using one/two/three pulses for
green/yellow/red; build flags configure its pin/polarity/brightness, or disable it
with `BUILTIN_LED_PIN=-1`. The user confirmed rapid blue pairing blinking;
other visible patterns remain unverified. A `?` serial diagnostic reports stored
state, commanded on/off and PWM duties, mode, pin and
pairing/bonds; these are not pin readback or proof of visible light. `N` cannot
distinguish disabled from absent Mac, so disconnected waiting pulses may resume;
`WAIT_LED_BRIGHTNESS=0` silences them. The hardware guide owns pattern timing and setup.

See the hardware guide for flashing commands and the confirmed hardware state.

## CI

`.github/workflows/ci.yaml` runs on PRs and pushes to master: `cargo fmt --check`,
then clippy (`-D warnings`) + `cargo test` on ubuntu/macos/windows. The tests/
suite drives the *compiled binary* end-to-end: Claude hook events on stdin →
`state.json` assertions, the auto-namer against a fake `claude` on PATH
(hook_lifecycle.rs, unix-only — `$HOME` can't be redirected on Windows),
normalized harness events → `state.json` (event_lifecycle.rs, unix-only, same
reason), Codex Claude-dialect payloads through the shim (codex_lifecycle.rs,
unix-only, same reason), Copilot per-event payloads through the shim
(copilot_lifecycle.rs, unix-only, same reason), install/uninstall round-trip
incl. the opencode plugin, Codex hooks.json and the Copilot hooks file (install.rs,
Linux-only — on macOS it would `launchctl bootout` the dev machine's real
daemon), and CLI smoke tests incl. a `node --check` parse of the embedded
opencode plugin when node is present (cli.rs, all platforms). Prefer extending
these over unit tests for new behavior. A `Stop` event with a `transcript_path` spawns the real detached
namer — tests must omit it or pre-seed a name.

`tests/js/` holds two `node --test` suites (Linux CI). opencode-integration.test.mjs
holds the opencode plugin **integration** tests: they drive the real `assets/opencode-plugin.js` with opencode's
actual event shapes and assert the `state.json` the built binary writes — the
one boundary the Rust suite (which feeds `broglow event` directly) can't cover.
`exit-child.mjs` is a helper process, not a test, so the plugin's
`process.on("exit")` shutdown path can be exercised for real.
lamp-status.test.mjs loads `assets/popover.html` and covers its worker-driven
lamp status and lamp controls, the Settings view and the session list, including
grouping by project.

`.github/workflows/release.yaml` builds and packages release binaries per platform.
CI also builds the pinned C3 PlatformIO environment and runs its host mock checks.
