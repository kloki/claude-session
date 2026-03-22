mod session;
mod waybar;

use std::io::Read;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use serde::Deserialize;
use session::{SessionState, SessionStore, read_custom_title};

#[derive(Parser)]
#[command(name = "claude-sessions", about = "Track Claude Code sessions")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Process a hook event from Claude hooks (reads JSON from stdin)
    ProcessHook,
    /// Process a notification hook and send a desktop notification via notify-send
    ProcessNotification,
    /// Clear all session state
    Clear,
    /// Output Waybar-compatible JSON
    Waybar,
    /// List sessions in terminal-friendly format
    Ps,
    /// Output sessions as a JSON array
    Json,
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        shell: Shell,
    },
}

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

fn process_notification() -> anyhow::Result<()> {
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

fn process_hook() -> anyhow::Result<()> {
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

fn format_age(dt: chrono::DateTime<chrono::Utc>) -> String {
    let dur = chrono::Utc::now() - dt;
    if dur.num_hours() >= 1 {
        format!("{}h{}m ago", dur.num_hours(), dur.num_minutes() % 60)
    } else if dur.num_minutes() >= 1 {
        format!("{}m ago", dur.num_minutes())
    } else {
        "just now".to_string()
    }
}

fn display_project(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

pub fn format_ps(store: &SessionStore, show_id: bool, max_name_width: Option<usize>) -> String {
    if store.sessions.is_empty() {
        return "No active sessions".to_string();
    }

    let groups = store.grouped_sessions();

    let all_sessions: Vec<_> = groups
        .iter()
        .flat_map(|(_, sessions)| sessions.iter())
        .collect();
    let mut name_width = all_sessions
        .iter()
        .map(|(id, s)| s.display_name(id).len())
        .max()
        .unwrap_or(4)
        .max(4);
    if let Some(max) = max_name_width {
        name_width = name_width.min(max);
    }
    let state_width = all_sessions
        .iter()
        .map(|(_, s)| s.state.label().len())
        .max()
        .unwrap_or(5)
        .max(5);
    let mode_width = all_sessions
        .iter()
        .map(|(_, s)| s.permission_mode.as_deref().unwrap_or("").len())
        .max()
        .unwrap_or(4)
        .max(4);

    let mut lines = Vec::new();

    for (i, (project, sessions)) in groups.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }

        let header = match project {
            Some(path) => display_project(path),
            None => "Unknown".to_string(),
        };
        lines.push(header);

        if show_id {
            lines.push(format!(
                "  {:<state_width$}  {:<name_width$}  {:<mode_width$}  {:<36}  {:>10}  {:>10}",
                "STATE", "NAME", "MODE", "ID", "STARTED", "UPDATED",
            ));
        } else {
            lines.push(format!(
                "  {:<state_width$}  {:<name_width$}  {:<mode_width$}  {:>10}  {:>10}",
                "STATE", "NAME", "MODE", "STARTED", "UPDATED",
            ));
        }

        for (id, s) in sessions {
            let mode = s.permission_mode.as_deref().unwrap_or("");
            let name = s.display_name(id);
            let name = if name.len() > name_width {
                &name[..name_width]
            } else {
                name
            };
            if show_id {
                lines.push(format!(
                    "  {:<state_width$}  {:<name_width$}  {:<mode_width$}  {:<36}  {:>10}  {:>10}",
                    s.state.label(),
                    name,
                    mode,
                    id,
                    format_age(s.started_at),
                    format_age(s.updated_at),
                ));
            } else {
                lines.push(format!(
                    "  {:<state_width$}  {:<name_width$}  {:<mode_width$}  {:>10}  {:>10}",
                    s.state.label(),
                    name,
                    mode,
                    format_age(s.started_at),
                    format_age(s.updated_at),
                ));
            }
        }
    }
    lines.join("\n")
}

fn ps() -> anyhow::Result<()> {
    let store = SessionStore::load_and_cleanup()?;
    println!("{}", format_ps(&store, true, None));
    Ok(())
}

#[derive(serde::Serialize)]
struct JsonSession {
    id: String,
    name: String,
    state: String,
    started_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    project: Option<String>,
    permission_mode: Option<String>,
}

fn json() -> anyhow::Result<()> {
    let store = SessionStore::load_and_cleanup()?;

    let sessions: Vec<JsonSession> = store
        .sorted_sessions()
        .iter()
        .map(|(id, s)| JsonSession {
            id: id.to_string(),
            name: s.display_name(id).to_string(),
            state: s.state.to_string(),
            started_at: s.started_at,
            updated_at: s.updated_at,
            project: s.project.clone(),
            permission_mode: s.permission_mode.clone(),
        })
        .collect();

    println!("{}", serde_json::to_string(&sessions)?);
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::ProcessHook => process_hook(),
        Command::ProcessNotification => process_notification(),
        Command::Clear => SessionStore::clear(),
        Command::Waybar => waybar::waybar(),
        Command::Ps => ps(),
        Command::Json => json(),
        Command::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "claude-sessions",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
