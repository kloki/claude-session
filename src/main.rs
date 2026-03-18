mod session;

use std::io::Read;

use clap::{Parser, Subcommand};
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
    /// Clear all session state
    Clear,
    /// Output Waybar-compatible JSON
    Waybar,
    /// List sessions in terminal-friendly format
    Ps,
    /// Output sessions as a JSON array
    Json,
}

#[derive(Deserialize)]
struct HookInput {
    session_id: String,
    hook_event_name: String,
    cwd: Option<String>,
    transcript_path: Option<String>,
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

#[derive(serde::Serialize)]
struct WaybarOutput {
    text: String,
    tooltip: String,
    class: String,
}

fn waybar_class(store: &SessionStore) -> &'static str {
    if store
        .sessions
        .values()
        .any(|s| s.state == SessionState::WaitingForInput)
    {
        "claude-waiting"
    } else if store
        .sessions
        .values()
        .any(|s| s.state == SessionState::Idle)
    {
        "claude-idle"
    } else if !store.sessions.is_empty() {
        "claude-active"
    } else {
        "claude-empty"
    }
}

fn waybar() -> anyhow::Result<()> {
    let store = SessionStore::load_and_cleanup()?;

    let count = store.sessions.len();
    let tooltip = store
        .sorted_sessions()
        .iter()
        .map(|(id, s)| format!("{}: {}", s.state.label(), s.display_name(id)))
        .collect::<Vec<_>>()
        .join("\n");

    let output = WaybarOutput {
        text: count.to_string(),
        tooltip,
        class: waybar_class(&store).to_string(),
    };

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

fn ps() -> anyhow::Result<()> {
    let store = SessionStore::load_and_cleanup()?;

    if store.sessions.is_empty() {
        println!("No active sessions");
        return Ok(());
    }

    for (id, s) in store.sorted_sessions() {
        println!("{} {}", s.state.label(), s.display_name(id));
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct JsonSession {
    id: String,
    name: String,
    state: String,
    started_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
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
        })
        .collect();

    println!("{}", serde_json::to_string(&sessions)?);
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::ProcessHook => process_hook(),
        Command::Clear => SessionStore::clear(),
        Command::Waybar => waybar(),
        Command::Ps => ps(),
        Command::Json => json(),
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
