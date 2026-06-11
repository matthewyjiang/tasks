use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[allow(dead_code)]
pub struct CliTestEnv {
    _temp: TempDir,
    pub config: PathBuf,
    pub db: PathBuf,
    pub key_dir: PathBuf,
}

#[allow(dead_code)]
impl CliTestEnv {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("settings.json");
        let db = temp.path().join("tasks.db");
        let key_dir = temp.path().join("keys");
        Self {
            _temp: temp,
            config,
            db,
            key_dir,
        }
    }

    pub fn temp_path(&self) -> &Path {
        self._temp.path()
    }

    pub fn command(&self) -> Command {
        Command::cargo_bin("tsk").unwrap()
    }

    pub fn db_json_command(&self) -> Command {
        let mut cmd = self.command();
        cmd.args(["--db", path_str(&self.db), "--output", "json"]);
        cmd
    }

    pub fn config_json_command(&self) -> Command {
        let mut cmd = self.command();
        cmd.args(["--config", path_str(&self.config), "--output", "json"]);
        cmd
    }

    pub fn keyed_db_json_command(&self) -> Command {
        let mut cmd = self.db_json_command();
        cmd.env("TASKMANAGER_INSECURE_KEY_DIR", &self.key_dir);
        cmd
    }

    pub fn db_json(&self, args: &[&str]) -> Value {
        json_from_success(self.db_json_command().args(args))
    }

    pub fn keyed_db_json(&self, args: &[&str]) -> Value {
        json_from_success(self.keyed_db_json_command().args(args))
    }

    pub fn config_json(&self, args: &[&str]) -> Value {
        json_from_success(self.config_json_command().args(args))
    }
}

pub fn json_from_success(cmd: &mut Command) -> Value {
    let output = cmd.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&output).unwrap()
}

pub fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}
