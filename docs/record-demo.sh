#!/usr/bin/env bash
# Record the README demo gif/mp4.
#
# Health is force-enabled for the recording so the player "dies" from misses
# and the Results screen appears automatically. Your config is restored
# afterwards.

set -euo pipefail

CONFIG="$HOME/.cascade/config.toml"
BACKUP="$HOME/.cascade/.config.toml.demo-backup"

cleanup() {
    if [[ -f "$BACKUP" ]]; then
        mv "$BACKUP" "$CONFIG"
        echo "Restored original config."
    fi
}
trap cleanup EXIT

if [[ ! -f "$CONFIG" ]]; then
    echo "No config at $CONFIG — run cascade once to create one." >&2
    exit 1
fi

cp "$CONFIG" "$BACKUP"
# Toggle health_enabled to true (works on both BSD and GNU sed).
sed -i.bak 's/^health_enabled = false/health_enabled = true/' "$CONFIG"
rm -f "${CONFIG}.bak"

echo "Recording demo..."
vhs docs/demo.tape

echo "Done. Outputs: docs/cascade-demo.gif, docs/cascade-demo.mp4"
