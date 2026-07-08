use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

/// Helper to parse version string (e.g., "v0.1.0" or "0.1.0") into a triple (major, minor, patch)
pub fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
    let clean = version.trim().trim_start_matches('v');
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        let patch = parts[2].parse::<u32>().ok()?;
        Some((major, minor, patch))
    } else {
        None
    }
}

/// Returns true if latest > current
pub fn compare_versions(current: &str, latest: &str) -> bool {
    if let (Some(cur), Some(lat)) = (parse_version(current), parse_version(latest)) {
        if lat.0 > cur.0 {
            return true;
        }
        if lat.0 == cur.0 && lat.1 > cur.1 {
            return true;
        }
        if lat.0 == cur.0 && lat.1 == cur.1 && lat.2 > cur.2 {
            return true;
        }
    }
    false
}

pub fn check_for_updates(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let url = "https://api.github.com/repos/AxelS27/liem-wallpaper/releases/latest";
    let agent = ureq::AgentBuilder::new()
        .user_agent("LiemWallpaperUpdater/1.0")
        .build();

    let response = agent.get(url)
        .call()
        .map_err(|e| format!("Failed to fetch release API: {e}"))?;

    let release: serde_json::Value = response.into_json::<serde_json::Value>()
        .map_err(|e| format!("Failed to parse release JSON: {e}"))?;

    let tag_name = release["tag_name"].as_str()
        .ok_or_else(|| "Missing tag_name in release".to_string())?;

    if compare_versions(current_version, tag_name) {
        // Find lw-setup.exe in assets
        if let Some(assets) = release["assets"].as_array() {
            for asset in assets {
                if let Some(name) = asset["name"].as_str() {
                    if name.eq_ignore_ascii_case("lw-setup.exe") {
                        if let Some(download_url) = asset["browser_download_url"].as_str() {
                            return Ok(Some(UpdateInfo {
                                version: tag_name.to_string(),
                                download_url: download_url.to_string(),
                            }));
                        }
                    }
                }
            }
        }
        return Err("lw-setup.exe asset not found in latest release".to_string());
    }

    Ok(None)
}

pub fn download_and_run_installer(url: &str) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .user_agent("LiemWallpaperUpdater/1.0")
        .build();

    let response = agent.get(url)
        .call()
        .map_err(|e| format!("Failed to download update: {e}"))?;

    let mut reader = response.into_reader();
    let mut file_bytes = Vec::new();
    std::io::copy(&mut reader, &mut file_bytes)
        .map_err(|e| format!("Failed to read update stream: {e}"))?;

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("lw-setup-update.exe");

    let mut file = File::create(&temp_path)
        .map_err(|e| format!("Failed to create temporary installer file: {e}"))?;

    file.write_all(&file_bytes)
        .map_err(|e| format!("Failed to write temporary installer file: {e}"))?;

    // Spawn the installer in the background
    std::process::Command::new(temp_path)
        .spawn()
        .map_err(|e| format!("Failed to launch installer: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("  v12.34.56  "), Some((12, 34, 56)));
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_version_comparison() {
        // Newer patches
        assert!(compare_versions("0.1.0", "0.1.1"));
        assert!(compare_versions("v0.1.0", "v0.1.5"));
        
        // Newer minor versions
        assert!(compare_versions("0.1.0", "0.2.0"));
        assert!(compare_versions("0.1.9", "0.2.0"));
        
        // Newer major versions
        assert!(compare_versions("0.9.9", "1.0.0"));
        assert!(compare_versions("v1.5.0", "v2.0.0"));

        // Equal or older versions should return false
        assert!(!compare_versions("0.1.0", "0.1.0"));
        assert!(!compare_versions("v1.2.3", "v1.2.3"));
        assert!(!compare_versions("0.2.0", "0.1.0"));
        assert!(!compare_versions("1.0.0", "0.9.9"));
    }
}
