use crate::session::SessionStore;

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

pub fn ps() -> anyhow::Result<()> {
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

pub fn json() -> anyhow::Result<()> {
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
