use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::state::ApprovalRow;

pub const LEVELS: &[&str] = &["read-only", "workspace-write", "trusted-write", "dangerous"];
pub const PERMISSION_MODES: &[&str] = &["review-first", "yolo"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    #[default]
    ReviewFirst,
    Yolo,
}

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReviewFirst => "review-first",
            Self::Yolo => "yolo",
        }
    }

    pub fn bypasses_approval(self) -> bool {
        matches!(self, Self::Yolo)
    }
}

pub fn validate_level(level: &str) -> crate::Result<()> {
    if LEVELS.contains(&level) {
        Ok(())
    } else {
        bail!(
            "unknown approval level `{level}`; expected one of {}",
            LEVELS.join(", ")
        )
    }
}

pub fn parse_permission_mode(mode: Option<&str>) -> crate::Result<PermissionMode> {
    match mode.unwrap_or("review-first") {
        "review-first" => Ok(PermissionMode::ReviewFirst),
        "yolo" => Ok(PermissionMode::Yolo),
        other => {
            bail!(
                "unknown permission mode `{other}`; expected one of {}",
                PERMISSION_MODES.join(", ")
            )
        }
    }
}

pub fn format_approval(approval: &ApprovalRow) -> String {
    format!(
        "{}\t{}\tlevel={}\ttask={}\tnode={}\treason={}",
        approval.id,
        approval.status,
        approval.level,
        approval.task_id.as_deref().unwrap_or("-"),
        approval.node_id.as_deref().unwrap_or("-"),
        approval.reason.as_deref().unwrap_or("-")
    )
}
