pub fn format_lock(resource: &str, owner: &str, task_id: Option<&str>, acquired_at: i64) -> String {
    format!(
        "{resource}\towner={owner}\ttask={}\tacquired_at={acquired_at}",
        task_id.unwrap_or("-")
    )
}
