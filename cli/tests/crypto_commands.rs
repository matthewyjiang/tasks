mod common;

use predicates::prelude::*;
use serde_json::Value;

use common::CliTestEnv;

type CryptoCli = CliTestEnv;

#[test]
fn crypto_encrypt_and_decrypt_task_round_trip() {
    let cli = CryptoCli::new();
    cli.keyed_db_json(&["account", "init"]);
    let task = cli.keyed_db_json(&["task", "create", "encrypted fixture"]);
    let task_id = task["result"]["id"].as_str().unwrap();

    let blob = cli.keyed_db_json(&["crypto", "encrypt-task", task_id]);
    assert!(blob["result"]["ciphertext"].as_array().unwrap().len() > 16);
    let blob_path = cli.temp_path().join("blob.json");
    std::fs::write(&blob_path, serde_json::to_string(&blob).unwrap()).unwrap();

    let decrypted = cli.keyed_db_json(&["crypto", "decrypt-blob", blob_path.to_str().unwrap()]);
    assert_eq!(decrypted["result"]["id"], task_id);
    assert_eq!(decrypted["result"]["title"], "encrypted fixture");

    let raw_blob_path = cli.temp_path().join("raw-blob.json");
    std::fs::write(
        &raw_blob_path,
        serde_json::to_string(&blob["result"]).unwrap(),
    )
    .unwrap();
    let raw_decrypted =
        cli.keyed_db_json(&["crypto", "decrypt-blob", raw_blob_path.to_str().unwrap()]);
    assert_eq!(raw_decrypted["result"]["id"], task_id);
}

#[test]
fn crypto_unwrap_data_key_is_secret_gated() {
    let profile_a = CryptoCli::new();
    let profile_b = CryptoCli::new();
    let public_a = profile_a.keyed_db_json(&["account", "init"])["result"]["public_key"]
        .as_str()
        .unwrap()
        .to_owned();
    let public_b = profile_b.keyed_db_json(&["device", "init-keypair"])["result"]["public_key"]
        .as_str()
        .unwrap()
        .to_owned();
    let wrapped = profile_a.keyed_db_json(&["crypto", "wrap-data-key", "--target", &public_b]);
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
        .keyed_db_json_command()
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

    cli.keyed_db_json_command()
        .args(["crypto", "verify-local"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("local storage error"));

    cli.keyed_db_json(&["account", "init"]);
    let verified = cli.keyed_db_json(&["crypto", "verify-local"]);
    assert_eq!(verified["result"]["data_key_present"], true);
    assert_eq!(verified["result"]["device_private_key_present"], true);
    assert_eq!(verified["result"]["encrypt_decrypt_ok"], true);
}

#[test]
fn crypto_decrypt_rejects_malformed_blob() {
    let cli = CryptoCli::new();
    cli.keyed_db_json(&["account", "init"]);
    let blob_path = cli.temp_path().join("bad.json");
    std::fs::write(&blob_path, "not json").unwrap();

    cli.keyed_db_json_command()
        .args(["crypto", "decrypt-blob", blob_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("serialization error"));
}
