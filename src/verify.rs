use std::{fs, process::Command};

use serde_json::json;
use uuid::Uuid;

use crate::state::Store;

pub fn run_verification(store: &Store, task_id: &str, command: &[String]) -> crate::Result<bool> {
    if command.is_empty() {
        anyhow::bail!("verification command is required");
    }
    store.record_event(
        Some(task_id),
        None,
        "verification.started",
        json!({ "program": command[0], "args": &command[1..] }),
    )?;
    let output = Command::new(&command[0]).args(&command[1..]).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let transcript = store
        .home()
        .task_dir(task_id)
        .join("transcripts")
        .join(format!("verification-{}.txt", Uuid::new_v4()));
    fs::write(
        &transcript,
        format!(
            "$ {}\n\n[stdout]\n{}\n[stderr]\n{}\n",
            command.join(" "),
            stdout,
            stderr
        ),
    )?;
    store.record_artifact(task_id, None, "verification", &transcript)?;
    store.record_event(
        Some(task_id),
        None,
        if output.status.success() {
            "verification.completed"
        } else {
            "verification.failed"
        },
        json!({
            "exit_code": output.status.code(),
            "stdout": stdout.trim_end(),
            "stderr": stderr.trim_end()
        }),
    )?;
    Ok(output.status.success())
}

#[cfg(test)]
mod tests {
    use crate::{home::Home, state::NodeSpec, state::Store};

    #[test]
    fn records_verification_run() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "verify",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo verified".to_string(),
        ];
        assert!(super::run_verification(&store, &task_id, &command).unwrap());
        assert!(store.task_event_count(&task_id).unwrap() >= 4);
    }
}
