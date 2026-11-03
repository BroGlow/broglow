<a href="../../">Back to the project</a>

<a name="troubleshooting"></a>

# If the lamp stays dark

For other problems (wrong, faint or missing color, `unavailable`, no menu-bar icon,
pairing or upload errors), see the [troubleshooting table](hardware.md#troubleshooting-and-service-recovery).

Without a live session the lamp shows no status. Start a new local task in a supported agent.
For Codex, follow the [Codex setup steps](agents.md#codex-setup) and check that the task appears in BroGlow.

If Terminal cannot find `broglow` or runs another copy, follow the [PATH checks](alternatives.md#path).

Then check, in BroGlow's **Lamps** view:

- **Updates paused** means lamp control is off: click **Enable lamp** (or **Enable lamps**).
- Bluetooth is on and BroGlow is allowed under
  System Settings → Privacy & Security → Bluetooth.
- Not set up yet? Follow [pairing](installation.md#pairing) and wait for **Connected**.
- Expand the intended lamp's row: at 0% **Light brightness** its LEDs stay dark even when
  connected. **Identify** briefly lights that lamp's LEDs one by one
  ([details](installation.md#controls)).
- **Troubleshooting** at the bottom of **Lamps** (or `broglow ble status` in Terminal)
  shows the background service's recent lamp status.

The lamp shows session status only while an awake Mac runs BroGlow: without updates for
10 seconds it goes dark. A bonded lamp stays dark while disconnected once its
disconnect fade has finished and the pairing window is closed.

Want a hand? Paste this prompt into Claude Code or Codex on this Mac.

```text
My BroGlow lamp stays dark. Help me find out why.
Follow {{ repo_url }}/blob/HEAD/site/guide/en/troubleshooting.md and
{{ repo_url }}/blob/HEAD/site/guide/en/hardware.md#troubleshooting-and-service-recovery
(for Codex also {{ repo_url }}/blob/HEAD/site/guide/en/agents.md#codex-setup).
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

<a name="codex-side-approval"></a>

## Known issue: false red while a Codex side chat sends a message

**Open; not fixed.** Reproduced on 2026-11-03 with BroGlow 1.4.1. When a manual
`/side` chat sends a message to the main chat, BroGlow can show **Needs help** and
red even though Codex is not asking the user to do anything. The three observed
waits lasted 3.2–5.0 seconds; these are observations, not a guaranteed time limit.
Check the affected chat in Codex for an actual question or approval prompt.

To reproduce, open `/side` from a local Codex chat and ask it to send a harmless
diagnostic message to the main chat. Watch the side chat's status in BroGlow
during **Sending message to chat**. In all three captured attempts, message
delivery succeeded and the side chat returned to **Working** after the tool
finished, then left live status after its response. Other waiting sessions can
keep the aggregate red throughout, so compare the affected session's status.

### Why this remains unresolved {#codex-side-approval-cause}

The recorded sequence was:

```text
PreToolUse: mcp__codex_app__send_message_to_thread
PermissionRequest: transcript_path=null; approvals_reviewer absent
side chat: Active -> NeedsHelp
PostToolUse: side chat returns to Active
```

Approval hooks run before the approval flow decides whether a person must act.
Saved chats can identify automatic review from their matching rollout turn;
ephemeral side chats have no rollout. Their null transcript initially classifies
them as background, skipping that check; remembered fork provenance later keeps
the side chat visible. The result is the conservative human-approval status.
Removing the background guard alone cannot supply the missing reviewer.

The hook contains `permission_mode`, but the investigation did not record its
value or establish that it distinguishes reviewers. `PermissionRequest` also
lacked `tool_use_id`; future correlation must account for concurrent calls.
See the upstream [hooks contract](https://learn.chatgpt.com/docs/hooks) and
[automatic-review behavior](https://learn.chatgpt.com/docs/sandboxing/auto-review).

### When to revisit {#codex-side-approval-revisit}

Revisit when Codex exposes either an authoritative reviewer for the exact side
chat and turn, or an accessible event that proves a human prompt was shown and
later resolved. New fields, flags or app-server events must be verified against
both automatic and human review. An existing `waitingOnApproval` flag alone has
not been shown to distinguish them, nor has a usable observer connection to the
running desktop server been established in this setup.

A fix must keep automatic forwarding out of **Needs help**, show genuine human
approvals and questions immediately, resolve them correctly, and preserve parent
and unrelated session state. Cover missing, null and empty transcript paths and
concurrent calls. No delay, blanket suppression of side-chat permissions or
exception based only on the tool name has been adopted: each could delay or hide
a genuine request. Temporary diagnostic code and private logs stay outside the
production change.
