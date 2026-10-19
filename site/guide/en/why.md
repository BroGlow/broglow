<a href="../../">Back to the project</a>

<a name="why"></a>

# Why BroGlow

<a name="clawlight"></a>

## Thanks to ClawLight

BroGlow would not exist without [ClawLight](https://github.com/clawlight/clawlight-cli).
Its idea, a small desk lamp that shows what your coding agents are doing, is the idea
BroGlow is built on, and BroGlow started as a fork of its code. We honor the original
project and its authors: ClawLight proved the concept and inspired us to take it
further. BroGlow is a separate, independently maintained application; the rest of this
page lists what it adds.

Sessions get real names, agent and host icons, project grouping and one-click opening
of the right conversation; the lamp works over Bluetooth on macOS with per-lamp setup,
naming and brightness; installation comes through Homebrew with safe upgrades; and the
printable enclosure has a swappable magnetic mascot with an interactive assembly guide.

<a name="sessions"></a>

## Sessions

| What's better | What you gain |
| --- | --- |
| **Open the right conversation** | Click a session to land in its Claude desktop chat, Codex desktop task or terminal window. |
| **Real session names** | Sessions carry Claude's own titles and renamed Codex tasks instead of "Session abc123". |
| **Agent and host at a glance** | Each row shows which agent is working (Claude, Codex, opencode, Copilot) and where (desktop app, terminal, IDE). |
| **Group by project** | Sessions sit under one heading per working directory; same-named folders are told apart. |
| **Improved status handling** | Working, idle and needs-input states follow what the agent is doing more closely, for Claude and Codex alike. |
| **Clear paused sessions** | Remove clutter in one click while keeping active work and approval requests visible. |

Details: [Working with sessions](sessions.md).

<a name="lamp"></a>

## Lamp

| What's better | What you gain |
| --- | --- |
| **Wireless lamp connection** | Put the lamp anywhere in Bluetooth range; no data cable to the Mac. |
| **Plug in, paired** | A new USB-attached lamp pairs itself on macOS; **Add lamp** remains as a manual retry. |
| **Lamps managed one at a time** | Name, forget or identify one lamp; the others stay connected, with status for each. |
| **Brightness with live preview** | Adjust each lamp to a comfortable level and see the result before saving. |
| **Identify a lamp** | Flash the selected lamp with a three-pass scan to confirm which one you're controlling. |
| **Safe firmware flashing** | One locked toolchain flashes and backs up the board the same way on every machine. |

Details: [Install and connect](installation.md#pairing).

<a name="install"></a>

## Install and upkeep

| What's better | What you gain |
| --- | --- |
| **Homebrew install** | `brew install broglow/tap/broglow`, then `broglow install`; upgrades the same way. |
| **Upgrades that don't break** | Config writes are atomic, a failed macOS upgrade rolls back, dotfile symlinks survive, uninstall removes every hook. |
| **Track token usage** | Opt-in view of your plan's 5-hour and weekly percentages and today's tokens, Claude and Codex together. |
| **120 fps popover** | The menu-bar interface renders at your display's full refresh rate: smooth scrolling, settings that stay responsive while agents work. |

Details: [Install and connect](installation.md).

<a name="build"></a>

## Build and docs

| What's better | What you gain |
| --- | --- |
| **Print the enclosure, swap the mascot** | The mascot holds on by magnets: attach any mascot you like and change it anytime. |
| **Interactive assembly guide** | Rotate and zoom the separated parts to understand how they fit together. |

Details: [Build your lamp](index.md) and [Printing and assembly](printing.md).

Bluetooth lamp features (wireless control, automatic pairing, naming, per-lamp
brightness, **Identify**) and direct opening of desktop sessions are macOS-only.
The lamp still needs power and is designed for DIY assembly.
