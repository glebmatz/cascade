use cascade::ui::theme::{self, ThemeFile};
use tempfile::TempDir;

#[test]
fn five_built_ins_shipped_with_unique_slugs() {
    assert_eq!(theme::BUILTINS.len(), 5);
    let mut slugs: Vec<&str> = theme::BUILTINS.iter().map(|t| t.slug).collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), 5, "theme slugs must be unique");
}

#[test]
fn every_built_in_theme_has_complete_palette() {
    for t in &theme::BUILTINS {
        assert!(!t.name.is_empty(), "theme {} missing name", t.slug);
        assert!(!t.slug.is_empty());
        assert_eq!(t.lane_colors.len(), 5);
        assert_eq!(t.judgement.len(), 4);
        assert_eq!(t.particle.len(), 3);
    }
}

#[test]
fn by_slug_is_case_insensitive_and_falls_back() {
    assert_eq!(theme::by_slug("classic").unwrap().slug, "classic");
    assert_eq!(theme::by_slug("CLASSIC").unwrap().slug, "classic");
    assert_eq!(theme::by_slug("Neon").unwrap().slug, "neon");
    assert!(theme::by_slug("does-not-exist").is_none());
    assert_eq!(theme::resolve_or_default("does-not-exist").slug, "classic");
}

#[test]
fn cycle_next_and_prev_wrap_around() {
    let start = "classic";
    let second = theme::cycle_next(start).slug;
    let back = theme::cycle_prev(second).slug;
    assert_eq!(back, start);

    let mut slug = start.to_string();
    for _ in 0..theme::all().len() {
        slug = theme::cycle_next(&slug).slug.to_string();
    }
    assert_eq!(slug, start);
}

#[test]
fn set_and_read_active_theme() {
    theme::set_active(theme::NEON);
    assert_eq!(theme::active().slug, "neon");
    theme::set_active(theme::CLASSIC);
    assert_eq!(theme::active().slug, "classic");
}

// -------- User-theme loader --------

fn write(dir: &std::path::Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).unwrap();
}

const VALID_CHERRY: &str = r#"
slug = "cherry"
name = "Cherry Blossom"
lane_colors = [[250,200,210],[255,170,190],[240,150,180],[200,120,170],[160,90,150]]
combo_heat = [255,150,180]
judgement = [[255,220,230],[230,180,200],[180,150,170],[200,80,120]]
particle = [[255,220,230],[230,180,200],[200,160,180]]
"#;

#[test]
fn load_themes_from_missing_dir_returns_empty() {
    let (themes, issues) = theme::load_themes_from(std::path::Path::new("/nonexistent/dir/abc"));
    assert!(themes.is_empty());
    assert!(issues.is_empty());
}

#[test]
fn load_valid_user_theme() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "cherry.toml", VALID_CHERRY);

    let (themes, issues) = theme::load_themes_from(dir.path());
    assert_eq!(issues.len(), 0);
    assert_eq!(themes.len(), 1);
    assert_eq!(themes[0].slug, "cherry");
    assert_eq!(themes[0].name, "Cherry Blossom");
    assert_eq!(themes[0].lane_colors[0], (250, 200, 210));
    assert_eq!(themes[0].combo_heat, (255, 150, 180));
}

#[test]
fn invalid_theme_files_are_reported_not_panicked() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a_bad_toml.toml", "this is [not valid toml==");
    write(
        dir.path(),
        "b_wrong_shape.toml",
        r#"
slug = "short"
name = "Short"
lane_colors = [[1,2,3]]
combo_heat = [0,0,0]
judgement = [[1,1,1],[2,2,2],[3,3,3],[4,4,4]]
particle = [[1,1,1],[2,2,2],[3,3,3]]
"#,
    );
    write(
        dir.path(),
        "c_conflict.toml",
        r#"
slug = "classic"
name = "User Classic"
lane_colors = [[1,1,1],[2,2,2],[3,3,3],[4,4,4],[5,5,5]]
combo_heat = [0,0,0]
judgement = [[1,1,1],[2,2,2],[3,3,3],[4,4,4]]
particle = [[1,1,1],[2,2,2],[3,3,3]]
"#,
    );
    write(dir.path(), "d_valid.toml", VALID_CHERRY);

    let (themes, issues) = theme::load_themes_from(dir.path());
    assert_eq!(themes.len(), 1, "only the valid non-conflicting file loads");
    assert_eq!(themes[0].slug, "cherry");
    assert_eq!(issues.len(), 3);
    // One issue per failing file.
    let reasons: Vec<_> = issues.iter().map(|i| i.reason.as_str()).collect();
    assert!(reasons.iter().any(|r| r.contains("parse error")));
    assert!(reasons.iter().any(|r| r.contains("lane_colors")));
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("conflicts with a built-in"))
    );
}

#[test]
fn duplicate_user_slug_is_reported() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.toml", VALID_CHERRY);
    write(dir.path(), "b.toml", VALID_CHERRY);
    let (themes, issues) = theme::load_themes_from(dir.path());
    assert_eq!(themes.len(), 1);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].reason.contains("duplicate slug"));
}

#[test]
fn theme_file_into_theme_rejects_bad_shape() {
    let bad = ThemeFile {
        slug: "x".to_string(),
        name: "X".to_string(),
        lane_colors: vec![[1, 1, 1]; 4], // should be 5
        combo_heat: [0, 0, 0],
        judgement: vec![[1, 1, 1]; 4],
        particle: vec![[1, 1, 1]; 3],
    };
    assert!(bad.into_theme().is_err());
}
