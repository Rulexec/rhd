use std::process::Command;

pub fn send_notification(title: &str, message: &str, open_url: Option<&str>) {
    if try_terminal_notifier(title, message, open_url) {
        return;
    }
    eprintln!("terminal-notifier not available, falling back to osascript");
    fallback_osascript(title, message);
}

fn try_terminal_notifier(title: &str, message: &str, open_url: Option<&str>) -> bool {
    let mut cmd = Command::new("terminal-notifier");
    cmd.arg("-title").arg(title);
    cmd.arg("-message").arg(message);
    cmd.arg("-group").arg("rhd-notifications");
    
    if let Some(url) = open_url {
        cmd.arg("-open").arg(url);
    }
    
    cmd.output().is_ok()
}

fn fallback_osascript(title: &str, message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        message.replace("\"", "\\\""),
        title.replace("\"", "\\\"")
    );
    
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();
}

