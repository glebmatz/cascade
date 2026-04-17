use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::audio::{analyzer, import};
use crate::beatmap::types::{Beatmap, Difficulty, SongMeta};
use crate::beatmap::{generator, loader};
use crate::config::Config;
use crate::score_store::ScoreStore;
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
         cascade play <slug> [--diff]    Launch straight into gameplay\n                                  \
         --easy / --medium / --hard / --expert\n  \
         cascade regen                   Regenerate beatmaps for every imported song\n  \
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

    println!("Generating beatmaps for {}...", song.title);
    regenerate_for_dir(&song.dir, &song.audio_path, &song.title)?;

    println!("Successfully imported: {}", song.title);
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
    entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

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

        let title = read_title(&path).unwrap_or_else(|| slug.clone());
        println!("Regenerating {}...", title);
        regenerate_for_dir(&path, &audio_path, &title)?;
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

fn read_title(dir: &Path) -> Option<String> {
    read_title_artist(dir).map(|(t, _)| t)
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

fn regenerate_for_dir(dir: &Path, audio_path: &Path, title: &str) -> Result<()> {
    let (samples, sample_rate) = analyzer::decode_audio(audio_path)?;
    let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;
    let audio_filename = audio_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let meta = SongMeta {
        title: title.to_string(),
        artist: String::new(),
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
