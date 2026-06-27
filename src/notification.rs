use std::process::Command;

pub fn send_macos_notification(title: &str, message: &str) {
    let script = format!(
        r#"display notification "{}" with title "{}""#,
        message.replace('"', "\\\""),
        title.replace('"', "\\\""),
    );

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .spawn();
}
