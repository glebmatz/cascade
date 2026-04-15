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
    use rusty_ytdl::Video;

    let video = Video::new(url).context("Failed to parse YouTube URL")?;

    let info = video.get_info().await.context("Failed to get video info")?;
    let title = info.video_details.title.clone();
    let author = info.video_details.author.as_ref()
        .map(|a| a.name.clone())
        .unwrap_or_default();

    let slug = slug_from_title(&title);
    let song_dir = songs_dir.join(&slug);
    std::fs::create_dir_all(&song_dir)?;

    let audio_path = song_dir.join("audio.mp3");

    // Download best audio stream
    video.download(&audio_path).await
        .context("Failed to download audio")?;

    // Write metadata
    let meta = serde_json::json!({
        "title": title,
        "artist": author,
        "source_url": url,
    });
    std::fs::write(
        song_dir.join("metadata.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    Ok(vec![ImportedSong {
        title,
        dir: song_dir,
        audio_path,
    }])
}
