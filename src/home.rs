use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};

#[derive(Debug, Clone)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    pub fn resolve(explicit: Option<PathBuf>, project: bool) -> crate::Result<Self> {
        if let Some(root) = explicit {
            return Ok(Self { root });
        }
        if project {
            return Self::for_project_from(env::current_dir()?);
        }
        if let Some(root) = env::var_os("ZGENT_HOME") {
            return Ok(Self {
                root: PathBuf::from(root),
            });
        }
        let home = env::var_os("HOME").context("HOME is not set; pass --home or set ZGENT_HOME")?;
        Ok(Self {
            root: PathBuf::from(home).join(".zgent"),
        })
    }

    pub fn for_project_from(start: impl AsRef<Path>) -> crate::Result<Self> {
        let root = find_repo_root(start.as_ref())?;
        Ok(Self {
            root: root.join(".zgent"),
        })
    }

    pub fn from_path(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn db(&self) -> PathBuf {
        self.state_dir().join("zgent.sqlite")
    }

    pub fn global_events(&self) -> PathBuf {
        self.state_dir().join("events.jsonl")
    }

    pub fn agents_dir(&self) -> PathBuf {
        self.root.join("agents")
    }

    pub fn adapters_dir(&self) -> PathBuf {
        self.root.join("adapters")
    }

    pub fn adapters_file(&self) -> PathBuf {
        self.adapters_dir().join("installed.json")
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.root.join("plugins")
    }

    pub fn marketplace_file(&self) -> PathBuf {
        self.plugins_dir().join("cache").join("marketplace.json")
    }

    pub fn collaboration_dir(&self) -> PathBuf {
        self.root.join("collaboration")
    }

    pub fn workflows_dir(&self) -> PathBuf {
        self.root.join("workflows")
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    pub fn task_dir(&self, task_id: &str) -> PathBuf {
        self.tasks_dir().join(task_id)
    }

    pub fn task_events(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("events.jsonl")
    }

    pub fn worktrees_dir(&self) -> PathBuf {
        self.root.join("worktrees")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn policy_dir(&self) -> PathBuf {
        self.root.join("policy")
    }

    pub fn require_initialized(&self) -> crate::Result<()> {
        if !self.config().exists() || !self.db().exists() {
            bail!(
                "{} is not initialized; run `zgent --home {} init` or `zgent --project init`",
                self.root.display(),
                self.root.display()
            );
        }
        Ok(())
    }
}

fn find_repo_root(start: &Path) -> crate::Result<PathBuf> {
    let mut current = fs::canonicalize(start)?;
    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }
        if !current.pop() {
            return Ok(fs::canonicalize(start)?);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn resolves_project_home_from_repo_root() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path().join("repo");
        let nested = repo.join("src/bin");
        fs::create_dir_all(repo.join(".git"))?;
        fs::create_dir_all(&nested)?;
        let home = super::Home::for_project_from(&nested)?;
        assert_eq!(home.root(), fs::canonicalize(&repo)?.join(".zgent"));
        Ok(())
    }

    #[test]
    fn resolves_project_home_from_current_dir_without_git() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = super::Home::for_project_from(temp.path())?;
        assert_eq!(home.root(), fs::canonicalize(temp.path())?.join(".zgent"));
        Ok(())
    }
}
