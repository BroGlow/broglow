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
    // Best effort: a failed notification must never take down the TUI.
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(message)
        .show();
}
