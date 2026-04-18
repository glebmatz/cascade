use cascade::achievements::AchievementStore;
use cascade::play_history::{PlayHistory, PlayRecord};
use cascade::stats::{self, HEATMAP_DAYS};

const DAY: u64 = 86_400;

fn rec(ts: u64, slug: &str, title: &str, diff: &str, score: u64, acc: f64) -> PlayRecord {
    PlayRecord {
        ts,
        slug: slug.to_string(),
        title: title.to_string(),
        difficulty: diff.to_string(),
        mods: String::new(),
        score,
        accuracy: acc,
        max_combo: 100,
        total_notes: 200,
        judgements: [150, 30, 10, 10],
        duration_played_ms: 120_000,
        song_duration_ms: 120_000,
        grade: "A".to_string(),
        died: false,
    }
}

#[test]
fn empty_history_gives_zero_totals() {
    let history = PlayHistory::default();
    let ach = AchievementStore::default();
    let s = stats::summarize(&history, &ach, 1_700_000_000);
    assert_eq!(s.total_plays, 0);
    assert_eq!(s.total_time_played_ms, 0);
    assert_eq!(s.total_notes_hit, 0);
    assert!(s.top_songs.is_empty());
    assert!(s.per_difficulty.is_empty());
    assert_eq!(s.heatmap_30d, [0u32; HEATMAP_DAYS]);
    assert!(s.accuracy_30d.iter().all(|v| v.is_none()));
    assert_eq!(s.achievements_total, 12);
    assert_eq!(s.achievements_unlocked, 0);
}

#[test]
fn totals_sum_correctly_and_excludes_misses_from_notes_hit() {
    let now = 1_700_000_000;
    let mut history = PlayHistory::default();
    let mut r1 = rec(now - DAY, "a", "A", "Hard", 100, 90.0);
    r1.judgements = [100, 10, 5, 20]; // hits = 115, miss = 20
    r1.duration_played_ms = 50_000;
    let mut r2 = rec(now - 2 * DAY, "b", "B", "Easy", 200, 80.0);
    r2.judgements = [80, 20, 10, 30];
    r2.duration_played_ms = 70_000;
    history.append(r1);
    history.append(r2);

    let s = stats::summarize(&history, &AchievementStore::default(), now);
    assert_eq!(s.total_plays, 2);
    assert_eq!(s.total_time_played_ms, 120_000);
    assert_eq!(s.total_notes_hit, 115 + 110);
}

#[test]
fn top_songs_sorted_by_plays_desc_and_truncated_to_five() {
    let mut history = PlayHistory::default();
    let now = 1_700_000_000;
    // "alpha" × 3, "bravo" × 5, "charlie" × 1, "delta" × 2, "echo" × 4, "foxtrot" × 1
    for slug in ["alpha", "alpha", "alpha"] {
        history.append(rec(now, slug, "Alpha", "Hard", 1, 50.0));
    }
    for _ in 0..5 {
        history.append(rec(now, "bravo", "Bravo", "Hard", 1, 50.0));
    }
    history.append(rec(now, "charlie", "Charlie", "Hard", 1, 50.0));
    history.append(rec(now, "delta", "Delta", "Hard", 1, 50.0));
    history.append(rec(now, "delta", "Delta", "Hard", 1, 50.0));
    for _ in 0..4 {
        history.append(rec(now, "echo", "Echo", "Hard", 1, 50.0));
    }
    history.append(rec(now, "foxtrot", "Foxtrot", "Hard", 1, 50.0));

    let s = stats::summarize(&history, &AchievementStore::default(), now);
    let slugs: Vec<&str> = s.top_songs.iter().map(|t| t.slug.as_str()).collect();
    assert_eq!(slugs, vec!["bravo", "echo", "alpha", "delta", "charlie"]);
    assert_eq!(s.top_songs.len(), 5);
    assert_eq!(s.top_songs[0].plays, 5);
}

#[test]
fn top_songs_title_reflects_latest_rename() {
    let now = 1_700_000_000;
    let mut history = PlayHistory::default();
    history.append(rec(now - DAY * 2, "song", "Old Title", "Hard", 1, 50.0));
    history.append(rec(now - DAY, "song", "Old Title", "Hard", 1, 50.0));
    history.append(rec(now, "song", "New Title", "Hard", 1, 50.0));

    let s = stats::summarize(&history, &AchievementStore::default(), now);
    assert_eq!(s.top_songs[0].slug, "song");
    assert_eq!(s.top_songs[0].title, "New Title");
    assert_eq!(s.top_songs[0].plays, 3);
}

#[test]
fn per_difficulty_groups_and_orders() {
    let now = 1_700_000_000;
    let mut history = PlayHistory::default();
    history.append(rec(now, "a", "A", "Hard", 500, 88.0));
    history.append(rec(now, "a", "A", "Hard", 700, 92.0));
    history.append(rec(now, "a", "A", "easy", 100, 99.0)); // lowercase, still Easy
    history.append(rec(now, "a", "A", "Expert", 900, 60.0));

    let s = stats::summarize(&history, &AchievementStore::default(), now);
    let labels: Vec<&str> = s
        .per_difficulty
        .iter()
        .map(|d| d.difficulty.as_str())
        .collect();
    assert_eq!(labels, vec!["Easy", "Hard", "Expert"]);

    let easy = &s.per_difficulty[0];
    assert_eq!(easy.plays, 1);
    assert!((easy.best_accuracy - 99.0).abs() < 1e-9);
    assert_eq!(easy.best_score, 100);

    let hard = &s.per_difficulty[1];
    assert_eq!(hard.plays, 2);
    assert!((hard.best_accuracy - 92.0).abs() < 1e-9);
    assert_eq!(hard.best_score, 700);
    assert!((hard.avg_accuracy - 90.0).abs() < 1e-9);
}

#[test]
fn heatmap_and_accuracy_windows_align_with_today() {
    let now = 100 * DAY + 500; // some time on day 100 — plenty of room for -30 days
    let mut history = PlayHistory::default();
    // Today (day 10): 2 plays, 80 and 90.
    history.append(rec(now, "a", "A", "Hard", 1, 80.0));
    history.append(rec(now + 60, "a", "A", "Hard", 1, 90.0));
    // 29 days ago (oldest in window): 1 play, 70%.
    history.append(rec(now - 29 * DAY, "a", "A", "Hard", 1, 70.0));
    // 30 days ago — outside the window.
    history.append(rec(now - 30 * DAY, "a", "A", "Hard", 1, 55.0));
    // Future timestamp — should be ignored.
    history.append(rec(now + DAY, "a", "A", "Hard", 1, 100.0));

    let s = stats::summarize(&history, &AchievementStore::default(), now);
    // Today bucket = last slot (index 29).
    assert_eq!(s.heatmap_30d[29], 2);
    assert!((s.accuracy_30d[29].unwrap() - 85.0).abs() < 1e-9);
    // 29 days ago = index 0.
    assert_eq!(s.heatmap_30d[0], 1);
    assert!((s.accuracy_30d[0].unwrap() - 70.0).abs() < 1e-9);
    // Outside-window play is ignored.
    let total_in_window: u32 = s.heatmap_30d.iter().sum();
    assert_eq!(total_in_window, 3);
}

#[test]
fn sparkline_outputs_30_chars_with_spaces_for_blank_days() {
    let mut data: [Option<f64>; 30] = [None; 30];
    data[5] = Some(80.0);
    data[10] = Some(90.0);
    data[29] = Some(100.0);
    let line = stats::sparkline_30d(&data);
    let chars: Vec<char> = line.chars().collect();
    assert_eq!(chars.len(), 30);
    assert_eq!(chars[0], ' ');
    assert!(chars[5] != ' ');
    assert!(chars[29] != ' ');
}

#[test]
fn heatmap_glyphs_maps_zero_days_to_dot() {
    let mut counts = [0u32; 30];
    counts[0] = 0;
    counts[1] = 5;
    counts[2] = 1;
    let line = stats::heatmap_glyphs(&counts);
    let chars: Vec<char> = line.chars().collect();
    assert_eq!(chars.len(), 30);
    assert_eq!(chars[0], '·');
    // Highest count should produce the densest glyph.
    assert_eq!(chars[1], '█');
    // Small count produces a non-empty, non-max glyph.
    assert!(chars[2] != '·' && chars[2] != '█');
}

#[test]
fn format_duration_handles_hours_minutes_seconds() {
    assert_eq!(stats::format_duration_ms(0), "0s");
    assert_eq!(stats::format_duration_ms(45_000), "45s");
    assert_eq!(stats::format_duration_ms(3_660_000), "1h 01m");
    assert_eq!(stats::format_duration_ms(125_000), "2m 05s");
}
