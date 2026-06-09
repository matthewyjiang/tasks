use assert_cmd::Command;
use predicates::prelude::*;
use taskmanager_cli::{args::Cli, context::CliContext, error::CliError, output::OutputFormat};

#[test]
fn help_exits_successfully() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Local-first encrypted task manager CLI",
        ));
}

#[test]
fn version_flag_exits_successfully() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_command_exits_with_code_one() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .arg("nope")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn accepts_global_json_output() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--output", "json", "version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\""));
}

#[test]
fn invalid_output_exits_with_code_one() {
    Command::cargo_bin("taskmanager")
        .unwrap()
        .args(["--output", "xml", "version"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn context_resolves_global_flags() {
    let cli = <Cli as clap::Parser>::try_parse_from([
        "taskmanager",
        "--profile",
        "ci",
        "--config",
        "/tmp/settings.json",
        "--db",
        "/tmp/tasks.db",
        "--server",
        "http://127.0.0.1:8080",
        "--output",
        "json",
        "--offline",
        "--quiet",
        "--yes",
        "--trace",
        "version",
    ])
    .unwrap();

    let ctx = CliContext::from_cli(&cli).unwrap();
    assert_eq!(ctx.profile, "ci");
    assert_eq!(
        ctx.config_path.unwrap().to_string_lossy(),
        "/tmp/settings.json"
    );
    assert_eq!(ctx.db_path.unwrap().to_string_lossy(), "/tmp/tasks.db");
    assert_eq!(ctx.server_url.as_deref(), Some("http://127.0.0.1:8080"));
    assert_eq!(ctx.output, OutputFormat::Json);
    assert!(ctx.offline);
    assert!(ctx.quiet);
    assert!(ctx.yes);
    assert!(ctx.trace);
}

#[test]
fn json_error_shape_is_stable() {
    let error = CliError::Network("server unavailable".to_string());
    let value: serde_json::Value = serde_json::from_str(&error.to_json_string()).unwrap();
    assert_eq!(value["error"]["code"], "network_error");
    assert_eq!(
        value["error"]["message"],
        "network error: server unavailable"
    );
    assert!(value["error"].get("details").is_some());
}

#[test]
fn exit_code_mapping_covers_error_classes() {
    assert_eq!(CliError::Input("x".into()).exit_code(), 1);
    assert_eq!(CliError::LocalStorage("x".into()).exit_code(), 2);
    assert_eq!(CliError::Crypto("x".into()).exit_code(), 3);
    assert_eq!(CliError::Network("x".into()).exit_code(), 4);
    assert_eq!(CliError::Conflict("x".into()).exit_code(), 5);
    assert_eq!(CliError::UnsupportedPlatform("x".into()).exit_code(), 6);
}
