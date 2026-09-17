<a href="../../">Back to the project</a>

# If the lamp stays dark { #troubleshooting }

The lamp stays off when there are no live sessions. Start a new local task in a supported
agent. For Codex, follow the [Codex setup steps](codex.md) and check that the task appears in ClawLight.
An ordinary conversational question from Codex does not itself turn the light red.

If Terminal cannot find `clawlight` or runs an older copy, follow the [PATH checks](installation.md#path).

Check power, that lamp control is enabled in ClawLight, and Bluetooth. Allow ClawLight under
System Settings → Privacy & Security → Bluetooth. The lamp turns off after 10 seconds without
updates. It needs an awake Mac running ClawLight. If an expected color does not light during a
task, disconnect power before checking wiring and polarity.

Want a hand? Open the project or unpacked package in your coding agent and paste this prompt.

```text
Help me find out why my ClawLight stays dark using docs/hardware-setup.md in this project or
package. Start with agent sessions, hooks, the background service, and Bluetooth status. For
Codex, check hook trust with the same CODEX_HOME as the app and verify that a real local task
appears in ClawLight. Guide me through /hooks approval if needed; leave approval to me.
Distinguish local Codex tasks from ordinary ChatGPT chats and cloud or remote tasks. Identify
my actual board before hardware commands and check its firmware and pin assignments. Do not
reflash it or clear pairings as a troubleshooting shortcut. If wiring needs checking, guide me
with USB power disconnected. Explain what you verified and what still needs a physical LED
check.
```
