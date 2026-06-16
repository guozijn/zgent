use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::home::Home;

#[derive(Debug, Clone, Serialize)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

pub fn install_defaults(home: &Home) -> crate::Result<Vec<SkillInfo>> {
    let specs = [
        (
            "plan",
            "Plan",
            "Create a concise execution plan before implementation.",
        ),
        (
            "code-review",
            "Code Review",
            "Review a patch for correctness, safety, and missing tests.",
        ),
        (
            "fix-ci",
            "Fix CI",
            "Triage failing CI logs and propose a minimal fix.",
        ),
        (
            "merge-review",
            "Merge Review",
            "Check final diff, verification evidence, and approval readiness.",
        ),
    ];
    for (id, name, description) in specs {
        let dir = home.skills_dir().join(id);
        fs::create_dir_all(dir.join("references"))?;
        fs::create_dir_all(dir.join("scripts"))?;
        fs::create_dir_all(dir.join("templates"))?;
        write_if_missing(
            &dir.join("SKILL.md"),
            &format!(
                "# {name}\n\n{description}\n\nUse repository context and emit concise, auditable results.\n"
            ),
        )?;
        write_if_missing(
            &dir.join("skill.toml"),
            &format!(
                "id = \"{id}\"\nname = \"{name}\"\ndescription = \"{description}\"\nversion = \"0.1.0\"\n\n[execution]\nmode = \"read-only\"\n"
            ),
        )?;
    }
    list(home)
}

pub fn list(home: &Home) -> crate::Result<Vec<SkillInfo>> {
    list_from(&home.skills_dir())
}

pub fn list_project() -> crate::Result<Vec<SkillInfo>> {
    list_from(Path::new(".zgent/skills"))
}

fn list_from(root: &Path) -> crate::Result<Vec<SkillInfo>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut skills = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let skill_toml = entry.path().join("skill.toml");
        let text = fs::read_to_string(&skill_toml).unwrap_or_default();
        let value: toml::Value = text
            .parse()
            .unwrap_or(toml::Value::Table(Default::default()));
        skills.push(SkillInfo {
            name: value
                .get("name")
                .and_then(toml::Value::as_str)
                .unwrap_or(&id)
                .to_string(),
            version: value
                .get("version")
                .and_then(toml::Value::as_str)
                .unwrap_or("0.1.0")
                .to_string(),
            id,
            path: entry.path(),
        });
    }
    skills.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(skills)
}

fn write_if_missing(path: &Path, text: &str) -> crate::Result<()> {
    if !path.exists() {
        fs::write(path, text)?;
    }
    Ok(())
}
