#!/usr/bin/env bash
set -euo pipefail

# AetherShift one-click installation script

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "==> [1/5] Building release binaries (workspace)..."
cd "${ROOT_DIR}"
cargo build --release --workspace

echo "==> [2/5] Installing binaries to ~/.local/bin/..."
BIN_DIR="${HOME}/.local/bin"
mkdir -p "${BIN_DIR}"
install -m 755 "${ROOT_DIR}/target/release/aethershift" "${BIN_DIR}/aethershift"
install -m 755 "${ROOT_DIR}/target/release/aethershift-daemon" "${BIN_DIR}/aethershift-daemon"
install -m 755 "${ROOT_DIR}/target/release/aethershift-tui" "${BIN_DIR}/aethershift-tui"

echo "==> [3/5] Setting up configuration & presets directory (~/.config/aethershift/presets/)..."
PRESET_DIR="${HOME}/.config/aethershift/presets"
mkdir -p "${PRESET_DIR}"
if [ -d "${ROOT_DIR}/presets" ]; then
    cp -u "${ROOT_DIR}/presets/"*.toml "${PRESET_DIR}/" 2>/dev/null || cp "${ROOT_DIR}/presets/"*.toml "${PRESET_DIR}/"
    echo "    Copied presets to ${PRESET_DIR}"
fi

echo "==> [4/5] Installing and enabling systemd user service..."
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
mkdir -p "${SYSTEMD_USER_DIR}"
install -m 644 "${ROOT_DIR}/systemd/aethershift.service" "${SYSTEMD_USER_DIR}/aethershift.service"

if command -v systemctl >/dev/null 2>&1; then
    echo "    Reloading systemd user daemon..."
    systemctl --user daemon-reload || true
    echo "    Enabling and starting aethershift.service..."
    systemctl --user enable --now aethershift.service || echo "    Notice: Couldn't start service now (perhaps graphical-session is inactive). Service is enabled."
fi

echo "==> [5/5] Checking Quickshell plugin integration for Omarchy..."
OMARCHY_SHELL_DIR="/usr/share/omarchy/shell"
PLUGIN_SRC="${ROOT_DIR}/extras/omarchy-plugin/omarchy.aethershift"
PLUGIN_DEST="${HOME}/.config/omarchy/plugins/omarchy.aethershift"

if [ -d "${OMARCHY_SHELL_DIR}" ]; then
    echo "    Omarchy Quickshell detected at ${OMARCHY_SHELL_DIR}."
    if [ -d "${PLUGIN_SRC}" ]; then
        mkdir -p "${HOME}/.config/omarchy/plugins"
        ln -sfn "${PLUGIN_SRC}" "${PLUGIN_DEST}"
        echo "    Linked Quickshell plugin: ${PLUGIN_DEST} -> ${PLUGIN_SRC}"
    else
        echo "    Notice: Quickshell plugin source directory not found at ${PLUGIN_SRC}, skipping plugin linking."
    fi
else
    echo "    Omarchy Quickshell not detected (/usr/share/omarchy/shell/ not found). Skipping plugin link."
fi

echo ""
echo "✨ AetherShift installed successfully!"
echo "   - CLI:      ~/.local/bin/aethershift"
echo "   - Daemon:   ~/.local/bin/aethershift-daemon"
echo "   - TUI:      ~/.local/bin/aethershift-tui"
echo "   - Presets:  ~/.config/aethershift/presets/"
echo "   - Service:  ~/.config/systemd/user/aethershift.service"
echo ""
echo "Try running 'aethershift status' or 'aethershift-tui' to explore."
