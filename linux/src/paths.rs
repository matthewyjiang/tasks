use std::path::PathBuf;

use directories::ProjectDirs;
use thiserror::Error;

pub const APP_ID: &str = "io.github.matthewyjiang.tsk";
pub const APP_NAME: &str = "tsk";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub database_path: PathBuf,
    pub settings_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("could not resolve Linux user directories")]
    ProjectDirsUnavailable,
    #[error("failed to create app directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn resolve_paths() -> Result<AppPaths, PathError> {
    let project_dirs = ProjectDirs::from("io.github", "matthewyjiang", APP_NAME)
        .ok_or(PathError::ProjectDirsUnavailable)?;

    let data_dir = project_dirs.data_dir().to_path_buf();
    let config_dir = project_dirs.config_dir().to_path_buf();
    let database_path = std::env::var_os("TSK_LINUX_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("tasks.sqlite3"));
    let settings_path = std::env::var_os("TSK_LINUX_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| config_dir.join("settings.json"));

    create_parent_or_dir(&data_dir)?;
    create_parent_or_dir(&config_dir)?;
    if let Some(parent) = database_path.parent() {
        create_parent_or_dir(parent)?;
    }
    if let Some(parent) = settings_path.parent() {
        create_parent_or_dir(parent)?;
    }

    Ok(AppPaths {
        data_dir,
        config_dir,
        database_path,
        settings_path,
    })
}

fn create_parent_or_dir(path: impl Into<PathBuf>) -> Result<(), PathError> {
    let path = path.into();
    std::fs::create_dir_all(&path).map_err(|source| PathError::CreateDir { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_overrides_database_and_config_paths() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("db.sqlite3");
        let config = temp.path().join("settings.json");
        std::env::set_var("TSK_LINUX_DB", &db);
        std::env::set_var("TSK_LINUX_CONFIG", &config);

        let paths = resolve_paths().unwrap();

        assert_eq!(paths.database_path, db);
        assert_eq!(paths.settings_path, config);
        assert!(paths.data_dir.exists());
        assert!(paths.config_dir.exists());

        std::env::remove_var("TSK_LINUX_DB");
        std::env::remove_var("TSK_LINUX_CONFIG");
    }
}
