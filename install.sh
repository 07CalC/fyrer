#!/usr/bin/env bash
#
# install fyrer from GitHub Releases.
#
# usage:
#   curl -fsSL https://raw.githubusercontent.com/07calc/fyrer/main/install.sh | sh
#   sh install.sh v0.3.0                 # pin a version
#   FYRER_INSTALL_DIR=/usr/local/bin sh install.sh
#   FYRER_TARGET=x86_64-unknown-linux-gnu sh install.sh
#
set -euo pipefail

REPO="07calc/fyrer"
VERSION="${1:-}"
INSTALL_DIR="${FYRER_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"

log() { printf '%s\n' "$*"; }
die() { log "error: $*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || die "curl is required"
command -v tar >/dev/null 2>&1 || die "tar is required"

case "$(uname -s)" in
  Linux) OS="linux" ;;
  Darwin) OS="macos" ;;
  *) die "unsupported OS: $(uname -s)" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) ARCH="x86_64" ;;
  aarch64 | arm64) ARCH="aarch64" ;;
  *) die "unsupported architecture: $(uname -m)" ;;
esac

case "$OS-$ARCH" in
  linux-x86_64) TARGET="x86_64-unknown-linux-musl" ;;
  linux-aarch64) TARGET="aarch64-unknown-linux-musl" ;;
  macos-x86_64) TARGET="x86_64-apple-darwin" ;;
  macos-aarch64) TARGET="aarch64-apple-darwin" ;;
  *) die "no release available for $OS-$ARCH" ;;
esac
TARGET="${FYRER_TARGET:-$TARGET}"

if [ -z "$VERSION" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -n 1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
fi
[ -n "$VERSION" ] || die "could not determine the latest release"

BASE="${FYRER_BASE_URL:-https://github.com/$REPO/releases/download/$VERSION}"
ASSET="fyrer-$TARGET.tar.gz"
SHAFILE="fyrer-$TARGET.sha256"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

log "downloading $BASE/$ASSET"
curl -fsSL -o "$TMPDIR/$ASSET" "$BASE/$ASSET"
curl -fsSL -o "$TMPDIR/$SHAFILE" "$BASE/$SHAFILE"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$TMPDIR" && sha256sum -c "$SHAFILE")
elif command -v shasum >/dev/null 2>&1; then
  (cd "$TMPDIR" && shasum -a 256 -c "$SHAFILE")
else
  die "sha256sum or shasum is required"
fi

tar -xzf "$TMPDIR/$ASSET" -C "$TMPDIR"

mkdir -p "$INSTALL_DIR"
install -m 0755 "$TMPDIR/fyrer" "$INSTALL_DIR/fyrer"

log "installed fyrer $VERSION to $INSTALL_DIR/fyrer"
log "ensure $INSTALL_DIR is on your PATH, e.g.: export PATH=\"$INSTALL_DIR:\$PATH\""
