# VHS demo recording for Cascade. Run with `vhs docs/demo.tape`.
# Outputs docs/cascade-demo.gif and docs/cascade-demo.mp4.
#
# Requires:
#   brew install vhs ttyd ffmpeg
#   brew install --cask font-jetbrains-mono-nerd-font  (or use Menlo below)

Output docs/cascade-demo.gif
Output docs/cascade-demo.mp4

Set FontFamily "JetBrainsMono Nerd Font Mono"
Set FontSize 14
Set Width 1100
Set Height 680
Set Padding 20
Set Margin 16
Set MarginFill "#1e1e2e"
Set Theme "Catppuccin Mocha"
Set TypingSpeed 50ms
Set PlaybackSpeed 1.0
Set LoopOffset 5%

# --- Boot into main menu (breathing logo + starfield) ---
Type "cascade"
Enter
Sleep 2500ms

# --- Open song select ---
Enter
Sleep 1500ms

# --- Cycle difficulty pills ---
Tab
Sleep 450ms
Tab
Sleep 450ms
Tab
Sleep 700ms

# --- Launch gameplay ---
Enter
Sleep 1200ms

# --- Spam lane keys to trigger hit flashes, particles, combo animations.
#     Mix of all 5 lanes (D F Space J K). Some will hit, most will miss
#     because timing is random. That's fine — the visual chaos is the point.
Type@180ms "dfjkdfjkdfjk jdkfjdfk jdfkjdfk jdfkjdfk jdfkjd"

# --- Stop input → next ~6 sec of notes auto-miss → health drains → death
#     triggers automatic Results screen. With ~12 misses needed (-0.08 hp each)
#     and ~2.3 notes/sec on Medium, this takes ~5–6 seconds.
Sleep 6000ms

# --- Linger on the Results screen so the score count-up + histogram
#     + big ASCII grade letter are all visible. Recording ends here so
#     the gif's "final frame" is the score, not a shell prompt.
Sleep 6000ms
