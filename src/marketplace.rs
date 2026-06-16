use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{home::Home, plugins::PluginManifest};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MarketplaceIndex {
    entries: Vec<MarketplaceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub description: Option<String>,
}

pub fn list(home: &Home) -> crate::Result<Vec<MarketplaceEntry>> {
    let mut entries = read_index(home)?.entries;
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

pub fn add_local(home: &Home, plugin_dir: &Path) -> crate::Result<MarketplaceEntry> {
    let manifest: PluginManifest =
        serde_json::from_str(&fs::read_to_string(plugin_dir.join("zgent.plugin.json"))?)?;
    let entry = MarketplaceEntry {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        source: fs::canonicalize(plugin_dir)?.display().to_string(),
        description: manifest.description,
    };
    let mut index = read_index(home)?;
    index.entries.retain(|existing| existing.id != entry.id);
    index.entries.push(entry.clone());
    write_index(home, &index)?;
    Ok(entry)
}

pub fn install(home: &Home, plugin_id: &str) -> crate::Result<PathBuf> {
    let entry = list(home)?
        .into_iter()
        .find(|entry| entry.id == plugin_id)
        .ok_or_else(|| anyhow::anyhow!("marketplace plugin not found: {plugin_id}"))?;
    let destination = home
        .plugins_dir()
        .join("installed")
        .join(plugin_id.replace(['/', '\\'], "_"));
    if destination.exists() {
        return Ok(destination);
    }
    copy_dir(Path::new(&entry.source), &destination)?;
    Ok(destination)
}

fn read_index(home: &Home) -> crate::Result<MarketplaceIndex> {
    let path = home.marketplace_file();
    if !path.exists() {
        return Ok(MarketplaceIndex::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write_index(home: &Home, index: &MarketplaceIndex) -> crate::Result<()> {
    if let Some(parent) = home.marketplace_file().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        home.marketplace_file(),
        serde_json::to_string_pretty(index)?,
    )?;
    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> crate::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(from, to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::home::Home;

    #[test]
    fn adds_and_installs_local_plugin() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        let source = temp.path().join("source-plugin");
        fs::create_dir_all(&source)?;
        fs::write(
            source.join("zgent.plugin.json"),
            r#"{"schema":"zgent.plugin.v1","id":"demo@local","name":"Demo","version":"0.1.0"}"#,
        )?;

        let entry = super::add_local(&home, &source)?;
        assert_eq!(entry.id, "demo@local");
        assert_eq!(super::list(&home)?.len(), 1);

        let installed = super::install(&home, "demo@local")?;
        assert!(installed.join("zgent.plugin.json").exists());
        Ok(())
    }
}
