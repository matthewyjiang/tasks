use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxSettings {
    pub server_url: String,
    #[serde(default)]
    pub sync_email: String,
    pub theme: ThemeChoice,
    pub show_completed: bool,
    #[serde(default)]
    pub sync_status: SyncStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_attempt_at: Option<i64>,
    pub last_success_at: Option<i64>,
    #[serde(default)]
    pub last_pushed: usize,
    #[serde(default)]
    pub last_pulled: usize,
    #[serde(default)]
    pub last_failed: usize,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub pending_retries: usize,
    #[serde(default)]
    pub dirty_count: usize,
    #[serde(default)]
    pub cursor: i64,
    #[serde(default)]
    pub conflicts: usize,
    #[serde(default = "default_network_available")]
    pub network_available: bool,
    #[serde(default = "default_backend_available")]
    pub backend_available: bool,
}

fn default_network_available() -> bool {
    true
}

fn default_backend_available() -> bool {
    true
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            last_attempt_at: None,
            last_success_at: None,
            last_pushed: 0,
            last_pulled: 0,
            last_failed: 0,
            last_error: String::new(),
            pending_retries: 0,
            dirty_count: 0,
            cursor: 0,
            conflicts: 0,
            network_available: true,
            backend_available: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

pub fn read_settings(path: &std::path::Path) -> std::io::Result<LinuxSettings> {
    if !path.exists() {
        return Ok(LinuxSettings::default());
    }
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

pub fn write_settings(path: &std::path::Path, settings: &LinuxSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_json_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let settings = LinuxSettings {
            server_url: "https://example.test".to_owned(),
            sync_email: "user@example.test".to_owned(),
            theme: ThemeChoice::Dark,
            show_completed: true,
            sync_status: SyncStatus::default(),
        };

        write_settings(&path, &settings).unwrap();

        assert_eq!(read_settings(&path).unwrap(), settings);
    }
}
