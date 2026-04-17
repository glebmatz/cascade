//! Regression guard for practice-mode time scaling.
//!
//! Every use of `self.audio.position_ms()` inside the gameplay screen must go
//! through the `position_ms_in_track()` helper — otherwise the practice speed
//! multiplier silently stops applying to that code path.

#[test]
fn gameplay_only_uses_position_via_helper() {
    let src = std::fs::read_to_string("src/screens/gameplay.rs")
        .expect("failed to read src/screens/gameplay.rs");

    let raw_uses: Vec<(usize, &str)> = src
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("self.audio.position_ms()"))
        .collect();

    // The only legal call site is inside the helper itself.
    let helper_ok = raw_uses.iter().any(|(_, line)| {
        line.contains("self.audio.position_ms() as f64 * self.speed as f64")
    });
    assert!(
        helper_ok,
        "position_ms_in_track helper must exist and reference self.audio.position_ms()"
    );

    let leaked: Vec<_> = raw_uses
        .iter()
        .filter(|(_, line)| {
            !line.contains("self.audio.position_ms() as f64 * self.speed as f64")
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "Found raw `self.audio.position_ms()` outside the helper — practice speed scaling will be wrong:\n{:?}",
        leaked
    );
}
