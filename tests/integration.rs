use std::{fs, path::Path};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("claude-sessions").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn send_event(
    home: &Path,
    session_id: &str,
    event: &str,
    cwd: Option<&str>,
    transcript_path: Option<&str>,
) {
    let mut input = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": event,
    });
    if let Some(cwd) = cwd {
        input["cwd"] = serde_json::Value::String(cwd.to_string());
    }
    if let Some(tp) = transcript_path {
        input["transcript_path"] = serde_json::Value::String(tp.to_string());
    }
    cmd(home)
        .arg("process-hook")
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
fn session_start_creates_idle_session() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
}

#[test]
fn session_end_removes_session() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "SessionEnd", None, None);

    let store = read_store(home.path());
    assert!(store["sessions"]["sess-1"].is_null());
}

#[test]
fn stop_marks_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "Stop", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
}

#[test]
fn notification_marks_waiting_for_input() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "Notification", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "WaitingForInput");
}

#[test]
fn user_prompt_submit_marks_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "Notification", None, None);
    send_event(home.path(), "sess-1", "UserPromptSubmit", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn upsert_creates_session_on_missed_start() {
    let home = TempDir::new().unwrap();
    // Skip SessionStart, go straight to Stop
    send_event(home.path(), "sess-1", "Stop", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
    assert!(store["sessions"]["sess-1"]["started_at"].is_string());
}

#[test]
fn clear_removes_state_file() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
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
    assert_eq!(out["class"], "claude-empty");
}

#[test]
fn waybar_counts_all_sessions() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-2", "SessionStart", None, None);
    send_event(home.path(), "sess-2", "Stop", None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "2");
}

#[test]
fn waybar_class_waiting_takes_priority() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-2", "SessionStart", None, None);
    send_event(home.path(), "sess-2", "Notification", None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-waiting");
}

#[test]
fn waybar_class_idle_over_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "UserPromptSubmit", None, None);
    send_event(home.path(), "sess-2", "SessionStart", None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-idle");
}

#[test]
fn waybar_class_idle_when_all_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "Stop", None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-idle");
}

#[test]
fn waybar_tooltip_truncates_long_ids() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "abcdefghij-long-id",
        "SessionStart",
        None,
        None,
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("abcdefgh"));
    assert!(!tooltip.contains("abcdefghij"));
}

#[test]
fn waybar_tooltip_keeps_short_ids() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "short", "SessionStart", None, None);

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("[Idle]       : short"));
}

#[test]
fn waybar_cleans_stale_sessions() {
    let home = TempDir::new().unwrap();
    // Create a session, then manually backdate its updated_at
    send_event(home.path(), "old-sess", "SessionStart", None, None);

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

    send_event(home.path(), "s1", "SessionStart", None, None);
    send_event(home.path(), "s2", "SessionStart", None, None);
    send_event(home.path(), "s3", "SessionStart", None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "3");

    send_event(home.path(), "s1", "Notification", None, None);
    send_event(home.path(), "s2", "Stop", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["s1"]["state"], "WaitingForInput");
    assert_eq!(store["sessions"]["s2"]["state"], "Idle");
    assert_eq!(store["sessions"]["s3"]["state"], "Idle");

    send_event(home.path(), "s1", "UserPromptSubmit", None, None);
    send_event(home.path(), "s2", "SessionEnd", None, None);
    send_event(home.path(), "s3", "SessionEnd", None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "1");
    assert_eq!(out["class"], "claude-active");
}

#[test]
fn process_hook_rejects_invalid_json() {
    let home = TempDir::new().unwrap();
    cmd(home.path())
        .arg("process-hook")
        .write_stdin("not json")
        .assert()
        .failure();
}

#[test]
fn process_hook_rejects_missing_fields() {
    let home = TempDir::new().unwrap();
    cmd(home.path())
        .arg("process-hook")
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

    send_event(
        home.path(),
        session_id,
        "SessionStart",
        None,
        Some(jsonl_path.to_str().unwrap()),
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(
        tooltip.contains("[Idle]       : my-label"),
        "tooltip was: {tooltip}"
    );
}

#[test]
fn tooltip_uses_cwd_last_component_when_no_title() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "cwd-sess",
        "SessionStart",
        Some("/home/koen/repos/myproject"),
        None,
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(
        tooltip.contains("[Idle]       : myproject"),
        "tooltip was: {tooltip}"
    );
}

#[test]
fn tooltip_falls_back_to_id_when_no_name_or_cwd() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "abcdefghijklmn", "SessionStart", None, None);

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(
        tooltip.contains("[Idle]       : abcdefgh"),
        "tooltip was: {tooltip}"
    );
    assert!(
        !tooltip.contains("abcdefghij"),
        "should truncate at 8 chars"
    );
}

#[test]
fn process_hook_ignores_extra_fields() {
    let home = TempDir::new().unwrap();
    let input = serde_json::json!({
        "session_id": "sess-1",
        "hook_event_name": "SessionStart",
        "cwd": "/some/path",
        "extra_field": 42,
    });
    cmd(home.path())
        .arg("process-hook")
        .write_stdin(input.to_string())
        .assert()
        .success();

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
}

#[test]
fn unknown_event_defaults_to_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SomeNewEvent", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn permission_request_marks_waiting_for_input() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "PermissionRequest", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "WaitingForInput");
}

#[test]
fn pre_tool_use_restores_active_after_permission() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None);
    send_event(home.path(), "sess-1", "PermissionRequest", None, None);
    send_event(home.path(), "sess-1", "PreToolUse", None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}
