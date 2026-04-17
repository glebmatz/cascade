#!/bin/sh
# Universal installer for Cascade.
#
# Usage:
#   curl --proto '=https' --tlsv1.2 -LsSf \
#     https://github.com/glebmatz/cascade/releases/latest/download/cascade-installer.sh | sh
#
# Or, to install a specific version:
#   CASCADE_VERSION=v0.1.0 curl ... | sh
#
# Or to install into a custom directory:
#   CASCADE_INSTALL_DIR=$HOME/.local/bin curl ... | sh

set -eu

REPO="glebmatz/cascade"
VERSION="${CASCADE_VERSION:-latest}"
INSTALL_DIR="${CASCADE_INSTALL_DIR:-}"

msg()  { printf '%s\n' "$*" >&2; }
die()  { msg "error: $*"; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

detect_target() {
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        darwin)
            case "$arch" in
                arm64|aarch64) echo "aarch64-apple-darwin" ;;
                x86_64|amd64)  echo "x86_64-apple-darwin" ;;
                *) die "unsupported macOS architecture: $arch" ;;
            esac
            ;;
        linux)
            case "$arch" in
                x86_64|amd64)  echo "x86_64-unknown-linux-gnu" ;;
                arm64|aarch64) die "ARM Linux is not supported by prebuilt binaries yet. Install with: cargo install cascade-rhythm" ;;
                *) die "unsupported Linux architecture: $arch" ;;
            esac
            ;;
        *)
            die "unsupported OS: $os (use the PowerShell installer on Windows)"
            ;;
    esac
}

pick_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        echo "$INSTALL_DIR"
        return
    fi
    # Prefer a PATH-listed directory we can write to without sudo.
    for d in "$HOME/.local/bin" "$HOME/bin" "/usr/local/bin"; do
        case ":$PATH:" in
            *":$d:"*)
                if mkdir -p "$d" 2>/dev/null && [ -w "$d" ]; then
                    echo "$d"; return
                fi
                ;;
        esac
    done
    # Fallback: create ~/.local/bin and warn the user to add to PATH.
    mkdir -p "$HOME/.local/bin"
    echo "$HOME/.local/bin"
}

download() {
    url=$1
    out=$2
    if have curl; then
        curl --proto '=https' --tlsv1.2 -fLsS -o "$out" "$url"
    elif have wget; then
        wget -qO "$out" "$url"
    else
        die "neither curl nor wget is available"
    fi
}

main() {
    target=$(detect_target)
    msg "Detected platform: $target"

    if [ "$VERSION" = "latest" ]; then
        base="https://github.com/$REPO/releases/latest/download"
    else
        base="https://github.com/$REPO/releases/download/$VERSION"
    fi
    archive="cascade-$target.tar.gz"
    url="$base/$archive"

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    msg "Downloading $url"
    download "$url" "$tmp/$archive"
    msg "Verifying..."
    download "$url.sha256" "$tmp/$archive.sha256" || msg "(no sha256 file; skipping verify)"
    if [ -s "$tmp/$archive.sha256" ] && have shasum; then
        expected=$(awk '{print $1}' "$tmp/$archive.sha256")
        actual=$(shasum -a 256 "$tmp/$archive" | awk '{print $1}')
        [ "$expected" = "$actual" ] || die "sha256 mismatch"
    fi

    msg "Extracting..."
    tar -xzf "$tmp/$archive" -C "$tmp"
    bin_dir=$(pick_install_dir)
    install -m 0755 "$tmp/cascade-$target/cascade" "$bin_dir/cascade"

    msg ""
    msg "Installed cascade to $bin_dir/cascade"
    case ":$PATH:" in
        *":$bin_dir:"*) : ;;
        *)
            msg ""
            msg "WARNING: $bin_dir is not on your PATH. Add this to your shell rc:"
            msg "    export PATH=\"$bin_dir:\$PATH\""
            ;;
    esac
    msg ""
    msg "Run 'cascade' to start, or 'cascade help' for usage."
}

main "$@"
