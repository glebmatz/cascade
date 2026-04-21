use crate::game::state::GameState;
use ratatui::prelude::*;
use ratatui::widgets::Widget;

pub struct HudTop<'a> {
    pub state: &'a GameState,
    pub health_enabled: bool,
    pub practice_label: Option<&'a str>,
}

impl<'a> Widget for HudTop<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 {
            return;
        }

        let cx = area.x + area.width / 2;

        // Health bar: left side (only if enabled)
        if self.health_enabled {
            let bar_width = 12u16.min(area.width / 4);
            let bar_x = area.x + 1;
            let filled = (self.state.health * bar_width as f64) as u16;
            let hp_color = if self.state.health > 0.5 {
                Color::Rgb(80, 200, 80)
            } else if self.state.health >= 0.25 {
                Color::Rgb(200, 200, 60)
            } else {
                Color::Rgb(200, 60, 60)
            };
            buf.set_string(
                bar_x,
                area.y,
                "[",
                Style::default().fg(Color::Rgb(100, 100, 100)),
            );
            for i in 0..bar_width {
                let style = if i < filled {
                    Style::default().bg(hp_color)
                } else {
                    Style::default().bg(Color::Rgb(30, 30, 30))
                };
                buf.set_string(bar_x + 1 + i, area.y, " ", style);
            }
            buf.set_string(
                bar_x + 1 + bar_width,
                area.y,
                "]",
                Style::default().fg(Color::Rgb(100, 100, 100)),
            );
        }

        if self.state.combo > 1 {
            let combo_text = format!("x{} COMBO", self.state.combo);
            let combo_w = combo_text.chars().count() as u16;
            let combo_x = cx.saturating_sub(combo_w + 8);
            buf.set_string(
                combo_x,
                area.y,
                &combo_text,
                Style::default().fg(Color::Rgb(180, 180, 180)),
            );
        }

        let score_text = format!("SCORE {}", self.state.score);
        let score_x = cx + 4;
        buf.set_string(
            score_x,
            area.y,
            &score_text,
            Style::default().fg(Color::Rgb(180, 180, 180)),
        );

        // Practice badge: rendered to the right of SCORE when terminal is wide
        // enough. Amber, bold. Hidden on narrow layouts to keep SCORE readable.
        if let Some(label) = self.practice_label {
            let badge = format!("PRACTICE {}", label);
            let bw = badge.chars().count() as u16;
            let badge_x = score_x + score_text.len() as u16 + 3;
            if badge_x + bw < area.x + area.width {
                buf.set_string(
                    badge_x,
                    area.y,
                    &badge,
                    Style::default().fg(Color::Rgb(255, 180, 80)).bold(),
                );
            }
        }
    }
}

pub struct HudBottom<'a> {
    pub state: &'a GameState,
    pub song_title: &'a str,
    pub progress: f64,
    pub difficulty: &'a str,
    pub total_notes: u32,
    /// Pre-downsampled peak amplitudes in `[0..=1]`. Empty disables the
    /// waveform and falls back to a plain dashed progress bar.
    pub waveform: &'a [f32],
}

impl<'a> Widget for HudBottom<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 {
            return;
        }

        let info = format!(" {} ", self.song_title);
        let truncated: String = info.chars().take((area.width / 2) as usize).collect();
        buf.set_string(
            area.x,
            area.y,
            &truncated,
            Style::default().fg(Color::Rgb(60, 60, 60)),
        );

        let note_counter = format!("{}/{}", self.state.total_notes, self.total_notes);
        let nc_x = area.x + area.width / 2 - note_counter.len() as u16 - 2;
        buf.set_string(
            nc_x,
            area.y,
            &note_counter,
            Style::default().fg(Color::Rgb(80, 80, 80)),
        );

        let acc = format!("ACC {:.1}%  {}", self.state.accuracy(), self.difficulty);
        let acc_x = area.x + area.width / 2;
        buf.set_string(
            acc_x,
            area.y,
            &acc,
            Style::default().fg(Color::Rgb(80, 80, 80)),
        );

        // Progress / waveform strip at the right edge. A wider terminal gets
        // a waveform of distinct peaks; narrow or unavailable data falls back
        // to a simple dashed progress bar.
        let bar_width = if self.waveform.is_empty() {
            10u16.min(area.width / 6)
        } else {
            // Eight-level vertical resolution via block glyphs — no blending,
            // so peaks stay visually distinct and don't smear into a rectangle.
            28u16.min(area.width / 3).max(12)
        };
        let bar_x = area.x + area.width.saturating_sub(bar_width + 1);
        let filled = (self.progress * bar_width as f64) as u16;

        if self.waveform.is_empty() {
            for i in 0..bar_width {
                let (ch, style) = if i < filled {
                    ("-", Style::default().fg(Color::Rgb(100, 100, 100)))
                } else {
                    ("-", Style::default().fg(Color::Rgb(30, 30, 30)))
                };
                buf.set_string(bar_x + i, area.y, ch, style);
            }
            return;
        }

        // Waveform peaks. Each column resamples the full song waveform into
        // one of 8 vertical block levels; the past portion lights up while
        // the upcoming portion stays dim. Subtle but readable — one glyph
        // per column guarantees visible detail instead of a smear.
        const LEVELS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
        let wave_len = self.waveform.len() as f32;
        for i in 0..bar_width {
            let t = i as f32 / bar_width.max(1) as f32;
            let src_idx = (t * wave_len) as usize;
            // Pick the MAX over the source window so peaks survive downsampling
            // at render-time. Otherwise many columns average to a flat line.
            let end_idx = (((i + 1) as f32 / bar_width.max(1) as f32) * wave_len) as usize;
            let mut amp: f32 = 0.0;
            let hi = end_idx.max(src_idx + 1).min(self.waveform.len());
            for &v in &self.waveform[src_idx.min(self.waveform.len() - 1)..hi] {
                if v > amp {
                    amp = v;
                }
            }
            // Perceptual compression so mid-volume passages look lively.
            let level_f = amp.powf(0.55) * 8.0;
            let level = (level_f as usize).min(8);
            let glyph = LEVELS[level];
            let color = if i < filled {
                Color::Rgb(150, 180, 200)
            } else {
                Color::Rgb(55, 55, 65)
            };
            buf.set_string(bar_x + i, area.y, glyph, Style::default().fg(color));
        }
    }
}
