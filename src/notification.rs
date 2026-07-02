//! Desktop notifications, fired when a session transitions to "needs help".
//!
//! macOS uses `osascript` (no extra dependency); Windows and Linux use the
//! `notify-rust` crate, which targets WinRT toast and the freedesktop
//! notification spec respectively.

#[cfg(target_os = "macos")]
pub fn send_notification(title: &str, message: &str) {
    use std::process::Command;

    let script = format!(
        r#"display notification "{}" with title "{}""#,
        message.replace('"', "\\\""),
        title.replace('"', "\\\""),
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
