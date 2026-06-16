use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub task_id: Option<String>,
    pub node_id: Option<String>,
    pub kind: String,
    pub payload: Value,
    pub created_at: i64,
}

impl Event {
    pub fn new(
        task_id: Option<&str>,
        node_id: Option<&str>,
        kind: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_id: task_id.map(str::to_owned),
            node_id: node_id.map(str::to_owned),
            kind: kind.into(),
            payload,
            created_at: now(),
        }
    }
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs() as i64
}
