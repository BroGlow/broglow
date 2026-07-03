//! Desktop notifications, fired when a session transitions to "needs help".
//!
//! macOS uses `osascript` (no extra dependency); Windows and Linux use the
//! `notify-rust` crate, which targets WinRT toast and the freedesktop
//! notification spec respectively.

#[cfg(target_os = "macos")]
pub fn send_notification(title: &str, message: &str) {
    use std::process::Command;

    // Escape for an AppleScript string literal. Backslash MUST be escaped first,
    // otherwise the backslashes we add for quotes would themselves be doubled.
    // Session names come from user prompts / LLM output, so a stray `\` or `"`
    // would otherwise produce malformed (or injectable) AppleScript.
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"display notification "{}" with title "{}""#,
        escape(message),
        escape(title),
    );

    let _ = Command::new("osascript").arg("-e").arg(&script).spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn send_notification(title: &str, message: &str) {
    // Best effort: a failed (or slow) notification must never take down the
    // TUI. notify_rust's `.show()` is synchronous and can block on a WinRT
    // or D-Bus round-trip (D-Bus timeouts can run up to ~25s on Linux), but
    // the caller here is the TUI's render/event loop, so it must not be made
    // to wait on that. Fire it off on its own thread instead, mirroring the
    // fire-and-forget semantics of the macOS `Command::spawn` path above.
    let title = title.to_string();
    let message = message.to_string();
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&message)
            .show();
    });
}
