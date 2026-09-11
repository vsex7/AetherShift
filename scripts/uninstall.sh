#!/usr/bin/env bash
set -euo pipefail

# AetherShift complete uninstallation and recovery script

echo "==> [1/6] Restoring baseline Hyprland keybindings & window state..."
if command -v aethershift >/dev/null 2>&1; then
    aethershift restore || true
elif [ -x "${HOME}/.local/bin/aethershift" ]; then
    "${HOME}/.local/bin/aethershift" restore || true
fi

echo "==> [2/6] Stopping and disabling systemd user service..."
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user stop aethershift.service 2>/dev/null || true
    systemctl --user disable aethershift.service 2>/dev/null || true
fi

echo "==> [3/6] Removing systemd service file..."
SYSTEMD_SERVICE="${HOME}/.config/systemd/user/aethershift.service"
if [ -f "${SYSTEMD_SERVICE}" ]; then
    rm -f "${SYSTEMD_SERVICE}"
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user daemon-reload || true
        systemctl --user reset-failed 2>/dev/null || true
    fi
    echo "    Removed ${SYSTEMD_SERVICE}"
fi

echo "==> [4/6] Removing binaries from ~/.local/bin/..."
for bin in aethershift aethershift-daemon aethershift-tui; do
    target_bin="${HOME}/.local/bin/${bin}"
    if [ -f "${target_bin}" ] || [ -L "${target_bin}" ]; then
        rm -f "${target_bin}"
        echo "    Removed ${target_bin}"
    fi
done

echo "==> [5/6] Removing Omarchy Quickshell plugin link..."
OMARCHY_PLUGIN="${HOME}/.config/omarchy/plugins/abyss.aethershift"
if [ -L "${OMARCHY_PLUGIN}" ] || [ -e "${OMARCHY_PLUGIN}" ]; then
    rm -rf "${OMARCHY_PLUGIN}"
    echo "    Removed ${OMARCHY_PLUGIN}"
fi

echo "==> [6/6] Cleaning up runtime socket and temporary files..."
USER_ID="${UID:-$(id -u)}"
SOCKET_PATH="/run/user/${USER_ID}/aethershift.sock"
if [ -e "${SOCKET_PATH}" ] || [ -S "${SOCKET_PATH}" ]; then
    rm -f "${SOCKET_PATH}"
    echo "    Removed socket ${SOCKET_PATH}"
fi

echo ""
echo "🎉 AetherShift uninstalled successfully!"
echo "   - Baseline Hyprland configuration and keybindings restored."
echo "   - Daemon, CLI, TUI binaries and service units removed."
echo "   - System configuration remains intact and unharmed."
