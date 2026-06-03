#!/usr/bin/env bash
set -euo pipefail

# self-dojo installer
# Installs the Rust engine and bash wrapper

DOJO_SRC="${HOME}/dojo"
BIN_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/self-dojo"

echo "self-dojo installer"
echo "···················"
echo ""

# Build engine
echo "Building Rust engine..."
cd "${DOJO_SRC}"
cargo build --release 2>&1 | tail -1
echo "  Engine: ${DOJO_SRC}/target/release/dojo-engine"

# Install wrapper
mkdir -p "${BIN_DIR}"
cp "${DOJO_SRC}/dojo" "${BIN_DIR}/dojo"
chmod +x "${BIN_DIR}/dojo"
echo "  Wrapper: ${BIN_DIR}/dojo"

# Ensure data dirs
mkdir -p "${DATA_DIR}" "${DATA_DIR}/entries"

# Check PATH
case ":${PATH}:" in
  *:"${BIN_DIR}":*) ;;
  *) echo ""
     echo "  NOTE: ${BIN_DIR} is not in your PATH."
     echo "  Add this to your ~/.bashrc or ~/.zshrc:"
     echo "    export PATH=\"\${HOME}/.local/bin:\${PATH}\"" ;;
esac

# Check deps
echo ""
echo "Dependencies:"
for cmd in gum sqlite3; do
  if command -v "${cmd}" &>/dev/null; then
    echo "  ✓ ${cmd}"
  else
    echo "  ✗ ${cmd}  (install with: sudo pacman -S ${cmd})"
  fi
done

echo ""
echo "Done. Type 'dojo' to start."
