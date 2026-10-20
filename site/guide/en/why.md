<a href="../../">Back to the project</a>

<a name="why"></a>

# Why BroGlow

<a name="clawlight"></a>

## Thanks to ClawLight

BroGlow is a separate, independently maintained application inspired by
[ClawLight](https://github.com/clawlight/clawlight-cli). ClawLight showed how a
small desk lamp could make coding-agent activity visible. BroGlow started as a fork
of its code; most of the app has since been rewritten, with a substantially
redesigned interface and expanded capabilities. We thank ClawLight's authors for
the original idea and project.

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
| **Plug in, paired** | A new USB-attached lamp pairs itself on macOS; **Add lamp** retries manually. |
| **Lamps managed one at a time** | Name or forget one lamp; the others stay connected, with status for each. |
| **Brightness with live preview** | Adjust each lamp to a comfortable level and see the result before saving. |
| **Identify a lamp** | A three-pass scan on the selected lamp shows which one it is. |
| **Safe firmware flashing** | One locked toolchain flashes and backs up the board the same way on every machine. |

Details: [Install and connect](installation.md#pairing).

<a name="install"></a>

## Install and upkeep

| What's better | What you gain |
| --- | --- |
| **Homebrew install** | `brew install broglow/tap/broglow`, then `broglow install`; upgrades the same way. |
| **Upgrades that don't break** | Config writes are atomic, a failed macOS upgrade rolls back, dotfile symlinks survive, uninstall removes every hook. |
| **Track token usage** | Opt-in view of your plan's 5-hour and weekly percentages and today's tokens, Claude and Codex together. |
| **120 fps popover** | The menu-bar interface renders at your display's full refresh rate. |

Details: [Install and connect](installation.md).

<a name="build"></a>

## Build and docs

| What's better | What you gain |
| --- | --- |
| **Print the enclosure, swap the mascot** | The mascot holds on by magnets: attach any mascot you like and change it anytime. |
| **Interactive assembly guide** | Rotate and zoom the separated parts. |

Details: [Build your lamp](index.md) and [Printing and assembly](printing.md).

Bluetooth lamp features (including naming, brightness and **Identify**) and
direct opening of desktop sessions are macOS-only.
