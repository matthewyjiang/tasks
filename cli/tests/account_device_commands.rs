use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct KeyCli {
    _temp: TempDir,
    key_dir: std::path::PathBuf,
    config: std::path::PathBuf,
}

impl KeyCli {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let key_dir = temp.path().join("keys");
        let config = temp.path().join("settings.json");
        Self {
            _temp: temp,
            key_dir,
            config,
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("taskmanager").unwrap();
        cmd.args([
            "--output",
            "json",
            "--config",
            self.config.to_str().unwrap(),
        ])
        .env("TASKMANAGER_INSECURE_KEY_DIR", &self.key_dir);
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
fn configure_initializes_account_settings_and_tokens_non_interactively() {
    let cli = KeyCli::new();

    let value = cli.json(&[
        "configure",
        "--server-url",
        "https://api.example.com",
        "--access-token",
        "access",
        "--refresh-token",
        "refresh",
    ]);

    assert_eq!(value["result"]["account_initialized"], true);
    assert_eq!(value["result"]["server_url"], "https://api.example.com");
    assert_eq!(value["result"]["access_token_stored"], true);
    assert_eq!(value["result"]["refresh_token_stored"], true);
    assert_eq!(
        cli.json(&["settings", "get", "server_url"])["result"],
        "https://api.example.com"
    );

    let rerun = cli.json(&[
        "configure",
        "--server-url",
        "https://next.example.com",
        "--access-token",
        "next-access",
    ]);
    assert_eq!(rerun["result"]["account_initialized"], false);
    assert_eq!(rerun["result"]["refresh_token_stored"], false);
}

#[test]
fn configure_accepts_interactive_input_on_stdin() {
    let cli = KeyCli::new();

    let output = cli
        .command()
        .args(["configure"])
        .write_stdin("https://api.example.com\naccess\n\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(value["result"]["server_url"], "https://api.example.com");
    assert_eq!(value["result"]["refresh_token_stored"], false);
}

#[test]
fn account_init_initializes_local_keys_and_returns_public_key() {
    let cli = KeyCli::new();

    let value = cli.json(&["account", "init"]);

    let public_key = value["result"]["public_key"].as_str().unwrap();
    assert!(!public_key.is_empty());
    assert!(!public_key.contains("private"));
}

#[test]
fn account_init_rerun_returns_stable_already_exists_error() {
    let cli = KeyCli::new();

    cli.json(&["account", "init"]);

    cli.command()
        .args(["account", "init"])
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("account already exists"));
}

#[test]
fn auth_login_and_logout_use_platform_key_store_without_deleting_tasks() {
    let cli = KeyCli::new();
    let db = cli._temp.path().join("tasks.db");

    cli.command()
        .args([
            "--db",
            db.to_str().unwrap(),
            "task",
            "create",
            "keep local task",
        ])
        .assert()
        .success();
    let login = cli.json(&[
        "auth",
        "login",
        "--access-token",
        "access-secret",
        "--refresh-token",
        "refresh-secret",
    ]);
    assert_eq!(login["result"]["stored"], true);

    let logout = cli.json(&["auth", "logout"]);
    assert_eq!(logout["result"]["logged_out"], true);

    cli.command()
        .args(["--db", db.to_str().unwrap(), "task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("keep local task"));
}

#[test]
fn device_init_keypair_prints_only_public_key() {
    let cli = KeyCli::new();

    let output = cli
        .command()
        .args(["device", "init-keypair"])
        .assert()
        .success()
        .stdout(predicate::str::contains("private").not())
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert!(!value["result"]["public_key"].as_str().unwrap().is_empty());
}

#[test]
fn wrap_on_profile_a_and_unwrap_on_profile_b_recovers_data_key() {
    let profile_a = KeyCli::new();
    let profile_b = KeyCli::new();

    let account = profile_a.json(&["account", "init"]);
    let public_a = account["result"]["public_key"].as_str().unwrap();
    let device_b = profile_b.json(&["device", "init-keypair"]);
    let public_b = device_b["result"]["public_key"].as_str().unwrap();

    let wrapped_for_b = profile_a.json(&["device", "wrap-key", "--target", public_b]);
    let ciphertext = wrapped_for_b["result"]["ciphertext"].as_str().unwrap();
    let nonce = wrapped_for_b["result"]["nonce"].as_str().unwrap();

    let unwrapped = profile_b.json(&[
        "device",
        "unwrap-key",
        "--from",
        public_a,
        "--ciphertext",
        ciphertext,
        "--nonce",
        nonce,
    ]);
    assert_eq!(unwrapped["result"]["stored"], true);

    profile_b
        .command()
        .args(["device", "wrap-key", "--target", public_a])
        .assert()
        .success();
}

#[test]
fn malformed_peer_public_key_fails_with_crypto_exit_code() {
    let cli = KeyCli::new();
    cli.json(&["account", "init"]);

    cli.command()
        .args(["device", "wrap-key", "--target", "00"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("crypto error"));
}

#[test]
fn non_ascii_hex_fails_without_panicking() {
    let cli = KeyCli::new();
    cli.json(&["account", "init"]);

    cli.command()
        .args(["device", "wrap-key", "--target", "éé"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("hex value must contain ASCII"));
}
