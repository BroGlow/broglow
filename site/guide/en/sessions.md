<a href="../../">Back to the project</a>

<a name="sessions"></a>

# Working with sessions

Every connected agent's sessions appear in the same list, in the menu-bar popover and
in the terminal dashboard, and feed the same lamp. What you can do with them is the
same for every agent.

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
