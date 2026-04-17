use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

use crate::achievements::{AchievementId, AchievementStore};
use crate::audio::{analyzer, import, metadata};
use crate::beatmap::types::{Beatmap, Difficulty, SongMeta};
use crate::beatmap::{generator, loader};
use crate::config::Config;
use crate::game::practice::{self, PracticeSettings};
use crate::score_store::ScoreStore;
use crate::score_store::decompose_key;
use crate::screens::song_select::find_audio_file;

pub fn parse_difficulty_flag(args: &[String]) -> Option<Difficulty> {
    args.iter().find_map(|a| match a.as_str() {
        "--easy" | "-e" => Some(Difficulty::Easy),
        "--medium" | "-m" => Some(Difficulty::Medium),
        "--hard" | "-h" => Some(Difficulty::Hard),
        "--expert" | "-x" => Some(Difficulty::Expert),
        _ => None,
    })
}

/// Pull `--flag value` out of an argv slice (any position).
pub fn extract_flag(args: &[String], flag: &str) -> Option<String> {
    let pos = args.iter().position(|a| a == flag)?;
    args.get(pos + 1).cloned()
}

/// Parse `MM:SS` into milliseconds. Minutes can exceed 59 (for long tracks),
/// but seconds must be `00..=59`.
pub fn parse_mmss(s: &str) -> Result<u64> {
    let (m, sec) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("expected MM:SS, got `{s}`"))?;
    let minutes: u64 = m
        .parse()
        .map_err(|_| anyhow!("minutes must be a number, got `{m}`"))?;
    let seconds: u64 = sec
        .parse()
        .map_err(|_| anyhow!("seconds must be a number, got `{sec}`"))?;
    if seconds > 59 {
        return Err(anyhow!("seconds must be 0..=59, got {seconds}"));
    }
    Ok((minutes * 60 + seconds) * 1000)
}

/// Parse `MM:SS-MM:SS` into `(start_ms, end_ms)`, ensuring `start < end`.
pub fn parse_section(s: &str) -> Result<(u64, u64)> {
    let (a, b) = s
        .split_once('-')
        .ok_or_else(|| anyhow!("expected MM:SS-MM:SS, got `{s}`"))?;
    let start = parse_mmss(a)?;
    let end = parse_mmss(b)?;
    if start >= end {
        return Err(anyhow!("section start must be before end: {a} < {b}"));
    }
    Ok((start, end))
}

/// Parse a speed multiplier, clamping into the supported range.
pub fn parse_speed(s: &str) -> Result<f32> {
    let v: f32 = s
        .parse()
        .map_err(|_| anyhow!("speed must be a number, got `{s}`"))?;
    if !v.is_finite() || v <= 0.0 {
        return Err(anyhow!("speed must be positive, got {v}"));
    }
    Ok(practice::clamp_speed(v))
}

/// Pull `--section` and `--speed` out of argv and build a [`PracticeSettings`].
/// Returns `Ok(None)` when neither flag is present.
pub fn extract_practice(args: &[String]) -> Result<Option<PracticeSettings>> {
    let section = extract_flag(args, "--section");
    let speed = extract_flag(args, "--speed");
    if section.is_none() && speed.is_none() {
        return Ok(None);
    }
    let mut settings = PracticeSettings::default();
    if let Some(s) = section {
        let (start, end) = parse_section(&s)?;
        settings.section_start_ms = Some(start);
        settings.section_end_ms = Some(end);
    }
    if let Some(s) = speed {
        settings.speed = parse_speed(&s)?;
    }
    Ok(Some(settings))
}

pub fn song_slug_from_path(p: &Option<PathBuf>) -> String {
    p.as_ref()
        .and_then(|bp| bp.parent())
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn print_help() -> Result<()> {
    println!(
        "Cascade — terminal rhythm game\n\n\
         Usage:\n  \
         cascade                         Launch the interactive UI\n  \
         cascade add <path>              Import an audio file and generate beatmaps\n  \
         cascade list                    List all imported songs with best scores\n  \
         cascade song <slug>             Show detailed stats for one song (all mods/diffs)\n  \
         cascade play <slug> [--diff] [--mods CODES] [--section RANGE] [--speed N]\n                                  \
         Launch straight into gameplay\n                                  \
         --easy / --medium / --hard / --expert\n                                  \
         --mods hd,fl,sd,po (Hidden/Flashlight/SuddenDeath/PerfectOnly)\n                                  \
         --section MM:SS-MM:SS  practice: loop a section\n                                  \
         --speed 0.25..2.0      practice: slow down / speed up\n                                  \
         (practice ignores mods and does not save scores)\n  \
         cascade achievements            List all achievements with unlock status\n  \
         cascade regen                   Regenerate beatmaps for every imported song\n  \
         cascade rename <slug> [--title NAME] [--artist NAME]\n                                  \
         Edit a song's title or artist\n  \
         cascade help                    Show this help"
    );
    Ok(())
}

pub fn add(path_str: &str) -> Result<()> {
    let file_path = PathBuf::from(path_str);
    let songs_dir = Config::cascade_dir().join("songs");
    std::fs::create_dir_all(&songs_dir)?;

    println!("Importing {}...", file_path.display());
    let song = import::import_local_file(&file_path, &songs_dir)?;

    let display = if song.artist.is_empty() {
        song.title.clone()
    } else {
        format!("{} — {}", song.artist, song.title)
    };
    println!("Generating beatmaps for {}...", display);
    regenerate_for_dir(&song.dir, &song.audio_path, &song.title, &song.artist)?;

    println!("Successfully imported: {}", display);
    Ok(())
}

pub fn rename(slug: &str, new_title: Option<&str>, new_artist: Option<&str>) -> Result<()> {
    let dir = Config::cascade_dir().join("songs").join(slug);
    if !dir.exists() {
        anyhow::bail!("Song '{}' not found.", slug);
    }
    let meta_path = dir.join("metadata.json");
    let raw = std::fs::read_to_string(&meta_path)?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let title = new_title
        .map(String::from)
        .unwrap_or_else(|| v["title"].as_str().unwrap_or("").to_string());
    let artist = new_artist
        .map(String::from)
        .unwrap_or_else(|| v["artist"].as_str().unwrap_or("").to_string());
    import::rename_song(&dir, &title, &artist)?;
    println!("Renamed: {} — {}", artist, title);
    Ok(())
}

pub fn list() -> Result<()> {
    let songs_dir = Config::cascade_dir().join("songs");
    let scores_path = Config::cascade_dir().join("scores.json");
    let scores = ScoreStore::load(&scores_path);

    if !songs_dir.exists() {
        println!("No songs imported yet. Use `cascade add <path>` to import.");
        return Ok(());
    }

    let mut entries = collect_song_summaries(&songs_dir)?;
    entries.sort_by_key(|a| a.title.to_lowercase());

    if entries.is_empty() {
        println!("No songs imported yet.");
        return Ok(());
    }

    println!(
        "{:<3}{:<24}{:<18}{:<6}{:<7}{:<8}",
        "#", "SLUG", "TITLE", "BPM", "LEN", "DIFFS"
    );
    println!("{}", "─".repeat(70));
    for (idx, e) in entries.iter().enumerate() {
        let display_name = if e.artist.is_empty() {
            e.title.clone()
        } else {
            format!("{} — {}", e.title, e.artist)
        };
        let dur_str = format!(
            "{}:{:02}",
            e.duration_ms / 60_000,
            (e.duration_ms / 1000) % 60
        );
        let short_name: String = display_name.chars().take(17).collect();
        let short_slug: String = e.slug.chars().take(22).collect();
        println!(
            "{:<3}{:<24}{:<18}{:<6}{:<7}",
            idx + 1,
            short_slug,
            short_name,
            e.bpm,
            dur_str
        );

        let scores_line = format_best_scores(&scores, &e.slug);
        if !scores_line.is_empty() {
            println!("   best:{}", scores_line);
        }
    }
    println!("\nUse `cascade play <slug> --hard` to play.");
    Ok(())
}

pub fn song(slug: &str) -> Result<()> {
    let songs_dir = Config::cascade_dir().join("songs");
    let dir = songs_dir.join(slug);
    if !dir.is_dir() {
        anyhow::bail!(
            "Song '{}' not found. Use `cascade list` to see imported songs.",
            slug
        );
    }

    let (title, artist) = read_title_artist(&dir).unwrap_or((slug.to_string(), String::new()));
    let (bpm, duration_ms) = read_bpm_duration(&dir);
    let scores = ScoreStore::load(&Config::cascade_dir().join("scores.json"));

    let display = if artist.is_empty() {
        title.clone()
    } else {
        format!("{} — {}", artist, title)
    };
    println!("{}", display);
    println!("{}", "─".repeat(display.chars().count().max(40)));
    println!("  slug:     {}", slug);
    println!("  bpm:      {}", bpm);
    println!(
        "  length:   {}:{:02}",
        duration_ms / 60_000,
        (duration_ms / 1000) % 60
    );

    println!();
    println!("Note counts:");
    for d in Difficulty::all() {
        let p = dir.join(d.filename());
        let count = std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str::<Beatmap>(&s).ok())
            .map(|bm| bm.notes.len())
            .unwrap_or(0);
        if count > 0 {
            println!("  {:<8}{:>5} notes", d.to_string().to_uppercase(), count);
        }
    }

    println!();
    println!("Best scores:");
    let entries: Vec<(&String, &crate::score_store::BestScore)> =
        scores.all_for_song(slug).collect();
    if entries.is_empty() {
        println!("  (none yet — go play this song!)");
    } else {
        let mut sorted: Vec<_> = entries.into_iter().collect();
        sorted.sort_by_key(|(k, _)| k.to_string());
        for (key, bs) in sorted {
            let (diff, mods) = decompose_key(key);
            let mod_label = if mods.is_empty() {
                String::new()
            } else {
                format!("  +[{}]", mods.to_uppercase())
            };
            println!(
                "  {:<8}{:>10}  {:>5.1}%  {:>4} combo  grade {}{}",
                diff.to_uppercase(),
                bs.score,
                bs.accuracy,
                bs.max_combo,
                bs.grade,
                mod_label,
            );
        }
    }
    Ok(())
}

pub fn achievements_list() -> Result<()> {
    let path = Config::cascade_dir().join("achievements.json");
    let store = AchievementStore::load(&path);
    let total = AchievementId::ALL.len();
    let unlocked = AchievementId::ALL
        .iter()
        .filter(|id| store.is_unlocked(**id))
        .count();

    println!("Achievements: {}/{} unlocked\n", unlocked, total);
    for id in AchievementId::ALL {
        let mark = if store.is_unlocked(*id) { "★" } else { " " };
        println!("  {}  {:<20} {}", mark, id.name(), id.description());
    }
    Ok(())
}

pub fn regen() -> Result<()> {
    let songs_dir = Config::cascade_dir().join("songs");
    if !songs_dir.exists() {
        println!("No songs to regenerate.");
        return Ok(());
    }
    for entry in std::fs::read_dir(&songs_dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let slug = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let Some(audio_path) = find_audio_file(&path) else {
            println!("Skip {} (no audio)", slug);
            continue;
        };

        let (mut title, mut artist) =
            read_title_artist(&path).unwrap_or((slug.clone(), String::new()));

        // If metadata looks untouched (default title from filename + empty artist),
        // try to backfill from the audio file's embedded tags.
        let audio_stem = audio_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let looks_default = artist.is_empty() && (title == audio_stem || title == slug);
        if looks_default {
            let tags = metadata::read(&audio_path);
            if let Some(t) = tags.title.filter(|t| !t.is_empty()) {
                title = t;
            }
            if let Some(a) = tags.artist.filter(|a| !a.is_empty()) {
                artist = a;
            }
            if title != audio_stem || !artist.is_empty() {
                let audio_filename = audio_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let _ = import::write_metadata_file(&path, &title, &artist, &audio_filename);
            }
        }

        let display = if artist.is_empty() {
            title.clone()
        } else {
            format!("{} — {}", artist, title)
        };
        println!("Regenerating {}...", display);
        regenerate_for_dir(&path, &audio_path, &title, &artist)?;
    }
    println!("Done.");
    Ok(())
}

struct SongSummary {
    slug: String,
    title: String,
    artist: String,
    bpm: u32,
    duration_ms: u64,
}

fn collect_song_summaries(songs_dir: &Path) -> Result<Vec<SongSummary>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(songs_dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let slug = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (title, artist) = read_title_artist(&path).unwrap_or((slug.clone(), String::new()));
        let (bpm, duration_ms) = read_bpm_duration(&path);
        out.push(SongSummary {
            slug,
            title,
            artist,
            bpm,
            duration_ms,
        });
    }
    Ok(out)
}

fn read_title_artist(dir: &Path) -> Option<(String, String)> {
    let raw = std::fs::read_to_string(dir.join("metadata.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some((
        v["title"].as_str()?.to_string(),
        v["artist"].as_str().unwrap_or("").to_string(),
    ))
}

fn read_bpm_duration(dir: &Path) -> (u32, u64) {
    for d in Difficulty::all() {
        let p = dir.join(d.filename());
        if !p.exists() {
            continue;
        }
        let Ok(s) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(bm) = serde_json::from_str::<Beatmap>(&s) else {
            continue;
        };
        return (bm.song.bpm, bm.song.duration_ms);
    }
    (0, 0)
}

fn format_best_scores(scores: &ScoreStore, slug: &str) -> String {
    let mut out = String::new();
    for d in Difficulty::all() {
        let name = d.to_string();
        if let Some(bs) = scores.get(slug, &name) {
            out.push_str(&format!(
                "   {:<8}{:>7} ({})",
                name.to_uppercase(),
                bs.score,
                bs.grade
            ));
        }
    }
    out
}

fn regenerate_for_dir(dir: &Path, audio_path: &Path, title: &str, artist: &str) -> Result<()> {
    let (samples, sample_rate) = analyzer::decode_audio(audio_path)?;
    let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;
    let audio_filename = audio_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let meta = SongMeta {
        title: title.to_string(),
        artist: artist.to_string(),
        audio_file: audio_filename,
        bpm: 120,
        duration_ms,
    };

    let beatmaps = generator::generate_all_beatmaps(&samples, sample_rate, meta);
    for bm in &beatmaps {
        let path = dir.join(bm.difficulty.filename());
        let _ = loader::save(bm, &path);
    }
    Ok(())
}
