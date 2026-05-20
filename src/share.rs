//! `.cscd` share-package format: metadata + beatmaps, audio fetched separately.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use crate::audio::import::{self, MetadataFields};
use crate::beatmap::loader;
use crate::beatmap::types::{Beatmap, Difficulty};

pub const FORMAT_TAG: &str = "cascade-share";
pub const FORMAT_VERSION: u32 = 1;
pub const PACK_FORMAT_TAG: &str = "cascade-pack";
pub const PACK_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePackage {
    pub format: String,
    pub version: u32,
    pub song: ShareMetadata,
    pub beatmaps: BTreeMap<String, Beatmap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePack {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub packages: Vec<SharePackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareMetadata {
    pub title: String,
    pub artist: String,
    pub audio_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_sha256: Option<String>,
}

pub fn build_from_dir(song_dir: &Path) -> Result<SharePackage> {
    let meta = read_metadata(song_dir)?;
    let mut beatmaps = BTreeMap::new();
    for d in Difficulty::all() {
        let p = song_dir.join(d.filename());
        if !p.exists() {
            continue;
        }
        let bm = loader::load(&p).with_context(|| format!("loading beatmap {}", p.display()))?;
        beatmaps.insert(diff_key(*d).to_string(), bm);
    }
    if beatmaps.is_empty() {
        return Err(anyhow!(
            "no beatmaps found under {} — run `cascade regen` first",
            song_dir.display()
        ));
    }
    Ok(SharePackage {
        format: FORMAT_TAG.to_string(),
        version: FORMAT_VERSION,
        song: meta,
        beatmaps,
    })
}

pub fn build_pack_from_dirs(name: &str, song_dirs: &[std::path::PathBuf]) -> Result<SharePack> {
    if name.trim().is_empty() {
        return Err(anyhow!("pack name cannot be empty"));
    }
    if song_dirs.is_empty() {
        return Err(anyhow!("pack needs at least one song"));
    }

    let mut packages = Vec::with_capacity(song_dirs.len());
    for dir in song_dirs {
        packages.push(build_from_dir(dir)?);
    }

    Ok(SharePack {
        format: PACK_FORMAT_TAG.to_string(),
        version: PACK_FORMAT_VERSION,
        name: name.to_string(),
        packages,
    })
}

pub fn save_package(pkg: &SharePackage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(pkg)?)?;
    Ok(())
}

pub fn save_pack(pack: &SharePack, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(pack)?)?;
    Ok(())
}

pub fn load_package(path: &Path) -> Result<SharePackage> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let pkg: SharePackage = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {} as a Cascade share package", path.display()))?;
    if pkg.format != FORMAT_TAG {
        return Err(anyhow!(
            "{} is not a Cascade share package (format = {:?})",
            path.display(),
            pkg.format
        ));
    }
    if pkg.version > FORMAT_VERSION {
        return Err(anyhow!(
            "{} requires a newer version of Cascade (package format v{}, this binary supports v{})",
            path.display(),
            pkg.version,
            FORMAT_VERSION
        ));
    }
    Ok(pkg)
}

pub fn load_pack(path: &Path) -> Result<SharePack> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let pack: SharePack = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {} as a Cascade pack", path.display()))?;
    if pack.format != PACK_FORMAT_TAG {
        return Err(anyhow!(
            "{} is not a Cascade pack (format = {:?})",
            path.display(),
            pack.format
        ));
    }
    if pack.version > PACK_FORMAT_VERSION {
        return Err(anyhow!(
            "{} requires a newer version of Cascade (pack format v{}, this binary supports v{})",
            path.display(),
            pack.version,
            PACK_FORMAT_VERSION
        ));
    }
    if pack.packages.is_empty() {
        return Err(anyhow!("{} contains no songs", path.display()));
    }
    Ok(pack)
}

pub struct ImportOutcome {
    pub song_dir: std::path::PathBuf,
    pub slug: String,
    pub audio_status: AudioStatus,
}

pub enum AudioStatus {
    Downloaded { bytes: u64, hash_verified: bool },
    HashMismatch { expected: String, got: String },
    Missing { expected_filename: String },
    SkippedByFlag,
}

pub fn install_package(
    pkg: &SharePackage,
    songs_dir: &Path,
    fetch_audio: bool,
) -> Result<ImportOutcome> {
    let slug_source = if !pkg.song.artist.is_empty() {
        format!("{} {}", pkg.song.artist, pkg.song.title)
    } else {
        pkg.song.title.clone()
    };
    let slug = import::slug_from_title(&slug_source);
    let slug = if slug.is_empty() {
        import::slug_from_title(&pkg.song.audio_file)
    } else {
        slug
    };
    if slug.is_empty() {
        return Err(anyhow!("could not derive a slug from package metadata"));
    }

    let song_dir = songs_dir.join(&slug);
    std::fs::create_dir_all(&song_dir)?;

    for (key, bm) in &pkg.beatmaps {
        let Some(d) = parse_diff_key(key) else {
            continue;
        };
        loader::save(bm, &song_dir.join(d.filename()))
            .with_context(|| format!("writing beatmap {}", key))?;
    }

    import::write_metadata(
        &song_dir,
        MetadataFields {
            title: &pkg.song.title,
            artist: &pkg.song.artist,
            audio_file: &pkg.song.audio_file,
            source_url: pkg.song.source_url.as_deref(),
            audio_sha256: pkg.song.audio_sha256.as_deref(),
        },
    )?;

    let audio_status = match (&pkg.song.source_url, fetch_audio) {
        (Some(url), true) => {
            let dest = song_dir.join(&pkg.song.audio_file);
            let bytes = download_to_file(url, &dest)?;
            match &pkg.song.audio_sha256 {
                Some(expected) => {
                    let got = import::sha256_of(&dest)?;
                    if got.eq_ignore_ascii_case(expected) {
                        AudioStatus::Downloaded {
                            bytes,
                            hash_verified: true,
                        }
                    } else {
                        let mis = song_dir.join(format!("{}.mismatch", pkg.song.audio_file));
                        let _ = std::fs::rename(&dest, &mis);
                        AudioStatus::HashMismatch {
                            expected: expected.clone(),
                            got,
                        }
                    }
                }
                None => AudioStatus::Downloaded {
                    bytes,
                    hash_verified: false,
                },
            }
        }
        (Some(_), false) => AudioStatus::SkippedByFlag,
        (None, _) => AudioStatus::Missing {
            expected_filename: pkg.song.audio_file.clone(),
        },
    };

    Ok(ImportOutcome {
        song_dir,
        slug,
        audio_status,
    })
}

pub fn install_pack(
    pack: &SharePack,
    songs_dir: &Path,
    fetch_audio: bool,
) -> Result<Vec<ImportOutcome>> {
    let mut outcomes = Vec::with_capacity(pack.packages.len());
    for pkg in &pack.packages {
        outcomes.push(install_package(pkg, songs_dir, fetch_audio)?);
    }
    Ok(outcomes)
}

fn read_metadata(song_dir: &Path) -> Result<ShareMetadata> {
    let meta_path = song_dir.join("metadata.json");
    let raw = std::fs::read_to_string(&meta_path)
        .with_context(|| format!("reading {}", meta_path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let title = v["title"].as_str().unwrap_or("").to_string();
    let artist = v["artist"].as_str().unwrap_or("").to_string();
    let audio_file = v["audio_file"].as_str().unwrap_or("").to_string();
    if title.is_empty() || audio_file.is_empty() {
        return Err(anyhow!(
            "metadata.json is missing required fields (title / audio_file)"
        ));
    }
    let source_url = v["source_url"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let audio_sha256 = v["audio_sha256"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Ok(ShareMetadata {
        title,
        artist,
        audio_file,
        source_url,
        audio_sha256,
    })
}

fn diff_key(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "easy",
        Difficulty::Medium => "medium",
        Difficulty::Hard => "hard",
        Difficulty::Expert => "expert",
    }
}

fn parse_diff_key(s: &str) -> Option<Difficulty> {
    match s.to_lowercase().as_str() {
        "easy" => Some(Difficulty::Easy),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::Hard),
        "expert" => Some(Difficulty::Expert),
        _ => None,
    }
}

pub fn looks_like_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn ext_from_url(url: &str) -> &'static str {
    let path = url
        .split('?')
        .next()
        .unwrap_or(url)
        .split('#')
        .next()
        .unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or("");
    let ext = last
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp3" => "mp3",
        "wav" => "wav",
        "flac" => "flac",
        "ogg" => "ogg",
        "m4a" => "m4a",
        "webm" => "webm",
        "opus" => "opus",
        _ => "mp3",
    }
}

pub fn download_to_file(url: &str, dest: &Path) -> Result<u64> {
    const MAX_BYTES: u64 = 50 * 1024 * 1024;
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(60))
        .user_agent(concat!("cascade/", env!("CARGO_PKG_VERSION")))
        .build();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| anyhow!("HTTP request failed: {e}"))?;
    let status = resp.status();
    if !(200..300).contains(&status) {
        return Err(anyhow!("HTTP {status} from {url}"));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut reader = resp.into_reader().take(MAX_BYTES + 1);
    let mut file =
        std::fs::File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_BYTES {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(anyhow!(
                "download aborted: response exceeds {} MB cap",
                MAX_BYTES / 1024 / 1024
            ));
        }
        file.write_all(&buf[..n])?;
    }
    file.flush()?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beatmap::types::{Note, SongMeta};

    fn write_minimal_song(dir: &Path, source_url: Option<&str>, sha: Option<&str>) {
        std::fs::create_dir_all(dir).unwrap();
        let mut meta = serde_json::json!({
            "title": "T",
            "artist": "A",
            "audio_file": "audio.mp3",
        });
        if let Some(u) = source_url {
            meta["source_url"] = serde_json::Value::String(u.into());
        }
        if let Some(h) = sha {
            meta["audio_sha256"] = serde_json::Value::String(h.into());
        }
        std::fs::write(
            dir.join("metadata.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();
        let bm = Beatmap {
            version: 1,
            song: SongMeta {
                title: "T".into(),
                artist: "A".into(),
                audio_file: "audio.mp3".into(),
                bpm: 120,
                duration_ms: 180_000,
            },
            difficulty: Difficulty::Hard,
            notes: vec![Note {
                time_ms: 1000,
                lane: 2,
                duration_ms: 0,
                slide_to: None,
            }],
        };
        loader::save(&bm, &dir.join("hard.json")).unwrap();
    }

    #[test]
    fn url_detection() {
        assert!(looks_like_url("https://example.com/song.mp3"));
        assert!(looks_like_url("http://EXAMPLE.com/x"));
        assert!(looks_like_url("HTTPS://EXAMPLE.COM"));
        assert!(!looks_like_url("/Users/me/song.mp3"));
        assert!(!looks_like_url("song.mp3"));
        assert!(!looks_like_url("ftp://example.com/song.mp3"));
        assert!(!looks_like_url(""));
        assert!(!looks_like_url("file:///etc/passwd"));
    }

    #[test]
    fn ext_extraction() {
        assert_eq!(ext_from_url("https://x.com/a.mp3"), "mp3");
        assert_eq!(ext_from_url("https://x.com/a.FLAC"), "flac");
        assert_eq!(ext_from_url("https://x.com/a.ogg?v=2"), "ogg");
        assert_eq!(ext_from_url("https://x.com/a.wav#frag"), "wav");
        assert_eq!(ext_from_url("https://x.com/track"), "mp3");
        assert_eq!(ext_from_url("https://x.com/track.xyz"), "mp3");
        assert_eq!(ext_from_url("https://x.com/a.OPUS"), "opus");
        assert_eq!(
            ext_from_url("https://x.com/path/to/song.m4a?token=abc#t=1"),
            "m4a"
        );
    }

    #[test]
    fn package_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let song_dir = dir.path().join("songs/test-song");
        write_minimal_song(&song_dir, Some("https://x/a.mp3"), Some("deadbeef"));

        let pkg = build_from_dir(&song_dir).unwrap();
        assert_eq!(pkg.song.title, "T");
        assert_eq!(pkg.song.source_url.as_deref(), Some("https://x/a.mp3"));
        assert_eq!(pkg.song.audio_sha256.as_deref(), Some("deadbeef"));
        assert!(pkg.beatmaps.contains_key("hard"));

        let out = dir.path().join("test.cscd");
        save_package(&pkg, &out).unwrap();
        let parsed = load_package(&out).unwrap();
        assert_eq!(parsed.format, FORMAT_TAG);
        assert_eq!(parsed.beatmaps["hard"].notes.len(), 1);
        assert_eq!(parsed.beatmaps["hard"].notes[0].lane, 2);
    }

    #[test]
    fn package_rejects_foreign_format() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("foo.json");
        std::fs::write(
            &p,
            r#"{"format":"something-else","version":1,"song":{"title":"x","artist":"","audio_file":"a"},"beatmaps":{}}"#,
        )
        .unwrap();
        let err = load_package(&p).unwrap_err().to_string();
        assert!(err.contains("not a Cascade share package"));
    }

    #[test]
    fn package_rejects_future_version() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("future.cscd");
        std::fs::write(
            &p,
            format!(
                r#"{{"format":"{}","version":999,"song":{{"title":"x","artist":"","audio_file":"a"}},"beatmaps":{{}}}}"#,
                FORMAT_TAG
            ),
        )
        .unwrap();
        let err = load_package(&p).unwrap_err().to_string();
        assert!(err.contains("newer version of Cascade"));
    }

    #[test]
    fn build_fails_when_no_beatmaps() {
        let dir = tempfile::tempdir().unwrap();
        let song_dir = dir.path().join("song");
        std::fs::create_dir_all(&song_dir).unwrap();
        std::fs::write(
            song_dir.join("metadata.json"),
            r#"{"title":"T","artist":"A","audio_file":"audio.mp3"}"#,
        )
        .unwrap();
        assert!(build_from_dir(&song_dir).is_err());
    }

    #[test]
    fn build_fails_when_metadata_missing_required_fields() {
        let dir = tempfile::tempdir().unwrap();
        let song_dir = dir.path().join("song");
        std::fs::create_dir_all(&song_dir).unwrap();
        std::fs::write(song_dir.join("metadata.json"), r#"{"artist":"A"}"#).unwrap();
        let bm = Beatmap {
            version: 1,
            song: SongMeta {
                title: "T".into(),
                artist: "A".into(),
                audio_file: "audio.mp3".into(),
                bpm: 120,
                duration_ms: 1000,
            },
            difficulty: Difficulty::Easy,
            notes: vec![],
        };
        loader::save(&bm, &song_dir.join("easy.json")).unwrap();
        assert!(build_from_dir(&song_dir).is_err());
    }

    #[test]
    fn diff_key_round_trip() {
        for d in Difficulty::all() {
            assert_eq!(parse_diff_key(diff_key(*d)), Some(*d));
        }
        assert_eq!(parse_diff_key("EASY"), Some(Difficulty::Easy));
        assert_eq!(parse_diff_key("nightmare"), None);
    }
}
