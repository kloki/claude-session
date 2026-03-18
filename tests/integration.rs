use std::{fs, path::Path};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("claude-sessions").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn send_event(home: &Path, session_id: &str, event: &str) {
    let input = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": event,
    });
    cmd(home)
        .arg("process-webhook")
        .write_stdin(input.to_string())
        .assert()
        .success();
}

fn send_event_with_cwd(home: &Path, session_id: &str, event: &str, cwd: &str) {
    let input = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": event,
        "cwd": cwd,
    });
    cmd(home)
        .arg("process-webhook")
        .write_stdin(input.to_string())
        .assert()
        .success();
}

fn read_store(home: &std::path::Path) -> Value {
    let path = home.join(".claude_sessions");
    let contents = fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

fn waybar_output(home: &std::path::Path) -> Value {
    let output = cmd(home)
        .arg("waybar")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn session_start_creates_active_session() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn session_end_removes_session() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "SessionEnd");

    let store = read_store(home.path());
    assert!(store["sessions"]["sess-1"].is_null());
}

#[test]
fn stop_marks_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "Stop");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
}

#[test]
fn notification_marks_waiting_for_input() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "Notification");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "WaitingForInput");
}

#[test]
fn user_prompt_submit_marks_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "Notification");
    send_event(home.path(), "sess-1", "UserPromptSubmit");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn upsert_creates_session_on_missed_start() {
    let home = TempDir::new().unwrap();
    // Skip SessionStart, go straight to Stop
    send_event(home.path(), "sess-1", "Stop");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
    assert!(store["sessions"]["sess-1"]["started_at"].is_string());
}

#[test]
fn clear_removes_state_file() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    assert!(home.path().join(".claude_sessions").exists());

    cmd(home.path()).arg("clear").assert().success();
    assert!(!home.path().join(".claude_sessions").exists());
}

#[test]
fn clear_succeeds_when_no_state_file() {
    let home = TempDir::new().unwrap();
    cmd(home.path()).arg("clear").assert().success();
}

#[test]
fn waybar_empty_store() {
    let home = TempDir::new().unwrap();
    let out = waybar_output(home.path());
    assert_eq!(out["text"], "0");
    assert_eq!(out["tooltip"], "");
    assert_eq!(out["class"], "claude-idle");
}

#[test]
fn waybar_counts_all_sessions() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-2", "SessionStart");
    send_event(home.path(), "sess-2", "Stop");

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "2");
}

#[test]
fn waybar_class_waiting_takes_priority() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-2", "SessionStart");
    send_event(home.path(), "sess-2", "Notification");

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-waiting");
}

#[test]
fn waybar_class_active_over_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-2", "SessionStart");
    send_event(home.path(), "sess-2", "Stop");

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-active");
}

#[test]
fn waybar_class_idle_when_all_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "Stop");

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-idle");
}

#[test]
fn waybar_tooltip_truncates_long_ids() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "abcdefghij-long-id", "SessionStart");

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("abcdefgh"));
    assert!(!tooltip.contains("abcdefghij"));
}

#[test]
fn waybar_tooltip_keeps_short_ids() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "short", "SessionStart");

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("short: Thinking"));
}

#[test]
fn waybar_cleans_stale_sessions() {
    let home = TempDir::new().unwrap();
    // Create a session, then manually backdate its updated_at
    send_event(home.path(), "old-sess", "SessionStart");

    let path = home.path().join(".claude_sessions");
    let contents = fs::read_to_string(&path).unwrap();
    let mut store: Value = serde_json::from_str(&contents).unwrap();

    // Set updated_at to 25 hours ago
    let old_time = chrono::Utc::now() - chrono::Duration::hours(25);
    store["sessions"]["old-sess"]["updated_at"] = Value::String(old_time.to_rfc3339());
    fs::write(&path, serde_json::to_string_pretty(&store).unwrap()).unwrap();

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "0");
}

#[test]
fn multiple_sessions_full_lifecycle() {
    let home = TempDir::new().unwrap();

    send_event(home.path(), "s1", "SessionStart");
    send_event(home.path(), "s2", "SessionStart");
    send_event(home.path(), "s3", "SessionStart");

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "3");

    send_event(home.path(), "s1", "Notification");
    send_event(home.path(), "s2", "Stop");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["s1"]["state"], "WaitingForInput");
    assert_eq!(store["sessions"]["s2"]["state"], "Idle");
    assert_eq!(store["sessions"]["s3"]["state"], "Active");

    send_event(home.path(), "s1", "UserPromptSubmit");
    send_event(home.path(), "s2", "SessionEnd");
    send_event(home.path(), "s3", "SessionEnd");

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "1");
    assert_eq!(out["class"], "claude-active");
}

#[test]
fn process_webhook_rejects_invalid_json() {
    let home = TempDir::new().unwrap();
    cmd(home.path())
        .arg("process-webhook")
        .write_stdin("not json")
        .assert()
        .failure();
}

#[test]
fn process_webhook_rejects_missing_fields() {
    let home = TempDir::new().unwrap();
    cmd(home.path())
        .arg("process-webhook")
        .write_stdin(r#"{"session_id":"x"}"#)
        .assert()
        .failure();
}

#[test]
fn tooltip_shows_custom_title_from_jsonl() {
    let home = TempDir::new().unwrap();
    let session_id = "test-session-rename";

    let projects_dir = home.path().join(".claude/projects/proj1");
    fs::create_dir_all(&projects_dir).unwrap();
    let jsonl_path = projects_dir.join(format!("{session_id}.jsonl"));
    let entry = serde_json::json!({
        "type": "custom-title",
        "customTitle": "my-label",
        "sessionId": session_id,
    });
    fs::write(&jsonl_path, format!("{}\n", entry)).unwrap();

    let input = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": "SessionStart",
        "transcript_path": jsonl_path.to_str().unwrap(),
    });
    cmd(home.path())
        .arg("process-webhook")
        .write_stdin(input.to_string())
        .assert()
        .success();

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(
        tooltip.contains("my-label: Thinking"),
        "tooltip was: {tooltip}"
    );
}

#[test]
fn tooltip_uses_cwd_last_component_when_no_title() {
    let home = TempDir::new().unwrap();
    send_event_with_cwd(
        home.path(),
        "cwd-sess",
        "SessionStart",
        "/home/koen/repos/myproject",
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(
        tooltip.contains("myproject: Thinking"),
        "tooltip was: {tooltip}"
    );
}

#[test]
fn tooltip_falls_back_to_id_when_no_name_or_cwd() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "abcdefghijklmn", "SessionStart");

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(
        tooltip.contains("abcdefgh: Thinking"),
        "tooltip was: {tooltip}"
    );
    assert!(
        !tooltip.contains("abcdefghij"),
        "should truncate at 8 chars"
    );
}

#[test]
fn process_webhook_ignores_extra_fields() {
    let home = TempDir::new().unwrap();
    let input = serde_json::json!({
        "session_id": "sess-1",
        "hook_event_name": "SessionStart",
        "cwd": "/some/path",
        "extra_field": 42,
    });
    cmd(home.path())
        .arg("process-webhook")
        .write_stdin(input.to_string())
        .assert()
        .success();

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn unknown_event_defaults_to_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SomeNewEvent");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn permission_request_marks_waiting_for_input() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "PermissionRequest");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "WaitingForInput");
}

#[test]
fn pre_tool_use_restores_active_after_permission() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart");
    send_event(home.path(), "sess-1", "PermissionRequest");
    send_event(home.path(), "sess-1", "PreToolUse");

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}
