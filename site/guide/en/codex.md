<a href="../../">Back to the project</a>

# Connect Codex { #codex-setup }

Installing ClawLight adds hooks, but Codex still needs your approval before running them.

1. Open Codex CLI (0.144+) in Terminal. For desktop tasks, use the same user and CODEX_HOME as the app — either its bundled CLI or an installed codex command.
2. Enter /hooks. Review and trust the six ClawLight entries running clawlight codex-hook. Leave other hooks unchanged. Changed hook definitions may need approval again.
3. Send a prompt in a new local Codex task, in the CLI or desktop app. In ClawLight’s session list, check that this task changes from Working to Idle/Paused after the response. Needs you/Needs help appears for a permission request. Other live sessions also affect the lamp’s color.

Existing sidebar chats need a new prompt. If no session appears, finish active work, restart
Codex, and try again. Bluetooth “connected” alone does not verify hooks; desktop delivery
remains unverified until a desktop task passes this check.

This setup covers tasks running on this Mac. Ordinary ChatGPT chats and cloud or remote tasks
do not send events to this Mac’s local ClawLight service.
