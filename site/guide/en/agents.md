<a href="../../">Back to the project</a>

<a name="agents"></a>

# Connect your coding agents

`broglow install` registers hooks for Claude Code and for every other supported agent
it finds on this computer. Only Codex asks you to confirm them. The proof that an
agent is connected is a status change in BroGlow's session list, not a lit lamp.

| Agent | What `broglow install` does | What you do |
| --- | --- | --- |
| Claude Code | Adds hooks to `~/.claude/settings.json`. | Nothing; open sessions report from their next prompt. |
| Codex | Adds hooks to `$CODEX_HOME/hooks.json` (default `~/.codex`). | [Trust them in `/hooks`](#codex-setup). |
| opencode | Installs the plugin `~/.config/opencode/plugins/broglow.js`. | Restart running sessions. |
| GitHub Copilot CLI | Writes `~/.copilot/hooks/broglow.json`. | Restart sessions open during installation. |

Installed an agent after BroGlow? Run `broglow install` again.

Only tasks running on this computer are tracked: cloud or remote tasks and ordinary
ChatGPT chats never reach the local BroGlow service. Returning to,
naming and dismissing sessions is the same for every agent: [Working with sessions](sessions.md).

<a name="usage"></a>

## Usage across agents

Turn on **Settings → Track usage** in the menu-bar popover. Tracking is off by
default. Claude, Codex and opencode each get their own token total for today;
**All agents today** adds those local totals. BroGlow does not price tokens or
estimate bills. Token counts are local activity totals, not subscription balances.

| Agent | Tokens for today | Limits |
| --- | --- | --- |
| Claude Code | Local Claude transcripts. | Claude's OAuth usage endpoint, when available. |
| Codex | Local Codex rollout records. | The latest limits observed in those records, when available; they can lag account usage. |
| opencode | Completed assistant messages observed by the BroGlow plugin after tracking is enabled. | No subscription limits are inferred from tokens. |
| GitHub Copilot CLI | Token usage is unavailable. | Unavailable. |

Claude and Codex limits are independent: BroGlow labels each source and never adds
their percentages. A weekly limit can appear on its own. Missing limits stay
absent rather than showing zero. On macOS, the menu bar pairs each available
percentage with the app's monochrome Claude or Codex icon; without limits it shows
the combined local token total. Windows uses agent names in the tray tooltip.

**Show usage in menu bar**, directly below **Track usage**, is on by default.
Turn it off to hide the usage numbers and agent icons beside the status light;
background tracking and the popover's usage totals continue. The checkbox is
disabled while tracking is off and keeps your saved display choice. On Windows,
it controls usage in the tray tooltip.

Claude totals include local subagent transcripts; Codex totals include archived
chats and exclude copied fork history. Legacy fork logs with ambiguous ownership
may be omitted to avoid counting inherited usage again. Codex limits are the
last observed account values, not a live account query.

opencode with an API key contributes only its locally observed tokens. API usage
does not consume your Claude or Codex subscription allowance. After upgrading
BroGlow, run `broglow install` to update the opencode plugin and restart opencode.
BroGlow does not import opencode's historical usage; only messages observed after
opt-in with the updated plugin are counted.

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
   **Needs help**. An unanswered async question
   [expires](sessions.md#question-expiry) once Codex confirms that its turn has ended.
   Automatic approval review stays **Working** if Codex records the
   reviewer; otherwise permission requests show **Needs help**. Interrupting a turn
   pauses it; ending the session removes it from live status.

Unsaved background jobs, internal subagents, approval reviewers and temporary
helpers that prepare app suggestions do not appear as separate chats or affect
the lamp. A subagent's activity does not clear its parent chat's unanswered
question or change its status. Saved scheduled chats, top-level chats created or
forked by an agent and accepted suggestion tasks remain monitored, even when their
title or prompt starts with `This is an app-generated request`. BroGlow uses
session metadata to identify background work; that phrase alone is not enough.
If an older stale row remains without identifying metadata,
[dismiss it](sessions.md#dismiss) once.

<a name="codex-missing-hooks"></a>

### If the BroGlow entries are missing

If you installed Codex after BroGlow, run `broglow install` again, then reopen `/hooks`.

If Codex reports that hooks are disabled, enable `hooks = true` under `[features]` in
Codex's `config.toml` yourself. BroGlow never edits that file.

<a name="codex-no-session"></a>

### If no session appears

Existing sidebar tasks need a new prompt. If that does not help, finish active work,
restart Codex and try again.
