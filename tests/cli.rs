use std::{
    io::Write,
    process::{Command, Stdio},
};

fn zgent(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_zgent"))
        .arg("--home")
        .arg(home)
        .args(args)
        .output()
        .expect("run zgent")
}

fn zgent_in(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_zgent"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run zgent")
}

fn zgent_with_stdin(home: &std::path::Path, args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zgent"))
        .arg("--home")
        .arg(home)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zgent");
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().expect("wait zgent")
}

#[test]
fn cli_init_task_locks_and_lists() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join(".zgent");

    let init = zgent(&home, &["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(home.join("config.toml").exists());
    assert!(home.join("state/zgent.sqlite").exists());
    assert!(home.join("adapters/codex.toml").exists());

    let tui = zgent(&home, &[]);
    assert!(
        tui.status.success(),
        "{}",
        String::from_utf8_lossy(&tui.stderr)
    );
    assert!(String::from_utf8_lossy(&tui.stdout).contains("zgent TUI requires"));

    let agents = zgent(&home, &["agents", "list"]);
    assert!(
        agents.status.success(),
        "{}",
        String::from_utf8_lossy(&agents.stderr)
    );
    assert!(String::from_utf8_lossy(&agents.stdout).contains("codex"));
    let opencode_plan = zgent(
        &home,
        &[
            "agents",
            "opencode-serve-plan",
            "--hostname",
            "127.0.0.1",
            "--port",
            "4096",
        ],
    );
    assert!(
        opencode_plan.status.success(),
        "{}",
        String::from_utf8_lossy(&opencode_plan.stderr)
    );
    assert!(String::from_utf8_lossy(&opencode_plan.stdout).contains("opencode serve"));

    let created = zgent(&home, &["task", "create", "fix", "the", "test"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_id = String::from_utf8_lossy(&created.stdout).trim().to_string();
    assert!(task_id.starts_with("task-"));

    let status = zgent(&home, &["task", "status", &task_id]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(String::from_utf8_lossy(&status.stdout).contains("workflow: plan-only"));

    let leased = zgent(&home, &["task", "lease", &task_id, "--owner", "cli-test"]);
    assert!(
        leased.status.success(),
        "{}",
        String::from_utf8_lossy(&leased.stderr)
    );
    let leased_text = String::from_utf8_lossy(&leased.stdout);
    let node_id = leased_text.split_whitespace().next().unwrap().to_string();
    assert!(node_id.starts_with("node-"));

    let heartbeat = zgent(
        &home,
        &["task", "heartbeat", &node_id, "--owner", "cli-test"],
    );
    assert!(
        heartbeat.status.success(),
        "{}",
        String::from_utf8_lossy(&heartbeat.stderr)
    );

    let complete = zgent(&home, &["task", "complete", &node_id]);
    assert!(
        complete.status.success(),
        "{}",
        String::from_utf8_lossy(&complete.stderr)
    );

    let retryable = zgent(&home, &["task", "create", "retry", "me"]);
    assert!(
        retryable.status.success(),
        "{}",
        String::from_utf8_lossy(&retryable.stderr)
    );
    let retryable_task_id = String::from_utf8_lossy(&retryable.stdout)
        .trim()
        .to_string();
    let retryable_lease = zgent(
        &home,
        &["task", "lease", &retryable_task_id, "--owner", "cli-test"],
    );
    assert!(
        retryable_lease.status.success(),
        "{}",
        String::from_utf8_lossy(&retryable_lease.stderr)
    );
    let retryable_node_id = String::from_utf8_lossy(&retryable_lease.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    assert!(
        zgent(&home, &["task", "fail", &retryable_node_id])
            .status
            .success()
    );
    let retry = zgent(&home, &["task", "retry", &retryable_node_id]);
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
    assert!(String::from_utf8_lossy(&retry.stdout).contains("retrying"));

    let cancellable = zgent(&home, &["task", "create", "cancel", "me"]);
    assert!(
        cancellable.status.success(),
        "{}",
        String::from_utf8_lossy(&cancellable.stderr)
    );
    let cancellable_task_id = String::from_utf8_lossy(&cancellable.stdout)
        .trim()
        .to_string();
    let cancel = zgent(&home, &["task", "cancel", &cancellable_task_id]);
    assert!(
        cancel.status.success(),
        "{}",
        String::from_utf8_lossy(&cancel.stderr)
    );
    assert!(String::from_utf8_lossy(&cancel.stdout).contains("cancelled"));

    let runnable = zgent(
        &home,
        &["task", "create", "--workflow", "fix-ci", "run", "fake"],
    );
    assert!(
        runnable.status.success(),
        "{}",
        String::from_utf8_lossy(&runnable.stderr)
    );
    let runnable_task_id = String::from_utf8_lossy(&runnable.stdout).trim().to_string();
    let run_next = zgent(
        &home,
        &[
            "task",
            "run-next",
            &runnable_task_id,
            "--owner",
            "cli-test",
            "--adapter",
            "fake",
            "--",
            "/bin/sh",
            "-c",
            "echo ok",
        ],
    );
    assert!(
        run_next.status.success(),
        "{}",
        String::from_utf8_lossy(&run_next.stderr)
    );
    assert!(String::from_utf8_lossy(&run_next.stdout).contains("ran node-"));

    let workflow = zgent(&home, &["workflow", "run", "fix-ci"]);
    assert!(
        workflow.status.success(),
        "{}",
        String::from_utf8_lossy(&workflow.stderr)
    );
    let workflow_task_id = String::from_utf8_lossy(&workflow.stdout).trim().to_string();
    let run_all = zgent(
        &home,
        &[
            "task",
            "run-all",
            &workflow_task_id,
            "--owner",
            "cli-test",
            "--adapter",
            "fake",
            "--",
            "/bin/sh",
            "-c",
            "echo ok",
        ],
    );
    assert!(
        run_all.status.success(),
        "{}",
        String::from_utf8_lossy(&run_all.stderr)
    );
    assert!(String::from_utf8_lossy(&run_all.stdout).contains("ran 4 node"));

    let lock = zgent(
        &home,
        &[
            "locks",
            "acquire",
            "file:src/lib.rs",
            "--owner",
            "cli-test",
            "--task",
            &task_id,
        ],
    );
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );

    let locks = zgent(&home, &["locks", "list"]);
    assert!(
        String::from_utf8_lossy(&locks.stdout).contains("file:src/lib.rs"),
        "{}",
        String::from_utf8_lossy(&locks.stdout)
    );

    let approval = zgent(
        &home,
        &[
            "approvals",
            "request",
            "--task",
            &task_id,
            "--level",
            "dangerous",
            "--reason",
            "test",
        ],
    );
    assert!(
        approval.status.success(),
        "{}",
        String::from_utf8_lossy(&approval.stderr)
    );
    let approval_id = String::from_utf8_lossy(&approval.stdout).trim().to_string();
    assert!(approval_id.starts_with("approval-"));
    let approve = zgent(&home, &["approvals", "approve", &approval_id]);
    assert!(
        approve.status.success(),
        "{}",
        String::from_utf8_lossy(&approve.stderr)
    );

    let skills = zgent(&home, &["skills", "list"]);
    assert!(skills.status.success());
    assert!(String::from_utf8_lossy(&skills.stdout).contains("code-review"));
}

#[test]
fn cli_project_mode_persists_under_repo_zgent() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let nested = repo.join("src");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();

    let init = zgent_in(&nested, &["--project", "init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(repo.join(".zgent/config.toml").exists());
    assert!(repo.join(".zgent/state/zgent.sqlite").exists());
    assert!(!nested.join(".zgent/config.toml").exists());

    let created = zgent_in(
        &nested,
        &["--project", "task", "create", "project", "state"],
    );
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_id = String::from_utf8_lossy(&created.stdout).trim().to_string();
    assert!(repo.join(".zgent/tasks").join(&task_id).exists());
}

#[test]
fn cli_dashboard_workers_gateways_marketplace_and_collaboration() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join(".zgent");
    assert!(zgent(&home, &["init"]).status.success());

    let created = zgent(&home, &["task", "create", "coordinate", "agents"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_id = String::from_utf8_lossy(&created.stdout).trim().to_string();

    let register = zgent(
        &home,
        &[
            "workers",
            "register",
            "worker-1",
            "--endpoint",
            "ssh://worker",
            "--capability",
            "codex",
        ],
    );
    assert!(
        register.status.success(),
        "{}",
        String::from_utf8_lossy(&register.stderr)
    );
    let workers = zgent(&home, &["workers", "list"]);
    assert!(String::from_utf8_lossy(&workers.stdout).contains("worker-1"));
    let dispatch = zgent(&home, &["workers", "dispatch", "worker-1", &task_id]);
    assert!(
        dispatch.status.success(),
        "{}",
        String::from_utf8_lossy(&dispatch.stderr)
    );
    let worker_run = zgent(
        &home,
        &[
            "workers",
            "run-next",
            "worker-1",
            &task_id,
            "--adapter",
            "fake",
            "--",
            "/bin/sh",
            "-c",
            "echo ok",
        ],
    );
    assert!(
        worker_run.status.success(),
        "{}",
        String::from_utf8_lossy(&worker_run.stderr)
    );
    assert!(String::from_utf8_lossy(&worker_run.stdout).contains("worker worker-1 ran"));

    let dashboard = temp.path().join("dashboard.html");
    let dashboard_out = dashboard.to_string_lossy().to_string();
    let export = zgent(&home, &["dashboard", "export", "--out", &dashboard_out]);
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(
        std::fs::read_to_string(&dashboard)
            .unwrap()
            .contains("worker-1")
    );

    let gateways = zgent(&home, &["gateways", "list"]);
    assert!(String::from_utf8_lossy(&gateways.stdout).contains("available-local"));
    let card = zgent(&home, &["gateways", "a2a-card"]);
    assert!(String::from_utf8_lossy(&card.stdout).contains("\"name\": \"zgent\""));
    let acp = zgent_with_stdin(
        &home,
        &["gateways", "acp-stdio"],
        br#"{"jsonrpc":"2.0","id":1,"method":"_zgent/task/create","params":{"goal":"from acp"}}
"#,
    );
    assert!(
        acp.status.success(),
        "{}",
        String::from_utf8_lossy(&acp.stderr)
    );
    assert!(String::from_utf8_lossy(&acp.stdout).contains("task-"));

    let plugin = temp.path().join("plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(
        plugin.join("zgent.plugin.json"),
        r#"{"schema":"zgent.plugin.v1","id":"smoke@local","name":"Smoke","version":"0.1.0"}"#,
    )
    .unwrap();
    let plugin_path = plugin.to_string_lossy().to_string();
    let add = zgent(&home, &["marketplace", "add-local", &plugin_path]);
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let market = zgent(&home, &["marketplace", "list"]);
    assert!(String::from_utf8_lossy(&market.stdout).contains("smoke@local"));
    let install = zgent(&home, &["marketplace", "install", "smoke@local"]);
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );

    let start = zgent(
        &home,
        &[
            "collaboration",
            "start",
            "--mode",
            "hosted",
            "--endpoint",
            "https://example.invalid",
        ],
    );
    assert!(
        start.status.success(),
        "{}",
        String::from_utf8_lossy(&start.stderr)
    );
    let session_id = String::from_utf8_lossy(&start.stdout).trim().to_string();
    let join = zgent(
        &home,
        &[
            "collaboration",
            "join",
            &session_id,
            "--participant",
            "reviewer",
        ],
    );
    assert!(
        join.status.success(),
        "{}",
        String::from_utf8_lossy(&join.stderr)
    );
    let sessions = zgent(&home, &["collaboration", "list"]);
    assert!(String::from_utf8_lossy(&sessions.stdout).contains("reviewer"));
}
