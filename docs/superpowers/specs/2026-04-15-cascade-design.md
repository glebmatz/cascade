# Cascade — Terminal Rhythm Game

Terminal-based rhythm game in the style of Guitar Hero / OSU!mania, written in Rust. Notes fall down a perspective highway synced to music. Import songs from YouTube, auto-generate beatmaps via audio analysis.

## Tech Stack

- **ratatui** + **crossterm** — TUI rendering and input
- **rodio** — audio playback
- **rustfft** — FFT for visualizer and onset detection
- **symphonia** — mp3/ogg/opus decoding to PCM
- **serde** + **serde_json** — beatmap and metadata serialization
- **toml** — config parsing
- **tokio** — async for yt-dlp subprocess
- **yt-dlp** — external binary for YouTube audio download

## Architecture: Game Loop

Fixed tick rate ~60 FPS game loop:

```
Input (crossterm) → Update (game state) → Render (ratatui)
```

Three threads:
- **Main thread** — game loop (input → update → render)
- **Audio thread** — rodio playback, exposes current position via `Arc<AtomicU64>` (milliseconds)
- **FFT thread** — reads audio samples from file, computes spectrum for visualizer, writes to lock-free buffer

Note synchronization: notes are bound to timestamps in the beatmap. Each frame: `current_audio_pos` is compared to each note's timestamp to determine Y-position on the highway.

## Gameplay: 5-Lane Highway

### Controls

5 lanes mapped to `D F Space J K`. Configurable in settings.

### Visual Style: Trapezoid Perspective

The highway is rendered as a trapezoid — narrow at top (vanishing point), wide at bottom (hit zone). Diagonal walls `╲` `╱` converge toward the top.

Notes grow as they approach:
- Far: `◇` (small, dim)
- Mid: `◈` (medium, brighter)
- Near: `◆` (large, bright)

### Visualizer

Two layers, both driven by real-time FFT data:
- **Top border**: wave visualization using `▁▂▃▅▇█` characters, amplitude from frequency bands
- **Side borders**: `░▒▓█` blocks pulsing with music energy

Style: retro-minimalist — few colors, clean lines, monochrome with subtle accents. Arcade spirit without visual clutter.

### HUD

- **Top**: combo counter left, lane labels center, score right
- **Bottom**: judgement feedback (PERFECT/GREAT/GOOD/MISS), accuracy %, song title, progress bar, difficulty label

## Screens and Navigation

```
Main Menu → Song Select → Gameplay → Results → Song Select
                ↑                       ↑
            Settings                  (retry)
```

### Main Menu
- ASCII art "CASCADE" logo
- Items: Play, Settings, Quit
- Background visualizer pulsing subtly

### Song Select
- Song list with arrow/jk navigation
- Difficulty selector: Easy / Medium / Hard / Expert
- Import button: paste YouTube URL or playlist URL
- Import progress bar: downloading → analyzing → generating → done

### Gameplay
- Trapezoid highway with falling notes
- ESC → pause overlay (resume / restart / quit)

### Results (post-game only)
- Final score
- Accuracy %
- Max combo
- Grade (S/A/B/C/D)
- Options: Retry / Back to songs

### Settings
- Scroll speed (0.5 — 2.0)
- Key bindings
- Audio offset (±200ms calibration)
- Volume

## Beatmap Format

JSON files, one per difficulty level:

```json
{
  "version": 1,
  "song": {
    "title": "Neon Dreams",
    "artist": "The Midnight",
    "audio_file": "audio.mp3",
    "bpm": 120,
    "duration_ms": 227000
  },
  "difficulty": "hard",
  "notes": [
    { "time_ms": 1200, "lane": 2 },
    { "time_ms": 1450, "lane": 0 },
    { "time_ms": 1450, "lane": 4 }
  ]
}
```

`lane`: 0..4 mapping to D F Space J K.

## Beatmap Auto-Generation Pipeline

```
YouTube URL
  → yt-dlp (download audio → mp3)
  → symphonia (decode → PCM samples)
  → BPM detection (autocorrelation)
  → Onset detection (spectral flux — peaks in spectral difference between adjacent frames)
  → Energy band separation (low/mid/high frequency)
  → Note placement (difficulty filter)
  → Lane assignment (frequency band → lane mapping)
  → 4x beatmap JSON files
```

### Difficulty Levels

| Difficulty | Onset threshold | Max simultaneous | Density |
|-----------|----------------|-----------------|---------|
| Easy | strong onsets only | 1 note | ~2 notes/sec |
| Medium | medium onsets | 2 notes | ~4 notes/sec |
| Hard | most onsets | 2-3 notes | ~6 notes/sec |
| Expert | all onsets | 3-4 notes | ~8+ notes/sec |

### Lane Assignment

Frequency bands determine lane placement:
- Low frequency → lanes 0, 1 (D, F)
- Mid frequency → lane 2 (Space)
- High frequency → lanes 3, 4 (J, K)

Randomization added for variety, with constraint: no extreme jumps (lane 0 → lane 4) in quick succession.

## Scoring and Hit Detection

### Hit Windows

| Judgement | Window | Base points | Combo effect |
|-----------|--------|------------|-------------|
| PERFECT | ±30ms | 300 | continues |
| GREAT | ±60ms | 200 | continues |
| GOOD | ±100ms | 100 | continues |
| MISS | >100ms or not pressed | 0 | resets to 0 |

### Score Calculation

```
note_score = base_points × (1 + combo / 50)
```

Maximum multiplier: x5 (at combo 200+).

### Grades

| Grade | Accuracy |
|-------|----------|
| S | ≥ 95% |
| A | ≥ 90% |
| B | ≥ 80% |
| C | ≥ 70% |
| D | < 70% |

Accuracy = earned points / maximum possible points × 100%.

### Audio Offset

Configurable ±200ms offset in settings to compensate for audio output latency. Applied to all hit window checks.

## File Structure

```
~/.cascade/
├── config.toml
└── songs/
    └── <song-slug>/
        ├── audio.mp3
        ├── metadata.json
        ├── easy.json
        ├── medium.json
        ├── hard.json
        └── expert.json
```

YouTube playlist import: each track gets its own subfolder. Progress shown in UI: "Importing 3/12..."

## Code Structure

```
src/
├── main.rs                  # entry point, terminal init, app loop
├── app.rs                   # App struct, state, screen routing
├── input.rs                 # key handling, action mapping
│
├── screens/
│   ├── menu.rs              # Main Menu
│   ├── song_select.rs       # Song Select + import UI
│   ├── gameplay.rs          # Gameplay orchestration
│   ├── results.rs           # Results screen
│   └── settings.rs          # Settings screen
│
├── audio/
│   ├── player.rs            # rodio playback, track position, volume
│   ├── analyzer.rs          # real-time FFT for visualizer
│   └── import.rs            # yt-dlp wrapper, download, conversion
│
├── beatmap/
│   ├── types.rs             # Beatmap, Note, Difficulty structs
│   ├── generator.rs         # onset detection, BPM, note placement
│   └── loader.rs            # JSON read/write
│
├── game/
│   ├── state.rs             # GameState: score, combo, accuracy, judgements
│   ├── hit_judge.rs         # hit window checks
│   └── highway.rs           # note positions, scroll logic
│
├── ui/
│   ├── highway_render.rs    # trapezoid perspective, note rendering
│   ├── visualizer.rs        # FFT → wave/block visualizer
│   ├── hud.rs               # combo, score, accuracy, feedback
│   └── widgets.rs           # shared components (lists, progress bar)
│
└── config.rs                # config.toml read/write
```

## MVP Scope

Everything above is MVP. Explicitly excluded:
- Persistent high score table / history across sessions
- Manual beatmap editor
- Import of .osu or other external beatmap formats
- Multiplayer
- Long hold notes (only tap notes in MVP)
