use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct CliFixture {
    _temp: TempDir,
    profile: String,
    config: PathBuf,
    db: PathBuf,
    key_dir: PathBuf,
}

impl CliFixture {
    fn new(profile: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        Self {
            _temp: temp,
            profile: profile.to_string(),
            config: root.join("config.json"),
            db: root.join("tasks.db"),
            key_dir: root.join("keys"),
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("taskmanager").unwrap();
        cmd.args([
            "--profile",
            &self.profile,
            "--config",
            self.config.to_str().unwrap(),
            "--db",
            self.db.to_str().unwrap(),
        ])
        .env("TASKMANAGER_INSECURE_KEY_DIR", &self.key_dir);
        cmd
    }
}

#[test]
fn json_output_contains_deterministic_envelope() {
    let output = Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--output", "json", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["result"]["name"], "taskmanager-cli");
    assert_eq!(value["result"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn jsonl_output_emits_one_json_object_per_line() {
    let output = Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--output", "jsonl", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1);
    let value: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(value["result"]["name"], "taskmanager-cli");
}

#[test]
fn table_output_is_human_readable() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--output", "table", "version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("taskmanager-cli "))
        .stdout(predicate::str::contains("{\n").not());
}

#[test]
fn quiet_suppresses_non_result_messages() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--quiet", "--output", "json", "version"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"result\""));
}

#[test]
fn trace_writes_to_stderr_not_stdout() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--trace", "--output", "json", "version"])
        .assert()
        .success()
        .stderr(predicate::str::contains("trace:"))
        .stdout(
            predicate::str::contains("\"result\"").and(predicate::str::contains("trace:").not()),
        );
}

#[test]
fn temp_profile_fixture_creates_isolated_paths() {
    let alpha = CliFixture::new("alpha");
    let beta = CliFixture::new("beta");

    assert_ne!(alpha.config, beta.config);
    assert_ne!(alpha.db, beta.db);
    assert_ne!(alpha.key_dir, beta.key_dir);

    alpha
        .command()
        .args(["--output", "json", "version"])
        .assert()
        .success();
    beta.command()
        .args(["--output", "json", "version"])
        .assert()
        .success();
}
