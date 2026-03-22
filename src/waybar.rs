use crate::{
    format_ps,
    session::{SessionState, SessionStore},
};

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

pub fn waybar() -> anyhow::Result<()> {
    let store = SessionStore::load_and_cleanup()?;

    let output = WaybarOutput {
        text: store.sessions.len().to_string(),
        tooltip: format_ps(&store, false, Some(12)),
        class: waybar_class(&store).to_string(),
    };

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
