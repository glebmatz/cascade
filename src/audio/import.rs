use anyhow::{Result, Context};
use std::path::{Path, PathBuf};

pub fn slug_from_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

pub struct ImportedSong {
    pub title: String,
    pub dir: PathBuf,
    pub audio_path: PathBuf,
}

/// Import a local audio file: copy to songs dir, generate beatmaps.
pub fn import_local_file(file_path: &Path, songs_dir: &Path) -> Result<ImportedSong> {
    if !file_path.exists() {
        anyhow::bail!("File not found: {}", file_path.display());
    }

    let ext = file_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "webm" | "opus" => {}
        _ => anyhow::bail!("Unsupported format: .{}. Use mp3, wav, flac, ogg, m4a, opus, or webm.", ext),
    }

    // Derive song name from filename
    let stem = file_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let slug = slug_from_title(stem);
    let song_dir = songs_dir.join(&slug);
    std::fs::create_dir_all(&song_dir)?;

    let audio_dest = song_dir.join(format!("audio.{}", ext));

    // Copy file
    std::fs::copy(file_path, &audio_dest)
        .context("Failed to copy audio file")?;

    // Write metadata
    let meta = serde_json::json!({
        "title": stem,
        "artist": "",
        "audio_file": format!("audio.{}", ext),
    });
    std::fs::write(
        song_dir.join("metadata.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    Ok(ImportedSong {
        title: stem.to_string(),
        dir: song_dir,
        audio_path: audio_dest,
    })
}
