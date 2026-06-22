use std::{fs, process::Command};

use anyhow::bail;
use serde_json::json;

use crate::{approvals::PermissionMode, normalizer, state::NodeRow, state::Store};

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub required_locks: Vec<String>,
    pub permission_mode: PermissionMode,
}

pub fn run_next_subprocess(
    store: &Store,
    task_id: &str,
    owner: &str,
    adapter: &str,
    command: &[String],
) -> crate::Result<Option<NodeRow>> {
    run_next_command(
        store,
        task_id,
        owner,
        adapter,
        command,
        RunOptions::default(),
    )
}

pub fn run_next_command(
    store: &Store,
    task_id: &str,
    owner: &str,
    adapter: &str,
    command: &[String],
    options: RunOptions,
) -> crate::Result<Option<NodeRow>> {
    if command.is_empty() {
        bail!("command is required");
    }
    ensure_locks(
        store,
        owner,
        &options.required_locks,
        options.permission_mode,
    )?;
    let Some(node) = store.lease_next_node(task_id, owner, 300)? else {
        return Ok(None);
    };
    ensure_command_policy(
        store,
        task_id,
        &node.id,
        &node.name,
        command,
        options.permission_mode,
    )?;
    store.record_event(
        Some(task_id),
        Some(&node.id),
        "run.started",
        json!({
            "adapter": adapter,
            "program": command[0],
            "args": &command[1..],
            "permission_mode": options.permission_mode.as_str()
        }),
    )?;
    let session_id = store.create_session(adapter, Some(task_id), Some(&node.id), None)?;

    let output = Command::new(&command[0]).args(&command[1..]).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if let Some(provider_session_id) =
        record_output_events(store, task_id, &node.id, &stdout, &stderr)?
    {
        store.update_session_provider_id(&session_id, &provider_session_id)?;
    }

    let transcript = store
        .home()
        .task_dir(task_id)
        .join("transcripts")
        .join(format!("{}.txt", node.id));
    fs::write(
        &transcript,
        format!(
            "$ {}\n\n[stdout]\n{}\n[stderr]\n{}\n",
            shell_words(command),
            stdout,
            stderr
        ),
    )?;
    store.record_artifact(task_id, Some(&node.id), "transcript", &transcript)?;

    let code = output.status.code();
    if output.status.success() {
        store.record_event(
            Some(task_id),
            Some(&node.id),
            "run.completed",
            json!({ "adapter": adapter, "exit_code": code }),
        )?;
        store.update_session(&session_id, "COMPLETED")?;
        store.finish_node(&node.id, "COMPLETED")?;
    } else {
        store.record_event(
            Some(task_id),
            Some(&node.id),
            "run.failed",
            json!({ "adapter": adapter, "exit_code": code }),
        )?;
        store.update_session(&session_id, "FAILED")?;
        store.finish_node(&node.id, "FAILED")?;
    }
    Ok(Some(node))
}

pub fn run_all_subprocess(
    store: &Store,
    task_id: &str,
    owner: &str,
    adapter: &str,
    command: &[String],
) -> crate::Result<usize> {
    run_all_command(
        store,
        task_id,
        owner,
        adapter,
        command,
        RunOptions::default(),
    )
}

pub fn run_all_command(
    store: &Store,
    task_id: &str,
    owner: &str,
    adapter: &str,
    command: &[String],
    options: RunOptions,
) -> crate::Result<usize> {
    let mut count = 0;
    while run_next_command(store, task_id, owner, adapter, command, options.clone())?.is_some() {
        count += 1;
    }
    Ok(count)
}

fn ensure_locks(
    store: &Store,
    owner: &str,
    locks: &[String],
    permission_mode: PermissionMode,
) -> crate::Result<()> {
    if permission_mode.bypasses_approval() {
        return Ok(());
    }
    for resource in locks {
        if !store.lock_held(resource, owner)? {
            bail!("required lock `{resource}` is not held by `{owner}`");
        }
    }
    Ok(())
}

fn ensure_command_policy(
    store: &Store,
    task_id: &str,
    node_id: &str,
    node_name: &str,
    command: &[String],
    permission_mode: PermissionMode,
) -> crate::Result<()> {
    if permission_mode.bypasses_approval() {
        store.record_event(
            Some(task_id),
            Some(node_id),
            "permission.yolo",
            json!({ "node": node_name, "reason": "approval checks bypassed" }),
        )?;
        return Ok(());
    }
    if !is_dangerous_command(command)
        || store.has_approved_approval(task_id, Some(node_id), "dangerous")?
    {
        return Ok(());
    }
    let reason = format!("dangerous command requires approval before node `{node_name}` runs");
    store.wait_for_approval(node_id, &reason)?;
    store.request_approval(Some(task_id), Some(node_id), "dangerous", Some(&reason))?;
    bail!("{reason}");
}

fn is_dangerous_command(command: &[String]) -> bool {
    let text = command.join(" ").to_ascii_lowercase();
    [
        "rm -rf",
        "git reset --hard",
        "git clean -fd",
        "git push",
        "sudo ",
        "chmod -r",
        "chown -r",
        "curl ",
        "wget ",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn record_output_events(
    store: &Store,
    task_id: &str,
    node_id: &str,
    stdout: &str,
    stderr: &str,
) -> crate::Result<Option<String>> {
    let mut provider_session_id = None;
    for (stream, text) in [("stdout", stdout), ("stderr", stderr)] {
        for event in normalizer::normalize_output(stream, text) {
            if provider_session_id.is_none() {
                provider_session_id = event.provider_session_id.clone();
            }
            store.record_event(Some(task_id), Some(node_id), &event.kind, event.payload)?;
        }
    }
    Ok(provider_session_id)
}

fn shell_words(command: &[String]) -> String {
    command.join(" ")
}

#[cfg(test)]
mod tests {
    use crate::{home::Home, state::NodeSpec, state::Store};

    #[test]
    fn runs_fake_subprocess_and_completes_node() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "say ok",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo ok".to_string(),
        ];
        let node = super::run_next_subprocess(&store, &task_id, "tester", "fake", &command)
            .unwrap()
            .expect("node");
        assert_eq!(store.node(&node.id).unwrap().unwrap().state, "COMPLETED");
        assert!(store.task_event_count(&task_id).unwrap() >= 5);
        assert!(
            store
                .home()
                .task_dir(&task_id)
                .join("transcripts")
                .join(format!("{}.txt", node.id))
                .exists()
        );
    }

    #[test]
    fn runs_all_fake_workflow_nodes() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task("fix ci", "fix-ci", crate::workflows::nodes_for("fix-ci"))
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo ok".to_string(),
        ];
        let count =
            super::run_all_subprocess(&store, &task_id, "tester", "fake", &command).unwrap();
        assert_eq!(count, 4);
        assert_eq!(store.task(&task_id).unwrap().unwrap().status, "COMPLETED");
    }

    #[test]
    fn captures_provider_session_id_from_jsonl_output() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task("jsonl", "plan-only", vec![NodeSpec::new("plan", "planner")])
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf '%s\n' '{\"type\":\"assistant_message\",\"session_id\":\"provider-123\",\"content\":\"ok\"}'"
                .to_string(),
        ];
        super::run_next_subprocess(&store, &task_id, "tester", "fake", &command)
            .unwrap()
            .expect("node");
        assert_eq!(
            store.session_provider_ids(&task_id).unwrap(),
            vec![Some("provider-123".to_string())]
        );
    }

    #[test]
    fn enforces_required_locks_before_leasing_node() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "locked",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo ok".to_string(),
        ];
        let options = super::RunOptions {
            required_locks: vec!["repo:test".to_string()],
            ..Default::default()
        };
        assert!(
            super::run_next_command(
                &store,
                &task_id,
                "tester",
                "fake",
                &command,
                options.clone()
            )
            .is_err()
        );
        assert_eq!(store.nodes(&task_id).unwrap()[0].state, "PENDING");
        assert!(
            store
                .acquire_lock("repo:test", "tester", Some(&task_id))
                .unwrap()
        );
        assert!(
            super::run_next_command(&store, &task_id, "tester", "fake", &command, options)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn requires_approval_for_dangerous_commands() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "dangerous",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo rm -rf safe".to_string(),
        ];
        assert!(
            super::run_next_command(
                &store,
                &task_id,
                "tester",
                "fake",
                &command,
                Default::default()
            )
            .is_err()
        );
        let node = store.nodes(&task_id).unwrap().remove(0);
        assert_eq!(node.state, "WAITING_APPROVAL");
        let approval = store.approvals().unwrap().remove(0);
        assert_eq!(approval.level, "dangerous");

        assert!(store.decide_approval(&approval.id, "APPROVED").unwrap());
        assert_eq!(store.node(&node.id).unwrap().unwrap().state, "PENDING");
        assert!(
            super::run_next_command(
                &store,
                &task_id,
                "tester",
                "fake",
                &command,
                Default::default()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(store.node(&node.id).unwrap().unwrap().state, "COMPLETED");
    }

    #[test]
    fn yolo_bypasses_lock_and_dangerous_command_approval() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task("yolo", "plan-only", vec![NodeSpec::new("plan", "planner")])
            .unwrap();
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo rm -rf safe".to_string(),
        ];
        let node = super::run_next_command(
            &store,
            &task_id,
            "tester",
            "fake",
            &command,
            super::RunOptions {
                required_locks: vec!["repo:test".to_string()],
                permission_mode: crate::approvals::PermissionMode::Yolo,
            },
        )
        .unwrap()
        .expect("node");
        assert_eq!(store.node(&node.id).unwrap().unwrap().state, "COMPLETED");
        assert!(store.approvals().unwrap().is_empty());
    }
}
