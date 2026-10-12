<a href="../../">Back to the project</a>

<a name="codex-setup"></a>

# Connect Codex

Installing BroGlow adds hooks to `$CODEX_HOME/hooks.json` (default `~/.codex/hooks.json`),
but Codex still needs your approval before running them.

1. Open Codex CLI (0.144 or later) in a terminal. For desktop tasks, use the same user
   and `CODEX_HOME` as the app: either the app's bundled CLI (for example
   `/Applications/ChatGPT.app/Contents/Resources/codex`) or an installed `codex`
   command that uses the same `CODEX_HOME`.
2. Enter `/hooks`. Review and trust the six BroGlow entries: `SessionStart`,
   `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest` and `Stop`.
   Each runs the full path to `broglow` followed by `codex-hook`, for example
   `"/path/to/broglow" codex-hook`. Leave other hooks unchanged. Changed hook
   definitions may need approval again.
3. Send a prompt in a new local Codex task, in the CLI or desktop app.
   In BroGlow's session list, check that this task shows **Working** while it runs and
   **Paused** after the response. A permission request shows **Needs help**. Other live
   sessions and the Yellow mode setting also affect the lamp's color.

<a name="codex-missing-hooks"></a>

## If the BroGlow entries are missing

BroGlow adds the Codex hooks only if Codex was already installed when BroGlow was set up.
If you installed Codex later, run `broglow install` again, then reopen `/hooks`.

If Codex reports that hooks are disabled, enable `hooks = true` under `[features]` in
Codex's `config.toml` yourself. BroGlow never edits that file.

<a name="codex-no-session"></a>

## If no session appears

Existing sidebar tasks need a new prompt. If no session appears, finish active work,
restart Codex and try again. A connected lamp does not prove that the hooks work; only
a status change in BroGlow's session list does.

This setup covers tasks running on this computer. Ordinary ChatGPT chats and cloud or
remote tasks do not send events to the local BroGlow service.

<a name="codex-open-session"></a>

## Return to a session

Click a local Codex session (TUI: **Enter**) to return to it. On macOS, desktop tasks
open directly in Codex using their task link, also when the app is named ChatGPT.
CLI sessions focus their captured terminal; if no host can be focused, BroGlow tries
the task link in the installed Codex app.

<a name="codex-session-names"></a>

## Session names

Until Codex saves a title, BroGlow names a session after the first ~50 characters of
its first prompt. This also works for temporary side chats without a transcript.
Later prompts do not change that name. Before any prompt, the session shows as
`Session <id>`.

While the BroGlow list is open, a title that Codex generates or that you rename appears
within five seconds. No new prompt is needed; the session's status and activity time
stay the same. Hover over a session name to see its full saved title.

<a name="codex-dismiss"></a>

## Dismiss a session

Use **×** beside a session (TUI: **x**) to dismiss it from BroGlow's list and lamp
status. The Codex conversation stays intact. It reappears when Codex sends a new hook
event, for example on your next prompt.

In the menu-bar popover, **Clear paused** beside the Paused heading dismisses all
yellow sessions at once. Green working sessions and red sessions that need help stay
in the list.
