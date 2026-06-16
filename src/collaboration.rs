use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::home::Home;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationSession {
    pub id: String,
    pub mode: String,
    pub endpoint: Option<String>,
    pub participants: Vec<String>,
    pub created_at: i64,
}

pub fn start(
    home: &Home,
    mode: &str,
    endpoint: Option<String>,
) -> crate::Result<CollaborationSession> {
    let session = CollaborationSession {
        id: format!("collab-{}", Uuid::new_v4()),
        mode: mode.to_string(),
        endpoint,
        participants: vec!["local".to_string()],
        created_at: crate::events::now(),
    };
    write(home, &session)?;
    Ok(session)
}

pub fn join(home: &Home, id: &str, participant: &str) -> crate::Result<CollaborationSession> {
    let mut session = read(home, id)?;
    if !session
        .participants
        .iter()
        .any(|existing| existing == participant)
    {
        session.participants.push(participant.to_string());
    }
    write(home, &session)?;
    Ok(session)
}

pub fn list(home: &Home) -> crate::Result<Vec<CollaborationSession>> {
    let dir = sessions_dir(home);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            sessions.push(serde_json::from_str(&fs::read_to_string(entry.path())?)?);
        }
    }
    sessions.sort_by(|a: &CollaborationSession, b| a.id.cmp(&b.id));
    Ok(sessions)
}

fn read(home: &Home, id: &str) -> crate::Result<CollaborationSession> {
    let path = sessions_dir(home).join(format!("{id}.json"));
    if !path.exists() {
        anyhow::bail!("collaboration session not found: {id}");
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write(home: &Home, session: &CollaborationSession) -> crate::Result<()> {
    let dir = sessions_dir(home);
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join(format!("{}.json", session.id)),
        serde_json::to_string_pretty(session)?,
    )?;
    Ok(())
}

fn sessions_dir(home: &Home) -> PathBuf {
    home.collaboration_dir().join("sessions")
}

#[cfg(test)]
mod tests {
    use crate::home::Home;

    #[test]
    fn starts_and_joins_session() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        let session = super::start(&home, "hosted", Some("https://example.test".to_string()))?;
        assert!(session.id.starts_with("collab-"));

        let joined = super::join(&home, &session.id, "worker-1")?;
        assert_eq!(joined.participants, ["local", "worker-1"]);
        assert_eq!(super::list(&home)?.len(), 1);
        Ok(())
    }
}
