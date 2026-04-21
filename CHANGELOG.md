# Changelog

All notable changes to Cascade are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] — 2026-04-21

### Added

- **Slide notes** — a hold can now end on a different lane than it started.
  The trail curves from source to target via smoothstep with a target-lane
  tip marker; pressing `slide_to` while the source is still held counts as
  Perfect, releasing the source early during the transition is forgiven.
  Auto-assigned on long holds: 25% on Hard, 45% on Expert. Existing beatmap
  JSON is backward-compatible (`slide_to` defaults to `None`).
- **Drain mode** — new Settings toggle. Health bleeds continuously at
  5%/s; only Perfects meaningfully restore it (+3.5%), Greats barely
  (+0.5%), Goods actively hurt (−1%). Automatically disabled in practice.
- **Chromatic aberration** post-process on Perfect hits — a 6-frame R/B
  channel split over the highway that punctuates clean timing without
  distracting from the notes.
- **Smarter note placement** — onsets are now classified as melodic vs
  percussive. Lead notes on melodic onsets follow the spectral-centroid
  contour so rising melodies drift right, falling drift left, while
  drums keep the existing band-to-lane mapping.
- **Batch import** — `cascade add <dir>` now recurses through a folder
  and imports every supported audio file. Per-file failures are reported
  inline and don't abort the run.
- **Hold-release emulation** on terminals without the kitty keyboard
  protocol. Uses OS key-repeat events plus a 550 ms initial grace and
  120 ms repeat grace to synthesize a release; on macOS Terminal.app
  where OS repeats arrive as plain Press events, a same-lane press
  within 80 ms while a hold is active is treated as a refresh rather
  than a new tap.

### Changed

- **Bottom HUD progress bar** is now a waveform preview: peak-amplitude
  glyphs (`▁▂▃▄▅▆▇█`) at one column each, past portion lit and upcoming
  dim, so the whole song shape is visible at a glance. Falls back to the
  old dashed bar on very narrow terminals.
- **`Note` type** gained a `slide_to: Option<u8>` field. Serialization is
  `skip_serializing_if = "Option::is_none"` so old beatmaps stay pristine
  on disk when regenerated.

## [0.6.1] — 2026-04-20

### Added

- **Custom themes** — drop `*.toml` files into `~/.cascade/themes/` and they
  show up in the Settings cycle next to the five built-ins. Slug collisions
  with built-ins are ignored; duplicate user slugs are skipped with a
  reported issue. See `README.md → Custom themes` for the file format.
- **`cascade themes`** CLI command — lists built-in + user themes and
  reports per-file validation issues (bad TOML, wrong palette shape, slug
  conflicts, duplicates).

## [0.6.0] — 2026-04-20

### Added

- **Themes** — five built-in palettes (`Classic`, `Neon`, `Mono`, `Sunset`,
  `Ocean`) that recolor lane backgrounds, note/hold trails, hit-zone bursts,
  judgement splashes and hit particles. Pick one in
  `Settings → Theme` with <kbd>D</kbd>/<kbd>F</kbd> (prev) and
  <kbd>J</kbd>/<kbd>K</kbd> (next), or <kbd>Enter</kbd> to advance; changes
  apply instantly without restart and persist to
  `~/.cascade/config.toml → display.theme`.
- **Palette preview** on the Settings screen — while the Theme row is
  focused, a five-block preview of the current palette renders below it.

## [0.5.0] — 2026-04-18

### Added

- **Stats dashboard** — a new `Stats` entry in the main menu and a matching
  `cascade stats` CLI command. Shows total plays, total time played, total
  notes hit, top-5 most-played songs, per-difficulty breakdown (plays / best
  accuracy / best score / average accuracy), a 30-day accuracy sparkline,
  a 30-day activity heatmap, and achievement unlock progress.
- **Play history** persistence. Every non-practice run is appended to
  `~/.cascade/play_history.json` with score, accuracy, combo, judgement
  counts, duration played, and a `died` flag. Practice runs are still
  excluded — they don't count for scores, achievements, or stats.

### Fixed

- **Main menu centering** — menu items now share a single left edge defined
  by the widest item, so longer entries (like `Settings`) no longer push
  shorter ones off-center.

## [0.4.1] — 2026-04-17

### Fixed

- Formatting pass with `cargo fmt` so the CI format check is green.

## [0.4.0] — 2026-04-17

### Added

- **Practice mode** — loop a section of any song at any speed to drill the
  parts you keep bailing on. Accessible from the CLI
  (`cascade play <slug> --section 1:30-2:00 --speed 0.7`) or from song
  select with <kbd>p</kbd>. Speed ranges 0.25× – 2.0× in 0.05 steps.
  While practising, modifiers are disabled, no score or achievement is
  recorded, and the song never auto-finishes — exit with <kbd>Esc</kbd> then
  <kbd>Q</kbd>. A practice badge is visible both in song select and in the
  top HUD during the run.
- **Practice overlay** in song select: press <kbd>p</kbd> to dial in section
  start/end and speed. <kbd>Tab</kbd> cycles fields; <kbd>←</kbd>/<kbd>→</kbd>
  nudge the focused field (±1 s for section times, ±0.05 for speed); digits
  type `MM:SS` directly; <kbd>C</kbd> clears.
- **`AudioPlayer::seek_to_ms` / `set_speed`** — thin wrappers over `rodio`'s
  `Sink::try_seek` and `set_speed`, used by practice-mode looping.

### Internal

- Gameplay time is now consistently expressed in *track-time* milliseconds
  via a new `position_ms_in_track()` helper. A lint test guards against raw
  uses of `self.audio.position_ms()` that would silently skip the practice
  speed multiplier.

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

[Unreleased]: https://github.com/glebmatz/cascade/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/glebmatz/cascade/releases/tag/v0.7.0
[0.6.1]: https://github.com/glebmatz/cascade/releases/tag/v0.6.1
[0.6.0]: https://github.com/glebmatz/cascade/releases/tag/v0.6.0
[0.5.0]: https://github.com/glebmatz/cascade/releases/tag/v0.5.0
[0.4.1]: https://github.com/glebmatz/cascade/releases/tag/v0.4.1
[0.4.0]: https://github.com/glebmatz/cascade/releases/tag/v0.4.0
[0.3.0]: https://github.com/glebmatz/cascade/releases/tag/v0.3.0
[0.2.0]: https://github.com/glebmatz/cascade/releases/tag/v0.2.0
[0.1.0]: https://github.com/glebmatz/cascade/releases/tag/v0.1.0
