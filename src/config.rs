use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub home: String,
    pub default_agent: String,
    pub state_db: String,
    pub event_log: String,
    pub profile: Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub approval_level: String,
    pub sandbox: String,
}

impl Config {
    pub fn new(home: &crate::home::Home) -> Self {
        Self {
            version: 1,
            home: home.root().display().to_string(),
            default_agent: "zgent-core".to_string(),
            state_db: "state/zgent.sqlite".to_string(),
            event_log: "state/events.jsonl".to_string(),
            profile: Profile {
                name: "default".to_string(),
                approval_level: "review-first".to_string(),
                sandbox: "workspace-write".to_string(),
            },
        }
    }
}
