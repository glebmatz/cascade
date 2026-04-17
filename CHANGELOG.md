# Changelog

All notable changes to Cascade are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-04-17

First public release.

### Added

- **Five-lane rhythm gameplay** with D / F / Space / J / K by default.
- **Automatic beatmap generation** from any imported audio file:
  log-spectral flux onset detection, adaptive peak picking with local
  MAD threshold, BPM via autocorrelation, downbeat phase via comb
  cross-correlation, four difficulty levels (Easy / Medium / Hard /
  Expert) with per-difficulty note density, chord detection (up to
  3-note chords on Expert), and sustained-energy hold notes.
- **Audio calibration screen**: 16-beat metronome tap-along with
  IQR-trimmed median offset estimation.
- **Genre-standard hit windows** — Perfect ±35 ms, Great ±75 ms, Good
  ±120 ms, Miss > 160 ms.
- **Real audio position** via `sink.get_pos()` — no more wall-clock
  drift from audio buffer latency.
- **Kitty keyboard protocol** support for proper hold-note release
  detection in modern terminals; graceful fallback to tap-only where
  unsupported.
- **Half-block pixel rendering** for 2× vertical resolution and smooth
  note scrolling.
- **Rich gameplay visuals**: multi-row notes with gradient and glow
  halos, approach easing, receptor anticipation, lane burst streaks on
  hit, scrolling starfield background, vignette, per-frame particle
  physics, live spectrum bars in side margins, combo heat tint.
- **Synthesized sound effects**: Perfect / Great / Good / Miss clicks,
  milestone bell, menu navigation ticks — all generated in code, no
  audio assets required.
- **Combo milestones** at 25 / 50 / 100 / 200 / 300 / 500 / 750 / 1000
  with splash text and sound.
- **Menu polish**: ASCII logo with breathing brightness, starfield
  background, consistent top-bar breadcrumbs and bottom-bar keymap
  hints, difficulty pills in consistent colors.
- **Rich song select**: BPM / duration / note count / best score / four
  difficulty indicators per song; `/` to search.
- **Best scores** persisted per song / difficulty at
  `~/.cascade/scores.json`; "NEW BEST" overlay on the results screen.
- **Animated results screen**: big ASCII grade letter colored by grade,
  ease-out score count-up over 1.5 s, judgement distribution histogram.
- **Hold notes toggle** in Settings for terminals without key-release
  support.
- **CLI subcommands**: `cascade add <path>`, `cascade list`,
  `cascade play <slug> [--easy/--medium/--hard/--expert]`,
  `cascade regen`, `cascade help`.
- **Dual MIT / Apache-2.0 licensing**.

[Unreleased]: https://github.com/glebmatz/cascade/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/glebmatz/cascade/releases/tag/v0.1.0
