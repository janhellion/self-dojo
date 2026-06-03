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
  RUST_PKG="rust"
  SQLITE_PKG="sqlite"
elif [[ -f /etc/debian_version ]]; then
  PKG="sudo apt-get install -y"
  RUST_PKG="cargo"
  SQLITE_PKG="sqlite3"
elif [[ -f /etc/fedora-release ]]; then
  PKG="sudo dnf install -y"
  RUST_PKG="rust cargo"
  SQLITE_PKG="sqlite"
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
  
  # Map missing commands to package names
  pkgs_to_install=""
  for m in "${missing[@]}"; do
    case "$m" in
      rustc|cargo) pkgs_to_install="$pkgs_to_install $RUST_PKG" ;;
      sqlite3)     pkgs_to_install="$pkgs_to_install $SQLITE_PKG" ;;
      openssl)     pkgs_to_install="$pkgs_to_install openssl" ;;
    esac
  done
  # Deduplicate
  pkgs_to_install=$(echo "$pkgs_to_install" | tr ' ' '\n' | sort -u | tr '\n' ' ')
  
  echo ""
  echo "  Installing:$pkgs_to_install"
  $PKG $pkgs_to_install 2>&1 | tail -5 || {
    echo "  Failed to install. Try manually: $PKG$pkgs_to_install"
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
echo "  Run:  source ~/.bashrc   (or open a new terminal)"
echo "  Then: dojo"
echo ""
echo "  Or run directly with:  ~/.local/bin/dojo"
echo ""
echo "  Type ${BOLD}dojo${RESET} to start."
echo "  Your journal will be stored in ${DATA_DIR}"
echo ""
