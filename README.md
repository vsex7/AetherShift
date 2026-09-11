# AetherShift

Pure-Rust, in-memory keybinding hot-swap daemon for Hyprland. Migrate muscle
memory between Windows, macOS, and Linux layouts in under 3 milliseconds, with
zero config pollution and instant restore.

## How it works

AetherShift runs as a user-space daemon connected to Hyprland via its Lua
socket. It injects keybinding overlays into the running compositor without
touching any config file. When you switch profiles or restore, changes apply
in a single round-trip through a Unix domain socket. No `hyprctl reload`, no
restart, no flicker.

```
 CLI / TUI / Quickshell
         |
     UDS socket
         |
   aethershift-daemon  <-- Hyprland Lua socket (inject/uninject bindings)
         |
   Hyprland (in-session, zero file writes)
```

## Prerequisites

- Hyprland >= 0.50
- Rust (stable, edition 2021)
- Linux with systemd user sessions (optional, for auto-start)

## Install

```bash
git clone https://github.com/abyss/aethershift ~/Projects/AetherShift
cd ~/Projects/AetherShift
./scripts/install.sh
```

What gets installed:

| Component | Path |
|---|---|
| CLI | `~/.local/bin/aethershift` |
| Daemon | `~/.local/bin/aethershift-daemon` |
| TUI | `~/.local/bin/aethershift-tui` |
| Presets | `~/.config/aethershift/presets/` |
| systemd unit | `~/.config/systemd/user/aethershift.service` |
| Quickshell plugin | `~/.config/omarchy/plugins/omarchy.aethershift` |

The service auto-starts if systemd user sessions are available. Otherwise run
`aethershift daemon` manually before using any other command.

## Uninstall

```bash
systemctl --user disable --now aethershift.service
rm ~/.local/bin/aethershift{,-daemon,-tui}
rm -rf ~/.config/aethershift
rm ~/.config/systemd/user/aethershift.service
```

## Quick start

```bash
aethershift switch windows     # Windows muscle memory, ~3ms
aethershift snap half-left     # Alt+Left (remapped) snaps left
aethershift restore            # back to baseline
```

## Windows profile cheat sheet

| Key | Action |
|---|---|
| Alt+F4 | Close active window |
| Alt+Tab | Cycle windows forward |
| Alt+Shift+Tab | Cycle windows backward |
| Super+Left | Snap active window left |
| Super+Right | Snap active window right |
| Super+Up | Maximize active window |
| Super+Down | Restore active window |
| Super+D | Toggle desktop (special workspace) |
| Ctrl+Alt+Left | Previous workspace |
| Ctrl+Alt+Right | Next workspace |
| Super+E | Open file manager |
| Super+L | Lock system |
| Super+V | Toggle clipboard manager |
| Print | Take screenshot |
| Super+Shift+S | Region screenshot |
| Ctrl+Shift+Escape | Open task manager (btop) |
| Ctrl+Alt+T | Open terminal |
| Alt+Print | Toggle screen recording |
| Super+Period | Toggle emoji picker |
| Super+Tab | Open task overview |

## Commands

| Command | Description |
|---|---|
| `aethershift status` | Daemon state, active profile, latency |
| `aethershift switch <name>` | Switch to preset or custom profile |
| `aethershift cycle` | Cycle through profiles |
| `aethershift restore` | Remove all overlays, return to baseline |
| `aethershift snap <layout>` | Apply snap layout to active window |
| `aethershift snap <layout> --preview` | Show target geometry without moving |
| `aethershift window-mode` | Query current new-window policy |
| `aethershift window-mode set <p>` | Set new-window policy (tiled/floating/follow-profile/omarchy) |
| `aethershift bindings [profile]` | List bindings (also aliased as `cheatsheet`) |
| `aethershift history` | Recent switch history |
| `aethershift doctor` | Environment diagnostics |
| `aethershift metrics` | Runtime performance metrics |

Use `--policy strict|force|smart` on `switch` to control conflict behavior.

## Presets

Built-in presets ship in `~/.config/aethershift/presets/`:

- `windows.toml`: Windows muscle memory (Alt+F4, Alt+Tab, Ctrl+Alt+arrows)
- `macos.toml`: macOS-style (Cmd+Q, Ctrl+Cmd+arrows)
- `hybrid.toml`: Mixed Windows and macOS shortcuts for dual-workflow users
- `native.toml`: Pass-through baseline, no overlays

Drop your own `*.toml` files into the same directory for custom profiles. The
daemon hot-reloads on `aethershift reload-presets`.

## Conflict policies

| Policy | Behavior |
|---|---|
| `strict` (default) | Skip conflicting bindings, report them |
| `force` | Override all existing bindings |
| `smart` | Keep user-defined bindings, override only compositor defaults |

## Snap layouts

`half-left`, `half-right`, `half-top`, `half-bottom`, `two-thirds-left`,
`two-thirds-right`, `one-third-left`, `one-third-right`, `three-columns-left`,
`three-columns-center`, `three-columns-right`, `maximize`, `center-floating`,
`restore`.

## Crash recovery

The daemon registers SIGTERM and SIGINT handlers that call `restore` before
exiting, so the baseline keybindings survive crashes. Under systemd, `kill -9`
triggers an automatic restart with a clean baseline.

## Development

```bash
cargo build --workspace       # build all crates
cargo test --workspace        # unit tests (66+)
./scripts/test-e2e-live.sh    # live Hyprland regression + latency benchmark
```

The E2E script creates disposable `foot` windows, runs geometry checks, and
always calls `restore` on exit. It will not modify your user config files.

## License

MIT
