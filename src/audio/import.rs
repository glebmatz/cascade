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

/// Get the path to yt-dlp binary, downloading it if needed.
async fn ensure_ytdlp(cascade_dir: &Path) -> Result<PathBuf> {
    let bin_dir = cascade_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;

    // Also check if yt-dlp is already in PATH
    if let Ok(output) = tokio::process::Command::new("yt-dlp").arg("--version").output().await {
        if output.status.success() {
            return Ok(PathBuf::from("yt-dlp"));
        }
    }

    #[cfg(target_os = "macos")]
    let (filename, dl_url) = {
        // macos = universal binary (arm64+x86_64), macos_legacy = old x86_64 only
        ("yt-dlp", "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos".to_string())
    };

    #[cfg(target_os = "linux")]
    let (filename, dl_url) = (
        "yt-dlp",
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux".to_string(),
    );

    #[cfg(target_os = "windows")]
    let (filename, dl_url) = (
        "yt-dlp.exe",
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe".to_string(),
    );

    let ytdlp_path = bin_dir.join(filename);

    if ytdlp_path.exists() {
        return Ok(ytdlp_path);
    }

    // Download yt-dlp binary from GitHub releases
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client.get(&dl_url)
        .header("User-Agent", "cascade-game/0.1")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Download yt-dlp failed: {} (url: {})", e, dl_url))?;

    if !response.status().is_success() {
        anyhow::bail!("Download yt-dlp failed: HTTP {} from {}", response.status(), dl_url);
    }

    let bytes = response.bytes().await
        .context("Failed to read yt-dlp download body")?;

    if bytes.len() < 1000 {
        anyhow::bail!("Downloaded yt-dlp is too small ({}b) — likely an error page", bytes.len());
    }

    std::fs::write(&ytdlp_path, &bytes)
        .context("Failed to write yt-dlp binary")?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ytdlp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(ytdlp_path)
}

pub async fn download_audio(url: &str, songs_dir: &Path) -> Result<Vec<ImportedSong>> {
    let cascade_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cascade");

    let ytdlp = ensure_ytdlp(&cascade_dir).await
        .context("Could not get yt-dlp")?;

    // Get metadata first
    let output = tokio::process::Command::new(&ytdlp)
        .args(["--flat-playlist", "--dump-json", url])
        .output()
        .await
        .context("Failed to run yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp error: {}", stderr.lines().last().unwrap_or(&stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    if entries.is_empty() {
        anyhow::bail!("No videos found at this URL");
    }

    let mut results = Vec::new();

    for entry in &entries {
        let title = entry["title"].as_str().unwrap_or("unknown");
        let entry_url = entry["url"].as_str()
            .or_else(|| entry["webpage_url"].as_str())
            .unwrap_or(url);

        let slug = slug_from_title(title);
        if slug.is_empty() { continue; }

        let song_dir = songs_dir.join(&slug);
        std::fs::create_dir_all(&song_dir)?;

        let audio_path = song_dir.join("audio.mp3");

        // Download audio
        let dl_output = tokio::process::Command::new(&ytdlp)
            .args([
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", "0",
                "--no-playlist",
                "-o", audio_path.to_str().unwrap(),
                entry_url,
            ])
            .output()
            .await
            .context("Failed to run yt-dlp download")?;

        if !dl_output.status.success() {
            let stderr = String::from_utf8_lossy(&dl_output.stderr);
            eprintln!("Warning: failed to download {}: {}", title, stderr.lines().last().unwrap_or_default());
            continue;
        }

        // yt-dlp may add extension, find the actual file
        let actual_audio = if audio_path.exists() {
            audio_path.clone()
        } else {
            // yt-dlp sometimes creates audio.mp3.mp3 or similar
            let pattern = song_dir.join("audio*");
            let found = glob_first(&song_dir, "audio");
            found.unwrap_or(audio_path.clone())
        };

        // Write metadata
        let artist = entry.get("uploader").and_then(|v| v.as_str()).unwrap_or("");
        let meta = serde_json::json!({
            "title": title,
            "artist": artist,
            "source_url": entry_url,
        });
        std::fs::write(
            song_dir.join("metadata.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        // Rename to audio.mp3 if needed
        if actual_audio != audio_path && actual_audio.exists() {
            let _ = std::fs::rename(&actual_audio, &audio_path);
        }

        results.push(ImportedSong {
            title: title.to_string(),
            dir: song_dir,
            audio_path,
        });
    }

    Ok(results)
}

fn glob_first(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return None };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(prefix) {
            return Some(entry.path());
        }
    }
    None
}
