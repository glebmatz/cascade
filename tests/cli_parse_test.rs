use cascade::cli;

#[test]
fn parse_mmss_basic() {
    assert_eq!(cli::parse_mmss("0:00").unwrap(), 0);
    assert_eq!(cli::parse_mmss("1:30").unwrap(), 90_000);
    assert_eq!(cli::parse_mmss("10:05").unwrap(), 605_000);
    assert_eq!(cli::parse_mmss("59:59").unwrap(), 3_599_000);
}

#[test]
fn parse_mmss_allows_long_tracks() {
    // Minutes can exceed 59 for long sets / podcasts.
    assert_eq!(cli::parse_mmss("90:00").unwrap(), 5_400_000);
}

#[test]
fn parse_mmss_rejects_malformed() {
    assert!(cli::parse_mmss("").is_err());
    assert!(cli::parse_mmss("1").is_err());
    assert!(cli::parse_mmss("1:").is_err());
    assert!(cli::parse_mmss(":30").is_err());
    assert!(cli::parse_mmss("abc").is_err());
    assert!(cli::parse_mmss("1:60").is_err());
    assert!(cli::parse_mmss("1:99").is_err());
}

#[test]
fn parse_section_happy_path() {
    let (a, b) = cli::parse_section("1:30-2:00").unwrap();
    assert_eq!(a, 90_000);
    assert_eq!(b, 120_000);
}

#[test]
fn parse_section_rejects_reversed() {
    assert!(cli::parse_section("2:00-1:30").is_err());
    assert!(cli::parse_section("1:00-1:00").is_err());
}

#[test]
fn parse_section_rejects_malformed() {
    assert!(cli::parse_section("1:30").is_err());
    assert!(cli::parse_section("1:30-").is_err());
    assert!(cli::parse_section("-1:30").is_err());
    assert!(cli::parse_section("xx-yy").is_err());
}

#[test]
fn parse_speed_clamps() {
    assert!((cli::parse_speed("0.7").unwrap() - 0.7).abs() < 0.001);
    assert_eq!(cli::parse_speed("0.1").unwrap(), 0.25);
    assert_eq!(cli::parse_speed("3.0").unwrap(), 2.0);
    assert!((cli::parse_speed("1.0").unwrap() - 1.0).abs() < 0.001);
}

#[test]
fn parse_speed_rejects_non_numeric_or_zero() {
    assert!(cli::parse_speed("abc").is_err());
    assert!(cli::parse_speed("").is_err());
    assert!(cli::parse_speed("0").is_err());
    assert!(cli::parse_speed("-0.5").is_err());
}

#[test]
fn extract_practice_none_when_no_flags() {
    let args = vec!["--hard".to_string()];
    assert!(cli::extract_practice(&args).unwrap().is_none());
}

#[test]
fn extract_practice_section_only() {
    let args = vec!["--section".to_string(), "1:30-2:00".to_string()];
    let p = cli::extract_practice(&args).unwrap().unwrap();
    assert_eq!(p.section_start_ms, Some(90_000));
    assert_eq!(p.section_end_ms, Some(120_000));
    assert!((p.speed - 1.0).abs() < 0.001);
    assert!(p.is_active());
}

#[test]
fn extract_practice_speed_only() {
    let args = vec!["--speed".to_string(), "0.7".to_string()];
    let p = cli::extract_practice(&args).unwrap().unwrap();
    assert!(p.section_start_ms.is_none());
    assert!(p.section_end_ms.is_none());
    assert!((p.speed - 0.7).abs() < 0.001);
    assert!(p.is_active());
}

#[test]
fn extract_practice_both_flags() {
    let args = vec![
        "--speed".to_string(),
        "0.8".to_string(),
        "--section".to_string(),
        "0:30-1:00".to_string(),
    ];
    let p = cli::extract_practice(&args).unwrap().unwrap();
    assert_eq!(p.section_start_ms, Some(30_000));
    assert_eq!(p.section_end_ms, Some(60_000));
    assert!((p.speed - 0.8).abs() < 0.001);
}

#[test]
fn extract_practice_bubbles_up_parse_errors() {
    let args = vec!["--section".to_string(), "invalid".to_string()];
    assert!(cli::extract_practice(&args).is_err());
}
