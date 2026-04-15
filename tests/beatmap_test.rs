use cascade::beatmap::types::{Beatmap, Note, SongMeta, Difficulty};
use cascade::beatmap::loader;
use tempfile::TempDir;

#[test]
fn test_beatmap_serialization_roundtrip() {
    let beatmap = Beatmap {
        version: 1,
        song: SongMeta {
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            audio_file: "audio.mp3".to_string(),
            bpm: 120,
            duration_ms: 180000,
        },
        difficulty: Difficulty::Hard,
        notes: vec![
            Note { time_ms: 1000, lane: 0 },
            Note { time_ms: 1200, lane: 2 },
            Note { time_ms: 1200, lane: 4 },
            Note { time_ms: 1500, lane: 1 },
        ],
    };

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("hard.json");

    loader::save(&beatmap, &path).unwrap();
    let loaded = loader::load(&path).unwrap();

    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.song.title, "Test Song");
    assert_eq!(loaded.song.bpm, 120);
    assert_eq!(loaded.difficulty, Difficulty::Hard);
    assert_eq!(loaded.notes.len(), 4);
    assert_eq!(loaded.notes[0].time_ms, 1000);
    assert_eq!(loaded.notes[0].lane, 0);
    assert_eq!(loaded.notes[2].lane, 4);
}

#[test]
fn test_difficulty_ordering() {
    assert!(Difficulty::Easy < Difficulty::Medium);
    assert!(Difficulty::Medium < Difficulty::Hard);
    assert!(Difficulty::Hard < Difficulty::Expert);
}

#[test]
fn test_difficulty_filename() {
    assert_eq!(Difficulty::Easy.filename(), "easy.json");
    assert_eq!(Difficulty::Expert.filename(), "expert.json");
}
