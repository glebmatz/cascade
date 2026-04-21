use super::hit_judge::Judgement;

pub struct GameState {
    pub score: u64,
    pub combo: u32,
    pub max_combo: u32,
    pub total_notes: u32,
    pub earned_points: u64,
    pub max_possible_points: u64,
    pub last_judgement: Option<Judgement>,
    pub judgement_counts: [u32; 4],
    pub health: f64,
    /// Drain mode: health falls over time. Only Perfects give meaningful restore.
    pub drain_mode: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    pub fn new() -> Self {
        Self {
            score: 0,
            combo: 0,
            max_combo: 0,
            total_notes: 0,
            earned_points: 0,
            max_possible_points: 0,
            last_judgement: None,
            judgement_counts: [0; 4],
            health: 1.0,
            drain_mode: false,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    /// Constant health drain per second. Calibrated so a run that lands mostly
    /// Greats barely stays alive; Perfect-heavy runs comfortably top up.
    pub const DRAIN_PER_SECOND: f64 = 0.05;

    /// Apply continuous drain for a frame of `dt_ms` milliseconds. No-op unless
    /// `drain_mode` is set. Does not clamp below 0 — callers check `is_dead`.
    pub fn tick_drain(&mut self, dt_ms: u64) {
        if !self.drain_mode {
            return;
        }
        let dt_s = dt_ms as f64 / 1000.0;
        self.health = (self.health - Self::DRAIN_PER_SECOND * dt_s).max(0.0);
    }

    pub fn register_judgement(&mut self, judgement: Judgement) {
        self.total_notes += 1;
        self.max_possible_points += Judgement::max_points();
        self.last_judgement = Some(judgement);

        // Drain mode rebalances health deltas: Perfects heal more to offset
        // constant drain; non-Perfects give almost nothing.
        let health_delta = if self.drain_mode {
            match judgement {
                Judgement::Perfect => 0.035,
                Judgement::Great => 0.005,
                Judgement::Good => -0.01,
                Judgement::Miss => -0.10,
            }
        } else {
            match judgement {
                Judgement::Perfect => 0.02,
                Judgement::Great => 0.01,
                Judgement::Good => 0.0,
                Judgement::Miss => -0.08,
            }
        };
        self.health = (self.health + health_delta).clamp(0.0, 1.0);

        let idx = match judgement {
            Judgement::Perfect => 0,
            Judgement::Great => 1,
            Judgement::Good => 2,
            Judgement::Miss => 3,
        };
        self.judgement_counts[idx] += 1;

        if judgement == Judgement::Miss {
            self.combo = 0;
        } else {
            let multiplier = (1.0 + (self.combo as f64 / 50.0)).min(5.0);
            let points = (judgement.base_points() as f64 * multiplier) as u64;
            self.score += points;
            self.earned_points += judgement.base_points();
            self.combo += 1;
            if self.combo > self.max_combo {
                self.max_combo = self.combo;
            }
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.max_possible_points == 0 {
            return 100.0;
        }
        (self.earned_points as f64 / self.max_possible_points as f64) * 100.0
    }

    pub fn grade(&self) -> &'static str {
        let acc = self.accuracy();
        if acc >= 100.0 {
            "SS"
        } else if acc >= 95.0 {
            "S"
        } else if acc >= 90.0 {
            "A"
        } else if acc >= 80.0 {
            "B"
        } else if acc >= 70.0 {
            "C"
        } else {
            "D"
        }
    }
}
