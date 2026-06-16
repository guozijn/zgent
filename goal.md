# zgent Goal

## Vision

`zgent` is a local-first coordination system for AI coding agents. It should let a developer run Codex, Claude Code, Cursor Agent, opencode, and future agents as interchangeable workers under one durable control plane.

The goal is not to build another chat UI. The goal is to build the missing runtime layer around coding agents:

- durable task state
- provider-neutral event logs
- agent capability discovery
- session resume and cancellation
- file/resource locks
- patch and artifact capture
- workflow orchestration
- plugin and skill extension
- audit, policy, and observability

`zgent` owns coordination. External agents own execution.

## Product Thesis

The agent ecosystem is converging around four patterns:

1. Agents need durable execution, not only prompt loops.
2. Agents need controlled orchestration: routing, parallelism, handoffs, reviewer loops, and human approvals.
3. Agents need clear interoperability boundaries: subprocess adapters first, then ACP/A2A only when external coordination surfaces become useful.
4. Production systems need AgentOps: traces, evals, cost, latency, replay, and policy.

`zgent` should sit at the intersection of these patterns as a local agent federation controller.

## Engineering Constraints

`zgent` should be implemented in Rust.

Code should stay concise and maintainable:

- prefer small modules with clear ownership
- prefer explicit state transitions over broad abstraction
- avoid massive defensive programming
- validate data at process, file, IPC, and provider boundaries
- trust internal invariants after validation
- keep adapter code isolated from core state-machine code
- add abstractions only when they remove real duplication or clarify behavior
- keep the first implementation boring: CLI, SQLite, subprocess adapters, and clear tests

## Codex-Inspired Design Principles

Codex is a useful design reference because key parts are open source, including the CLI, SDK, app server, and skills ecosystem. `zgent` should borrow the durable product patterns, not clone the Codex UI.

Reference points:

- Codex CLI: `https://github.com/openai/codex`
- Codex SDK: `https://github.com/openai/codex/tree/main/sdk`
- Codex app server: `https://github.com/openai/codex/tree/main/codex-rs/app-server`
- OpenAI skills: `https://github.com/openai/skills`

Patterns to adopt:

1. **Local-first CLI as the primary surface.** Start with a fast terminal tool that works in a repository and can be scripted.
2. **Separate interactive and automation modes.** Keep interactive coordination distinct from non-interactive `run`/`exec` workflows.
3. **Make permissions explicit.** Represent read-only, workspace-write, and full-access modes as first-class runtime choices.
4. **Use JSONL for machine-readable automation.** Stream normalized events instead of hiding all work behind a final message.
5. **Persist resumable sessions.** Store task, run, and adapter session IDs so work can continue after interruption.
6. **Layer configuration.** Support defaults, user config, project config, profiles, and per-run overrides.
7. **Treat skills as progressive disclosure.** Load a short skill index first, then read full skill instructions only when selected.
8. **Treat plugins as installable capability bundles.** Plugins can ship skills, adapters, hooks, policies, and workflow templates.
9. **Prefer auditable review flows.** A review workflow should inspect diffs and report findings without modifying the tree.
10. **Keep app-server/daemon optional at first.** The CLI should work before a long-running daemon is mandatory.
11. **Design around git.** Prefer repository-aware workflows, patch capture, worktree isolation, and easy rollback.

Patterns to avoid copying blindly:

- provider-specific auth assumptions
- UI-specific concepts that do not help coordination
- broad feature flags before the runtime has stable primitives
- cloud-first assumptions
- complex plugin marketplace mechanics before local plugins work

## Brand Positioning

Name: `zgent`

Short description: the ultimate local agent coordinator.

Practical description: a durable control plane for coordinating coding agents across providers.

Tone:

- local-first
- precise
- inspectable
- provider-neutral
- automation-ready
- safe by default

## Home Directory

`zgent` must use `~/.zgent` as its default global home.

Proposed layout:

```text
~/.zgent/
  config.toml
  state/
    zgent.sqlite
    events.jsonl
  agents/
    default.toml
    codex.toml
    claude.toml
    cursor.toml
    opencode.toml
  adapters/
    installed.json
  skills/
    code-review/
      SKILL.md
      skill.toml
    fix-ci/
      SKILL.md
      skill.toml
  plugins/
    installed/
    cache/
  tasks/
    <task-id>/
      task.toml
      events.jsonl
      artifacts/
      patches/
      transcripts/
  worktrees/
    <repo-name>/<task-id>/<agent-id>/
  logs/
    zgentd.log
  policy/
    default.toml
    trusted-tools.toml
```

The repo should remain source code and documentation by default. When project-local coordination is requested, `zgent --project` uses the current repository's `.zgent/` directory for runtime state and project-specific declarative files.

Project-local layout:

```text
.zgent/
  config.toml
  state/
    zgent.sqlite
    events.jsonl
  tasks/
  worktrees/
  logs/
  workflows/
  skills/
  policy/
  plugins/
```

Recommended ownership:

- `~/.zgent`: user/global agent profiles, global plugins, marketplace cache, and defaults
- `.zgent`: project tasks, locks, events, patches, transcripts, project workflows, project skills, and project policy

## First Bootstrap

Yes, `zgent` should create a default agent at first bootstrap.

The default agent should be a coordinator profile, not a provider-specific LLM. Recommended name:

```text
zgent-core
```

Purpose:

- inspect local capabilities
- detect installed agents and versions
- create default provider profiles
- initialize policy and state
- route tasks to provider adapters
- avoid model calls during bootstrap unless explicitly requested

Bootstrap command:

```bash
zgent init
zgent --project init
```

Expected bootstrap behavior:

1. Create the selected home directory: `~/.zgent` by default or `.zgent` with `--project`.
2. Create `config.toml`.
3. Create SQLite state under `state/zgent.sqlite`.
4. Detect available CLIs: `codex`, `claude`, `cursor-agent`, `opencode`.
5. Detect safe capabilities without printing secrets.
6. Create `agents/default.toml`.
7. Create provider profiles for detected agents.
8. Create default skills: `plan`, `code-review`, `fix-ci`, `merge-review`.
9. Print a concise readiness report.

Default profile sketch:

```toml
[agent]
id = "zgent-core"
kind = "coordinator"
description = "Default local coordinator for routing tasks to installed coding agents."

[routing]
strategy = "capability_then_cost_then_recency"
allow_parallel = true
require_resource_locks = true

[safety]
default_mode = "review-first"
write_requires_lock = true
dangerous_commands_require_approval = true
```

## System Architecture

### Components

```text
zgent CLI
  user-facing command entrypoint

zgentd daemon
  durable local scheduler and state machine

state store
  SQLite plus append-only JSONL event export

adapter runtime
  launches and supervises external agents

normalizer
  converts provider-specific events into zgent events

resource lock manager
  protects files, directories, tools, and artifacts

workflow engine
  DAG tasks, leases, retries, dependencies, and approvals

plugin host
  loads adapters, skills, hooks, policies, and workflow templates

optional ACP/A2A gateways
  later interoperability surfaces
```

### Core Data Model

Minimum tables:

- `tasks`
- `task_nodes`
- `events`
- `agent_profiles`
- `adapter_capabilities`
- `agent_sessions`
- `resource_locks`
- `artifacts`
- `patches`
- `approvals`
- `plugins`
- `skills`
- `runs`

Node states:

```text
PENDING
RUNNING
COMPLETED
FAILED
CANCELLED
BLOCKED
WAITING_APPROVAL
```

Resource lock examples:

```text
file:src/lib.rs
dir:src
tool:cargo-test
tool:npm-install
artifact:api-spec.json
repo:/Users/me/project
```

## Adapter Contract

Every provider adapter should implement the same conceptual interface:

```text
detect()
  find binary, version, auth status, supported modes

start(task, node, workspace, options)
  launch a fresh agent run

resume(session_id, prompt, options)
  continue an existing run

cancel(session_id)
  stop or abort the run when supported

stream()
  emit normalized events

collect_result()
  final response, artifacts, diffs, status, cost metadata
```

Initial adapters:

| Adapter | Control surface | Priority |
| --- | --- | --- |
| `codex` | `codex exec --json`, SDK later | P0 |
| `claude` | `claude -p --output-format stream-json` | P0 |
| `opencode` | `opencode run --format json`, HTTP server | P0 |
| `cursor` | `cursor-agent -p --output-format stream-json` | P1 |

Normalized event types:

```text
run.started
run.resumed
run.completed
run.failed
agent.message
agent.reasoning
tool.started
tool.completed
tool.failed
file.read
file.write
patch.created
lock.requested
lock.acquired
lock.denied
lock.released
approval.requested
approval.granted
approval.denied
verification.started
verification.completed
cost.updated
```

## Workflow Model

### Basic Task Flow

```text
user goal
  -> task registered
  -> planner creates DAG
  -> scheduler leases runnable node
  -> adapter starts selected agent
  -> normalized events stream into SQLite
  -> resource locks protect edits
  -> patch/artifacts captured
  -> verification runs
  -> reviewer agent or human approves
  -> node completes
  -> dependent nodes unlock
```

### Recommended Default Workflows

#### 1. Plan Only

Use for research and architecture.

```text
planner -> reviewer -> final plan
```

#### 2. Implement With Review

Default coding workflow.

```text
planner -> implementer -> verifier -> reviewer -> merge decision
```

#### 3. Parallel Proposal

Use when multiple agents can solve the same task independently.

```text
planner
  -> codex proposal
  -> claude proposal
  -> opencode proposal
judge -> chosen patch -> verifier
```

#### 4. Fix CI

Use for failing tests or logs.

```text
triage -> implement fix -> run tests -> review diff
```

#### 5. Research Then Build

Use for unknown technologies or external docs.

```text
researcher -> planner -> implementer -> verifier -> docs update
```

## Plugin System

Plugins are installable packages that can extend `zgent` with executable behavior.

Plugin capabilities:

- provider adapters
- workflow templates
- skills
- policies
- hooks
- command aliases
- UI panels later
- exporters for traces and evals

Plugin manifest:

```json
{
  "schema": "zgent.plugin.v1",
  "id": "example@local",
  "name": "Example Plugin",
  "version": "0.1.0",
  "description": "Adds an example workflow.",
  "capabilities": {
    "adapters": [],
    "skills": ["skills/example/SKILL.md"],
    "workflows": ["workflows/example.toml"],
    "hooks": []
  }
}
```

Plugin locations:

```text
project: .zgent/plugins/
user:    ~/.zgent/plugins/installed/
```

Project plugins should require trust before execution.

## Skill System

Skills are reusable task procedures. They should be lighter than plugins and easy for agents to read.

Skill layout:

```text
skills/<name>/
  SKILL.md
  skill.toml
  references/
  scripts/
  templates/
```

Skill metadata:

```toml
id = "code-review"
name = "Code Review"
description = "Review a patch for correctness, safety, and missing tests."
version = "0.1.0"

[inputs]
requires_diff = true
requires_repo = true

[execution]
preferred_agents = ["claude", "codex"]
mode = "read-only"
```

Skills should be available to:

- `zgent` workflow planner
- users through CLI commands
- adapters through normalized task context

## ACP and A2A Strategy

ACP is useful for editor integration and coding-agent compatibility, especially because opencode already supports ACP. `zgent` should expose a small local JSON-RPC bridge first, then harden toward fuller compatibility as editor integrations become concrete.

A2A is useful for cross-host or cross-organization federation after the local coordinator is stable. `zgent` should start with agent-card discovery and task submission/status mapped onto local durable tasks, then add auth, streaming, signing, and hosted federation later.

Priority:

1. Local CLI adapters.
2. opencode HTTP/OpenAPI integration.
3. ACP compatibility.
4. A2A gateway.

## Safety Model

Default safety posture:

- no model call during `zgent init`
- no write without a resource lock
- no destructive shell command without approval
- no secrets printed in logs
- no automatic cross-agent merge
- all diffs captured before and after agent runs
- project plugins require trust
- credentials remain in each provider's own auth store

Approval levels:

```text
read-only
workspace-write
trusted-write
dangerous
```

Human approval should be required for:

- destructive commands
- dependency installation in untrusted repos
- writing outside the repo
- releasing patches into the primary checkout
- enabling untrusted plugins
- sharing data with remote agents

## Observability

`zgent` should record:

- task ID
- node ID
- agent adapter
- provider model
- session ID
- event stream
- tool calls
- file edits
- patches
- approvals
- verification commands
- exit status
- token/cost metadata when available
- wall time
- retries

OpenTelemetry export can come later. The MVP should use SQLite and JSONL first.

## Extension Boundary

`zgent` owns its runtime and persistence directly.

The integration boundary should be:

- provider adapters
- plugins
- skills
- hooks
- workflow templates
- gateway surfaces

This keeps `zgent` self-contained: it owns persistence, state transitions, scheduling, locks, events, patches, approvals, and policy.

## MVP Scope

P0:

- `zgent init`
- `~/.zgent` home creation
- project-local `.zgent` home creation
- SQLite state initialization
- adapter detection for Codex, Claude Code, opencode, Cursor Agent
- task registration
- simple DAG execution
- resource locks
- normalized event journal
- Codex adapter
- Claude adapter
- opencode adapter
- status command

P1:

- Cursor adapter
- skill loader
- workflow templates
- patch capture
- worktree isolation
- reviewer workflow

P2:

- plugin loader
- policy engine
- OpenTelemetry export
- local web dashboard
- ACP bridge

P3:

- A2A gateway
- remote workers
- agent marketplace integration
- hosted collaboration mode

## CLI Sketch

```bash
zgent init
zgent doctor
zgent agents list
zgent agents detect
zgent agents opencode-serve-plan --hostname 127.0.0.1 --port 4096
zgent agents opencode-openapi --url http://127.0.0.1:4096
zgent task create "fix the flaky auth test"
zgent task status <task-id>
zgent task events <task-id>
zgent task retry <node-id>
zgent run "review this repo for risky areas"
zgent workflow run fix-ci --log ci.log
zgent locks list
zgent plugins list
zgent skills list
zgent workers list
zgent dashboard export --out /tmp/zgent-dashboard.html
zgent gateways a2a-card
zgent marketplace list
zgent collaboration list
```

## Success Criteria

The first meaningful demo should show:

1. `zgent init` creates `~/.zgent` and detects local agents.
1. `zgent --project init` creates `.zgent` in the current repository when project-local persistence is desired.
2. A task is registered in durable state.
3. A node is leased to a provider adapter.
4. The adapter streams normalized events.
5. A file lock prevents conflicting edits.
6. A patch is captured.
7. A reviewer or verifier node runs.
8. `zgent task status` shows the full history.

## Non-Goals

For the MVP, avoid:

- building a chat UI
- replacing provider auth systems
- implementing A2A before local runtime stability
- relying on a single provider
- automatic merges without review
- remote multi-user coordination
- marketplace distribution

## Research Sources To Track

- Anthropic: Building Effective Agents
- Google Agent2Agent protocol
- Agent Client Protocol
- OpenAI Agents SDK
- Claude Agent SDK
- Azure AI agent orchestration patterns
- LangGraph durable multi-agent workflows
- Microsoft Agent Framework / AutoGen lineage
- CrewAI crews and flows
- Temporal durable execution patterns
- AgentOps / LangSmith / Langfuse observability patterns
