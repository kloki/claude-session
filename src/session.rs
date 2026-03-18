use std::{collections::HashMap, fs, io, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
    Active,
    Idle,
    WaitingForInput,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Active => write!(f, "Thinking"),
            SessionState::Idle => write!(f, "Idle"),
            SessionState::WaitingForInput => write!(f, "Waiting For Input"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub state: SessionState,
    pub name: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionStore {
    pub sessions: HashMap<String, Session>,
}

fn state_file_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".claude_sessions")
}

impl SessionStore {
    pub fn load() -> io::Result<Self> {
        let path = state_file_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)?;
        serde_json::from_str(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self) -> io::Result<()> {
        let path = state_file_path();
        let tmp = path.with_extension("tmp");
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, &contents)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn clear() -> io::Result<()> {
        let path = state_file_path();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Get or create a session (upsert pattern for resilience to missed SessionStart)
    pub fn upsert(&mut self, session_id: &str) -> &mut Session {
        let now = Utc::now();
        self.sessions
            .entry(session_id.to_string())
            .or_insert(Session {
                state: SessionState::Active,
                name: None,
                started_at: now,
                updated_at: now,
            })
    }

    /// Remove sessions older than 24 hours based on updated_at
    pub fn cleanup_stale(&mut self) {
        let cutoff = Utc::now() - chrono::Duration::hours(24);
        self.sessions.retain(|_, s| s.updated_at > cutoff);
    }
}

pub fn read_custom_title(transcript_path: &str) -> Option<String> {
    let content = fs::read_to_string(transcript_path).ok()?;
    content
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "custom-title")
        .last()
        .and_then(|v| v["customTitle"].as_str().map(str::to_string))
}
