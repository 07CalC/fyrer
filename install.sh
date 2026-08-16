#!/usr/bin/env bash
#
# Install Fyrer from GitHub Releases.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/07calc/fyrer/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/07calc/fyrer/main/install.sh | sh -s -- v0.4.0
#
# Environment:
#   FYRER_INSTALL_DIR   Installation directory
#   FYRER_VERSION       Version to install
#   FYRER_BASE_URL      Override release base URL
#

set -euo pipefail

REPO="07calc/fyrer"
VERSION="${1:-${FYRER_VERSION:-}}"
INSTALL_DIR="${FYRER_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"


bold='\033[1m'
dim='\033[2m'
green='\033[32m'
cyan='\033[36m'
yellow='\033[33m'
red='\033[31m'
reset='\033[0m'

info() {
    printf "  ${cyan}→${reset} %s\n" "$1"
}

success() {
    printf "  ${green}✓${reset} %s\n" "$1"
}

warn() {
    printf "  ${yellow}!${reset} %s\n" "$1"
}

error() {
    printf "  ${red}✗${reset} %s\n" "$1" >&2
}

die() {
    error "$1"
    exit 1
}

printf "\n"
printf "  ${bold}Fyrer Installer${reset}\n"
printf "  ${dim}Fast monorepo task runner${reset}\n"
printf "\n"


command -v curl >/dev/null 2>&1 || die "curl is required"
command -v uname >/dev/null 2>&1 || die "uname is required"

if ! command -v sha256sum >/dev/null 2>&1 &&
   ! command -v shasum >/dev/null 2>&1; then
    die "sha256sum or shasum is required"
fi


case "$(uname -s)" in
    Linux)
        OS="linux"
        ;;
    Darwin)
        OS="macos"
        ;;
    *)
        die "unsupported operating system: $(uname -s)"
        ;;
esac

case "$(uname -m)" in
    x86_64|amd64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        die "unsupported architecture: $(uname -m)"
        ;;
esac

case "$OS-$ARCH" in
    linux-x86_64)
        TARGET="x86_64-unknown-linux-musl"
        ;;
    linux-aarch64)
        TARGET="aarch64-unknown-linux-musl"
        ;;
    macos-x86_64)
        TARGET="x86_64-apple-darwin"
        ;;
    macos-aarch64)
        TARGET="aarch64-apple-darwin"
        ;;
    *)
        die "no release available for $OS-$ARCH"
        ;;
esac

info "Detected ${OS} ${ARCH}"
info "Target: ${TARGET}"


if [[ -z "$VERSION" ]]; then
    info "Finding latest release..."

    VERSION="$(
        curl -fsSL \
            "https://api.github.com/repos/$REPO/releases/latest" |
        grep '"tag_name"' |
        head -n 1 |
        sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
    )"
fi

[[ -n "$VERSION" ]] || die "could not determine release version"

success "Version ${VERSION}"

BASE="${FYRER_BASE_URL:-https://github.com/$REPO/releases/download/$VERSION}"

ASSET="fyrer-${TARGET}"
CHECKSUM="${ASSET}.sha256"


TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

info "Downloading Fyrer..."

if ! curl -fL --progress-bar \
    -o "$TMPDIR/$ASSET" \
    "$BASE/$ASSET"; then
    die "failed to download $ASSET"
fi


info "Verifying checksum..."

if curl -fsSL \
    -o "$TMPDIR/$CHECKSUM" \
    "$BASE/$CHECKSUM"; then

    (
        cd "$TMPDIR"

        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c "$CHECKSUM" >/dev/null
        else
            shasum -a 256 -c "$CHECKSUM" >/dev/null
        fi
    ) || die "checksum verification failed"

    success "Checksum verified"
else
    warn "No checksum available for this release"
fi


info "Installing to ${INSTALL_DIR}..."

mkdir -p "$INSTALL_DIR"

install -m 0755 \
    "$TMPDIR/$ASSET" \
    "$INSTALL_DIR/fyrer"

success "Installed Fyrer ${VERSION}"

printf "\n"


case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
        printf "  ${green}${bold}Fyrer is ready!${reset}\n"
        ;;
    *)
        printf "  ${yellow}${bold}One more step:${reset}\n"
        printf "\n"
        printf "  Add Fyrer to your PATH:\n"
        printf "\n"
        printf "    export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
        printf "\n"
        printf "  ${dim}Add that line to ~/.bashrc, ~/.zshrc, etc. to make it permanent.${reset}\n"
        ;;
esac

printf "\n"
printf "  Run ${bold}fyrer --help${reset} to get started.\n"
printf "\n"
