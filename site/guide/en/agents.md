<a href="../../">Back to the project</a>

<a name="agents"></a>

# Connect your coding agents

`broglow install` registers hooks for Claude Code and for every other supported agent
it finds on this computer. Only Codex asks you to confirm them. The proof that an
agent is connected is a status change in BroGlow's session list, not a lit lamp.

| Agent | What `broglow install` does | What you do |
| --- | --- | --- |
| Claude Code | Adds hooks to `~/.claude/settings.json`. | Nothing; new sessions report at once. |
| Codex | Adds hooks to `$CODEX_HOME/hooks.json`. | [Trust them in `/hooks`](#codex-setup). |
| opencode | Installs the plugin `~/.config/opencode/plugins/broglow.js`. | Restart running sessions. |
| GitHub Copilot CLI | Writes `~/.copilot/hooks/broglow.json`. | Restart sessions open during installation. |

Only tasks running on this computer are tracked: cloud or remote tasks and ordinary
ChatGPT chats never reach the local BroGlow service. Each agent's status mapping is in
the [hardware reference](hardware.md#connect-local-codex-sessions).

<a name="claude-code"></a>

## Claude Code

Nothing to configure. Sessions in the terminal and in the Claude desktop app report
their status from the next prompt on.

<a name="codex-setup"></a>

## Codex

Installing BroGlow adds hooks to `$CODEX_HOME/hooks.json` (default `~/.codex/hooks.json`),
but Codex still needs your approval before running them.

1. Open Codex CLI (0.144 or later) in a terminal. For desktop tasks, use the same user
   and `CODEX_HOME` as the app: either the app's bundled CLI (for example
   `/Applications/ChatGPT.app/Contents/Resources/codex`) or an installed `codex`
   command that uses the same `CODEX_HOME`.
2. Enter `/hooks`. Review and trust the eight BroGlow entries: `SessionStart`,
   `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`,
   `Interrupt` and `SessionEnd` (the last two require a recent Codex version).
   Each is labeled **BroGlow: update session status** and runs the full path to
   `broglow` followed by `codex-hook`, for example
   `"/path/to/broglow" codex-hook`. Leave other hooks unchanged. Changed hook
   definitions may need approval again.
3. Send a prompt in a new local Codex task, in the CLI or desktop app.
   In BroGlow's session list, check that this task shows **Working** while it runs and
   **Paused** after the response. A human approval request or blocking question shows
   **Needs help**. Automatic approval review stays **Working** when the current turn's
   reviewer is available in the transcript; without that metadata, permission requests
   still show **Needs help**. Interrupting a turn pauses it; ending the session removes
   it from live status. Other live
   sessions and the **Settings → Mixed sessions** choice also affect the lamp's color.

<a name="codex-missing-hooks"></a>

### If the BroGlow entries are missing

BroGlow adds the Codex hooks only if Codex was already installed when BroGlow was set up.
If you installed Codex later, run `broglow install` again, then reopen `/hooks`.

If Codex reports that hooks are disabled, enable `hooks = true` under `[features]` in
Codex's `config.toml` yourself. BroGlow never edits that file.

<a name="codex-no-session"></a>

### If no session appears

Existing sidebar tasks need a new prompt. If no session appears, finish active work,
restart Codex and try again. A connected lamp does not prove that the hooks work; only
a status change in BroGlow's session list does.

Ordinary ChatGPT chats and cloud or remote tasks do not send events to the local
BroGlow service.

<a name="opencode"></a>

## opencode

opencode loads plugins at startup, so sessions that were already running when you
installed BroGlow appear only after you restart them. If you install opencode later,
run `broglow install` again to add the plugin.

<a name="copilot"></a>

## GitHub Copilot CLI

Copilot reads its hooks when a session starts, so restart any session that was open
during installation. If you install Copilot later, run `broglow install` again.

<a name="open-session"></a>

## Return to a session

Click a session in BroGlow (TUI: **Enter**) to bring it to the front: the terminal
window or tab that hosts it, the conversation in the Claude desktop app, or a Codex
desktop task through its task link (also when the app is named ChatGPT). If no host
can be focused, a Codex session falls back to its task link in the installed Codex app.

<a name="session-names"></a>

## Session names

BroGlow shows the title the agent itself keeps: Claude Code's saved title (the desktop
app's automatic one or `/rename`), Codex's thread name, opencode's session title.
Until there is one, a session is named after the first ~50 characters of its first
prompt (Copilot sessions always are); a Claude Code session without a title gets a
short generated one at the end of its first turn. Before any prompt, a session shows
as `Session <id>`. A new or renamed title appears within five seconds while the list
is open, without changing the session's status or activity time; hover over a name to
see the full title.

<a name="dismiss"></a>

## Dismiss a session

Use **×** beside a session (TUI: **x**) to hide it from BroGlow's list and lamp status.
The conversation stays intact in its agent, and the session reappears on its next hook
event, for example your next prompt. In the menu-bar popover, **Clear paused** beside
the Paused heading dismisses all yellow sessions at once; green working sessions and
red sessions that need help stay in the list.
