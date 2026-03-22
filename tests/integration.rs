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
    permission_mode: Option<&str>,
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
    if let Some(mode) = permission_mode {
        input["permission_mode"] = serde_json::Value::String(mode.to_string());
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
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
}

#[test]
fn session_end_removes_session() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "SessionEnd", None, None, None);

    let store = read_store(home.path());
    assert!(store["sessions"]["sess-1"].is_null());
}

#[test]
fn stop_marks_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "Stop", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
}

#[test]
fn notification_marks_waiting_for_input() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "Notification", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "WaitingForInput");
}

#[test]
fn user_prompt_submit_marks_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "Notification", None, None, None);
    send_event(home.path(), "sess-1", "UserPromptSubmit", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn upsert_creates_session_on_missed_start() {
    let home = TempDir::new().unwrap();
    // Skip SessionStart, go straight to Stop
    send_event(home.path(), "sess-1", "Stop", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Idle");
    assert!(store["sessions"]["sess-1"]["started_at"].is_string());
}

#[test]
fn clear_removes_state_file() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
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
    assert_eq!(out["tooltip"], "No active sessions");
    assert_eq!(out["class"], "claude-empty");
}

#[test]
fn waybar_counts_all_sessions() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-2", "SessionStart", None, None, None);
    send_event(home.path(), "sess-2", "Stop", None, None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "2");
}

#[test]
fn waybar_class_waiting_takes_priority() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-2", "SessionStart", None, None, None);
    send_event(home.path(), "sess-2", "Notification", None, None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-waiting");
}

#[test]
fn waybar_class_idle_over_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "UserPromptSubmit", None, None, None);
    send_event(home.path(), "sess-2", "SessionStart", None, None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["class"], "claude-idle");
}

#[test]
fn waybar_class_idle_when_all_idle() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "Stop", None, None, None);

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
        None,
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    // Waybar tooltip doesn't show ID column; name column truncates to 8 chars
    assert!(tooltip.contains("abcdefgh"), "tooltip was: {tooltip}");
    assert!(
        !tooltip.contains("abcdefghij-long-id"),
        "full ID should not appear in waybar tooltip (no ID column): {tooltip}"
    );
}

#[test]
fn waybar_tooltip_keeps_short_ids() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "short", "SessionStart", None, None, None);

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("short"), "tooltip was: {tooltip}");
}

#[test]
fn waybar_cleans_stale_sessions() {
    let home = TempDir::new().unwrap();
    // Create a session, then manually backdate its updated_at
    send_event(home.path(), "old-sess", "SessionStart", None, None, None);

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

    send_event(home.path(), "s1", "SessionStart", None, None, None);
    send_event(home.path(), "s2", "SessionStart", None, None, None);
    send_event(home.path(), "s3", "SessionStart", None, None, None);

    let out = waybar_output(home.path());
    assert_eq!(out["text"], "3");

    send_event(home.path(), "s1", "Notification", None, None, None);
    send_event(home.path(), "s2", "Stop", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["s1"]["state"], "WaitingForInput");
    assert_eq!(store["sessions"]["s2"]["state"], "Idle");
    assert_eq!(store["sessions"]["s3"]["state"], "Idle");

    send_event(home.path(), "s1", "UserPromptSubmit", None, None, None);
    send_event(home.path(), "s2", "SessionEnd", None, None, None);
    send_event(home.path(), "s3", "SessionEnd", None, None, None);

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
        None,
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("my-label"), "tooltip was: {tooltip}");
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
        None,
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("myproject"), "tooltip was: {tooltip}");
}

#[test]
fn tooltip_falls_back_to_id_when_no_name_or_cwd() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "abcdefghijklmn",
        "SessionStart",
        None,
        None,
        None,
    );

    let out = waybar_output(home.path());
    let tooltip = out["tooltip"].as_str().unwrap();
    // Waybar tooltip doesn't show ID column; name column truncates to 8 chars
    assert!(tooltip.contains("abcdefgh"), "tooltip was: {tooltip}");
    assert!(
        !tooltip.contains("abcdefghijklmn"),
        "full ID should not appear in waybar tooltip (no ID column): {tooltip}"
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
fn process_notification_parses_input_and_resolves_name() {
    let home = TempDir::new().unwrap();

    // Create a session with a known name first
    send_event(
        home.path(),
        "test-notif-sess",
        "SessionStart",
        Some("/home/user/my-project"),
        None,
        None,
    );

    let input = serde_json::json!({
        "session_id": "test-notif-sess",
        "message": "Task complete",
        "cwd": "/home/user/my-project",
    });

    // notify-send may not exist in CI, but the command should still succeed
    // (we .ok() the notify-send call)
    cmd(home.path())
        .arg("process-notification")
        .write_stdin(input.to_string())
        .assert()
        .success();
}

#[test]
fn process_notification_uses_fallback_when_no_message() {
    let home = TempDir::new().unwrap();

    let input = serde_json::json!({
        "session_id": "test-notif-sess-2",
    });

    cmd(home.path())
        .arg("process-notification")
        .write_stdin(input.to_string())
        .assert()
        .success();
}

#[test]
fn process_notification_resolves_name_from_transcript() {
    let home = TempDir::new().unwrap();
    let session_id = "test-notif-transcript";

    let projects_dir = home.path().join(".claude/projects/proj1");
    fs::create_dir_all(&projects_dir).unwrap();
    let jsonl_path = projects_dir.join(format!("{session_id}.jsonl"));
    let entry = serde_json::json!({
        "type": "custom-title",
        "customTitle": "my-notification-label",
        "sessionId": session_id,
    });
    fs::write(&jsonl_path, format!("{}\n", entry)).unwrap();

    let input = serde_json::json!({
        "session_id": session_id,
        "message": "Done!",
        "transcript_path": jsonl_path.to_str().unwrap(),
    });

    cmd(home.path())
        .arg("process-notification")
        .write_stdin(input.to_string())
        .assert()
        .success();
}

#[test]
fn process_notification_rejects_invalid_json() {
    let home = TempDir::new().unwrap();
    cmd(home.path())
        .arg("process-notification")
        .write_stdin("not json")
        .assert()
        .failure();
}

#[test]
fn unknown_event_defaults_to_active() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SomeNewEvent", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn permission_request_marks_waiting_for_input() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "PermissionRequest", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "WaitingForInput");
}

#[test]
fn pre_tool_use_restores_active_after_permission() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);
    send_event(home.path(), "sess-1", "PermissionRequest", None, None, None);
    send_event(home.path(), "sess-1", "PreToolUse", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["state"], "Active");
}

#[test]
fn permission_mode_stored_when_present() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        None,
        None,
        Some("plan"),
    );

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["permission_mode"], "plan");
}

#[test]
fn permission_mode_not_cleared_when_absent() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        None,
        None,
        Some("plan"),
    );
    send_event(home.path(), "sess-1", "UserPromptSubmit", None, None, None);

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["permission_mode"], "plan");
}

#[test]
fn permission_mode_updated_on_new_value() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        None,
        None,
        Some("plan"),
    );
    send_event(
        home.path(),
        "sess-1",
        "UserPromptSubmit",
        None,
        None,
        Some("default"),
    );

    let store = read_store(home.path());
    assert_eq!(store["sessions"]["sess-1"]["permission_mode"], "default");
}

#[test]
fn project_stored_from_cwd() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        Some("/home/user/repos/myproject"),
        None,
        None,
    );

    let store = read_store(home.path());
    assert_eq!(
        store["sessions"]["sess-1"]["project"],
        "/home/user/repos/myproject"
    );
}

#[test]
fn project_not_overwritten_on_subsequent_events() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        Some("/home/user/repos/first"),
        None,
        None,
    );
    send_event(
        home.path(),
        "sess-1",
        "UserPromptSubmit",
        Some("/home/user/repos/second"),
        None,
        None,
    );

    let store = read_store(home.path());
    assert_eq!(
        store["sessions"]["sess-1"]["project"],
        "/home/user/repos/first"
    );
}

#[test]
fn ps_groups_sessions_by_project() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        Some("/home/user/repos/alpha"),
        None,
        None,
    );
    send_event(
        home.path(),
        "sess-2",
        "SessionStart",
        Some("/home/user/repos/beta"),
        None,
        None,
    );

    let output = cmd(home.path())
        .arg("ps")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ps = String::from_utf8(output).unwrap();

    // Both project headers should appear (HOME in tests is tmpdir, not /home/user)
    assert!(ps.contains("/home/user/repos/alpha"), "ps was: {ps}");
    assert!(ps.contains("/home/user/repos/beta"), "ps was: {ps}");
    // Alpha should come before beta (alphabetical)
    let alpha_pos = ps.find("/home/user/repos/alpha").unwrap();
    let beta_pos = ps.find("/home/user/repos/beta").unwrap();
    assert!(alpha_pos < beta_pos, "alpha should sort before beta");
}

#[test]
fn ps_unknown_group_for_sessions_without_project() {
    let home = TempDir::new().unwrap();
    send_event(home.path(), "sess-1", "SessionStart", None, None, None);

    let output = cmd(home.path())
        .arg("ps")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ps = String::from_utf8(output).unwrap();

    assert!(ps.contains("Unknown"), "ps was: {ps}");
}

#[test]
fn ps_shows_permission_mode() {
    let home = TempDir::new().unwrap();
    send_event(
        home.path(),
        "sess-1",
        "SessionStart",
        None,
        None,
        Some("plan"),
    );

    let output = cmd(home.path())
        .arg("ps")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ps = String::from_utf8(output).unwrap();

    assert!(ps.contains("MODE"), "ps was: {ps}");
    assert!(ps.contains("plan"), "ps was: {ps}");
}

#[test]
fn old_state_file_loads_without_new_fields() {
    let home = TempDir::new().unwrap();
    let state = serde_json::json!({
        "sessions": {
            "sess-1": {
                "state": "Idle",
                "name": null,
                "started_at": "2026-01-01T00:00:00Z",
                "updated_at": chrono::Utc::now().to_rfc3339(),
            }
        }
    });
    let path = home.path().join(".claude_sessions");
    fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Should load without error and show session
    let output = cmd(home.path())
        .arg("ps")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ps = String::from_utf8(output).unwrap();

    assert!(ps.contains("Unknown"), "ps was: {ps}");
    assert!(ps.contains("sess-1"), "ps was: {ps}");
}
