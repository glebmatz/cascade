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

    // Check if yt-dlp is already in PATH
    if let Ok(output) = tokio::process::Command::new("yt-dlp").arg("--version").output().await {
        if output.status.success() {
            return Ok(PathBuf::from("yt-dlp"));
        }
    }

    // Use nightly builds — latest YouTube fixes
    #[cfg(target_os = "macos")]
    let (filename, dl_url) = (
        "yt-dlp",
        "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/yt-dlp_macos".to_string(),
    );

    #[cfg(target_os = "linux")]
    let (filename, dl_url) = (
        "yt-dlp",
        "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/yt-dlp_linux".to_string(),
    );

    #[cfg(target_os = "windows")]
    let (filename, dl_url) = (
        "yt-dlp.exe",
        "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/yt-dlp.exe".to_string(),
    );

    let ytdlp_path = bin_dir.join(filename);

    if ytdlp_path.exists() {
        return Ok(ytdlp_path);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client.get(&dl_url)
        .header("User-Agent", "cascade-game/0.1")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Download yt-dlp failed: {}", e))?;

    if !response.status().is_success() {
        anyhow::bail!("Download yt-dlp failed: HTTP {}", response.status());
    }

    let bytes = response.bytes().await
        .context("Failed to read yt-dlp download body")?;

    if bytes.len() < 1000 {
        anyhow::bail!("Downloaded yt-dlp is too small ({}b)", bytes.len());
    }

    std::fs::write(&ytdlp_path, &bytes)
        .context("Failed to write yt-dlp binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ytdlp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(ytdlp_path)
}

/// Build cookie args for yt-dlp. Extracts cookies to a file once,
/// then reuses the file (no repeated keychain prompts).
fn cookie_args(cascade_dir: &Path, ytdlp: &Path) -> Vec<String> {
    let cookie_file = cascade_dir.join("cookies.txt");

    // If we already have a cookie file, just use it
    if cookie_file.exists() {
        return vec!["--cookies".to_string(), cookie_file.to_string_lossy().to_string()];
    }

    // Try to extract cookies from a browser that doesn't need keychain
    // Firefox stores cookies in its own sqlite — no keychain prompt on macOS
    #[cfg(target_os = "macos")]
    let browsers = &["firefox", "safari"];
    #[cfg(not(target_os = "macos"))]
    let browsers = &["firefox", "chrome"];

    for browser in browsers {
        // Try extracting cookies to file using yt-dlp
        let result = std::process::Command::new(ytdlp)
            .args([
                "--cookies-from-browser", browser,
                "--cookies", cookie_file.to_str().unwrap(),
                "--skip-download",
                "--flat-playlist",
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ", // dummy URL
            ])
            .output();

        if let Ok(output) = result {
            if cookie_file.exists() && std::fs::metadata(&cookie_file).map(|m| m.len() > 100).unwrap_or(false) {
                return vec!["--cookies".to_string(), cookie_file.to_string_lossy().to_string()];
            }
        }
    }

    // No cookies available — try without
    vec![]
}

pub async fn download_audio(url: &str, songs_dir: &Path) -> Result<Vec<ImportedSong>> {
    let cascade_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cascade");

    let ytdlp = ensure_ytdlp(&cascade_dir).await
        .context("Could not get yt-dlp")?;

    let cookies = cookie_args(&cascade_dir, &ytdlp);

    // Get metadata
    let mut cmd = tokio::process::Command::new(&ytdlp);
    cmd.args(["--flat-playlist", "--dump-json"]);
    for arg in &cookies {
        cmd.arg(arg);
    }
    cmd.arg(url);

    let output = cmd.output().await.context("Failed to run yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last_line = stderr.lines().last().unwrap_or(&stderr);

        // If bot detection, give helpful error
        if stderr.contains("Sign in") || stderr.contains("bot") {
            anyhow::bail!(
                "YouTube requires authentication. Please install Firefox and log into YouTube there, then retry import."
            );
        }
        anyhow::bail!("yt-dlp error: {}", last_line);
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

        // Download best audio stream in native format (no ffmpeg needed)
        let audio_template = song_dir.join("audio.%(ext)s");
        let mut dl_cmd = tokio::process::Command::new(&ytdlp);
        dl_cmd.args(["-f", "bestaudio", "--no-playlist"]);
        for arg in &cookies {
            dl_cmd.arg(arg);
        }
        dl_cmd.args(["-o", audio_template.to_str().unwrap(), entry_url]);

        let dl_output = dl_cmd.output().await
            .context("Failed to run yt-dlp download")?;

        if !dl_output.status.success() {
            let stderr = String::from_utf8_lossy(&dl_output.stderr);
            eprintln!("Warning: failed to download {}: {}", title, stderr.lines().last().unwrap_or_default());
            continue;
        }

        // Find the downloaded audio file (could be .webm, .m4a, .opus, etc)
        let actual_audio = glob_first(&song_dir, "audio")
            .context("Downloaded audio file not found")?;

        // Write metadata
        let audio_filename = actual_audio.file_name().unwrap().to_string_lossy().to_string();
        let artist = entry.get("uploader").and_then(|v| v.as_str()).unwrap_or("");
        let meta = serde_json::json!({
            "title": title,
            "artist": artist,
            "source_url": entry_url,
            "audio_file": audio_filename,
        });
        std::fs::write(
            song_dir.join("metadata.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        results.push(ImportedSong {
            title: title.to_string(),
            dir: song_dir,
            audio_path: actual_audio,
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
