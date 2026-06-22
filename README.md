# zgent

Local-first coordinator for AI coding agents. **zgent** owns durable task state, event logs, locks, workflows, skills, and audit trails. External agents (Codex, Claude, Cursor, OpenCode, and others) own execution.

Built in Rust as a small, useful vertical slice: bootstrap local state, detect installed agents, register tasks, record events, and coordinate runs through a CLI or TUI.

## Quickstart

**1. Install and initialize**

```bash
cargo install --path .
zgent init
```

Use `zgent --project init` to store state under the current repo's `.zgent/` instead of `~/.zgent`.

**2. Check your setup**

```bash
zgent doctor
zgent agents list
```

**3. Run a task**

```bash
# One-shot prompt (uses detected adapters)
zgent run "review this repo for risky areas"

# Or create a tracked task and run it with a provider
zgent task create "fix the flaky auth test"
zgent task run-provider-next <task-id> --adapter codex --owner codex -- "fix the task"
zgent task status <task-id>
```

**4. Use the TUI (optional)**

Running `zgent` with no subcommand opens the interactive TUI, which talks to the local daemon over a Unix socket.

```bash
zgent daemon serve   # in one terminal
zgent                # in another
```

## What you get

After `zgent init`, your home (or project) directory includes:

| Path | Purpose |
|------|---------|
| `config.toml` | Runtime configuration |
| `state/zgent.sqlite` | Tasks, locks, sessions, approvals |
| `state/events.jsonl` | Append-only event log |
| `adapters/*.toml` | Provider adapter manifests |
| `workflows/` | Workflow templates |
| `skills/` | Built-in skills (`plan`, `code-review`, `fix-ci`, `merge-review`) |

Bootstrap does not make model calls. Project-local runtime under `.zgent/state`, `.zgent/tasks`, `.zgent/worktrees`, and `.zgent/logs` is gitignored; declarative files like workflows, skills, and policy can be committed.

## Core concepts

**Tasks and workflows** — Create a task with a goal, lease nodes in a DAG, run agents against them, capture patches, verify, and complete or retry. Workflows chain nodes (planner → reviewer, etc.) from TOML templates in `workflows/`.

**Adapters** — Detected coding agents are registered as provider adapters. zgent normalizes their output into a shared event stream. Custom adapters are plain TOML manifests with start/resume args and capabilities.

**Locks and approvals** — Serialize access to repos or files with resource locks. Dangerous operations can require explicit approval before a run proceeds.

**Plugins and skills** — Extend zgent with local plugins (`zgent.plugin.json`), trusted hooks, marketplace installs, and skill files.

**Workers and gateways** — Dispatch tasks to remote workers, export OpenTelemetry traces, serve a dashboard, or expose A2A/ACP gateway surfaces for external integration.

## Configuration

| Flag / env | Effect |
|------------|--------|
| `--home <path>` or `ZGENT_HOME` | Override the zgent home directory (useful for tests) |
| `--project` | Use `.zgent/` in the current repository |
| `--permission-mode yolo` | Bypass local approval and lock gates for a single run |

## Extending zgent

**Workflow template** — Add `~/.zgent/workflows/<name>.toml` or `.zgent/workflows/<name>.toml`:

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

**Adapter manifest** — Add or edit `adapters/<provider>.toml`:

```toml
id = "cursor"
kind = "provider"
command = "cursor-agent"
start_args = ["-p", "{prompt}", "--output-format", "stream-json"]
resume_args = ["--print", "--output-format", "stream-json", "--resume", "{session_id}", "{prompt}"]
capabilities = ["detect", "start", "resume", "stream", "collect_result"]
permission_modes = ["review-first", "yolo"]
trusted = true
```

**Plugin manifest** — Declare skills, workflows, and hooks in `zgent.plugin.json`. User plugins under `~/.zgent/plugins/installed` are trusted by default; project plugins require `zgent plugins trust <plugin-id>`.

## CLI reference

Run `zgent --help` or `zgent <command> --help` for the full command tree. Main areas:

| Area | Examples |
|------|----------|
| Setup | `init`, `doctor`, `agents detect`, `agents list` |
| Tasks | `task create`, `task run-provider-next`, `task status`, `task events`, `run` |
| Workflows | `workflow list`, `workflow run fix-ci` |
| Coordination | `locks acquire`, `approvals request`, `worktrees create` |
| Ops | `daemon serve`, `dashboard serve`, `export otel` |
| Extensions | `plugins list`, `skills list`, `marketplace install` |

## Documentation

- [docs/coordination.md](docs/coordination.md) — persistence, plugins, gateways, workers, and collaboration model

## Development

```bash
cargo fmt --all
cargo test
```

The test suite covers adapter specs, SQLite state, DAG leasing, lock enforcement, workflow completion, approval gates, daemon handling, and CLI paths. GitHub Actions run formatting, clippy, tests, packaging, and tagged binary releases.
