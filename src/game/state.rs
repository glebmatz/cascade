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
        }
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    pub fn register_judgement(&mut self, judgement: Judgement) {
        self.total_notes += 1;
        self.max_possible_points += Judgement::max_points();
        self.last_judgement = Some(judgement);

        let health_delta = match judgement {
            Judgement::Perfect => 0.02,
            Judgement::Great => 0.01,
            Judgement::Good => 0.0,
            Judgement::Miss => -0.08,
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
