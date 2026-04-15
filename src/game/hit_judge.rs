#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judgement {
    Perfect,
    Great,
    Good,
    Miss,
}

impl Judgement {
    pub fn base_points(&self) -> u64 {
        match self {
            Judgement::Perfect => 300,
            Judgement::Great => 200,
            Judgement::Good => 100,
            Judgement::Miss => 0,
        }
    }

    pub fn max_points() -> u64 {
        300
    }

    pub fn label(&self) -> &'static str {
        match self {
            Judgement::Perfect => "PERFECT",
            Judgement::Great => "GREAT",
            Judgement::Good => "GOOD",
            Judgement::Miss => "MISS",
        }
    }
}

pub struct HitJudge {
    offset_ms: i64,
}

impl HitJudge {
    pub fn new(offset_ms: i32) -> Self {
        Self {
            offset_ms: offset_ms as i64,
        }
    }

    pub fn judge(&self, note_time_ms: u64, press_time_ms: u64) -> Judgement {
        let adjusted_press = press_time_ms as i64 - self.offset_ms;
        let diff = (adjusted_press - note_time_ms as i64).unsigned_abs();

        if diff <= 30 {
            Judgement::Perfect
        } else if diff <= 60 {
            Judgement::Great
        } else if diff <= 100 {
            Judgement::Good
        } else {
            Judgement::Miss
        }
    }

    pub fn is_expired(&self, note_time_ms: u64, current_time_ms: u64) -> bool {
        current_time_ms as i64 - self.offset_ms > note_time_ms as i64 + 100
    }
}
