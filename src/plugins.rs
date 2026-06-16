use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::home::Home;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub schema: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub capabilities: PluginCapabilities,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginCapabilities {
    #[serde(default)]
    pub adapters: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub workflows: Vec<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub trusted: bool,
    pub capabilities: PluginCapabilities,
}

pub fn list(home: &Home) -> crate::Result<Vec<PluginInfo>> {
    list_for_project(home, Path::new("."))
}

pub fn list_for_project(home: &Home, project_root: &Path) -> crate::Result<Vec<PluginInfo>> {
    let mut plugins = Vec::new();
    let trusted = trusted_ids(home)?;
    plugins.extend(list_from(&home.plugins_dir().join("installed"), true)?);
    plugins.extend(list_from_project(
        &project_root.join(".zgent/plugins"),
        &trusted,
    )?);
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plugins)
}

pub fn trust(home: &Home, plugin_id: &str) -> crate::Result<()> {
    let mut trusted = trusted_ids(home)?;
    trusted.insert(plugin_id.to_string());
    let mut ids: Vec<_> = trusted.into_iter().collect();
    ids.sort();
    fs::write(trusted_file(home), serde_json::to_string_pretty(&ids)?)?;
    Ok(())
}

pub fn run_hook(
    home: &Home,
    project_root: &Path,
    plugin_id: &str,
    hook: &str,
    args: &[String],
) -> crate::Result<std::process::Output> {
    let plugin = list_for_project(home, project_root)?
        .into_iter()
        .find(|plugin| plugin.id == plugin_id)
        .ok_or_else(|| anyhow::anyhow!("plugin not found: {plugin_id}"))?;
    if !plugin.trusted {
        bail!("plugin is not trusted: {plugin_id}");
    }
    if !plugin
        .capabilities
        .hooks
        .iter()
        .any(|declared| declared == hook)
    {
        bail!("hook `{hook}` is not declared by plugin `{plugin_id}`");
    }
    let hook_path = plugin.path.join(hook);
    let output = Command::new(&hook_path)
        .args(args)
        .current_dir(&plugin.path)
        .output()?;
    Ok(output)
}

fn list_from(root: &Path, trusted: bool) -> crate::Result<Vec<PluginInfo>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut plugins = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("zgent.plugin.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest: PluginManifest = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
        plugins.push(PluginInfo {
            id: manifest.id,
            name: manifest.name,
            version: manifest.version,
            path: entry.path(),
            trusted,
            capabilities: manifest.capabilities,
        });
    }
    Ok(plugins)
}

fn list_from_project(root: &Path, trusted: &HashSet<String>) -> crate::Result<Vec<PluginInfo>> {
    let mut plugins = list_from(root, false)?;
    for plugin in &mut plugins {
        plugin.trusted = trusted.contains(&plugin.id);
    }
    Ok(plugins)
}

fn trusted_ids(home: &Home) -> crate::Result<HashSet<String>> {
    let path = trusted_file(home);
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let ids: Vec<String> = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(ids.into_iter().collect())
}

fn trusted_file(home: &Home) -> PathBuf {
    home.plugins_dir().join("trusted.json")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::home::Home;

    #[test]
    fn lists_user_and_project_plugins_with_trust_state() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        let user_plugin = home.plugins_dir().join("installed/example");
        fs::create_dir_all(&user_plugin)?;
        fs::write(
            user_plugin.join("zgent.plugin.json"),
            r#"{
                "schema":"zgent.plugin.v1",
                "id":"user@example",
                "name":"User",
                "version":"0.1.0",
                "capabilities": {
                    "skills": ["skills/review/SKILL.md"],
                    "workflows": ["workflows/review.toml"],
                    "hooks": ["hooks/pre-run.sh"]
                }
            }"#,
        )?;

        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".zgent/plugins/project-example"))?;
        fs::write(
            project.join(".zgent/plugins/project-example/zgent.plugin.json"),
            r#"{"schema":"zgent.plugin.v1","id":"project@example","name":"Project","version":"0.1.0"}"#,
        )?;
        let plugins = super::list_for_project(&home, &project)?;
        let user = plugins
            .iter()
            .find(|plugin| plugin.id == "user@example")
            .unwrap();
        let project = plugins
            .iter()
            .find(|plugin| plugin.id == "project@example")
            .unwrap();
        assert!(user.trusted);
        assert_eq!(user.capabilities.skills, ["skills/review/SKILL.md"]);
        assert_eq!(user.capabilities.workflows, ["workflows/review.toml"]);
        assert_eq!(user.capabilities.hooks, ["hooks/pre-run.sh"]);
        assert!(!project.trusted);

        super::trust(&home, "project@example")?;
        let plugins = super::list_for_project(&home, &temp.path().join("project"))?;
        let project = plugins
            .iter()
            .find(|plugin| plugin.id == "project@example")
            .unwrap();
        assert!(project.trusted);
        Ok(())
    }

    #[test]
    fn runs_only_trusted_declared_hooks() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        let plugin = home.plugins_dir().join("installed/hooky");
        fs::create_dir_all(plugin.join("hooks"))?;
        fs::write(
            plugin.join("zgent.plugin.json"),
            r#"{
                "schema":"zgent.plugin.v1",
                "id":"hooky@example",
                "name":"Hooky",
                "version":"0.1.0",
                "capabilities": { "hooks": ["hooks/echo.sh"] }
            }"#,
        )?;
        let hook = plugin.join("hooks/echo.sh");
        fs::write(&hook, "#!/bin/sh\necho hook:$1\n")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
        }
        let output = super::run_hook(
            &home,
            temp.path(),
            "hooky@example",
            "hooks/echo.sh",
            &["ok".to_string()],
        )?;
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hook:ok");
        assert!(
            super::run_hook(&home, temp.path(), "hooky@example", "hooks/missing.sh", &[]).is_err()
        );
        Ok(())
    }
}
