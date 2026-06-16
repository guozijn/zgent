# zgent Implementation Status

Last updated: 2026-06-16

## Current Repo State

`zgent` is currently a very small repository.

Existing files before this planning pass:

- `README.md`
- `LICENSE`
- `.gitignore`

Planning files added earlier:

- `goal.md`
- `impl_status.md`

Runtime files now added:

- `Cargo.toml`
- `Cargo.lock`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `docs/coordination.md`
- `src/approvals.rs`
- `src/lib.rs`
- `src/main.rs`
- `src/bin/zgentd.rs`
- `src/adapters.rs`
- `src/cli.rs`
- `src/collaboration.rs`
- `src/config.rs`
- `src/daemon.rs`
- `src/dashboard.rs`
- `src/events.rs`
- `src/gateways.rs`
- `src/home.rs`
- `src/init.rs`
- `src/locks.rs`
- `src/marketplace.rs`
- `src/normalizer.rs`
- `src/otel.rs`
- `src/patches.rs`
- `src/plugins.rs`
- `src/redact.rs`
- `src/runtime.rs`
- `src/skills.rs`
- `src/state.rs`
- `src/tasks.rs`
- `src/verify.rs`
- `src/workers.rs`
- `src/workflows.rs`
- `src/worktrees.rs`
- `tests/cli.rs`

Current planning status:

- Product and architecture plan is captured in `goal.md`.
- Implementation tracker is captured in this file.
- The coordination design is intentionally centered on local subprocess adapters first, with lightweight ACP/A2A gateway surfaces now available.
- The first Rust vertical slice is implemented and tested.
- `zgent` owns persistence directly and exposes extension through plugin and adapter interfaces.

Implemented CLI surface:

- `zgent init`
- `zgent --project init`
- `zgent doctor`
- `zgent agents detect`
- `zgent agents list`
- `zgent task create`
- `zgent task lease`
- `zgent task heartbeat`
- `zgent task run-next`
- `zgent task run-all`
- `zgent task run-provider-next`
- `zgent task run-provider-all`
- `zgent task resume-provider-next`
- `zgent task resume-provider-all`
- `zgent task capture-patch`
- `zgent task verify`
- `zgent task complete`
- `zgent task fail`
- `zgent task retry`
- `zgent task cancel`
- `zgent task status`
- `zgent task events`
- `zgent run`
- `zgent workflow list`
- `zgent workflow run`
- `zgent locks list`
- `zgent locks acquire`
- `zgent locks release`
- `zgent approvals list`
- `zgent approvals request`
- `zgent approvals approve`
- `zgent approvals deny`
- `zgent plugins list`
- `zgent plugins trust`
- `zgent plugins run-hook`
- `zgent skills list`
- `zgent worktrees create`
- `zgent workers register`
- `zgent workers list`
- `zgent workers dispatch`
- `zgent dashboard export`
- `zgent dashboard serve`
- `zgent daemon health`
- `zgent daemon task-status`
- `zgent daemon locks`
- `zgent export otel`
- `zgent gateways list`
- `zgent gateways acp-stdio`
- `zgent gateways a2a-card`
- `zgent gateways a2a-serve`
- `zgent marketplace list`
- `zgent marketplace add-local`
- `zgent marketplace install`
- `zgent collaboration start`
- `zgent collaboration join`
- `zgent collaboration list`
- `zgentd once`
- `zgentd serve`

## Local Environment Findings

Detected local agent tools:

| Tool | Local status |
| --- | --- |
| Codex CLI | installed, `codex-cli 0.139.0`; saved auth file exists under `~/.codex/auth.json` |
| Claude Code | installed, `2.1.138`; logged in |
| Cursor Agent | installed, `2026.06.03-0bbb28e`; logged in |
| opencode | installed, `1.17.7`; one DeepSeek credential configured |

Codex reference checked:

- Local Codex npm package: `@openai/codex` version `0.139.0`.
- Package repository points to `https://github.com/openai/codex`.
- Codex manual confirms open-source components: CLI, SDK, app server, skills, and universal cloud environment.
- Codex design patterns to borrow: local-first CLI, explicit sandbox/approval modes, config layering, JSONL event output, resumable sessions, skills, plugins, and review workflows.

## Design Decisions Recorded

Completed:

- Use `zgent` as the product brand.
- Use `~/.zgent` as the default home directory.
- Support project-local persistence under `.zgent` with `--project`.
- Treat `zgent` as a local-first coordination system for external coding agents.
- Use provider adapters, plugins, skills, hooks, workflow templates, and gateways as the extension boundary.
- Make `zgent-core` the default bootstrap coordinator profile.
- Avoid model calls during first bootstrap.
- Start with CLI adapters, then add gateway surfaces once local state is stable.
- Do not add a separate tool/context protocol server to the current plan.
- Use SQLite plus append-only JSONL exports for local durability.
- Use provider-neutral resource locks for file and tool coordination.
- Add skills and plugins as separate extension layers.
- Implement `zgent` in Rust.
- Keep code concise and maintainable; avoid massive defensive programming.
- Validate at external boundaries, then rely on clear internal invariants.
- Use Codex's open-source architecture as a reference point, especially CLI ergonomics, JSONL automation, permission modes, config layering, skills/plugins, and resumable sessions.

## Implementation Progress

### Documentation

Status: complete for the initial planning milestone

- [x] Capture product vision.
- [x] Capture architecture plan.
- [x] Capture `~/.zgent` home layout.
- [x] Capture project-local `.zgent` persistence mode.
- [x] Capture workflow design.
- [x] Capture plugin system plan.
- [x] Capture skill system plan.
- [x] Capture bootstrap default-agent decision.
- [x] Capture MVP phases.
- [x] Capture current implementation status.
- [x] Capture Codex-inspired design principles.
- [x] Align the plan with local subprocess adapters first and deferred ACP/A2A gateways.

Remaining documentation work should be tied to implementation changes, not treated as a blocker for starting the Rust codebase.

### Core Runtime

Status: in progress

- [x] Choose implementation language: Rust.
- [x] Choose crate/package structure.
- [x] Create `zgent` CLI entrypoint.
- [x] Create `zgentd` daemon entrypoint.
- [x] Define config format.
- [x] Initialize `~/.zgent`.
- [x] Initialize project-local `.zgent` with `--project`.
- [x] Initialize SQLite schema.
- [x] Implement task registration.
- [x] Implement DAG node model.
- [x] Implement leases and heartbeats.
- [x] Implement retry for failed, blocked, waiting, or cancelled nodes.
- [x] Implement resource locks.
- [x] Implement event journal.
- [x] Add daemon `once` scheduler entrypoint.
- [x] Add Unix-socket JSONL IPC server.
- [x] Route main `zgent` CLI through daemon IPC when desired.

### Agent Adapters

Status: in progress

- [x] Define adapter trait/interface.
- [x] Implement adapter detection.
- [x] Add adapter command-plan interface.
- [x] Add fakeable subprocess adapter runtime.
- [x] Implement explicit Codex adapter command execution path.
- [x] Implement explicit Claude Code adapter command execution path.
- [x] Implement explicit opencode adapter command execution path.
- [x] Implement explicit Cursor Agent adapter command execution path.
- [x] Normalize fake subprocess run events.
- [x] Normalize provider-style JSONL/text subprocess output.
- [x] Persist adapter session rows for fake runs.
- [x] Capture provider session IDs when emitted in JSONL output.
- [x] Support resume command plans where available.
- [x] Support coordinator-side task cancellation.

### Workflows

Status: in progress

- [x] Define built-in workflow templates in code.
- [x] Implement plan-only workflow DAG template.
- [x] Implement implement-with-review workflow DAG template.
- [x] Implement parallel-proposal workflow DAG template.
- [x] Implement fix-CI workflow DAG template.
- [x] Implement research-then-build workflow DAG template.
- [x] Add file-backed workflow template format.
- [x] Connect workflow nodes to fake subprocess execution.
- [x] Connect workflow nodes to explicit provider command execution.
- [x] Add git worktree isolation command.

### Skills

Status: in progress

- [x] Define `skill.toml`.
- [x] Load skills from `~/.zgent/skills`.
- [x] Load project skills from `.zgent/skills`.
- [x] Add default `plan` skill.
- [x] Add default `code-review` skill.
- [x] Add default `fix-ci` skill.
- [x] Add default `merge-review` skill.
- [x] Expose skills through CLI.
- [x] Expose skills through workflow templates.

### Plugins

Status: in progress

- [x] Define basic `zgent.plugin.json` manifest loader.
- [x] Load user plugins from `~/.zgent/plugins/installed`.
- [x] Load project plugins from `.zgent/plugins`.
- [x] Add basic plugin trust distinction for user vs project plugins.
- [x] Require explicit trust approval before project plugin execution.
- [x] Add plugin capability registration.
- [x] Add trusted plugin hooks.
- [x] Add local marketplace index for plugin directories.
- [x] Add local marketplace install into `plugins/installed`.

### ACP / A2A

Status: local gateway surfaces implemented

- [x] Evaluate ACP bridge after local adapters work.
- [x] Evaluate A2A gateway after the local runtime is stable.
- [x] Expose gateway status through `zgent gateways list`.
- [x] Add ACP-style JSON-RPC stdio bridge for initialize, session list/cancel, and task create/status.
- [x] Add A2A-style agent card output.
- [x] Add A2A-style HTTP+JSON endpoint for agent card discovery and task submission/status/cancel.

Full protocol compliance, streaming, auth, and signing remain hardening work.

### Deferred Future Scope

Status: local-first surfaces implemented; production hardening remains

- [x] Implement local ACP bridge.
- [x] Implement local A2A gateway.
- [x] Add local web dashboard.
- [x] Add remote worker registration and dispatch.
- [x] Add local agent marketplace integration.
- [x] Add hosted collaboration session records.

### Safety And Policy

Status: in progress

- [x] Define policy files under `~/.zgent/policy`.
- [x] Implement approval levels.
- [x] Require declared locks before write-capable provider execution.
- [x] Require approval before locally detected dangerous shell commands execute.
- [x] Record approvals in event journal.
- [x] Redact secrets from event logs.
- [x] Add project trust checks.

### Observability

Status: in progress

- [x] Record task and lock events.
- [x] Record fake subprocess run events.
- [x] Record transcript artifacts for fake subprocess runs.
- [x] Record patch capture events.
- [x] Record session lifecycle events.
- [x] Record token/cost metadata events when emitted by adapters.
- [x] Record verification runs.
- [x] Export global and task events as JSONL.
- [x] Add `zgent task events`.
- [x] Add optional OpenTelemetry-shaped JSON exporter.

### Tests

Status: in progress

- [x] Unit test adapter specs.
- [x] Unit test adapter command plans.
- [x] Unit test adapter resume command plans.
- [x] Unit test provider-style JSONL normalization.
- [x] Unit test home initialization.
- [x] Unit test task, event, and lock state.
- [x] Unit test DAG leasing and completion.
- [x] Unit test fake subprocess execution and transcript capture.
- [x] Unit test provider session ID capture from JSONL output.
- [x] Unit test required lock enforcement before node leasing.
- [x] Unit test dangerous-command approval gating and release back to pending.
- [x] Unit test task cancellation.
- [x] Unit test event secret redaction.
- [x] Unit test file-backed workflow templates with skill references.
- [x] Unit test git patch capture.
- [x] Unit test approval decisions.
- [x] Unit test agent session lifecycle.
- [x] Integration test CLI init, adapter list, task lifecycle, locks, and skills.
- [x] Add provider-runtime tests with fake subprocesses.
- [x] Add plugin trust tests.
- [x] Unit test plugin capability parsing.
- [x] Unit test trusted plugin hook execution.
- [x] Unit test daemon health and task-status request handling.
- [x] Unit test dashboard rendering.
- [x] Unit test remote worker registration and dispatch.
- [x] Unit test local marketplace add/install.
- [x] Unit test collaboration start/join/list.
- [x] Unit test ACP task create/status gateway.
- [x] Unit test A2A message-send gateway.
- [x] Unit test verification run recording.
- [x] Unit test git worktree isolation.
- [x] Unit test OpenTelemetry-shaped export.
- [x] Add workflow execution tests with fake adapters.
- [x] Add CLI smoke coverage for project-local persistence.
- [x] Add CLI smoke coverage for failed-node retry.
- [x] Add CLI smoke coverage for dashboard, workers, gateways, marketplace, and collaboration.

## Recommended Next Implementation Step

The smallest useful vertical slice is now implemented:

```text
zgent init
  -> creates ~/.zgent
  -> writes config.toml
  -> detects installed adapters
  -> creates SQLite state
  -> creates zgent-core default agent profile
  -> prints readiness report
```

Next implementation step:

```text
real provider execution behind explicit commands
  -> Codex/Claude/opencode subprocess runners
  -> provider event-stream normalizers
  -> provider session ID capture
  -> lock enforcement before write-capable runs
  -> fake subprocess regression tests remain the default gate
```

Real provider runs should stay opt-in so tests and bootstrap never spend credits.

## Open Technical Questions

- Should the first IPC be Unix Domain Socket JSONL, HTTP, or both?
- Should worktree isolation be mandatory for all write-capable workflows?
- Should `zgent-core` be purely deterministic code, or eventually backed by a preferred default LLM?

## Current Recommendation

Use Rust for the durable daemon and CLI. Keep persistence inside `zgent` and expose extension through adapters/plugins/skills/hooks/workflows/gateways. The local runtime path is now in place: global `init`, project-local `--project init`, config, SQLite state, adapter detection, tasks, leases, locks, fake subprocess execution, events, sessions, patch capture, approvals, skills, plugins, marketplace, dashboard, workers, collaboration records, gateway surfaces, and tests. Keep full remote federation, gateway auth, streaming, and hosted multi-user execution as hardening layers on top of this local core.
