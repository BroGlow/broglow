<a href="../../">Back to the project</a>

<a name="troubleshooting"></a>

# If the lamp stays dark

For other problems (wrong color, `unavailable`, no menu-bar icon, pairing or upload errors),
see the [troubleshooting table](hardware.md#troubleshooting-and-service-recovery).

Without live sessions, there is no steady session light; startup, pairing, unpaired-blink
or waiting indications can still appear. Start a new local task in a supported agent.
For Codex, follow the [Codex setup steps](codex.md) and check that the task appears in BroGlow.
A question Codex asks in text ends the turn as **Paused**; only a permission request
shows **Needs help** (red).

If Terminal cannot find `broglow` or runs an older copy, follow the [PATH checks](alternatives.md#path).

Then check, in BroGlow's **Lamps** view:

- **Updates paused** means lamp control is off: click **Enable lamp** (or **Enable lamps**).
- Bluetooth is on and BroGlow is allowed under
  System Settings → Privacy & Security → Bluetooth.
- On macOS, a new lamp is set up automatically when you connect only that lamp by USB.
  Use **Add lamp** to retry, approve any macOS prompt and wait for **Connected**.
- Expand the intended lamp's row: at 0% **Light brightness** its LEDs stay dark even when
  connected. **Identify** briefly lights that lamp's LEDs one by one
  ([details](installation.md#controls)). If it is unavailable or nothing lights, the board
  needs matching firmware; updating the app does not flash it.
- **Troubleshooting** at the bottom of **Lamps** (or `broglow ble status` in Terminal)
  shows the background service's recent lamp status.

The lamp shows session status only while an awake Mac runs BroGlow: without updates for
10 seconds it goes dark (a disconnected lamp may show a faint pulse every 30 seconds).
If one color is still missing or faint, disconnect power before checking that LED's
polarity, resistor, contacts and shared GND.

Want a hand? Paste this prompt into Claude Code or Codex on this Mac.

```text
My BroGlow lamp stays dark. Help me find out why.
Follow {{ repo_url }}/blob/HEAD/site/guide/en/troubleshooting.md and
{{ repo_url }}/blob/HEAD/site/guide/en/hardware.md#troubleshooting-and-service-recovery
(for Codex also {{ repo_url }}/blob/HEAD/site/guide/en/codex.md).
Check in order: a live local agent session appears in BroGlow; the menu-bar service runs
(`broglow ble status`); Bluetooth is on and allowed for BroGlow; lamp updates are enabled;
the intended lamp is Connected with brightness above 0%.
Constraints:
- Do not change other lamps, names, settings or unrelated hooks.
- I do the physical and approval steps: plugging USB, wiring checks with power off,
  looking at the LEDs, macOS prompts and Codex /hooks approval. Ask me for them.
- Identify my board before any hardware command. Do not reflash, reinstall, uninstall
  or clear pairings as a shortcut; flash only if the board lacks working BroGlow
  firmware and I agree.
Report: what you checked and the result, the likely cause, and what I still need to check
by looking at the lamp.
```
