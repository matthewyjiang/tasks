use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct TaskCli {
    _temp: TempDir,
    db: std::path::PathBuf,
}

impl TaskCli {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("tasks.db");
        Self { _temp: temp, db }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("taskmanager").unwrap();
        cmd.args(["--db", self.db.to_str().unwrap(), "--output", "json"]);
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
fn creating_a_task_returns_generated_dirty_task() {
    let cli = TaskCli::new();

    let value = cli.json(&[
        "task",
        "create",
        "--title",
        "write tests",
        "--body",
        "cover cli",
        "--due",
        "123",
    ]);

    assert!(value["result"]["id"]
        .as_str()
        .unwrap()
        .parse::<uuid::Uuid>()
        .is_ok());
    assert_eq!(value["result"]["title"], "write tests");
    assert_eq!(value["result"]["body"], "cover cli");
    assert_eq!(value["result"]["due_at"], 123);
    assert_eq!(value["result"]["status"], "inbox");
    assert_eq!(value["result"]["dirty"], true);
}

#[test]
fn creating_a_task_accepts_human_due_date() {
    let cli = TaskCli::new();

    let value = cli.json(&["task", "create", "Buy milk", "--due", "tomorrow"]);

    assert_eq!(value["result"]["title"], "Buy milk");
    assert!(value["result"]["due_at"].as_i64().unwrap() > 0);
}

#[test]
fn creating_a_task_accepts_project_and_tags() {
    let cli = TaskCli::new();
    let project_id = "018f6f4a-c9f4-7724-91ef-2f7b38a62601";

    let value = cli.json(&[
        "task",
        "create",
        "--title",
        "tagged",
        "--project-id",
        project_id,
        "--tag",
        "work",
        "--tag",
        "urgent",
    ]);

    assert_eq!(value["result"]["title"], "tagged");
    assert_eq!(value["result"]["project_id"], project_id);
    assert_eq!(
        value["result"]["tags"],
        serde_json::json!(["work", "urgent"])
    );
    assert_eq!(value["result"]["dirty"], true);
}

#[test]
fn creating_a_task_keeps_positional_title_compatibility() {
    let cli = TaskCli::new();

    let value = cli.json(&["task", "create", "positional title"]);

    assert_eq!(value["result"]["title"], "positional title");
}

#[test]
fn getting_an_existing_task_returns_the_same_task() {
    let cli = TaskCli::new();
    let created = cli.json(&["task", "create", "read", "--body", "docs"]);
    let id = created["result"]["id"].as_str().unwrap();

    let fetched = cli.json(&["task", "get", id]);

    assert_eq!(fetched["result"], created["result"]);
}

#[test]
fn getting_a_missing_task_exits_with_not_found_error() {
    let cli = TaskCli::new();

    cli.command()
        .args(["task", "get", "018f6f4a-c9f4-7724-91ef-2f7b38a62600"])
        .assert()
        .failure()
        .code(1)
        .stderr(
            predicate::str::contains("\"error\"").and(predicate::str::contains("task not found")),
        );
}

#[test]
fn updating_patchable_fields_persists_and_marks_dirty() {
    let cli = TaskCli::new();
    let created = cli.json(&["task", "create", "draft"]);
    let id = created["result"]["id"].as_str().unwrap();
    let project_id = "018f6f4a-c9f4-7724-91ef-2f7b38a62601";

    let updated = cli.json(&[
        "task",
        "update",
        id,
        "--title",
        "done title",
        "--body",
        "done body",
        "--due-at",
        "456",
        "--status",
        "in-progress",
        "--project-id",
        project_id,
        "--tags",
        "work,urgent",
    ]);

    assert_eq!(updated["result"]["title"], "done title");
    assert_eq!(updated["result"]["body"], "done body");
    assert_eq!(updated["result"]["due_at"], 456);
    assert_eq!(updated["result"]["status"], "in_progress");
    assert_eq!(updated["result"]["project_id"], project_id);
    assert_eq!(
        updated["result"]["tags"],
        serde_json::json!(["work", "urgent"])
    );
    assert_eq!(updated["result"]["dirty"], true);
}

#[test]
fn conflicting_update_flags_are_rejected() {
    let cli = TaskCli::new();
    let created = cli.json(&["task", "create", "draft"]);
    let id = created["result"]["id"].as_str().unwrap();
    let project_id = "018f6f4a-c9f4-7724-91ef-2f7b38a62601";

    cli.command()
        .args(["task", "update", id, "--due-at", "123", "--clear-due-at"])
        .assert()
        .failure()
        .code(1);
    cli.command()
        .args([
            "task",
            "update",
            id,
            "--project-id",
            project_id,
            "--clear-project-id",
        ])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn deleting_a_task_creates_tombstone_not_hard_delete() {
    let cli = TaskCli::new();
    let created = cli.json(&["task", "create", "delete me"]);
    let id = created["result"]["id"].as_str().unwrap();

    let deleted = cli.json(&["task", "delete", id]);
    assert_eq!(deleted["result"]["deleted"], true);

    let fetched = cli.json(&["task", "get", id]);
    assert_eq!(fetched["result"]["deleted"], true);
    assert_eq!(fetched["result"]["dirty"], true);
}

#[test]
fn listing_supports_filters_and_sorting() {
    let cli = TaskCli::new();
    let first = cli.json(&["task", "create", "first", "--due-at", "100"]);
    let first_id = first["result"]["id"].as_str().unwrap();
    let second = cli.json(&["task", "create", "second", "--due-at", "200"]);
    let second_id = second["result"]["id"].as_str().unwrap();
    let project_id = "018f6f4a-c9f4-7724-91ef-2f7b38a62601";
    cli.json(&["task", "complete", second_id]);
    cli.json(&[
        "task",
        "update",
        first_id,
        "--project-id",
        project_id,
        "--tags",
        "work,urgent",
    ]);

    let done = cli.json(&["task", "list", "--status", "done"]);
    assert_eq!(done["result"].as_array().unwrap().len(), 1);
    assert_eq!(done["result"][0]["id"], second_id);

    let project = cli.json(&["task", "list", "--project-id", project_id]);
    assert_eq!(project["result"].as_array().unwrap().len(), 1);
    assert_eq!(project["result"][0]["id"], first_id);

    let tagged = cli.json(&["task", "list", "--tag", "work", "--tag", "urgent"]);
    assert_eq!(tagged["result"].as_array().unwrap().len(), 1);
    assert_eq!(tagged["result"][0]["id"], first_id);

    let missing_tag = cli.json(&["task", "list", "--tag", "missing"]);
    assert_eq!(missing_tag["result"].as_array().unwrap().len(), 0);

    let due = cli.json(&[
        "task",
        "list",
        "--due-after",
        "50",
        "--due-before",
        "150",
        "--sort",
        "due-at-asc",
    ]);
    assert_eq!(due["result"].as_array().unwrap().len(), 1);
    assert_eq!(due["result"][0]["id"], first_id);

    for sort in [
        "updated-at-desc",
        "updated-at-asc",
        "due-at-asc",
        "due-at-desc",
        "created-at-asc",
        "created-at-desc",
    ] {
        let sorted = cli.json(&["task", "list", "--sort", sort]);
        assert_eq!(sorted["result"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn search_returns_matches_from_title_and_body() {
    let cli = TaskCli::new();
    let title_match = cli.json(&["task", "create", "alpha needle"]);
    let body_match = cli.json(&["task", "create", "beta", "--body", "needle body"]);
    cli.json(&["task", "create", "gamma"]);

    let found = cli.json(&["task", "search", "needle"]);
    let ids: Vec<&str> = found["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["id"].as_str().unwrap())
        .collect();

    assert!(ids.contains(&title_match["result"]["id"].as_str().unwrap()));
    assert!(ids.contains(&body_match["result"]["id"].as_str().unwrap()));
    assert_eq!(ids.len(), 2);
}

#[test]
fn search_treats_punctuation_as_literal_text() {
    let cli = TaskCli::new();
    let cpp = cli.json(&["task", "create", "learn C++"]);
    let hyphen = cli.json(&["task", "create", "foo-bar"]);

    let cpp_found = cli.json(&["task", "search", "C++"]);
    let cpp_ids: Vec<&str> = cpp_found["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["id"].as_str().unwrap())
        .collect();
    assert!(cpp_ids.contains(&cpp["result"]["id"].as_str().unwrap()));

    let hyphen_found = cli.json(&["task", "search", "foo-bar"]);
    let hyphen_ids: Vec<&str> = hyphen_found["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["id"].as_str().unwrap())
        .collect();
    assert!(hyphen_ids.contains(&hyphen["result"]["id"].as_str().unwrap()));
}

#[test]
fn complete_and_reopen_map_to_status_patches() {
    let cli = TaskCli::new();
    let created = cli.json(&["task", "create", "toggle"]);
    let id = created["result"]["id"].as_str().unwrap();

    let done = cli.json(&["task", "complete", id]);
    assert_eq!(done["result"]["status"], "done");

    let reopened = cli.json(&["task", "reopen", id]);
    assert_eq!(reopened["result"]["status"], "inbox");
}

#[test]
fn task_commands_work_offline() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("tasks.db");
    let output = Command::cargo_bin("taskmanager")
        .unwrap()
        .args([
            "--offline",
            "--db",
            db.to_str().unwrap(),
            "--output",
            "json",
            "task",
            "create",
            "offline task",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["result"]["title"], "offline task");
}
