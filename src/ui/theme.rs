use crate::ui::color::Rgb;
use serde::Deserialize;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

/// All tunable palette entries for a theme. Each `Rgb` is a plain `(u8,u8,u8)`
/// triple so the struct is `Copy` and cheap to grab from the global at render
/// time.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub slug: &'static str,
    pub name: &'static str,
    /// Five lane colors (D / F / Space / J / K).
    pub lane_colors: [Rgb; 5],
    /// Warm tint added to the highway as combo rises.
    pub combo_heat: Rgb,
    /// Judgement splash colors, in order: Perfect / Great / Good / Miss.
    pub judgement: [Rgb; 4],
    /// Particle colors for Perfect / Great / Good.
    pub particle: [Rgb; 3],
}

pub const CLASSIC: Theme = Theme {
    slug: "classic",
    name: "Classic",
    lane_colors: [
        (220, 80, 80),
        (80, 200, 90),
        (230, 210, 80),
        (80, 140, 230),
        (190, 80, 220),
    ],
    combo_heat: (230, 80, 40),
    judgement: [
        (255, 240, 150),
        (180, 240, 180),
        (170, 170, 170),
        (200, 80, 80),
    ],
    particle: [(255, 240, 150), (180, 240, 180), (170, 170, 170)],
};

pub const NEON: Theme = Theme {
    slug: "neon",
    name: "Neon",
    lane_colors: [
        (255, 60, 180),
        (140, 70, 255),
        (80, 230, 250),
        (60, 130, 255),
        (255, 120, 220),
    ],
    combo_heat: (255, 50, 200),
    judgement: [
        (255, 120, 240),
        (120, 255, 230),
        (180, 180, 220),
        (255, 70, 120),
    ],
    particle: [(255, 120, 240), (120, 255, 230), (180, 180, 220)],
};

pub const MONO: Theme = Theme {
    slug: "mono",
    name: "Mono",
    lane_colors: [
        (210, 210, 210),
        (170, 170, 170),
        (240, 240, 240),
        (170, 170, 170),
        (210, 210, 210),
    ],
    combo_heat: (230, 230, 230),
    judgement: [
        (255, 255, 255),
        (200, 200, 200),
        (140, 140, 140),
        (90, 90, 90),
    ],
    particle: [(255, 255, 255), (200, 200, 200), (150, 150, 150)],
};

pub const SUNSET: Theme = Theme {
    slug: "sunset",
    name: "Sunset",
    lane_colors: [
        (230, 70, 90),
        (250, 130, 60),
        (250, 200, 90),
        (230, 100, 150),
        (180, 60, 110),
    ],
    combo_heat: (255, 140, 60),
    judgement: [
        (255, 220, 130),
        (255, 170, 120),
        (200, 150, 130),
        (220, 80, 90),
    ],
    particle: [(255, 220, 130), (255, 170, 120), (220, 150, 130)],
};

pub const OCEAN: Theme = Theme {
    slug: "ocean",
    name: "Ocean",
    lane_colors: [
        (70, 200, 210),
        (60, 150, 220),
        (90, 230, 180),
        (60, 100, 200),
        (130, 90, 220),
    ],
    combo_heat: (120, 220, 220),
    judgement: [
        (180, 240, 255),
        (130, 220, 200),
        (150, 170, 200),
        (220, 110, 140),
    ],
    particle: [(180, 240, 255), (130, 220, 200), (170, 180, 210)],
};

pub const BUILTINS: [Theme; 5] = [CLASSIC, NEON, MONO, SUNSET, OCEAN];

/// Global registry: built-ins come first, user themes from
/// `~/.cascade/themes/*.toml` get appended once at startup. Before
/// `init_registry` is called, `all()` falls back to `BUILTINS`.
static REGISTRY: OnceLock<Vec<Theme>> = OnceLock::new();

static ACTIVE: RwLock<Theme> = RwLock::new(CLASSIC);

/// Install the full theme list. Call this exactly once, at session startup.
/// Subsequent calls are ignored (OnceLock semantics).
pub fn init_registry(user_themes: Vec<Theme>) {
    let mut all = BUILTINS.to_vec();
    all.extend(user_themes);
    let _ = REGISTRY.set(all);
}

/// Snapshot of the full theme list (built-ins + user). Returns built-ins if
/// `init_registry` hasn't been called (e.g. inside tests).
pub fn all() -> &'static [Theme] {
    REGISTRY.get().map(|v| v.as_slice()).unwrap_or(&BUILTINS)
}

pub fn by_slug(slug: &str) -> Option<Theme> {
    all()
        .iter()
        .find(|t| t.slug.eq_ignore_ascii_case(slug))
        .copied()
}

/// Grab a copy of the currently active theme.
pub fn active() -> Theme {
    *ACTIVE.read().expect("theme lock poisoned")
}

/// Replace the active theme. Affects all subsequent renders.
pub fn set_active(theme: Theme) {
    *ACTIVE.write().expect("theme lock poisoned") = theme;
}

/// Resolve a slug to a theme, falling back to `CLASSIC` on unknown slugs.
pub fn resolve_or_default(slug: &str) -> Theme {
    by_slug(slug).unwrap_or(CLASSIC)
}

/// Cycle to the next theme in the registry, wrapping around.
pub fn cycle_next(current_slug: &str) -> Theme {
    let list = all();
    let idx = list
        .iter()
        .position(|t| t.slug.eq_ignore_ascii_case(current_slug))
        .unwrap_or(0);
    list[(idx + 1) % list.len()]
}

/// Cycle to the previous theme in the registry, wrapping around.
pub fn cycle_prev(current_slug: &str) -> Theme {
    let list = all();
    let idx = list
        .iter()
        .position(|t| t.slug.eq_ignore_ascii_case(current_slug))
        .unwrap_or(0);
    list[(idx + list.len() - 1) % list.len()]
}

// ---------------------------------------------------------------------------
// User-defined themes loaded from TOML files.
// ---------------------------------------------------------------------------

/// The on-disk format a user theme file must match. All color fields are
/// `[r, g, b]` arrays in 0..=255. See `README.md → Custom themes`.
#[derive(Debug, Deserialize)]
pub struct ThemeFile {
    pub slug: String,
    pub name: String,
    pub lane_colors: Vec<[u8; 3]>,
    pub combo_heat: [u8; 3],
    pub judgement: Vec<[u8; 3]>,
    pub particle: Vec<[u8; 3]>,
}

/// One problem encountered while loading a theme file. Returned from
/// [`load_themes_from`] so callers can surface friendly diagnostics.
#[derive(Debug, Clone)]
pub struct ThemeLoadIssue {
    pub path: String,
    pub reason: String,
}

impl ThemeFile {
    /// Validate shape (array lengths, non-empty slug/name) and convert into a
    /// runtime `Theme`. Strings are leaked into `'static` so the resulting
    /// theme can live in the global registry for the program lifetime.
    pub fn into_theme(self) -> Result<Theme, String> {
        if self.slug.trim().is_empty() {
            return Err("slug is empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("name is empty".to_string());
        }
        if self.lane_colors.len() != 5 {
            return Err(format!(
                "lane_colors must have 5 entries, found {}",
                self.lane_colors.len()
            ));
        }
        if self.judgement.len() != 4 {
            return Err(format!(
                "judgement must have 4 entries, found {}",
                self.judgement.len()
            ));
        }
        if self.particle.len() != 3 {
            return Err(format!(
                "particle must have 3 entries, found {}",
                self.particle.len()
            ));
        }

        let lane_colors: [Rgb; 5] = [
            rgb(self.lane_colors[0]),
            rgb(self.lane_colors[1]),
            rgb(self.lane_colors[2]),
            rgb(self.lane_colors[3]),
            rgb(self.lane_colors[4]),
        ];
        let judgement: [Rgb; 4] = [
            rgb(self.judgement[0]),
            rgb(self.judgement[1]),
            rgb(self.judgement[2]),
            rgb(self.judgement[3]),
        ];
        let particle: [Rgb; 3] = [
            rgb(self.particle[0]),
            rgb(self.particle[1]),
            rgb(self.particle[2]),
        ];

        Ok(Theme {
            slug: Box::leak(self.slug.to_lowercase().into_boxed_str()),
            name: Box::leak(self.name.into_boxed_str()),
            lane_colors,
            combo_heat: rgb(self.combo_heat),
            judgement,
            particle,
        })
    }
}

fn rgb(c: [u8; 3]) -> Rgb {
    (c[0], c[1], c[2])
}

/// Scan a directory for `*.toml` theme files. Returns the valid themes and a
/// list of issues for files that couldn't be loaded (bad TOML, wrong palette
/// shape, slug clashing with a built-in, or duplicate slug).
///
/// Non-existent or unreadable directories are treated as "no user themes"
/// — the empty case is normal, not an error.
pub fn load_themes_from(dir: &Path) -> (Vec<Theme>, Vec<ThemeLoadIssue>) {
    let mut themes = Vec::new();
    let mut issues = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return (themes, issues);
    };

    let builtin_slugs: std::collections::HashSet<&str> = BUILTINS.iter().map(|t| t.slug).collect();
    let mut seen_user_slugs: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
        .collect();
    // Deterministic order so `cascade themes` output is stable across runs.
    paths.sort();

    for path in paths {
        let issue = |reason: String| ThemeLoadIssue {
            path: path.display().to_string(),
            reason,
        };
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                issues.push(issue(format!("read error: {}", e)));
                continue;
            }
        };
        let file: ThemeFile = match toml::from_str(&raw) {
            Ok(f) => f,
            Err(e) => {
                issues.push(issue(format!("parse error: {}", e)));
                continue;
            }
        };
        let theme = match file.into_theme() {
            Ok(t) => t,
            Err(e) => {
                issues.push(issue(e));
                continue;
            }
        };
        if builtin_slugs.contains(theme.slug) {
            issues.push(issue(format!(
                "slug '{}' conflicts with a built-in theme",
                theme.slug
            )));
            continue;
        }
        if !seen_user_slugs.insert(theme.slug.to_string()) {
            issues.push(issue(format!("duplicate slug '{}'", theme.slug)));
            continue;
        }
        themes.push(theme);
    }

    (themes, issues)
}
