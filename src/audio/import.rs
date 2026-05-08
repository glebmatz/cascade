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

pub const SUPPORTED_EXTS: &[&str] = &["mp3", "wav", "flac", "ogg", "m4a", "webm", "opus"];

pub fn import_local_file(file_path: &Path, songs_dir: &Path) -> Result<ImportedSong> {
    import_local_file_with_source(file_path, songs_dir, None)
}

pub fn import_local_file_with_source(
    file_path: &Path,
    songs_dir: &Path,
    source_url: Option<&str>,
) -> Result<ImportedSong> {
    if !file_path.exists() {
        anyhow::bail!("File not found: {}", file_path.display());
    }

    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !SUPPORTED_EXTS.contains(&ext) {
        anyhow::bail!(
            "Unsupported format: .{}. Use mp3, wav, flac, ogg, m4a, opus, or webm.",
            ext
        );
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

    let sha = sha256_of(&audio_dest).ok();
    write_metadata(
        &song_dir,
        MetadataFields {
            title: &title,
            artist: &artist,
            audio_file: &format!("audio.{}", ext),
            source_url,
            audio_sha256: sha.as_deref(),
        },
    )?;

    Ok(ImportedSong {
        title,
        artist,
        dir: song_dir,
        audio_path: audio_dest,
    })
}

pub struct MetadataFields<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub audio_file: &'a str,
    pub source_url: Option<&'a str>,
    pub audio_sha256: Option<&'a str>,
}

pub fn write_metadata(dir: &Path, m: MetadataFields<'_>) -> Result<()> {
    let mut obj = serde_json::Map::new();
    obj.insert("title".into(), serde_json::Value::String(m.title.into()));
    obj.insert("artist".into(), serde_json::Value::String(m.artist.into()));
    obj.insert(
        "audio_file".into(),
        serde_json::Value::String(m.audio_file.into()),
    );
    if let Some(url) = m.source_url {
        obj.insert(
            "source_url".into(),
            serde_json::Value::String(url.to_string()),
        );
    }
    if let Some(hash) = m.audio_sha256 {
        obj.insert(
            "audio_sha256".into(),
            serde_json::Value::String(hash.to_string()),
        );
    }
    std::fs::write(
        dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::Value::Object(obj))?,
    )?;
    Ok(())
}

/// Updates the three core fields while preserving any other keys already in
/// metadata.json — `regen` calls this and must not lose `source_url` /
/// `audio_sha256` recorded by an earlier import.
pub fn write_metadata_file(dir: &Path, title: &str, artist: &str, audio_file: &str) -> Result<()> {
    let path = dir.join("metadata.json");
    let mut obj: serde_json::Map<String, serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .and_then(|v: serde_json::Value| v.as_object().cloned())
        .unwrap_or_default();
    obj.insert("title".into(), serde_json::Value::String(title.into()));
    obj.insert("artist".into(), serde_json::Value::String(artist.into()));
    obj.insert(
        "audio_file".into(),
        serde_json::Value::String(audio_file.into()),
    );
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::Value::Object(obj))?,
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

pub fn sha256_of(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path).context("opening file for hashing")?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(*b >> 4) as usize] as char);
        out.push(HEX[(*b & 0x0f) as usize] as char);
    }
    out
}
