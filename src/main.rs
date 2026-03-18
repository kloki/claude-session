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
    /// Process a webhook event from Claude hooks (reads JSON from stdin)
    ProcessWebhook,
    /// Clear all session state
    Clear,
    /// Output Waybar-compatible JSON
    Waybar,
    /// List sessions in terminal-friendly format
    Ps,
}

#[derive(Deserialize)]
struct HookInput {
    session_id: String,
    hook_event_name: String,
    cwd: Option<String>,
    transcript_path: Option<String>,
}

fn process_webhook() -> anyhow::Result<()> {
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

fn waybar() -> anyhow::Result<()> {
    let mut store = SessionStore::load()?;
    store.cleanup_stale();
    store.save()?;

    let count = store.sessions.len();
    let tooltip = store
        .sorted_sessions()
        .iter()
        .map(|(id, s)| format!("{}: {}", s.state.label(), s.display_name(id)))
        .collect::<Vec<_>>()
        .join("\n");

    let class = if store
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
    };

    let output = WaybarOutput {
        text: count.to_string(),
        tooltip,
        class: class.to_string(),
    };

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

fn ps() -> anyhow::Result<()> {
    let mut store = SessionStore::load()?;
    store.cleanup_stale();
    store.save()?;

    if store.sessions.is_empty() {
        println!("No active sessions");
        return Ok(());
    }

    for (id, s) in store.sorted_sessions() {
        println!("{} {}", s.state.label(), s.display_name(id));
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::ProcessWebhook => process_webhook(),
        Command::Clear => SessionStore::clear().map_err(Into::into),
        Command::Waybar => waybar(),
        Command::Ps => ps(),
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
