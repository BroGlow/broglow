<a href="../../">Back to the project</a>

<a name="sessions"></a>

# Working with sessions

Every connected agent's sessions appear in one list, in the menu-bar popover and the
terminal dashboard, and feed one lamp.

<a name="open-session"></a>

## Return to a session

Click a session in BroGlow (TUI: **Enter**) to bring it to the front: the terminal
window or tab that hosts it, the conversation in the Claude desktop app, or a Codex
desktop task through its task link (also when the app is named ChatGPT). If its host
can't be focused, a Codex session opens its task link in the Codex app.

<a name="session-names"></a>

## Session names

BroGlow shows the title the agent itself keeps: Claude Code's saved title (the desktop
app's automatic one or `/rename`), Codex's thread name, opencode's session title.
Until there is one, a session is named after the first ~50 characters of its first
prompt (Copilot sessions always are); a Claude Code session without a title gets a
short generated one at the end of its first turn. Before any prompt, a session shows
as `Session <id>`. A new or renamed title appears within five seconds while the list
is open; hover over a name to see the full title.

<a name="question-expiry"></a>

## Expired Codex questions

Codex can ask an optional question while continuing its work. If the turn ends
without an answer, BroGlow clears **Needs help** once Codex confirms completion.
A connected lamp flashes red three times, about 500 ms per flash, then returns to
the current session color. The list shows the actual session state throughout.

Answering clears the alert without flashing. If another session needs help, the
lamp stays steadily red. Old effects do not replay after reconnecting or restarting;
brightness, including zero, and lamp identification keep their normal behavior.

If completion cannot be confirmed, or the question was recorded by an older
BroGlow without tracking details, the alert remains. Use **×** to dismiss that row;
this does not answer the question or delete the chat.

<a name="model-capacity"></a>

## Codex model at capacity

When Codex confirms that a turn failed because the selected model is at capacity,
BroGlow shows **Needs help** with **Model at capacity** in the session row.
Choose **Open Codex**, then select another model or retry in Codex.

A connected lamp gives five fast red flashes, about **150 ms on / 150 ms off**,
then stays red while the session needs help. The menu-bar icon stays red throughout.
Each failed turn triggers one burst, even when the lamp was already red. Opening
the list, restarting BroGlow or reconnecting a lamp does not replay it.

A new attempt clears the warning. **×** dismisses it without changing the chat.
Brightness and Identify keep their normal priority; a masked or cancelled burst
does not play later. Bluetooth write latency can lengthen the flashes.

<a name="dismiss"></a>

## Dismiss a session

Use **×** beside a session (TUI: **x**) to hide it from BroGlow's list and lamp status.
The conversation stays intact in its agent, and the session reappears on its next hook
event, for example your next prompt. In the menu-bar popover, **Clear paused** beside
the Paused heading dismisses all yellow sessions at once.
