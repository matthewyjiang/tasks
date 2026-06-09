use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn generates_shell_completions_for_supported_shells() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = Command::cargo_bin("tsk")
            .unwrap()
            .args(["generate", "completion", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("tsk"))
            .get_output()
            .stdout
            .clone();
        assert!(!output.is_empty(), "empty completion for {shell}");
    }
}

#[test]
fn generates_man_page() {
    Command::cargo_bin("tsk")
        .unwrap()
        .args(["generate", "man"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tsk"))
        .stdout(predicate::str::contains("encrypted task manager CLI"));
}
