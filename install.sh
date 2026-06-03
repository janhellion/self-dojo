#!/usr/bin/env bash
set -euo pipefail

# self-dojo installer — one-command setup for any Linux machine
# Usage: curl -sL https://raw.githubusercontent.com/janhellion/self-dojo/master/install.sh | bash
#    or: git clone https://github.com/janhellion/self-dojo && cd self-dojo && bash install.sh

BOLD="$(tput bold 2>/dev/null || echo '')"
RESET="$(tput sgr0 2>/dev/null || echo '')"
GREEN="$(tput setaf 2 2>/dev/null || echo '')"
RED="$(tput setaf 1 2>/dev/null || echo '')"

echo ""
echo "  ${BOLD}self-dojo installer${RESET}"
echo "  ·················"

# ── Detect OS ────────────────────────────────────────────────────────
if [[ -f /etc/arch-release ]]; then
  PKG="sudo pacman -S --noconfirm"
elif [[ -f /etc/debian_version ]]; then
  PKG="sudo apt-get install -y"
elif [[ -f /etc/fedora-release ]]; then
  PKG="sudo dnf install -y"
else
  PKG=""
fi

# ── Dependencies ─────────────────────────────────────────────────────
echo ""
echo "  Checking dependencies..."

missing=()
for cmd in rustc cargo sqlite3 openssl; do
  if command -v "$cmd" &>/dev/null; then
    echo "    ${GREEN}✓${RESET} $cmd"
  else
    echo "    ${RED}✗${RESET} $cmd"
    missing+=("$cmd")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  if [[ -z "$PKG" ]]; then
    echo ""
    echo "  Install manually: ${missing[*]}"
    echo "  Then re-run this script."
    exit 1
  fi
  echo ""
  echo "  Installing: ${missing[*]}"
  $PKG rust sqlite openssl 2>&1 | tail -3 || {
    echo "  Failed to install. Try manually: $PKG rust sqlite openssl"
    exit 1
  }
fi

# ── Clone or use current dir ────────────────────────────────────────
if [[ -f "Cargo.toml" ]] && grep -q 'dojo-engine' Cargo.toml 2>/dev/null; then
  DOJO_DIR="$(pwd)"
  echo ""
  echo "  Using current directory: $DOJO_DIR"
else
  DOJO_DIR="${HOME}/dojo"
  if [[ -d "$DOJO_DIR" ]]; then
    echo ""
    echo "  Directory exists, pulling latest..."
    cd "$DOJO_DIR" && git pull 2>/dev/null || true
  else
    echo ""
    echo "  Cloning self-dojo..."
    git clone https://github.com/janhellion/self-dojo.git "$DOJO_DIR" 2>&1 | tail -1
  fi
fi

cd "$DOJO_DIR"

# ── Build engine ─────────────────────────────────────────────────────
echo ""
echo "  Building Rust engine..."
cargo build --release 2>&1 | tail -2

if [[ ! -x "target/release/dojo-engine" ]]; then
  echo "  ${RED}Build failed${RESET}"
  exit 1
fi
echo "  ${GREEN}Engine built${RESET}"

# ── Install wrapper ──────────────────────────────────────────────────
BIN_DIR="${HOME}/.local/bin"
mkdir -p "$BIN_DIR"
cp dojo "$BIN_DIR/dojo"
chmod +x "$BIN_DIR/dojo"
echo "  ${GREEN}Wrapper installed to ${BIN_DIR}/dojo${RESET}"

# ── PATH check ───────────────────────────────────────────────────────
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  echo ""
  echo "  ${BOLD}Add to your shell config:${RESET}"
  echo "    export PATH=\"\${HOME}/.local/bin:\${PATH}\""
  echo ""
  echo "  Or run:  echo 'export PATH=\"\${HOME}/.local/bin:\${PATH}\"' >> ~/.bashrc"
fi

# ── Data directory ───────────────────────────────────────────────────
DATA_DIR="${HOME}/.local/share/self-dojo"
mkdir -p "$DATA_DIR" "$DATA_DIR/entries"

echo ""
echo "  ${BOLD}Done.${RESET}"
echo ""
echo "  Type ${BOLD}dojo${RESET} to start."
echo "  Your journal will be stored in ${DATA_DIR}"
echo ""
