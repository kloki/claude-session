mod hooks;
mod output;
mod session;
mod waybar;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

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

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::ProcessHook => hooks::process_hook(),
        Command::ProcessNotification => hooks::process_notification(),
        Command::Clear => session::SessionStore::clear(),
        Command::Waybar => waybar::waybar(),
        Command::Ps => output::ps(),
        Command::Json => output::json(),
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
