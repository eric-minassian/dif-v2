use std::process::Command;

const GITHUB_REPO: &str = "eric-minassian/dif-v2";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.strip_prefix('v').unwrap_or(v);
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

pub fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");

    let output = Command::new("curl")
        .args(["-sfL", "-H", "Accept: application/vnd.github+json", &url])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !output.status.success() {
        return Err("Failed to fetch latest release".into());
    }

    let body: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let tag = body["tag_name"]
        .as_str()
        .ok_or("Missing tag_name in release")?;

    if !is_newer(tag, current_version()) {
        return Ok(None);
    }

    let download_url = body["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| {
                    a["name"]
                        .as_str()
                        .is_some_and(|n| n.contains("macos") && n.ends_with(".tar.gz"))
                })
                .and_then(|a| a["browser_download_url"].as_str())
        })
        .ok_or("No macOS asset found in release")?;

    Ok(Some(UpdateInfo {
        version: tag.to_string(),
        download_url: download_url.to_string(),
    }))
}

pub fn download_and_apply(url: &str) -> Result<(), String> {
    let tmp_dir = std::env::temp_dir().join("dif-update");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("mkdir failed: {e}"))?;

    let tarball = tmp_dir.join("Dif-macos.tar.gz");

    // Download
    let output = Command::new("curl")
        .args(["-sfL", "-o"])
        .arg(&tarball)
        .arg(url)
        .output()
        .map_err(|e| format!("curl download failed: {e}"))?;

    if !output.status.success() {
        return Err("Failed to download update".into());
    }

    // Extract
    let output = Command::new("tar")
        .args(["-xzf"])
        .arg(&tarball)
        .arg("-C")
        .arg(&tmp_dir)
        .output()
        .map_err(|e| format!("tar extract failed: {e}"))?;

    if !output.status.success() {
        return Err("Failed to extract update".into());
    }

    let new_app = tmp_dir.join("Dif.app");
    if !new_app.exists() {
        return Err("Dif.app not found in archive".into());
    }

    // Find the current .app bundle by walking up from the running binary
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot determine current exe: {e}"))?;
    let install_dir = find_app_bundle(&current_exe)?;

    let backup = install_dir.with_extension("app.bak");
    let _ = std::fs::remove_dir_all(&backup);

    // Backup current → .bak, move new → current
    std::fs::rename(&install_dir, &backup)
        .map_err(|e| format!("Failed to backup current app: {e}"))?;

    if let Err(e) = copy_dir_recursive(&new_app, &install_dir) {
        // Restore backup on failure
        let _ = std::fs::remove_dir_all(&install_dir);
        let _ = std::fs::rename(&backup, &install_dir);
        return Err(format!("Failed to install update: {e}"));
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&backup);
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // Relaunch
    let _ = Command::new("open").arg("-n").arg(&install_dir).spawn();

    std::process::exit(0);
}

fn find_app_bundle(exe: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let mut path = exe.to_path_buf();
    while let Some(parent) = path.parent() {
        if path.extension().is_some_and(|ext| ext == "app") {
            return Ok(path);
        }
        path = parent.to_path_buf();
    }
    Err("Not running from a .app bundle".into())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    let output = Command::new("cp")
        .args(["-R"])
        .arg(src)
        .arg(dst)
        .output()
        .map_err(|e| format!("cp failed: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    Ok(())
}
