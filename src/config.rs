use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub gameplay: GameplayConfig,
    pub keys: KeysConfig,
    pub audio: AudioConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameplayConfig {
    pub scroll_speed: f64,
    pub difficulty: String,
    #[serde(default = "default_true")]
    pub health_enabled: bool,
    #[serde(default = "default_true")]
    pub holds_enabled: bool,
    /// Drain mode: health continuously falls while playing. Only Perfects
    /// restore it. Implies `health_enabled`. Off by default.
    #[serde(default)]
    pub drain_mode: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysConfig {
    pub lanes: [char; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub volume: f64,
    pub offset_ms: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub fps: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "classic".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gameplay: GameplayConfig {
                scroll_speed: 1.0,
                difficulty: "hard".to_string(),
                health_enabled: true,
                holds_enabled: true,
                drain_mode: false,
            },
            keys: KeysConfig {
                lanes: ['d', 'f', ' ', 'j', 'k'],
            },
            audio: AudioConfig {
                volume: 0.8,
                offset_ms: 0,
            },
            display: DisplayConfig {
                fps: 60,
                theme: default_theme(),
            },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn cascade_dir() -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".cascade")
    }

    pub fn default_path() -> std::path::PathBuf {
        Self::cascade_dir().join("config.toml")
    }
}
