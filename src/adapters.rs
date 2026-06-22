use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::home::Home;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub id: String,
    pub command: String,
    pub binary: Option<String>,
    pub installed: bool,
    pub version: Option<String>,
    pub auth_status: String,
    pub control_surface: String,
    pub priority: String,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPlan {
    pub adapter: String,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterManifest {
    pub id: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub command: String,
    #[serde(default)]
    pub start_args: Vec<String>,
    #[serde(default)]
    pub resume_args: Vec<String>,
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub permission_modes: Vec<String>,
    #[serde(default)]
    pub trusted: bool,
}

pub trait AgentAdapter {
    fn detect(&self) -> AdapterInfo;
    fn start(&self, prompt: &str) -> CommandPlan;
    fn resume(&self, _session_id: &str, _prompt: &str) -> Option<CommandPlan> {
        None
    }
    fn cancel(&self, _session_id: &str) -> Option<CommandPlan> {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct AdapterSpec {
    id: &'static str,
    command: &'static str,
    control_surface: &'static str,
    priority: &'static str,
    auth_file: Option<&'static str>,
}

const SPECS: &[AdapterSpec] = &[
    AdapterSpec {
        id: "codex",
        command: "codex",
        control_surface: "codex exec --json",
        priority: "P0",
        auth_file: Some(".codex/auth.json"),
    },
    AdapterSpec {
        id: "claude",
        command: "claude",
        control_surface: "claude -p --output-format stream-json",
        priority: "P0",
        auth_file: Some(".claude.json"),
    },
    AdapterSpec {
        id: "opencode",
        command: "opencode",
        control_surface: "opencode run --format json",
        priority: "P0",
        auth_file: Some(".local/share/opencode/auth.json"),
    },
    AdapterSpec {
        id: "cursor",
        command: "cursor-agent",
        control_surface: "cursor-agent -p --output-format stream-json",
        priority: "P1",
        auth_file: None,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct BuiltinAdapter {
    spec: AdapterSpec,
}

impl AgentAdapter for BuiltinAdapter {
    fn detect(&self) -> AdapterInfo {
        detect(&self.spec)
    }

    fn start(&self, prompt: &str) -> CommandPlan {
        let args = match self.spec.id {
            "codex" => vec!["exec", "--json", prompt],
            "claude" => vec!["-p", prompt, "--output-format", "stream-json"],
            "opencode" => vec!["run", "--format", "json", prompt],
            "cursor" => vec!["-p", prompt, "--output-format", "stream-json"],
            _ => vec![prompt],
        };
        CommandPlan {
            adapter: self.spec.id.to_string(),
            program: self.spec.command.to_string(),
            args: args.into_iter().map(str::to_string).collect(),
        }
    }

    fn resume(&self, session_id: &str, prompt: &str) -> Option<CommandPlan> {
        let args = match self.spec.id {
            "codex" => vec!["exec", "resume", "--json", session_id, prompt],
            "claude" => vec![
                "--resume",
                session_id,
                "-p",
                prompt,
                "--output-format",
                "stream-json",
            ],
            "opencode" => vec!["run", "--format", "json", "--session", session_id, prompt],
            "cursor" => vec![
                "--print",
                "--output-format",
                "stream-json",
                "--resume",
                session_id,
                prompt,
            ],
            _ => return None,
        };
        Some(CommandPlan {
            adapter: self.spec.id.to_string(),
            program: self.spec.command.to_string(),
            args: args.into_iter().map(str::to_string).collect(),
        })
    }
}

impl BuiltinAdapter {
    pub fn id(&self) -> &'static str {
        self.spec.id
    }
}

pub fn builtin_adapters() -> Vec<BuiltinAdapter> {
    SPECS
        .iter()
        .copied()
        .map(|spec| BuiltinAdapter { spec })
        .collect()
}

pub fn builtin_adapter(id: &str) -> Option<BuiltinAdapter> {
    builtin_adapters()
        .into_iter()
        .find(|adapter| adapter.spec.id == id)
}

pub fn detect_all() -> Vec<AdapterInfo> {
    builtin_adapters()
        .iter()
        .map(AgentAdapter::detect)
        .collect()
}

pub fn write_manifest(home: &Home, info: &AdapterInfo) -> crate::Result<()> {
    fs::create_dir_all(home.adapters_dir())?;
    let manifest = manifest_for_info(info);
    fs::write(
        home.adapters_dir().join(format!("{}.toml", manifest.id)),
        toml::to_string_pretty(&manifest)?,
    )?;
    Ok(())
}

pub fn load_manifests(home: &Home) -> crate::Result<Vec<AdapterManifest>> {
    let dir = home.adapters_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut manifests = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let manifest: AdapterManifest = toml::from_str(&fs::read_to_string(path)?)?;
        manifests.push(manifest);
    }
    manifests.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(manifests)
}

pub fn registered_infos(home: &Home) -> crate::Result<Vec<AdapterInfo>> {
    let detected = detect_all();
    let detected_by_id: BTreeMap<_, _> = detected
        .into_iter()
        .map(|adapter| (adapter.id.clone(), adapter))
        .collect();
    let manifests = load_manifests(home)?;
    if manifests.is_empty() {
        return Ok(detected_by_id.into_values().collect());
    }
    let mut infos = Vec::new();
    for manifest in manifests {
        let detected = detected_by_id.get(&manifest.id);
        let binary = find_binary(&manifest.command);
        infos.push(AdapterInfo {
            id: manifest.id,
            command: manifest.command,
            installed: binary.is_some(),
            binary: binary.map(|path| path.display().to_string()),
            version: detected.and_then(|adapter| adapter.version.clone()),
            auth_status: detected
                .map(|adapter| adapter.auth_status.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            control_surface: manifest.output,
            priority: detected
                .map(|adapter| adapter.priority.clone())
                .unwrap_or_else(|| "custom".to_string()),
            modes: manifest.capabilities,
        });
    }
    Ok(infos)
}

pub fn plan_start(home: &Home, adapter_id: &str, prompt: &str) -> crate::Result<CommandPlan> {
    if let Some(manifest) = load_manifests(home)?
        .into_iter()
        .find(|manifest| manifest.id == adapter_id)
    {
        return Ok(plan_from_manifest(
            &manifest,
            &manifest.start_args,
            prompt,
            None,
        ));
    }
    let adapter = builtin_adapter(adapter_id)
        .ok_or_else(|| anyhow::anyhow!("unknown adapter: {adapter_id}"))?;
    Ok(AgentAdapter::start(&adapter, prompt))
}

pub fn plan_resume(
    home: &Home,
    adapter_id: &str,
    session_id: &str,
    prompt: &str,
) -> crate::Result<CommandPlan> {
    if let Some(manifest) = load_manifests(home)?
        .into_iter()
        .find(|manifest| manifest.id == adapter_id)
    {
        if manifest.resume_args.is_empty() {
            bail!("adapter does not support resume: {adapter_id}");
        }
        return Ok(plan_from_manifest(
            &manifest,
            &manifest.resume_args,
            prompt,
            Some(session_id),
        ));
    }
    let adapter = builtin_adapter(adapter_id)
        .ok_or_else(|| anyhow::anyhow!("unknown adapter: {adapter_id}"))?;
    AgentAdapter::resume(&adapter, session_id, prompt)
        .ok_or_else(|| anyhow::anyhow!("adapter does not support resume: {adapter_id}"))
}

pub fn command_from_plan(plan: CommandPlan) -> Vec<String> {
    let mut command = vec![plan.program];
    command.extend(plan.args);
    command
}

fn manifest_for_info(info: &AdapterInfo) -> AdapterManifest {
    let builtin = builtin_adapter(&info.id);
    let (start_args, resume_args) = if let Some(adapter) = builtin {
        let start = AgentAdapter::start(&adapter, "{prompt}");
        let resume = AgentAdapter::resume(&adapter, "{session_id}", "{prompt}");
        (start.args, resume.map(|plan| plan.args).unwrap_or_default())
    } else {
        (vec!["{prompt}".to_string()], Vec::new())
    };
    AdapterManifest {
        id: info.id.clone(),
        kind: "provider".to_string(),
        command: info.command.clone(),
        start_args,
        resume_args,
        output: info.control_surface.clone(),
        capabilities: info.modes.clone(),
        permission_modes: vec!["review-first".to_string(), "yolo".to_string()],
        trusted: info.installed,
    }
}

fn plan_from_manifest(
    manifest: &AdapterManifest,
    args: &[String],
    prompt: &str,
    session_id: Option<&str>,
) -> CommandPlan {
    CommandPlan {
        adapter: manifest.id.clone(),
        program: manifest.command.clone(),
        args: args
            .iter()
            .map(|arg| {
                arg.replace("{prompt}", prompt)
                    .replace("{session_id}", session_id.unwrap_or(""))
            })
            .collect(),
    }
}

fn detect(spec: &AdapterSpec) -> AdapterInfo {
    let binary = find_binary(spec.command);
    let version = binary.as_deref().and_then(command_version);
    AdapterInfo {
        id: spec.id.to_string(),
        command: spec.command.to_string(),
        installed: binary.is_some(),
        binary: binary.map(|path| path.display().to_string()),
        version,
        auth_status: auth_status(spec.auth_file),
        control_surface: spec.control_surface.to_string(),
        priority: spec.priority.to_string(),
        modes: vec![
            "detect".to_string(),
            "start".to_string(),
            "resume".to_string(),
            "cancel".to_string(),
            "stream".to_string(),
            "collect_result".to_string(),
        ],
    }
}

fn default_kind() -> String {
    "provider".to_string()
}

fn default_output() -> String {
    "stream-json".to_string()
}

fn command_version(binary: &Path) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr)
    } else {
        String::from_utf8_lossy(&output.stdout)
    };
    let version = text.trim();
    (!version.is_empty()).then(|| version.to_string())
}

fn auth_status(relative_auth_file: Option<&str>) -> String {
    let Some(relative) = relative_auth_file else {
        return "unknown".to_string();
    };
    let Some(home) = env::var_os("HOME") else {
        return "unknown".to_string();
    };
    if PathBuf::from(home).join(relative).exists() {
        "configured".to_string()
    } else {
        "not-detected".to_string()
    }
}

fn find_binary(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(command))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
mod tests {
    #[test]
    fn detects_known_adapter_specs() {
        let adapters = super::detect_all();
        assert_eq!(adapters.len(), 4);
        assert!(adapters.iter().any(|adapter| adapter.id == "codex"));
        assert!(adapters.iter().any(|adapter| adapter.id == "claude"));
        assert!(adapters.iter().any(|adapter| adapter.id == "opencode"));
        assert!(adapters.iter().any(|adapter| adapter.id == "cursor"));
    }

    #[test]
    fn builds_provider_start_command_plans() {
        let adapters = super::builtin_adapters();
        let codex = adapters
            .iter()
            .find(|adapter| adapter.spec.id == "codex")
            .unwrap();
        let plan = super::AgentAdapter::start(codex, "hello");
        assert_eq!(plan.program, "codex");
        assert_eq!(plan.args, ["exec", "--json", "hello"]);
    }

    #[test]
    fn builds_provider_resume_command_plans() {
        let adapters = super::builtin_adapters();
        let opencode = adapters
            .iter()
            .find(|adapter| adapter.spec.id == "opencode")
            .unwrap();
        let plan =
            super::AgentAdapter::resume(opencode, "session-1", "continue").expect("resume plan");
        assert_eq!(plan.program, "opencode");
        assert_eq!(
            plan.args,
            [
                "run",
                "--format",
                "json",
                "--session",
                "session-1",
                "continue"
            ]
        );
    }

    #[test]
    fn writes_and_loads_adapter_manifests() {
        let temp = tempfile::tempdir().unwrap();
        let home = crate::home::Home::from_path(temp.path().join(".zgent"));
        let info = super::AdapterInfo {
            id: "custom".to_string(),
            command: "echo".to_string(),
            binary: None,
            installed: false,
            version: None,
            auth_status: "unknown".to_string(),
            control_surface: "text".to_string(),
            priority: "custom".to_string(),
            modes: vec!["start".to_string()],
        };
        super::write_manifest(&home, &info).unwrap();
        let manifests = super::load_manifests(&home).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].id, "custom");
        let plan = super::plan_start(&home, "custom", "hello").unwrap();
        assert_eq!(plan.program, "echo");
        assert_eq!(plan.args, ["hello"]);
    }
}
