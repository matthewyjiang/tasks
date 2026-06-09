use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct SyncCli {
    _temp: TempDir,
    db: std::path::PathBuf,
}

impl SyncCli {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("tasks.db");
        Self { _temp: temp, db }
    }

    fn command(&self) -> Command {
        Self::command_with_db(&self.db)
    }

    fn command_with_db(db: &std::path::Path) -> Command {
        let mut cmd = Command::cargo_bin("taskmanager").unwrap();
        cmd.args(["--db", db.to_str().unwrap(), "--output", "json"]);
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
fn sync_status_reports_dirty_count_retry_depth_and_cursor() {
    let cli = SyncCli::new();

    let initial = cli.json(&["sync", "status"]);
    assert_eq!(initial["result"]["dirty_count"], 0);
    assert_eq!(initial["result"]["retry_queue_depth"], 0);
    assert_eq!(initial["result"]["cursor"], 0);

    let created = cli.json(&["task", "create", "sync me"]);
    let task_id = created["result"]["id"].as_str().unwrap();
    cli.json(&["sync", "retry", task_id]);

    let status = cli.json(&["sync", "status"]);
    assert_eq!(status["result"]["dirty_count"], 1);
    assert_eq!(status["result"]["retry_queue_depth"], 1);
    assert_eq!(status["result"]["cursor"], 0);
}

#[test]
fn sync_retry_updates_queue_state_for_selected_task() {
    let cli = SyncCli::new();
    let created = cli.json(&["task", "create", "retry me"]);
    let task_id = created["result"]["id"].as_str().unwrap();

    let first = cli.json(&["sync", "retry", task_id]);
    assert_eq!(first["result"]["task_id"], task_id);
    assert_eq!(first["result"]["attempt"], 1);
    assert!(first["result"]["next_retry"].as_i64().unwrap() > 0);

    let second = cli.json(&["sync", "retry", task_id]);
    assert_eq!(second["result"]["attempt"], 2);
}

#[test]
fn sync_retry_rejects_unknown_tasks_without_queuing_orphan_entries() {
    let cli = SyncCli::new();
    cli.json(&["task", "create", "known task"]);

    cli.command()
        .args(["sync", "retry", "00000000-0000-0000-0000-000000000000"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("task not found"));

    let status = cli.json(&["sync", "status"]);
    assert_eq!(status["result"]["retry_queue_depth"], 0);
}

#[test]
fn server_sync_commands_require_server_url() {
    let cli = SyncCli::new();

    cli.command()
        .args(["sync", "push"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("--server is required"));
}

#[test]
fn server_sync_commands_without_server_do_not_open_or_create_database() {
    let temp = tempfile::tempdir().unwrap();
    let blocked_parent = temp.path().join("not-a-directory");
    std::fs::write(&blocked_parent, b"file").unwrap();
    let invalid_db = blocked_parent.join("tasks.db");

    SyncCli::command_with_db(&invalid_db)
        .args(["sync", "push"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("--server is required"));
}
