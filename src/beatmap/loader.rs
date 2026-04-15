use anyhow::Result;
use std::path::Path;
use super::types::Beatmap;

pub fn load(path: &Path) -> Result<Beatmap> {
    let content = std::fs::read_to_string(path)?;
    let beatmap: Beatmap = serde_json::from_str(&content)?;
    Ok(beatmap)
}

pub fn save(beatmap: &Beatmap, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(beatmap)?;
    std::fs::write(path, content)?;
    Ok(())
}
