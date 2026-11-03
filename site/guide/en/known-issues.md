<a href="../../">Back to the project</a>

<a name="known-issues"></a>

# Known issues

<a name="codex-side-approval"></a>

## False red in Codex side chats

BroGlow may show **Needs help** and turn red during a permission check in a Codex
`/side` chat, even when no user action is required.

**Why unresolved:** The inspected Codex `/side` hook events do not reliably
distinguish automatic checks from requests requiring a person. We need that
distinction to fix this without hiding real requests.
