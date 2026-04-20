<div align="center">

```
  ____                        _
 / ___|__ _ ___  ___ __ _  __| | ___
| |   / _` / __|/ __/ _` |/ _` |/ _ \
| |__| (_| \__ \ (_| (_| | (_| |  __/
 \____\__,_|___/\___\__,_|\__,_|\___|
```

### A terminal rhythm game that plays any song you throw at it.

[![CI](https://github.com/glebmatz/cascade/actions/workflows/ci.yml/badge.svg)](https://github.com/glebmatz/cascade/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/cascade-rhythm.svg)](https://crates.io/crates/cascade-rhythm)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Downloads](https://img.shields.io/github/downloads/glebmatz/cascade/total.svg)](https://github.com/glebmatz/cascade/releases)

</div>

<p align="center">
  <a href="https://github.com/glebmatz/cascade">
    <img src="docs/cascade-demo.gif" alt="Cascade gameplay demo" width="820">
  </a>
  <br>
  <sub><i>Drop in any audio file — Cascade analyses it, generates a beatmap, and you start playing.</i></sub>
</p>

<p align="center">
  <a href="#install"><strong>↓ Install</strong></a>
  &nbsp;·&nbsp;
  <a href="#quick-start">Quick start</a>
  &nbsp;·&nbsp;
  <a href="#controls">Controls</a>
  &nbsp;·&nbsp;
  <a href="#modifiers">Modifiers</a>
  &nbsp;·&nbsp;
  <a href="#practice-mode">Practice</a>
  &nbsp;·&nbsp;
  <a href="#how-the-beatmap-generator-works">How it works</a>
  &nbsp;·&nbsp;
  <a href="#contributing">Contribute</a>
</p>

---

Cascade is a five-lane rhythm game that runs entirely in your terminal. Drop
in any `.mp3` / `.flac` / `.ogg` / `.wav` and it will analyse the audio,
detect onsets and beat, and generate beatmaps for four difficulties — all
offline, with no server round-trip.

Because it's built on [ratatui][ratatui] and `rodio`, it works in any
truecolor-capable terminal; modern terminals that support the kitty keyboard
protocol (Kitty, WezTerm, foot, Alacritty 0.13+) additionally unlock proper
hold-note release detection.

## Features

- **Works with your library** — import any audio file, no curated song pack required; ID3/Vorbis tags are read automatically
- **Smart beatmaps** — spectral-flux onset detection, autocorrelated BPM, downbeat phase alignment, per-difficulty density
- **Chords + holds** — up to 3-note chords and sustained holds, tuned per difficulty
- **Rich visuals** — half-block rendering, particle physics, starfield background, vignette, live spectrum bars, beat-synced receptor glow
- **Synthesized hit feedback** — every judgement has its own click; menu navigation has its own sound
- **Modifiers** — Hidden, Flashlight, Sudden Death, Perfect Only. Each combo gets its own best-score slot
- **Practice mode** — loop any section at any speed from 0.25× to 2.0×, perfect for drilling that one run you keep bailing on
- **Achievements** — 12 unlockables for combos, full clears, mod runs
- **Stats dashboard** — aggregate play history with a 30-day accuracy sparkline and activity heatmap, both in the UI and via `cascade stats`
- **Themes** — five built-in palettes (Classic / Neon / Mono / Sunset / Ocean); switch instantly from Settings
- **Best scores** — persisted per song / difficulty / mod combo
- **Calibration** — built-in metronome calibrator removes audio latency drift
- **Terminal-native** — no Electron, no GPU, ~10 MB binary

## Install

### Homebrew *(macOS / Linux)*

```sh
brew install glebmatz/cascade/cascade
```

### One-line install *(Linux / macOS, any shell)*

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/glebmatz/cascade/releases/latest/download/cascade-installer.sh | sh
```

### PowerShell *(Windows)*

```powershell
irm https://github.com/glebmatz/cascade/releases/latest/download/cascade-installer.ps1 | iex
```

### Cargo

```sh
# Full compile from source
cargo install cascade-rhythm

# Prebuilt binary via cargo-binstall (much faster)
cargo binstall cascade-rhythm
```

### From source

```sh
git clone https://github.com/glebmatz/cascade
cd cascade
cargo build --release
./target/release/cascade
```

## Quick start

```sh
# 1. Import a song. ID3 tags are read automatically; beatmaps for all
#    4 difficulties are generated.
cascade add ~/Music/my-favorite-song.mp3

# 2. Check what's imported (with best scores per difficulty).
cascade list

# 3. Launch straight into gameplay.
cascade play my-favorite-song --hard

# 4. Push yourself with modifiers. Each (difficulty + mod combo) gets
#    its own best-score slot.
cascade play my-favorite-song --hard --mods hd,sd

# Fix a typo in tags later:
cascade rename my-favorite-song --title "Numb" --artist "Linkin Park"

# Detailed stats for one song (all difficulties + mod combos):
cascade song my-favorite-song

# Track unlocks:
cascade achievements

# Aggregate play stats (top songs, 30-day accuracy sparkline, activity heatmap):
cascade stats

# …or just run `cascade` for the full interactive UI.
cascade
```

### First-time setup: calibrate audio latency

The first thing you should do on a new machine is calibrate. Audio output
has 20–80 ms of platform-dependent latency and the game needs to know how
much to compensate.

```
Main menu → Settings → Calibrate Audio
```

Tap <kbd>Space</kbd> on the beat for ~16 beats. The game takes the IQR-trimmed
median of your timing errors and stores it as `offset_ms`. Takes 15 seconds
once, and every hit after that will feel honest.

## Controls

### Gameplay

| Key | Action |
|-----|--------|
| <kbd>D</kbd> <kbd>F</kbd> <kbd>Space</kbd> <kbd>J</kbd> <kbd>K</kbd> | Lanes 1 – 5 (hold for hold notes) |
| <kbd>Esc</kbd> | Pause |
| <kbd>Q</kbd> | Quit to song select (while paused) |

### Menus

| Key | Action |
|-----|--------|
| <kbd>↑</kbd> / <kbd>↓</kbd> or <kbd>K</kbd> / <kbd>J</kbd> | Move cursor |
| <kbd>Enter</kbd> | Confirm |
| <kbd>Esc</kbd> | Back |
| <kbd>Tab</kbd> | Cycle difficulty (song select) |
| <kbd>s</kbd> | Cycle sort: Title / Artist / Recently added / BPM (song select) |
| <kbd>m</kbd> | Toggle modifiers overlay: Hidden / Flashlight / Sudden Death / Perfect Only |
| <kbd>p</kbd> | Toggle practice overlay: loop a section, adjust speed |
| <kbd>/</kbd> | Search (song select) |
| <kbd>r</kbd> | Rename selected song (song select) |
| <kbd>i</kbd> | Import audio file (song select) |
| <kbd>x</kbd> | Delete song (song select) |

## Terminal compatibility

Cascade degrades gracefully across terminals. The table below notes the
minimum level you need for each feature.

| Terminal | Truecolor | Kitty kbd protocol | Hold notes |
|----------|-----------|--------------------|------------|
| Kitty    | ✅        | ✅                 | ✅         |
| WezTerm  | ✅        | ✅                 | ✅         |
| foot     | ✅        | ✅                 | ✅         |
| Alacritty 0.13+ | ✅ | ✅                 | ✅         |
| iTerm2   | ✅        | ❌                 | ⚠️ tap-only |
| Terminal.app | ✅ (Sonoma+) | ❌          | ⚠️ tap-only |
| Windows Terminal | ✅ | 🔄 (partial)       | ⚠️ tap-only |
| tmux     | passthrough | passthrough     | depends on host |

If hold notes are important to you and you're on macOS, Kitty and WezTerm
are both free and excellent.

## Configuration

Config lives at `~/.cascade/config.toml` — edit directly, or use the
in-game Settings screen.

```toml
[gameplay]
scroll_speed = 1.0        # 0.5 – 2.0
difficulty = "hard"
health_enabled = true

[keys]
lanes = ["d", "f", " ", "j", "k"]

[audio]
volume = 0.8
offset_ms = 0             # set by calibrator

[display]
fps = 60
theme = "classic"         # classic / neon / mono / sunset / ocean
```

Imported songs + generated beatmaps + best scores + play history live under
`~/.cascade/songs/`, `~/.cascade/scores.json`, `~/.cascade/achievements.json`,
and `~/.cascade/play_history.json`.

## Modifiers

Press <kbd>m</kbd> in song select to toggle modifiers, or pass `--mods` on the
CLI. Each `(difficulty + mods)` combination gets its own best-score slot, so a
clean Hard run and a Hard + Hidden + Sudden Death run are tracked separately.

| Code | Name | Effect |
|------|------|--------|
| `hd` | Hidden | Notes appear only close to the hit zone. |
| `fl` | Flashlight | Highway is dark except a narrow band around the hit zone. |
| `sd` | Sudden Death | The first miss ends the run immediately. |
| `po` | Perfect Only | Anything below Perfect counts as a Miss. |

```sh
cascade play my-favorite-song --hard --mods hd,fl
```

## Practice mode

Stuck on one part of a song? Practice mode loops a section of your choice
at a speed of your choice. Score and achievements are **not** saved while
practising — it's for learning, not for scoreboards.

### From the CLI

```sh
# Loop 1:30 → 2:00 at 70% speed until it clicks.
cascade play my-favorite-song --section 1:30-2:00 --speed 0.7

# Full song, slowed down.
cascade play my-favorite-song --speed 0.5

# Any difficulty still applies.
cascade play my-favorite-song --hard --section 1:30-2:00
```

### From the UI

In song select, press <kbd>p</kbd> to open the practice overlay. <kbd>Tab</kbd>
cycles fields, <kbd>←</kbd>/<kbd>→</kbd> nudges speed by 0.05, <kbd>C</kbd>
clears, <kbd>Enter</kbd> applies, <kbd>Esc</kbd> closes. The practice badge
stays visible in song select so you don't forget it's on, and mirrors in the
top HUD during gameplay.

Practice mode silently disables modifiers for the run — by design,
distractions are not what you want while learning a pattern.

### Known limits

- Slowing down lowers the pitch (`rodio` doesn't time-stretch).
- The loop seek produces a tiny (~50 ms) audible gap — fine for learning,
  not designed for perfect-timing play.

## Achievements

Twelve unlockable achievements ship with the game — full combos, grade
milestones, and clearing songs with each modifier. The unlock list lives at
`~/.cascade/achievements.json` and you can see your progress with:

```sh
cascade achievements
```

## Themes

Five palettes ship with the game and recolor lane backgrounds, notes, hit
bursts, judgement splashes, and hit particles.

| Slug | Vibe |
|---|---|
| `classic` | Default bright rainbow (red / green / yellow / blue / purple) |
| `neon` | Cyberpunk — hot pink, cyan, electric violet |
| `mono` | Grayscale, minimal, focused |
| `sunset` | Warm gradient of crimson / orange / amber / rose |
| `ocean` | Cool aqua / seafoam / deep blue / indigo |

Pick one in `Main menu → Settings → Theme`. <kbd>D</kbd>/<kbd>F</kbd> cycles
backward, <kbd>J</kbd>/<kbd>K</kbd> forward, <kbd>Enter</kbd> also advances.
Changes apply instantly — no restart — and persist to
`~/.cascade/config.toml` under `display.theme`.

### Custom themes

Drop a `.toml` file into `~/.cascade/themes/` and it shows up next to the
built-ins in the Settings cycle. Built-in slugs (`classic`, `neon`, `mono`,
`sunset`, `ocean`) are reserved — conflicting slugs are ignored. Each color
is an `[r, g, b]` triple in `0..=255`.

```toml
# ~/.cascade/themes/cherry.toml
slug = "cherry"
name = "Cherry Blossom"

# 5 lanes, in order: D / F / Space / J / K
lane_colors = [
  [250, 200, 210],
  [255, 170, 190],
  [240, 150, 180],
  [200, 120, 170],
  [160,  90, 150],
]

# Warm tint added to the highway as combo rises.
combo_heat = [255, 150, 180]

# Splash colors for Perfect / Great / Good / Miss.
judgement = [
  [255, 220, 230],
  [230, 180, 200],
  [180, 150, 170],
  [200,  80, 120],
]

# Hit-particle colors for Perfect / Great / Good.
particle = [
  [255, 220, 230],
  [230, 180, 200],
  [200, 160, 180],
]
```

Check that your files are valid with:

```sh
cascade themes
```

It lists built-ins + user themes and reports per-file issues (bad TOML,
wrong array length, slug conflicts, duplicates).

## Stats

Every non-practice run is logged to `~/.cascade/play_history.json`. Open the
`Stats` entry from the main menu, or run:

```sh
cascade stats
```

You get:

- **Totals** — lifetime plays, time played, notes hit.
- **Top 5 songs** — ranked by play count; title updates with renames.
- **Per-difficulty breakdown** — plays, best accuracy, best score, average
  accuracy.
- **30-day accuracy sparkline** — daily average accuracy for the last 30 days
  rendered as unicode block glyphs. Blanks are days with no plays.
- **30-day activity heatmap** — play count per day in five density levels
  (`· ░ ▒ ▓ █`).
- **Achievement progress** — unlocked out of total.

Practice runs are excluded — they are for learning, not for stats. The file
grows by about one kilobyte per 10 plays; a few thousand plays still loads
instantly.

## How the beatmap generator works

1. **Novelty**: short-time FFT with Hann window (2048 / 512 hop), per-band
   log-magnitude with running-max whitening, half-wave-rectified spectral
   flux summed across 8 logarithmic bands.
2. **Peak picking**: 95th-percentile normalization; local max within ±50 ms;
   must exceed `median + 1.5 × MAD` over a ±200 ms window.
3. **BPM**: autocorrelation of the novelty signal in the 60 – 200 BPM
   window, biased toward 120 BPM to avoid half/double confusion.
4. **Downbeat phase**: comb cross-correlation at the estimated BPM with ±1
   frame neighbourhood so the grid snaps to the nearest peak, not a hop
   boundary.
5. **Notes**: quantized to the phase-aligned grid, filtered per-difficulty
   by strength; lane chosen from the dominant band with repeat-avoidance
   hysteresis; per-peak secondary bands (≥50 % of top flux) can produce
   2- or 3-note chords on harder difficulties.
6. **Holds**: if the dominant band's whitened energy stays ≥75 % of its
   peak level for ≥1.5 beats, the note becomes a hold of that duration.

No labeled training data, no downloads, all deterministic.

## Contributing

Contributions are very welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for
the workflow, code style, and testing conventions.

Good places to start:

- Issues tagged [`good first issue`][gfi] and [`help wanted`][hw]
- New particle effects, judgement animations, menu screens
- Better onset detection (e.g. CNN-free drum transcription) — the
  generator is self-contained in `src/beatmap/generator.rs`
- Extra terminals validated and added to the compatibility table

## Roadmap

- [ ] ID3/Vorbis/MP4 tag parsing at import (proper title/artist)
- [ ] Online song sharing (upload beatmap JSON, not audio)
- [ ] Multiplayer via terminal-to-terminal
- [ ] Note editor for hand-tuning generated maps

See [CHANGELOG.md](CHANGELOG.md) for release history.

## License

Cascade is dual-licensed under either of

- Apache License, Version 2.0 — [LICENSE-APACHE](LICENSE-APACHE)
- MIT license — [LICENSE-MIT](LICENSE-MIT)

at your option.

### Contribution intent

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.

[ratatui]: https://github.com/ratatui-org/ratatui
[gfi]: https://github.com/glebmatz/cascade/issues?q=is%3Aopen+is%3Aissue+label%3A%22good+first+issue%22
[hw]: https://github.com/glebmatz/cascade/issues?q=is%3Aopen+is%3Aissue+label%3A%22help+wanted%22
