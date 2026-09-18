<a href="../../">Back to the project</a>

<a name="troubleshooting"></a>

# If the lamp stays dark

Without live sessions, there is no steady session light. Brief startup, pairing or waiting
indications can still appear. Start a new local task in a supported
agent. For Codex, follow the [Codex setup steps](codex.md) and check that the task appears in ClawLight.
An ordinary conversational question from Codex does not itself turn the light red.

If Terminal cannot find `clawlight` or runs an older copy, follow the [PATH checks](installation.md#path).

Check power, that lamp control is enabled in ClawLight, and Bluetooth. Allow ClawLight under
System Settings → Privacy & Security → Bluetooth. For a new lamp, connect it alone by USB
and use **Lamps → Add lamp**; approve the macOS prompt and wait for **Connected**.
In **Lamps**, expand the intended lamp's row and check its **Light brightness**:
0% keeps the controlled LEDs dark even when connected. Move that slider to preview all three,
or use **Identify** for three red → yellow → green passes on that connected lamp at 100%,
100 ms per LED (0.9 seconds total), then the latest status and saved brightness return.
Identify never changes preferences. Older firmware silently ignores this scan mode;
it needs a matching firmware update, which updating the app does not perform.
**All lamps** replaces individual brightness levels with one shared value.
The session color clears after 10 seconds without updates; a disconnected lamp may resume a
quiet waiting pulse. It needs an awake Mac running ClawLight. If a color is still missing or
faint, disconnect power before checking that LED's polarity, resistor, contacts and shared GND.
See the [detailed troubleshooting steps](hardware.md#troubleshooting-and-service-recovery).

Want a hand? Open the project or unpacked package in your coding agent and paste this prompt.

```text
Help me find out why my ClawLight stays dark using docs/hardware-setup.md in this project or
package. Start with agent sessions, hooks, the background service, Bluetooth status, and
the intended lamp's saved brightness (0% is intentionally dark). Check its row's brightness
preview or Identify (three red/yellow/green passes); if firmware lacks the scan mode,
report that a matching firmware update is needed. Preserve other lamps, names and preferences.
If setup is needed, use
Lamps → Add lamp with one USB-connected lamp and let me approve macOS prompts. For
Codex, check hook trust with the same CODEX_HOME as the app and verify that a real local task
appears in ClawLight. Guide me through /hooks approval if needed; leave approval to me.
Distinguish local Codex tasks from ordinary ChatGPT chats and cloud or remote tasks. Identify
my actual board before hardware commands and check its firmware and pin assignments. Do not
reflash it or clear pairings as a troubleshooting shortcut. If wiring needs checking, guide me
with USB power disconnected. Explain what you verified and what still needs a physical LED
check.
```
