<a href="../../">Back to the project</a>

<a name="agents"></a>

# Connect your coding agents

`broglow install` registers hooks for Claude Code and for every other supported agent
it finds on this computer. Only Codex asks you to confirm them. The proof that an
agent is connected is a status change in BroGlow's session list, not a lit lamp.

| Agent | What `broglow install` does | What you do |
| --- | --- | --- |
| Claude Code | Adds hooks to `~/.claude/settings.json`. | Nothing; new sessions report at once. |
| Codex | Adds hooks to `$CODEX_HOME/hooks.json` (default `~/.codex`). | [Trust them in `/hooks`](#codex-setup). |
| opencode | Installs the plugin `~/.config/opencode/plugins/broglow.js`. | Restart running sessions. |
| GitHub Copilot CLI | Writes `~/.copilot/hooks/broglow.json`. | Restart sessions open during installation. |

Only tasks running on this computer are tracked: cloud or remote tasks and ordinary
ChatGPT chats never reach the local BroGlow service. Each agent's status mapping is in
the [hardware reference](hardware.md#connect-local-codex-sessions). Returning to,
naming and dismissing sessions is the same for every agent: [Working with sessions](sessions.md).

<a name="claude-code"></a>

## Claude Code

Sessions in the terminal and in the Claude desktop app report their status from the
next prompt on.

<a name="codex-setup"></a>

## Codex

1. Open Codex CLI (0.144 or later) in a terminal. For desktop tasks, run it as the same
   user with the same `CODEX_HOME` as the app, for example the bundled
   `/Applications/ChatGPT.app/Contents/Resources/codex`.
2. Enter `/hooks`. Review and trust the eight BroGlow entries: `SessionStart`,
   `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`,
   `Interrupt` and `SessionEnd` (the last two require a recent Codex version).
   Each is labeled **BroGlow: update session status** and runs
   `"/path/to/broglow" codex-hook`. Leave other hooks unchanged. Changed hook
   definitions may need approval again.
3. Send a prompt in a new local Codex task, in the CLI or desktop app.
   In BroGlow's session list, check that this task shows **Working** while it runs and
   **Paused** after the response. A human approval request or question shows
   **Needs help**. An unanswered quiz stays red even after Codex finishes its response,
   until you reply, interrupt the turn or end the session.
   Automatic approval review stays **Working** if Codex records the
   reviewer; otherwise permission requests show **Needs help**. Interrupting a turn
   pauses it; ending the session removes it from live status. Other live sessions and
   the **Settings → Mixed sessions** choice also affect the lamp's color.

Unsaved background jobs, internal subagents, approval reviewers and temporary
helpers that prepare app suggestions do not appear as separate chats or affect
the lamp. A subagent's activity does not clear its parent chat's unanswered
question or change its status. Saved scheduled chats, top-level chats created or
forked by an agent and accepted suggestion tasks remain monitored, even when their
title or prompt starts with `This is an app-generated request`. BroGlow uses
session metadata to identify background work; that phrase alone is not enough.
If an older stale row remains without identifying metadata, dismiss it once with
its × button. This does not delete the Codex chat.

<a name="codex-missing-hooks"></a>

### If the BroGlow entries are missing

If you installed Codex after BroGlow, run `broglow install` again, then reopen `/hooks`.

If Codex reports that hooks are disabled, enable `hooks = true` under `[features]` in
Codex's `config.toml` yourself. BroGlow never edits that file.

<a name="codex-no-session"></a>

### If no session appears

Existing sidebar tasks need a new prompt. If that does not help, finish active work,
restart Codex and try again.

<a name="opencode"></a>

## opencode

opencode loads plugins at startup. If you install opencode later, run
`broglow install` again.

<a name="copilot"></a>

## GitHub Copilot CLI

Copilot reads its hooks when a session starts. If you install Copilot later, run
`broglow install` again.
