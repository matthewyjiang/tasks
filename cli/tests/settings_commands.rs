use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct SettingsCli {
    _temp: TempDir,
    config: std::path::PathBuf,
}

impl SettingsCli {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("settings.json");
        Self {
            _temp: temp,
            config,
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("taskmanager").unwrap();
        cmd.args([
            "--config",
            self.config.to_str().unwrap(),
            "--output",
            "json",
        ]);
        cmd
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self
            .command()
            .args(args)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice(&output).unwrap()
    }
}

#[test]
fn settings_get_returns_defaults_before_file_exists() {
    let cli = SettingsCli::new();

    let output = cli.json(&["settings", "get"]);
    assert_eq!(output["result"]["server_url"], "");
    assert_eq!(output["result"]["auth_method"], "password");
    assert_eq!(output["result"]["language"], "en");
    assert_eq!(output["result"]["last_sync_cursor"], 0);
}

#[test]
fn settings_set_validates_and_persists_supported_keys() {
    let cli = SettingsCli::new();

    cli.json(&["settings", "set", "server_url", "https://api.example.com"]);
    cli.json(&["settings", "set", "auth_method", "pin"]);
    cli.json(&["settings", "set", "language", "es"]);
    cli.json(&["settings", "set", "last_sync_cursor", "42"]);

    assert_eq!(
        cli.json(&["settings", "get", "server_url"])["result"],
        "https://api.example.com"
    );
    assert_eq!(
        cli.json(&["settings", "get", "auth_method"])["result"],
        "pin"
    );
    assert_eq!(cli.json(&["settings", "get", "language"])["result"], "es");
    assert_eq!(
        cli.json(&["settings", "get", "last_sync_cursor"])["result"],
        42
    );
}

#[test]
fn settings_reject_invalid_values() {
    let cli = SettingsCli::new();

    cli.command()
        .args(["settings", "set", "server_url", "ftp://example.com"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("server_url"));

    cli.command()
        .args(["settings", "set", "auth_method", "magic"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("auth_method"));

    cli.command()
        .args(["settings", "set", "last_sync_cursor", "not-an-int"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("last_sync_cursor"));
}

#[test]
fn settings_plaintext_sync_excludes_device_local_cursor() {
    let cli = SettingsCli::new();

    cli.json(&["settings", "set", "server_url", "https://api.example.com"]);
    cli.json(&["settings", "set", "last_sync_cursor", "99"]);

    let payload = cli.json(&["settings", "pull-plaintext"]);
    assert_eq!(payload["result"]["server_url"], "https://api.example.com");
    assert!(payload["result"].get("last_sync_cursor").is_none());

    cli.json(&[
        "settings",
        "push-plaintext",
        r#"{"schema_version":1,"server_url":"https://next.example.com","auth_method":"biometric","language":"fr"}"#,
    ]);
    let settings = cli.json(&["settings", "get"]);
    assert_eq!(settings["result"]["server_url"], "https://next.example.com");
    assert_eq!(settings["result"]["auth_method"], "biometric");
    assert_eq!(settings["result"]["language"], "fr");
    assert_eq!(settings["result"]["last_sync_cursor"], 99);
}

#[test]
fn settings_push_plaintext_rejects_invalid_synced_values() {
    let cli = SettingsCli::new();

    for payload in [
        r#"{"schema_version":1,"server_url":"ftp://bad","auth_method":"password","language":"en"}"#,
        r#"{"schema_version":1,"server_url":"https://api.example.com","auth_method":"password","language":""}"#,
        r#"{"schema_version":99,"server_url":"https://api.example.com","auth_method":"password","language":"en"}"#,
    ] {
        cli.command()
            .args(["settings", "push-plaintext", payload])
            .assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains("input error"));
    }

    let settings = cli.json(&["settings", "get"]);
    assert_eq!(settings["result"]["server_url"], "");
    assert_eq!(settings["result"]["language"], "en");
}

#[test]
fn settings_migrate_writes_default_file() {
    let cli = SettingsCli::new();

    cli.json(&["settings", "migrate"]);

    assert!(cli.config.exists());
}
