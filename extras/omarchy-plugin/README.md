# AetherShift Omarchy 4.x Quickshell Plugin

This is the optional Omarchy 4.x Quickshell bar widget and popup overlay for AetherShift.

## Installation

Symlink or copy this directory into your user Omarchy plugins folder:

```bash
mkdir -p ~/.config/omarchy/plugins
ln -s "$(pwd)/extras/omarchy-plugin/omarchy.aethershift" ~/.config/omarchy/plugins/omarchy.aethershift
```

## Features

- **Status Indicator**: Shows the active profile (`🪟 Win`, `🍎 Mac`, `⚡ Hyb`, `🛡️ Nat`) and active overlay count.
- **Left Click**: Instant cycle to the next profile (`aethershift cycle`).
- **Right Click**: Launches the interactive TUI console (`aethershift-tui`).
