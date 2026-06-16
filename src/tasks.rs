use std::fs;

use anyhow::bail;

use crate::state::Store;

pub fn print_status(store: &Store, task_id: &str) -> crate::Result<()> {
    let Some(task) = store.task(task_id)? else {
        bail!("task not found: {task_id}");
    };
    println!("task {}", task.id);
    println!("status: {}", task.status);
    println!("workflow: {}", task.workflow);
    println!("goal: {}", task.goal);
    println!("events: {}", store.task_event_count(task_id)?);
    println!("nodes:");
    for node in store.nodes(task_id)? {
        let agent = node.agent.as_deref().unwrap_or("-");
        let skill = node.skill.as_deref().unwrap_or("-");
        let deps = if node.depends_on.is_empty() {
            "-".to_string()
        } else {
            node.depends_on.join(",")
        };
        println!(
            "  {} {} role={} agent={} skill={} depends_on={}",
            node.state, node.name, node.role, agent, skill, deps
        );
    }
    Ok(())
}

pub fn print_events(store: &Store, task_id: &str) -> crate::Result<()> {
    let path = store.home().task_events(task_id);
    if path.exists() {
        print!("{}", fs::read_to_string(path)?);
    }
    Ok(())
}
