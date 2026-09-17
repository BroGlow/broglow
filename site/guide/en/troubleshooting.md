<a href="../../">Back to the project</a>

# If the lamp stays dark { #troubleshooting }

Without live sessions, there is no steady session light. Brief startup, pairing or waiting
indications can still appear. Start a new local task in a supported
agent. For Codex, follow the [Codex setup steps](codex.md) and check that the task appears in ClawLight.
An ordinary conversational question from Codex does not itself turn the light red.

If Terminal cannot find `clawlight` or runs an older copy, follow the [PATH checks](installation.md#path).

Check power, that lamp control is enabled in ClawLight, and Bluetooth. Allow ClawLight under
System Settings → Privacy & Security → Bluetooth. In **Lamps**, check **Light brightness**:
0% keeps the controlled LEDs dark even when connected. Move the slider to preview all three.
The session color clears after 10 seconds without updates; a disconnected lamp may resume a
quiet waiting pulse. It needs an awake Mac running ClawLight. If a color is still missing or
faint, disconnect power before checking that LED's polarity, resistor, contacts and shared GND.
See the [detailed troubleshooting steps](hardware.md#troubleshooting-and-service-recovery).

Want a hand? Open the project or unpacked package in your coding agent and paste this prompt.

```text
Help me find out why my ClawLight stays dark using docs/hardware-setup.md in this project or
package. Start with agent sessions, hooks, the background service, Bluetooth status, and
the saved brightness (0% is intentionally dark). Check the brightness preview. For
Codex, check hook trust with the same CODEX_HOME as the app and verify that a real local task
appears in ClawLight. Guide me through /hooks approval if needed; leave approval to me.
Distinguish local Codex tasks from ordinary ChatGPT chats and cloud or remote tasks. Identify
my actual board before hardware commands and check its firmware and pin assignments. Do not
reflash it or clear pairings as a troubleshooting shortcut. If wiring needs checking, guide me
with USB power disconnected. Explain what you verified and what still needs a physical LED
check.
```
