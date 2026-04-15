use anyhow::{Result, Context};
use std::path::{Path, PathBuf};

pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/") || url.contains("youtu.be/")
}

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

pub async fn download_audio(url: &str, songs_dir: &Path) -> Result<Vec<ImportedSong>> {
    let output = tokio::process::Command::new("yt-dlp")
        .args(["--flat-playlist", "--dump-json", url])
        .output()
        .await
        .context("Failed to run yt-dlp. Is it installed?")?;

    if !output.status.success() {
        anyhow::bail!("yt-dlp metadata fetch failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    let mut results = Vec::new();

    for entry in &entries {
        let title = entry["title"].as_str().unwrap_or("unknown");
        let entry_url = entry["url"].as_str()
            .or_else(|| entry["webpage_url"].as_str())
            .unwrap_or(url);

        let slug = slug_from_title(title);
        let song_dir = songs_dir.join(&slug);
        std::fs::create_dir_all(&song_dir)?;

        let audio_path = song_dir.join("audio.mp3");

        let dl_status = tokio::process::Command::new("yt-dlp")
            .args([
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", "0",
                "-o", audio_path.to_str().unwrap(),
                entry_url,
            ])
            .status()
            .await
            .context("Failed to run yt-dlp download")?;

        if !dl_status.success() {
            eprintln!("Warning: failed to download {}", title);
            continue;
        }

        let meta = serde_json::json!({
            "title": title,
            "artist": entry.get("uploader").and_then(|v| v.as_str()).unwrap_or(""),
            "source_url": entry_url,
        });
        std::fs::write(
            song_dir.join("metadata.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        results.push(ImportedSong {
            title: title.to_string(),
            dir: song_dir,
            audio_path,
        });
    }

    Ok(results)
}
