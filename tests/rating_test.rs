use cascade::beatmap::rating::{preview_start_ms, star_rating};
use cascade::beatmap::types::Note;

fn stream(start_ms: u64, count: u64, gap_ms: u64, lanes: &[u8]) -> Vec<Note> {
    (0..count)
        .map(|i| Note {
            time_ms: start_ms + i * gap_ms,
            lane: lanes[i as usize % lanes.len()],
            duration_ms: 0,
            slide_to: None,
        })
        .collect()
}

#[test]
fn empty_map_has_no_stars() {
    assert_eq!(star_rating(&[]), 0.0);
}

#[test]
fn denser_maps_rate_higher() {
    let slow = star_rating(&stream(0, 120, 500, &[0, 2, 4]));
    let fast = star_rating(&stream(0, 480, 125, &[0, 2, 4]));
    assert!(slow > 0.5 && fast > slow * 2.0, "slow {slow}, fast {fast}");
}

#[test]
fn jacks_rate_higher_than_alternation() {
    let alternating = star_rating(&stream(0, 200, 150, &[1, 3]));
    let jack = star_rating(&stream(0, 200, 150, &[2]));
    assert!(jack > alternating, "jack {jack}, alternating {alternating}");
}

#[test]
fn preview_starts_near_the_densest_section() {
    let mut notes = stream(0, 60, 1000, &[0]);
    notes.extend(stream(90_000, 100, 100, &[1, 3]));
    let start = preview_start_ms(&notes, 180_000);
    assert!((85_000..=90_000).contains(&start), "preview at {start}");
}
