use cascade::game::hit_judge::{HitJudge, Judgement};
use cascade::game::state::GameState;

#[test]
fn test_perfect_hit() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1000), Judgement::Perfect);
    assert_eq!(judge.judge(1000, 1025), Judgement::Perfect);
    assert_eq!(judge.judge(1000, 975), Judgement::Perfect);
}

#[test]
fn test_great_hit() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1045), Judgement::Great);
    assert_eq!(judge.judge(1000, 955), Judgement::Great);
}

#[test]
fn test_good_hit() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1080), Judgement::Good);
    assert_eq!(judge.judge(1000, 920), Judgement::Good);
}

#[test]
fn test_miss() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1150), Judgement::Miss);
    assert_eq!(judge.judge(1000, 800), Judgement::Miss);
}

#[test]
fn test_offset_shifts_window() {
    let judge = HitJudge::new(50);
    assert_eq!(judge.judge(1000, 1050), Judgement::Perfect);
    assert_eq!(judge.judge(1000, 1075), Judgement::Perfect);
}

#[test]
fn test_game_state_combo() {
    let mut state = GameState::new();
    state.register_judgement(Judgement::Perfect);
    assert_eq!(state.combo, 1);
    assert_eq!(state.max_combo, 1);
    assert_eq!(state.score, 300);

    state.register_judgement(Judgement::Great);
    assert_eq!(state.combo, 2);

    state.register_judgement(Judgement::Miss);
    assert_eq!(state.combo, 0);
    assert_eq!(state.max_combo, 2);
}

#[test]
fn test_game_state_score_multiplier() {
    let mut state = GameState::new();
    for _ in 0..50 {
        state.register_judgement(Judgement::Perfect);
    }
    assert_eq!(state.combo, 50);
    let score_before = state.score;
    state.register_judgement(Judgement::Perfect);
    assert_eq!(state.score - score_before, 600);
}

#[test]
fn test_game_state_accuracy() {
    let mut state = GameState::new();
    state.register_judgement(Judgement::Perfect);
    state.register_judgement(Judgement::Miss);
    let acc = state.accuracy();
    assert!((acc - 50.0).abs() < 0.1);
}

#[test]
fn test_game_state_grade() {
    let mut state = GameState::new();
    for _ in 0..10 {
        state.register_judgement(Judgement::Perfect);
    }
    assert_eq!(state.grade(), "SS");
}

#[test]
fn test_game_state_grade_s_below_perfect() {
    let mut state = GameState::new();
    for _ in 0..19 {
        state.register_judgement(Judgement::Perfect);
    }
    state.register_judgement(Judgement::Great);
    let grade = state.grade();
    assert!(
        grade == "S" || grade == "SS",
        "expected S or SS for 95%+ accuracy, got {}",
        grade
    );
}

#[test]
fn drain_mode_bleeds_health_over_time() {
    let mut state = GameState::new();
    state.drain_mode = true;
    let before = state.health;
    state.tick_drain(2_000); // 2s
    assert!(
        state.health < before,
        "drain should lower health, got {} -> {}",
        before,
        state.health
    );
}

#[test]
fn drain_mode_perfect_outpaces_drain() {
    let mut state = GameState::new();
    state.drain_mode = true;
    state.health = 0.5;
    state.register_judgement(Judgement::Perfect);
    assert!(
        state.health > 0.5,
        "Perfect should raise health in drain mode, got {}",
        state.health
    );
}

#[test]
fn drain_is_noop_when_disabled() {
    let mut state = GameState::new();
    let before = state.health;
    state.tick_drain(10_000);
    assert_eq!(state.health, before);
}
