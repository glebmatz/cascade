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

    #[allow(dead_code)]
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

        if diff <= Self::PERFECT_MS {
            Judgement::Perfect
        } else if diff <= Self::GREAT_MS {
            Judgement::Great
        } else if diff <= Self::GOOD_MS {
            Judgement::Good
        } else {
            Judgement::Miss
        }
    }

    pub fn is_expired(&self, note_time_ms: u64, current_time_ms: u64) -> bool {
        current_time_ms as i64 - self.offset_ms > note_time_ms as i64 + Self::MISS_MS as i64
    }

    #[allow(dead_code)]
    pub fn hit_window_ms(&self) -> u64 {
        Self::MISS_MS
    }

    pub const PERFECT_MS: u64 = 35;
    pub const GREAT_MS: u64 = 75;
    pub const GOOD_MS: u64 = 120;
    pub const MISS_MS: u64 = 160;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_within_35ms() {
        let j = HitJudge::new(0);
        assert_eq!(j.judge(1000, 1030), Judgement::Perfect);
        assert_eq!(j.judge(1000, 970), Judgement::Perfect);
        assert_eq!(j.judge(1000, 1000), Judgement::Perfect);
    }

    #[test]
    fn great_between_35_and_75() {
        let j = HitJudge::new(0);
        assert_eq!(j.judge(1000, 1070), Judgement::Great);
        assert_eq!(j.judge(1000, 930), Judgement::Great);
    }

    #[test]
    fn good_between_75_and_120() {
        let j = HitJudge::new(0);
        assert_eq!(j.judge(1000, 1110), Judgement::Good);
        assert_eq!(j.judge(1000, 890), Judgement::Good);
    }

    #[test]
    fn miss_beyond_120() {
        let j = HitJudge::new(0);
        assert_eq!(j.judge(1000, 1150), Judgement::Miss);
        assert_eq!(j.judge(1000, 800), Judgement::Miss);
    }

    #[test]
    fn offset_shifts_judgement() {
        let j = HitJudge::new(50);
        // press at 1050 with +50 offset == effective press at 1000
        assert_eq!(j.judge(1000, 1050), Judgement::Perfect);
    }

    #[test]
    fn expired_at_miss_boundary() {
        let j = HitJudge::new(0);
        assert!(!j.is_expired(1000, 1150));
        assert!(j.is_expired(1000, 1161));
    }
}
