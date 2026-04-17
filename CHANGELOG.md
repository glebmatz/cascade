# Changelog

All notable changes to Cascade are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] — 2026-04-17

### Added

- **Gameplay modifiers** — Hidden, Flashlight, Sudden Death, Perfect Only.
  Toggle in song-select with `m`, or pass via CLI: `--mods hd,fl,sd,po`.
  Each (difficulty + mod combo) gets its own best-score slot.
- **Achievements** — 12 unlockable achievements covering combos, grades,
  full-combos, and mod completions. Persisted to `~/.cascade/achievements.json`,
  shown as a splash on the Results screen, and listable via
  `cascade achievements`.
- **`SS` grade** for 100% accuracy runs. Big ASCII grade letter now renders
  multi-character ranks (so `SS` displays as two stacked S glyphs).
- **`cascade song <slug>`** CLI command — detailed view of one song:
  metadata, note counts per difficulty, all best scores including
  per-modifier records.
- **Active mods badge** in song-select and on Results screen.

### Changed

- `cascade list` stays compact; deeper per-song stats moved to the new
  `cascade song <slug>` command.

## [0.2.0] — 2026-04-17

### Added

- **ID3 / Vorbis tag parsing on import** — title and artist are now read
  from the audio file's embedded metadata (works for MP3, FLAC, OGG, M4A,
  Opus, WebM via `symphonia`). Falls back to filename if no tags are present.
- **In-game rename**: press `r` on a song in the song-select screen to edit
  its title and artist. `Tab` switches between fields, `Enter` saves.
- **Sort modes**: press `s` in song-select to cycle through `Title`,
  `Artist`, `Recently added`, and `BPM`.
- **`cascade rename <slug> [--title NAME] [--artist NAME]`** CLI command.
- **Smart `cascade regen`**: when an existing song's metadata looks like a
  default (title equals the filename stem and artist is empty), regen now
  re-reads embedded tags and back-fills the metadata file. Manual renames
  are preserved.

### Fixed

- **Release workflow**: Homebrew tap update no longer fails when the tap
  repo doesn't yet contain a `Formula/` directory.
- **Release workflow**: dropped the unreliable `aarch64-unknown-linux-gnu`
  prebuilt target. ARM Linux users can install via `cargo install
  cascade-rhythm`.
- **CI**: pinned Rust toolchain to 1.95.0 via `rust-toolchain.toml` so new
  clippy lints don't break builds unexpectedly.

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

[Unreleased]: https://github.com/glebmatz/cascade/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/glebmatz/cascade/releases/tag/v0.3.0
[0.2.0]: https://github.com/glebmatz/cascade/releases/tag/v0.2.0
[0.1.0]: https://github.com/glebmatz/cascade/releases/tag/v0.1.0
