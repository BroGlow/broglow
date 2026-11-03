<a href="../../">Back to the project</a>

<a name="known-issues"></a>

# Known issues

Confirmed issues that remain unresolved, with reproduction steps and conditions for revisiting a fix.

<a name="codex-side-approval"></a>

## False red during Codex side-chat permission checks

**Open; not fixed.** Reproduced on 2026-11-03 with BroGlow 1.4.1. When a manual
`/side` chat checks permission for an operation, BroGlow can show **Needs help**
and red even though Codex is not asking the user to do anything. This was confirmed
while sending a message to the main chat. The three observed
waits lasted 3.2–5.0 seconds; these are observations, not a guaranteed time limit.
Check the affected chat in Codex for an actual question or approval prompt.

To reproduce, open `/side` from a local Codex chat and ask it to send a harmless
diagnostic message to the main chat. Watch the side chat's status in BroGlow
during **Sending message to chat**. In all three captured attempts, message
delivery succeeded and the side chat returned to **Working** after the tool
finished, then left live status after its response. Other waiting sessions can
keep the aggregate red throughout, so compare the affected session's status.

An additional screenshot report involved a local edit and commit in another
`/side` chat. On follow-up that session was already **Done**, with no pending
question. No hook trace was available for its red interval, so the exact trigger
and whether it had the same cause remain unconfirmed.

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
This permission mapping is not restricted to message forwarding: another tool's
`PermissionRequest` can take the same path.

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

A fix must keep automatic permission checks, including forwarding and local tools,
out of **Needs help**, show genuine human
approvals and questions immediately, resolve them correctly, and preserve parent
and unrelated session state. Cover missing, null and empty transcript paths and
concurrent calls. No delay, blanket suppression of side-chat permissions or
exception based only on the tool name has been adopted: each could delay or hide
a genuine request. Temporary diagnostic code and private logs stay outside the
production change.
