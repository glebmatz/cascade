use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::audio::metadata;

pub fn slug_from_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

pub struct ImportedSong {
    pub title: String,
    pub artist: String,
    pub dir: PathBuf,
    pub audio_path: PathBuf,
}

pub fn import_local_file(file_path: &Path, songs_dir: &Path) -> Result<ImportedSong> {
    if !file_path.exists() {
        anyhow::bail!("File not found: {}", file_path.display());
    }

    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "webm" | "opus" => {}
        _ => anyhow::bail!(
            "Unsupported format: .{}. Use mp3, wav, flac, ogg, m4a, opus, or webm.",
            ext
        ),
    }

    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let tags = metadata::read(file_path);
    let title = tags.title.clone().unwrap_or_else(|| stem.to_string());
    let artist = tags.artist.clone().unwrap_or_default();

    let slug_source = if !artist.is_empty() {
        format!("{} {}", artist, title)
    } else {
        title.clone()
    };
    let slug = slug_from_title(&slug_source);
    let slug = if slug.is_empty() {
        slug_from_title(stem)
    } else {
        slug
    };

    let song_dir = songs_dir.join(&slug);
    std::fs::create_dir_all(&song_dir)?;

    let audio_dest = song_dir.join(format!("audio.{}", ext));
    std::fs::copy(file_path, &audio_dest).context("Failed to copy audio file")?;

    write_metadata_file(&song_dir, &title, &artist, &format!("audio.{}", ext))?;

    Ok(ImportedSong {
        title,
        artist,
        dir: song_dir,
        audio_path: audio_dest,
    })
}

pub fn write_metadata_file(dir: &Path, title: &str, artist: &str, audio_file: &str) -> Result<()> {
    let meta = serde_json::json!({
        "title": title,
        "artist": artist,
        "audio_file": audio_file,
    });
    std::fs::write(
        dir.join("metadata.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;
    Ok(())
}

pub fn rename_song(dir: &Path, new_title: &str, new_artist: &str) -> Result<()> {
    let meta_path = dir.join("metadata.json");
    let raw = std::fs::read_to_string(&meta_path).unwrap_or_else(|_| "{}".to_string());
    let mut v: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    v["title"] = serde_json::Value::String(new_title.to_string());
    v["artist"] = serde_json::Value::String(new_artist.to_string());
    std::fs::write(&meta_path, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}
