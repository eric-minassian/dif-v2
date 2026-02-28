use std::path::PathBuf;
use std::process::Command;

pub fn choose_folder() -> Option<PathBuf> {
    let output = Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Select a Git repository\")",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let path = value.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}
