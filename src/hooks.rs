use std::io::Read;

use serde::Deserialize;

use crate::session::{SessionState, SessionStore, read_custom_title};

#[derive(Deserialize)]
struct HookInput {
    session_id: String,
    hook_event_name: String,
    cwd: Option<String>,
    transcript_path: Option<String>,
    permission_mode: Option<String>,
}

#[derive(Deserialize)]
struct NotificationInput {
    session_id: String,
    message: Option<String>,
    cwd: Option<String>,
    transcript_path: Option<String>,
}

pub fn process_notification() -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let notif: NotificationInput = serde_json::from_str(&input)?;

    let store = SessionStore::load()?;
    let session_name = store
        .sessions
        .get(&notif.session_id)
        .and_then(|s| s.name.clone())
        .or_else(|| notif.transcript_path.as_deref().and_then(read_custom_title))
        .or_else(|| {
            notif.cwd.as_deref().and_then(|cwd| {
                std::path::Path::new(cwd)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| notif.session_id[..notif.session_id.len().min(8)].to_string());

    let title = format!("Claude: {session_name}");
    let body = notif
        .message
        .unwrap_or_else(|| "Needs attention".to_string());

    std::process::Command::new("notify-send")
        .arg(&title)
        .arg(&body)
        .status()
        .ok();

    Ok(())
}

pub fn process_hook() -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let hook: HookInput = serde_json::from_str(&input)?;

    let mut store = SessionStore::load()?;

    if hook.hook_event_name == "SessionEnd" {
        store.sessions.remove(&hook.session_id);
    } else {
        let session = store.upsert(&hook.session_id);
        session.updated_at = chrono::Utc::now();
        session.state = match hook.hook_event_name.as_str() {
            "UserPromptSubmit" | "PreToolUse" => SessionState::Active,
            "SessionStart" | "Stop" => SessionState::Idle,
            "Notification" | "PermissionRequest" => SessionState::WaitingForInput,
            _ => SessionState::Active,
        };
        if session.project.is_none()
            && let Some(ref cwd) = hook.cwd
        {
            session.project = Some(cwd.clone());
        }
        if let Some(ref mode) = hook.permission_mode {
            session.permission_mode = Some(mode.clone());
        }
        if let Some(title) = hook.transcript_path.as_deref().and_then(read_custom_title) {
            session.name = Some(title);
        } else if session.name.is_none()
            && let Some(ref cwd) = hook.cwd
        {
            session.name = std::path::Path::new(cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string);
        }
    }

    store.save()?;
    Ok(())
}
