use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct CryptoCli {
    _temp: TempDir,
    key_dir: std::path::PathBuf,
    db: std::path::PathBuf,
}

impl CryptoCli {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let key_dir = temp.path().join("keys");
        let db = temp.path().join("tasks.db");
        Self {
            _temp: temp,
            key_dir,
            db,
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("taskmanager").unwrap();
        cmd.args(["--output", "json", "--db", self.db.to_str().unwrap()])
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
fn crypto_encrypt_and_decrypt_task_round_trip() {
    let cli = CryptoCli::new();
    cli.json(&["account", "init"]);
    let task = cli.json(&["task", "create", "encrypted fixture"]);
    let task_id = task["result"]["id"].as_str().unwrap();

    let blob = cli.json(&["crypto", "encrypt-task", task_id]);
    assert!(blob["result"]["ciphertext"].as_array().unwrap().len() > 16);
    let blob_path = cli._temp.path().join("blob.json");
    std::fs::write(&blob_path, serde_json::to_string(&blob["result"]).unwrap()).unwrap();

    let decrypted = cli.json(&["crypto", "decrypt-blob", blob_path.to_str().unwrap()]);
    assert_eq!(decrypted["result"]["id"], task_id);
    assert_eq!(decrypted["result"]["title"], "encrypted fixture");
}

#[test]
fn crypto_unwrap_data_key_is_secret_gated() {
    let profile_a = CryptoCli::new();
    let profile_b = CryptoCli::new();
    let public_a = profile_a.json(&["account", "init"])["result"]["public_key"]
        .as_str()
        .unwrap()
        .to_owned();
    let public_b = profile_b.json(&["device", "init-keypair"])["result"]["public_key"]
        .as_str()
        .unwrap()
        .to_owned();
    let wrapped = profile_a.json(&["crypto", "wrap-data-key", "--target", &public_b]);
    let ciphertext = wrapped["result"]["ciphertext"].as_str().unwrap();
    let nonce = wrapped["result"]["nonce"].as_str().unwrap();

    profile_b
        .command()
        .args([
            "crypto",
            "unwrap-data-key",
            "--from",
            &public_a,
            "--ciphertext",
            ciphertext,
            "--nonce",
            nonce,
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("--dangerously-print-secrets"));

    let output = profile_b
        .command()
        .args([
            "--dangerously-print-secrets",
            "crypto",
            "unwrap-data-key",
            "--from",
            &public_a,
            "--ciphertext",
            ciphertext,
            "--nonce",
            nonce,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let unwrapped: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(unwrapped["result"]["hex"].as_str().unwrap().len(), 64);
}

#[test]
fn crypto_verify_local_reports_success_and_missing_keys() {
    let cli = CryptoCli::new();

    cli.command()
        .args(["crypto", "verify-local"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("local storage error"));

    cli.json(&["account", "init"]);
    let verified = cli.json(&["crypto", "verify-local"]);
    assert_eq!(verified["result"]["data_key_present"], true);
    assert_eq!(verified["result"]["device_private_key_present"], true);
    assert_eq!(verified["result"]["encrypt_decrypt_ok"], true);
}

#[test]
fn crypto_decrypt_rejects_malformed_blob() {
    let cli = CryptoCli::new();
    cli.json(&["account", "init"]);
    let blob_path = cli._temp.path().join("bad.json");
    std::fs::write(&blob_path, "not json").unwrap();

    cli.command()
        .args(["crypto", "decrypt-blob", blob_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("serialization error"));
}
