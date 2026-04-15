use cascade::config::Config;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.gameplay.scroll_speed, 1.0);
    assert_eq!(config.gameplay.difficulty, "hard");
    assert_eq!(config.keys.lanes, ['d', 'f', ' ', 'j', 'k']);
    assert_eq!(config.audio.volume, 0.8);
    assert_eq!(config.audio.offset_ms, 0);
    assert_eq!(config.display.fps, 60);
}

#[test]
fn test_config_save_and_load() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let mut config = Config::default();
    config.gameplay.scroll_speed = 1.5;
    config.audio.offset_ms = -30;

    config.save(&path).unwrap();
    let loaded = Config::load(&path).unwrap();

    assert_eq!(loaded.gameplay.scroll_speed, 1.5);
    assert_eq!(loaded.audio.offset_ms, -30);
    assert_eq!(loaded.keys.lanes, ['d', 'f', ' ', 'j', 'k']);
}

#[test]
fn test_config_load_missing_file_returns_default() {
    let config = Config::load(Path::new("/nonexistent/config.toml")).unwrap();
    assert_eq!(config.gameplay.scroll_speed, 1.0);
}
