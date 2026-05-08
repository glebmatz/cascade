use cascade::audio::import::{self, MetadataFields};
use cascade::beatmap::loader;
use cascade::beatmap::types::{Beatmap, Difficulty, Note, SongMeta};
use cascade::share;

fn make_beatmap() -> Beatmap {
    Beatmap {
        version: 1,
        song: SongMeta {
            title: "Demo Track".into(),
            artist: "Demo Artist".into(),
            audio_file: "audio.mp3".into(),
            bpm: 128,
            duration_ms: 200_000,
        },
        difficulty: Difficulty::Hard,
        notes: vec![Note {
            time_ms: 1000,
            lane: 2,
            duration_ms: 0,
            slide_to: None,
        }],
    }
}

fn write_song_dir(dir: &std::path::Path, source_url: Option<&str>, sha: Option<&str>) {
    std::fs::create_dir_all(dir).unwrap();
    let mut meta = serde_json::json!({
        "title": "Demo Track",
        "artist": "Demo Artist",
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
    loader::save(&make_beatmap(), &dir.join("hard.json")).unwrap();
}

fn read_meta(dir: &std::path::Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(dir.join("metadata.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn export_then_install_round_trips_metadata_and_beatmaps() {
    let tmp = tempfile::tempdir().unwrap();
    let song_dir = tmp.path().join("songs/demo-artist-demo-track");
    write_song_dir(
        &song_dir,
        Some("https://example.invalid/a.mp3"),
        Some("cafe"),
    );

    let pkg = share::build_from_dir(&song_dir).unwrap();
    let cscd_path = tmp.path().join("share.cscd");
    share::save_package(&pkg, &cscd_path).unwrap();

    let new_songs = tmp.path().join("recipient/songs");
    std::fs::create_dir_all(&new_songs).unwrap();
    let outcome = share::install_package(&pkg, &new_songs, false).unwrap();

    assert_eq!(outcome.slug, "demo-artist-demo-track");
    assert!(outcome.song_dir.join("hard.json").is_file());
    assert!(outcome.song_dir.join("metadata.json").is_file());

    let v = read_meta(&outcome.song_dir);
    assert_eq!(
        v["source_url"].as_str(),
        Some("https://example.invalid/a.mp3")
    );
    assert_eq!(v["audio_sha256"].as_str(), Some("cafe"));
    assert_eq!(v["title"].as_str(), Some("Demo Track"));

    match outcome.audio_status {
        share::AudioStatus::SkippedByFlag => {}
        _ => panic!("expected SkippedByFlag when fetch_audio=false"),
    }
}

#[test]
fn install_without_source_url_marks_audio_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let song_dir = tmp.path().join("songs/no-url");
    write_song_dir(&song_dir, None, None);
    let pkg = share::build_from_dir(&song_dir).unwrap();

    let new_songs = tmp.path().join("recipient/songs");
    std::fs::create_dir_all(&new_songs).unwrap();
    let outcome = share::install_package(&pkg, &new_songs, true).unwrap();
    match outcome.audio_status {
        share::AudioStatus::Missing { expected_filename } => {
            assert_eq!(expected_filename, "audio.mp3");
        }
        _ => panic!("expected Missing when no source_url"),
    }
}

#[test]
fn build_export_drops_extra_metadata_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let song_dir = tmp.path().join("songs/junky");
    std::fs::create_dir_all(&song_dir).unwrap();
    std::fs::write(
        song_dir.join("metadata.json"),
        r#"{"title":"T","artist":"A","audio_file":"audio.mp3","secret":"do-not-leak"}"#,
    )
    .unwrap();
    loader::save(&make_beatmap(), &song_dir.join("easy.json")).unwrap();

    let pkg = share::build_from_dir(&song_dir).unwrap();
    let json = serde_json::to_string(&pkg).unwrap();
    assert!(!json.contains("secret"));
    assert!(!json.contains("do-not-leak"));
}

#[test]
fn write_metadata_file_preserves_source_url_and_sha() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    import::write_metadata(
        dir,
        MetadataFields {
            title: "Old",
            artist: "OldA",
            audio_file: "audio.mp3",
            source_url: Some("https://x/a.mp3"),
            audio_sha256: Some("abc123"),
        },
    )
    .unwrap();

    import::write_metadata_file(dir, "New", "NewA", "audio.mp3").unwrap();

    let v = read_meta(dir);
    assert_eq!(v["title"].as_str(), Some("New"));
    assert_eq!(v["artist"].as_str(), Some("NewA"));
    assert_eq!(v["source_url"].as_str(), Some("https://x/a.mp3"));
    assert_eq!(v["audio_sha256"].as_str(), Some("abc123"));
}

#[test]
fn rename_song_preserves_source_url_and_sha() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    import::write_metadata(
        dir,
        MetadataFields {
            title: "Old",
            artist: "OldA",
            audio_file: "audio.mp3",
            source_url: Some("https://x/a.mp3"),
            audio_sha256: Some("abc123"),
        },
    )
    .unwrap();

    import::rename_song(dir, "Renamed", "RenamedA").unwrap();

    let v = read_meta(dir);
    assert_eq!(v["title"].as_str(), Some("Renamed"));
    assert_eq!(v["artist"].as_str(), Some("RenamedA"));
    assert_eq!(v["source_url"].as_str(), Some("https://x/a.mp3"));
    assert_eq!(v["audio_sha256"].as_str(), Some("abc123"));
}

#[test]
fn sha256_detects_byte_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("a.bin");
    std::fs::write(&p, b"hello world").unwrap();
    let h1 = import::sha256_of(&p).unwrap();
    assert_eq!(
        h1,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );

    std::fs::write(&p, b"hello world!").unwrap();
    let h2 = import::sha256_of(&p).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn sha256_handles_empty_file() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("empty.bin");
    std::fs::write(&p, b"").unwrap();
    let h = import::sha256_of(&p).unwrap();
    assert_eq!(
        h,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn install_roundtrips_full_beatmap_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let song_dir = tmp.path().join("songs/orig");
    write_song_dir(&song_dir, Some("https://x/y.mp3"), Some("ff"));
    let pkg = share::build_from_dir(&song_dir).unwrap();

    let new_songs = tmp.path().join("recipient");
    std::fs::create_dir_all(&new_songs).unwrap();
    let outcome = share::install_package(&pkg, &new_songs, false).unwrap();

    let installed = loader::load(&outcome.song_dir.join("hard.json")).unwrap();
    let original = make_beatmap();
    assert_eq!(installed.notes.len(), original.notes.len());
    assert_eq!(installed.notes[0].time_ms, original.notes[0].time_ms);
    assert_eq!(installed.notes[0].lane, original.notes[0].lane);
    assert_eq!(installed.song.bpm, original.song.bpm);
    assert_eq!(installed.song.duration_ms, original.song.duration_ms);
}

#[test]
fn package_filename_extension_is_cscd() {
    let tmp = tempfile::tempdir().unwrap();
    let song_dir = tmp.path().join("songs/x");
    write_song_dir(&song_dir, None, None);
    let pkg = share::build_from_dir(&song_dir).unwrap();

    let p = tmp.path().join("nested/sub/out.cscd");
    share::save_package(&pkg, &p).unwrap();
    assert!(p.is_file());
    assert!(share::load_package(&p).is_ok());
}
