use cascade::play_history::{self, PlayHistory, PlayRecord, ReplayEvent};
use std::path::Path;
use tempfile::TempDir;

fn sample_record(ts: u64, slug: &str, score: u64) -> PlayRecord {
    PlayRecord {
        run_id: format!("run-{ts}"),
        ts,
        slug: slug.to_string(),
        title: "Song Title".to_string(),
        difficulty: "Hard".to_string(),
        mods: String::new(),
        score,
        accuracy: 92.5,
        max_combo: 101,
        total_notes: 200,
        judgements: [180, 10, 5, 5],
        duration_played_ms: 60_000,
        song_duration_ms: 60_000,
        grade: "A".to_string(),
        died: false,
        events: Vec::new(),
    }
}

#[test]
fn load_missing_file_returns_empty_history() {
    let hist = PlayHistory::load(Path::new("/nonexistent/path/play_history.json"));
    assert_eq!(hist.plays.len(), 0);
}

#[test]
fn load_corrupt_file_returns_empty_history() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("play_history.json");
    std::fs::write(&path, "{ not valid json").unwrap();
    let hist = PlayHistory::load(&path);
    assert_eq!(hist.plays.len(), 0);
}

#[test]
fn save_then_load_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("play_history.json");

    let mut hist = PlayHistory::default();
    hist.append(sample_record(100, "song-a", 10_000));
    hist.append(sample_record(200, "song-b", 20_000));
    hist.save(&path).unwrap();

    let loaded = PlayHistory::load(&path);
    assert_eq!(loaded.plays.len(), 2);
    assert_eq!(loaded.plays[0].slug, "song-a");
    assert_eq!(loaded.plays[0].score, 10_000);
    assert_eq!(loaded.plays[1].slug, "song-b");
    assert_eq!(loaded.plays[1].score, 20_000);
    assert_eq!(loaded.version, 1);
}

#[test]
fn append_preserves_order() {
    let mut hist = PlayHistory::default();
    hist.append(sample_record(10, "a", 1));
    hist.append(sample_record(20, "b", 2));
    hist.append(sample_record(30, "c", 3));
    let slugs: Vec<&str> = hist.plays.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, vec!["a", "b", "c"]);
}

#[test]
fn breakdown_reports_bias_and_rough_spots() {
    let events = vec![
        ReplayEvent {
            note_idx: 0,
            note_time_ms: 1_000,
            lane: 0,
            input_time_ms: Some(990),
            offset_ms: Some(-10),
            judgement: "great".to_string(),
            kind: "tap".to_string(),
        },
        ReplayEvent {
            note_idx: 1,
            note_time_ms: 2_000,
            lane: 1,
            input_time_ms: Some(2_030),
            offset_ms: Some(30),
            judgement: "perfect".to_string(),
            kind: "tap".to_string(),
        },
        ReplayEvent {
            note_idx: 2,
            note_time_ms: 16_000,
            lane: 1,
            input_time_ms: None,
            offset_ms: None,
            judgement: "miss".to_string(),
            kind: "miss".to_string(),
        },
    ];

    let b = play_history::breakdown(&events, 60_000);
    assert_eq!(b.hit_events, 2);
    assert_eq!(b.early, 1);
    assert_eq!(b.late, 1);
    assert_eq!(b.misses, 1);
    assert_eq!(b.worst_lane, Some((1, 1)));
    assert_eq!(b.worst_section, Some((15_000, 1)));
    assert!((b.avg_offset_ms.unwrap() - 10.0).abs() < 1e-9);
}
