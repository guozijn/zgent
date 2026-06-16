use anyhow::bail;

use crate::state::ApprovalRow;

pub const LEVELS: &[&str] = &["read-only", "workspace-write", "trusted-write", "dangerous"];

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
