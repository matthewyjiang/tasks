use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxSettings {
    pub server_url: String,
    pub theme: ThemeChoice,
    pub show_completed: bool,
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
            theme: ThemeChoice::Dark,
            show_completed: true,
        };

        write_settings(&path, &settings).unwrap();

        assert_eq!(read_settings(&path).unwrap(), settings);
    }
}
