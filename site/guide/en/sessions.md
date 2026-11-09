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
can't be focused, a Codex session opens its task link in the Codex app. On macOS,
a T3 Code row opens its exact thread in the matching T3 Code app.

<a name="session-names"></a>

## Session names

BroGlow shows the title the agent itself keeps: Claude Code's saved title (the desktop
app's automatic one or `/rename`), Codex's thread name, opencode's session title,
and T3 Code's thread title.
Claude, Codex and opencode fall back to the first ~50 characters of the first prompt
until a title is available (Copilot sessions always use that prompt); a Claude Code
session without a title gets a
short generated one at the end of its first turn. Before any prompt, a session shows
as `Session <id>`. A new or renamed title appears within five seconds while the list
is open; hover over a name to see the full title.

Inline Markdown links are displayed as their readable label: `[book](book)`
appears as `book`. This changes only the display, not the agent's stored title.

<a name="question-expiry"></a>

## Skipped questions

Pending T3 questions show **Needs help**, including asynchronous questions while the
model continues working. They stay red until T3 confirms an answer or closes the
question; a completed model turn alone does not close an answerable T3 question.
For native Codex questions, confirmed turn completion can expire an unanswered question.

When a displayed question closes unanswered, a connected lamp scans three loops in
one direction: **red → yellow → green → red**. It starts from the last successfully
written color, with at least **100 ms per transition**, then continues through the
intermediate colors to the latest session state. The nine visible transitions take
900 ms; each acknowledged phase, including the final color, holds for at least
100 ms. Settling can add two transitions, and write delays can extend the effect.
When no color is known, the scan starts red. It still plays over a red aggregate.

The affected chat gains a quiet **Question skipped · 2m ago** line below its agent
and project. Its actual status and activity time remain unchanged. The note follows
the session's normal visibility; it does not create a history section or keep a
finished chat in the list. Answered questions and failed question creation do not
produce a skipped-question effect.

Old effects do not replay after reconnecting or restarting. Brightness, including
zero, and Identify keep their normal priority. If capacity and a skipped question
arrive together, the capacity flashes play first, followed by the pending scan.
Unknown or missing evidence does not clear an outstanding question automatically.

<a name="model-capacity"></a>

## Model at capacity

When native Codex or a local T3 projection confirms that a turn failed because its
model is overloaded, BroGlow shows **Needs help** and **Model at capacity**. The
session button opens the corresponding app so you can choose another model or retry.
Generic provider failures, quota limits and text mentioning capacity do not trigger
the capacity animation.

A connected lamp gives five fast red flashes, about **150 ms on / 150 ms off**,
then returns to the latest session state, normally red while help is needed. Each
failed turn triggers one burst, even over red. A new attempt clears the warning;
**×** dismisses the row without changing the chat. Opening the list, restarting
BroGlow or reconnecting a lamp does not replay the effect.

The linked-machine MCP view currently reports pending-request counts without the
structured overload codes or all question-closure reasons. Linked questions still
turn red, and explicit cancelled question items produce the skipped note and scan.
BroGlow does not infer other closures or capacity from their text.

<a name="dismiss"></a>

## Dismiss a session

Use **×** beside a session (TUI: **x**) to hide it from BroGlow's list and lamp status.
The conversation stays intact in its agent, and the session reappears on new activity,
for example your next prompt. T3 Code is polled without hooks; renaming a dismissed
thread alone does not bring it back. In the menu-bar popover, **Clear paused** beside
the Paused heading dismisses all yellow sessions at once.
