<a href="../../">Back to the project</a>

<a name="agents"></a>

# Connect your coding agents

`broglow install` registers hooks or a plugin for supported agents it finds on this
computer. T3 Code sessions are detected automatically without hooks. Only Codex asks you to confirm them. The proof that an
agent is connected is a status change in BroGlow's session list, not a lit lamp.

| Agent | What `broglow install` does | What you do |
| --- | --- | --- |
| Claude Code | Adds hooks to `~/.claude/settings.json`. | Nothing; open sessions report from their next prompt. |
| Codex | Adds hooks to `$CODEX_HOME/hooks.json` (default `~/.codex`). | [Trust them in `/hooks`](#codex-setup). |
| opencode | Installs the plugin `~/.config/opencode/plugins/broglow.js`. | Restart running sessions. |
| GitHub Copilot CLI | Writes `~/.copilot/hooks/broglow.json`. | Restart sessions open during installation. |
| T3 Code | Reads local v2 state and already linked machines; changes no T3 settings. | Open T3 Code 0.1 or newer; see [T3 Code](#t3-code-setup). |

Installed an agent after BroGlow? Run `broglow install` again.

BroGlow reads local agent state and monitors already linked T3 peer machines by
default; see the T3 Code coverage and opt-out below. It does not collect ordinary
ChatGPT chats or cloud tasks outside these sources. Returning to,
naming and dismissing sessions is the same for every agent: [Working with sessions](sessions.md).

<a name="usage"></a>

## Usage across agents

Turn on **Settings → Track usage** in the menu-bar popover. Tracking is off by
default. Claude, Codex and opencode each get their own token total for today;
**All agents today** adds those local totals. BroGlow does not price tokens or
estimate bills. Token counts are local activity totals, not subscription balances.
The popover groups these totals under one **Usage** heading, with each agent's name and existing monochrome icon below it.

| Agent | Tokens for today | Limits |
| --- | --- | --- |
| Claude Code | Local Claude transcripts. | Claude's OAuth usage endpoint, when available. |
| Codex | Local Codex rollout records. | The latest limits observed in those records, when available; they can lag account usage. |
| opencode | Completed assistant messages observed by the BroGlow plugin after tracking is enabled. | No subscription limits are inferred from tokens. |
| GitHub Copilot CLI | Token usage is unavailable. | Unavailable. |
| T3 Code | No separate total; locally recorded usage remains in its Claude, Codex or opencode total. | No T3-specific limits are inferred. |

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

<a name="t3-code-setup"></a>

## T3 Code

T3 Code **0.1 or newer** with the v2 state database is monitored automatically for
all its providers. BroGlow reads `userdata/statev2.sqlite` and development-build
`dev/statev2.sqlite` under `~/.t3` (official app) and `~/.t3-yandex` (Yandex build).
For a custom location, set `T3CODE_HOME` for the BroGlow process to that T3 home
directory; an explicit home selects only its `userdata` database. The older pre-v2
database is not supported. BroGlow never changes T3 settings, installs hooks, or
writes to its database.

1. Open T3 Code and send a prompt in a visible, unarchived thread.
2. Check that the thread appears as **T3 Code** in BroGlow: **Working** during a turn,
   **Needs help** for a blocking approval or question, and **Paused** after a response
   or interruption. Allow up to five seconds for the open session list to refresh.
3. On macOS, click the row to return to that exact thread in the matching T3 app.

Titles and projects come from T3 itself; a thread's worktree takes precedence over
its project directory. Archived or deleted threads/projects and hidden subagents or
side chats do not affect the lamp. Pending background work keeps a parent working;
automatic idle compaction does not wake a dismissed thread. Local provider-session
references suppress duplicate Claude/Codex/opencode rows. When the local T3 server
exits, its threads leave live status.

Claude/Codex hooks identify native CLI resumes separately. OpenCode's shared SDK
server exposes no equivalent provenance: a native resume can remain represented
by its T3 row while that T3 server is running.

**Already linked peer machines are monitored by default.** BroGlow reads T3's peer
links and reuses their existing credentials for read-only MCP project/thread tools.
It does not pair machines or start remote tasks. Polling runs in the background, so
an unreachable machine does not block local updates. To turn this off, add this
field to your existing `~/.broglow/config.json` (or `$BROGLOW_HOME/config.json`),
preserving the other fields:

```json
{
  "t3_linked_machines_enabled": false
}
```

Set it to `true` or remove the field to restore monitoring. Saved connections in
the desktop app's encrypted connection catalog are not read; the connection must
exist as a T3 peer link. Remote MCP data exposes less detail than the local
database: pending approvals/questions conservatively show **Needs help**, and native
provider references, some background tasks and PR-watch activity are unavailable.
Remote provider rows therefore cannot always be deduplicated. Usage stays in the
existing local provider totals, as described [above](#usage); remote usage is not
added.

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

Manual `/side` chats show **Working** while they run and **Needs help** when they
need your input, without changing the original chat's status. After the response,
they leave live status rather than remaining paused. Ordinary saved forks still
show **Paused** after a response.

For a known false red signal during `/side` permission checks, see [Known issues](known-issues.md#codex-side-approval).

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
