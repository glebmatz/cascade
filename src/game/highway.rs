use crate::beatmap::types::Note;

pub struct Highway {
    pub scroll_speed: f64,
    pub visible_notes: Vec<VisibleNote>,
}

pub struct VisibleNote {
    pub note_index: usize,
    pub lane: u8,
    pub time_ms: u64,
    /// 0.0 = at hit zone, 1.0 = top of screen, negative = past hit zone
    pub position: f64,
    /// End position for hold notes (same scale as position). 0 for tap notes.
    pub end_position: f64,
    /// Duration in ms (0 = tap note)
    pub duration_ms: u64,
    pub hit: bool,
}

impl Highway {
    pub fn new(scroll_speed: f64) -> Self {
        Self {
            scroll_speed,
            visible_notes: Vec::new(),
        }
    }

    pub fn update(&mut self, notes: &[Note], current_time_ms: u64, look_ahead_ms: u64, hit_notes: &[bool]) {
        self.visible_notes.clear();
        let look_ahead = (look_ahead_ms as f64 / self.scroll_speed) as u64;

        for (i, note) in notes.iter().enumerate() {
            if hit_notes.get(i).copied().unwrap_or(false) {
                continue;
            }
            let note_end = note.time_ms + note.duration_ms;
            if note.time_ms > current_time_ms + look_ahead {
                continue;
            }
            if (current_time_ms as i64 - note_end as i64) > 500 {
                continue;
            }

            let time_diff = note.time_ms as f64 - current_time_ms as f64;
            let position = time_diff / look_ahead as f64;

            let end_position = if note.duration_ms > 0 {
                let end_diff = note_end as f64 - current_time_ms as f64;
                end_diff / look_ahead as f64
            } else {
                0.0
            };

            self.visible_notes.push(VisibleNote {
                note_index: i,
                lane: note.lane,
                time_ms: note.time_ms,
                position,
                end_position,
                duration_ms: note.duration_ms,
                hit: false,
            });
        }
    }
}
