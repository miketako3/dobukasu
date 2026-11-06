#!/bin/sh
# Install the prebuilt dobukasu binary for this OS and architecture.
#
#   curl -fsSL https://raw.githubusercontent.com/miketako3/dobukasu/main/install.sh | sh
#
# Environment:
#   DOBUKASU_VERSION      release tag to install (default: the latest release)
#   DOBUKASU_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   DOBUKASU_REPO         repository to download from (default: miketako3/dobukasu)

set -eu

REPO="${DOBUKASU_REPO:-miketako3/dobukasu}"
BIN="dobukasu"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Darwin)
    case "$arch" in
      arm64 | aarch64) target="aarch64-apple-darwin" ;;
      x86_64) target="x86_64-apple-darwin" ;;
      *) die "no prebuilt binary for macOS/$arch — build it with: cargo install --path crates/cli" ;;
    esac
    ;;
  Linux)
    case "$arch" in
      x86_64 | amd64) target="x86_64-unknown-linux-gnu" ;;
      *) die "no prebuilt binary for Linux/$arch — build it with: cargo install --path crates/cli" ;;
    esac
    ;;
  *)
    die "no prebuilt binary for $os — build it with: cargo install --path crates/cli"
    ;;
esac

version="${DOBUKASU_VERSION:-}"
if [ -z "$version" ]; then
  version=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  [ -n "$version" ] || die "could not find the latest release; set DOBUKASU_VERSION=v0.1.0"
fi

dir="${DOBUKASU_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$dir"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

url="https://github.com/$REPO/releases/download/$version/dobukasu-$target.tar.gz"
say "installing $BIN $version for $target"
curl -fsSL "$url" -o "$tmp/$BIN.tar.gz" || die "download failed: $url"
tar -xzf "$tmp/$BIN.tar.gz" -C "$dir" "$BIN"
chmod +x "$dir/$BIN"

say "installed $dir/$BIN"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) say "add it to PATH: export PATH=\"$dir:\$PATH\"" ;;
esac
"$dir/$BIN" --version
