# zgent

`zgent` is a local-first coordinator for AI coding agents. It owns durable task
state, normalized event logs, adapter detection, resource locks, workflows,
skills, plugins, and audit trails while external agents own execution.

The implementation is intentionally Rust-first and starts with the smallest
useful local vertical slice from [goal.md](goal.md): bootstrap `~/.zgent`,
detect installed coding agents, create SQLite state, register tasks, record
events, and expose concise CLI commands.

## Current Commands

```bash
cargo run -- init
cargo run -- --project init
cargo run -- doctor
cargo run -- agents detect
cargo run -- agents list
cargo run -- task create "fix the flaky auth test"
cargo run -- task lease <task-id> --owner codex
cargo run -- task heartbeat <node-id> --owner codex
cargo run -- task run-next <task-id> --owner fake --adapter fake -- /bin/sh -c "echo ok"
cargo run -- task run-all <task-id> --owner fake --adapter fake -- /bin/sh -c "echo ok"
cargo run -- task run-provider-next <task-id> --adapter codex --owner codex --require-lock repo:/path -- "fix the task"
cargo run -- task run-provider-all <task-id> --adapter claude --owner claude --require-lock repo:/path
cargo run -- task resume-provider-next <task-id> --adapter codex --session-id <session-id> --require-lock repo:/path
cargo run -- task resume-provider-all <task-id> --adapter opencode --session-id <session-id> --require-lock repo:/path
cargo run -- task capture-patch <task-id> --repo .
cargo run -- task verify <task-id> -- cargo test
cargo run -- task complete <node-id>
cargo run -- task fail <node-id>
cargo run -- task cancel <task-id>
cargo run -- task status <task-id>
cargo run -- task events <task-id>
cargo run -- run "review this repo for risky areas"
cargo run -- workflow list
cargo run -- workflow run fix-ci --log ci.log
cargo run -- locks list
cargo run -- locks acquire file:src/lib.rs --owner codex --task <task-id>
cargo run -- locks release file:src/lib.rs --owner codex
cargo run -- approvals request --task <task-id> --level dangerous --reason "needs install"
cargo run -- approvals approve <approval-id>
cargo run -- approvals deny <approval-id>
cargo run -- approvals list
cargo run -- plugins list
cargo run -- plugins trust <plugin-id>
cargo run -- plugins run-hook <plugin-id> hooks/pre-run.sh -- arg1 arg2
cargo run -- skills list
cargo run -- worktrees create <task-id> --agent codex --repo .
cargo run -- workers register worker-1 --endpoint ssh://worker --capability codex
cargo run -- workers list
cargo run -- workers dispatch worker-1 <task-id>
cargo run -- dashboard export --out /tmp/zgent-dashboard.html
cargo run -- dashboard serve --addr 127.0.0.1:8765
cargo run --bin zgentd -- once <task-id> --owner zgentd --adapter fake -- /bin/sh -c "echo ok"
cargo run --bin zgentd -- serve --socket /tmp/zgentd.sock
cargo run -- daemon health --socket /tmp/zgentd.sock
cargo run -- daemon task-status <task-id> --socket /tmp/zgentd.sock
cargo run -- daemon locks --socket /tmp/zgentd.sock
cargo run -- export otel <task-id> --out /tmp/zgent-otel.json
cargo run -- gateways list
cargo run -- gateways a2a-card --base-url http://127.0.0.1:8766
cargo run -- gateways a2a-serve --addr 127.0.0.1:8766
cargo run -- gateways acp-stdio
cargo run -- marketplace add-local ./path/to/plugin
cargo run -- marketplace list
cargo run -- marketplace install <plugin-id>
cargo run -- collaboration start --mode hosted --endpoint https://example.invalid
cargo run -- collaboration join <session-id> --participant reviewer
cargo run -- collaboration list
```

Use `--home <path>` or `ZGENT_HOME` for tests and isolated experiments. Use
`--project` to persist under the current repository's `.zgent/`. Without an
override, `zgent` uses `~/.zgent`.

## Bootstrap

```bash
cargo run -- --home /tmp/zgent-home init
cargo run -- --project init
```

`zgent init` creates:

- `config.toml`
- SQLite state at `state/zgent.sqlite`
- append-only event log at `state/events.jsonl`
- `agents/default.toml` for `zgent-core`
- provider profiles for detected agents
- `adapters/installed.json`
- default skills: `plan`, `code-review`, `fix-ci`, `merge-review`
- file-backed workflow template directory at `workflows/`
- policy, task, plugin, worktree, and log directories

Bootstrap does not make model calls.

Project-local runtime state under `.zgent/state`, `.zgent/tasks`,
`.zgent/worktrees`, and `.zgent/logs` is ignored by `.gitignore`. Declarative
project files such as `.zgent/workflows`, `.zgent/skills`, `.zgent/policy`, and
`.zgent/plugins` can be committed deliberately.

Extension happens through provider adapters, plugins, skills, hooks, workflow
templates, and gateway surfaces.

See [docs/coordination.md](docs/coordination.md) for the local persistence,
plugin, gateway, worker, and collaboration model.

## Workflow Templates

Built-in workflows are available by default. User and project templates can be
added at `~/.zgent/workflows/<name>.toml` or `.zgent/workflows/<name>.toml`:

```toml
[[nodes]]
name = "plan"
role = "planner"
skill = "plan"

[[nodes]]
name = "review"
role = "reviewer"
depends_on = ["plan"]
skill = "code-review"
```

## Plugins

Plugin manifests live in `zgent.plugin.json` and can declare local capabilities:

```json
{
  "schema": "zgent.plugin.v1",
  "id": "review@local",
  "name": "Review",
  "version": "0.1.0",
  "capabilities": {
    "adapters": [],
    "skills": ["skills/review/SKILL.md"],
    "workflows": ["workflows/review.toml"],
    "hooks": ["hooks/pre-run.sh"]
  }
}
```

User plugins under `~/.zgent/plugins/installed` are trusted. Project plugins
under `.zgent/plugins` require `zgent plugins trust <plugin-id>` before they are
treated as trusted. Hooks only run when the plugin is trusted and the hook path
is declared in the manifest.

## Development

```bash
cargo fmt --all
cargo test
```

The test suite covers adapter specs and command plans, home initialization,
SQLite task/event/lock/session/approval state, DAG leasing, fake subprocess
execution, provider-style JSONL normalization, required-lock enforcement,
workflow completion, task cancellation, event secret redaction, git patch
capture, verification recording, file-backed workflow templates, persistent
plugin trust, worktree isolation, daemon request handling, and the CLI
bootstrap/task/lock/approval/skill/export/gateway path. GitHub Actions run
formatting, clippy, tests, crate packaging, and tagged binary releases.
