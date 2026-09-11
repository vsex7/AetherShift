#!/usr/bin/env bash
# Live end-to-end regression and latency benchmark for AetherShift.
set -Eeuo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}PASS:${NC} $1"; }
fail() { echo -e "${RED}FAIL:${NC} $1"; exit 1; }
info() { echo -e "${BLUE}INFO:${NC} $1"; }
skip() { echo -e "${YELLOW}SKIP:${NC} $1"; }

RESTORE_DONE=false
restore_once() {
    if [ "$RESTORE_DONE" != true ]; then
        aethershift restore >/dev/null 2>&1 || true
        RESTORE_DONE=true
    fi
}

cleanup() {
    local rc="$1"
    restore_once
    trap - EXIT
    exit "$rc"
}

require_number() {
    if ! [[ "$1" =~ ^-?[0-9]+([.][0-9]+)?$ ]]; then
        fail "Expected a numeric value for $2, got '$1'"
    fi
}

window_geometry() {
    local json
    json="$(hyprctl activewindow -j)"
    jq -r '[.at[0], .at[1], .size[0], .size[1]] | @tsv' <<< "$json"
}

geometry_for_address() {
    local address="$1"
    hyprctl clients -j | jq -r --arg address "$address" \
        '.[] | select(.address == $address) | [.at[0], .at[1], .size[0], .size[1]] | @tsv'
}

wait_geometry() {
    local address="$1" expected="$2" label="$3" tolerance="$4" actual=""
    for _ in {1..50}; do
        actual="$(geometry_for_address "$address")"
        if [ -n "$actual" ] && check_geometry_close "$actual" "$expected" "$label" "$tolerance"; then
            return 0
        fi
        sleep 0.1
    done
    actual="$(geometry_for_address "$address")"
    assert_geometry_close "$actual" "$expected" "$label" "$tolerance"
}

check_geometry_close() {
    local actual="$1" expected="$2" label="$3" tolerance="${4:-8}"
    read -r ax ay aw ah <<< "$actual"
    read -r ex ey ew eh <<< "$expected"
    local pairs=("$ax $ex" "$ay $ey" "$aw $ew" "$ah $eh")
    for pair in "${pairs[@]}"; do
        read -r a e <<< "$pair"
        require_number "$a" "${label} actual"
        require_number "$e" "${label} expected"
        local delta
        delta="$(awk -v a="$a" -v e="$e" 'BEGIN {d=a-e; if (d < 0) d=-d; print d}')"
        if awk -v d="$delta" -v t="$tolerance" 'BEGIN {exit !(d > t)}'; then
            echo "${label} differs by ${delta} px (tolerance ${tolerance}px)" >&2
            return 1
        fi
    done
}

assert_geometry_close() {
    local actual="$1" expected="$2" label="$3" tolerance="${4:-8}"
    if ! check_geometry_close "$actual" "$expected" "$label" "$tolerance"; then
        fail "geometry check failed"
    fi
}

info "Starting AetherShift live E2E verification..."

if [ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
    fail "HYPRLAND_INSTANCE_SIGNATURE is not set; run inside an active Hyprland session"
fi
pass "Hyprland session detected"

DAEMON_READY=false
for _ in {1..30}; do
    if aethershift status >/dev/null 2>&1; then
        DAEMON_READY=true
        break
    fi
    sleep 0.1
done
if [ "$DAEMON_READY" != true ]; then
    fail "Cannot communicate with the AetherShift daemon; run 'aethershift daemon'"
fi
pass "AetherShift daemon is responsive"
trap 'cleanup "$?"' EXIT
trap 'cleanup 130' INT TERM

info "Testing Windows profile injection..."
aethershift switch windows --force >/dev/null
STATUS_JSON="$(aethershift status --json)"
ACTIVE_PROFILE="$(jq -r '.active_profile' <<< "$STATUS_JSON")"
ACTIVE_OVERLAYS="$(jq -r '.overlays_count' <<< "$STATUS_JSON")"
if [ "$ACTIVE_PROFILE" != "windows" ]; then
    fail "Active profile expected windows, got $ACTIVE_PROFILE"
fi
if [ "$ACTIVE_OVERLAYS" -lt 15 ]; then
    fail "Expected at least 15 active overlays, got $ACTIVE_OVERLAYS"
fi
pass "Switched to windows with $ACTIVE_OVERLAYS overlays"

HYPR_BINDS="$(hyprctl binds -j)"
if grep -q "Close active window" <<< "$HYPR_BINDS"; then
    pass "Alt+F4 injection is visible in Hyprland"
else
    fail "Alt+F4 binding was not injected"
fi
if grep -q "Snap active window left" <<< "$HYPR_BINDS"; then
    pass "Super+Left injection is visible in Hyprland"
else
    fail "Super+Left binding was not injected"
fi

info "Running 100-round windows/macos switch benchmark..."
START_BENCH="$(date +%s%N)"
for _ in {1..50}; do
    aethershift switch macos >/dev/null
    aethershift switch windows >/dev/null
done
END_BENCH="$(date +%s%N)"
TOTAL_NS=$((END_BENCH - START_BENCH))
AVG_US=$((TOTAL_NS / 100 / 1000))
AVG_MS="$(awk -v us="$AVG_US" 'BEGIN {print us / 1000}')"
info "Average end-to-end switch latency: ${AVG_MS} ms (${AVG_US} us)"
if [ "$AVG_US" -ge 5000 ]; then
    fail "Average switch latency exceeded 5ms"
fi
pass "Switch latency is below 5ms"

info "Testing window mode state transitions..."
aethershift window-mode set floating >/dev/null
WM="$(aethershift window-mode | awk '{print $NF}')"
if [ "$WM" != "floating" ]; then
    fail "Expected floating window mode, got $WM"
fi
aethershift window-mode set tiled >/dev/null
WM="$(aethershift window-mode | awk '{print $NF}')"
if [ "$WM" != "tiled" ]; then
    fail "Expected tiled window mode, got $WM"
fi
pass "Window mode transitions work"

info "Testing clean restore and zero residue..."
aethershift restore
STATUS_JSON="$(aethershift status --json)"
RESTORED_PROFILE="$(jq -r '.active_profile' <<< "$STATUS_JSON")"
RESTORED_OVERLAYS="$(jq -r '.overlays_count' <<< "$STATUS_JSON")"
RESTORED_WM="$(jq -r '.window_policy' <<< "$STATUS_JSON")"
if [ "$RESTORED_PROFILE" != "native" ] || [ "$RESTORED_OVERLAYS" != "0" ] || [ "$RESTORED_WM" != "omarchy" ]; then
    fail "Restore state invalid: profile=$RESTORED_PROFILE overlays=$RESTORED_OVERLAYS policy=$RESTORED_WM"
fi
if grep -q "Close active window" <<< "$(hyprctl binds -j)"; then
    fail "AetherShift bindings remain after restore"
fi
pass "Restore returned to the native baseline"
RESTORE_DONE=true

info "Checking categorized cheat sheet output..."
if ! aethershift cheatsheet windows | grep -q "ALT + F4"; then
    fail "'aethershift cheatsheet windows' did not contain Alt+F4"
fi
if ! aethershift cheatsheet windows | grep -q "Window Management"; then
    fail "'aethershift cheatsheet windows' did not include category output"
fi
pass "Cheat sheet works"

info "Testing snap geometry, restore, and preview..."
aethershift switch windows >/dev/null

# Create a disposable window for snap testing (isolated from user windows)
SNAP_TEST_TITLE="aethershift-snap-test"
SNAP_TEST_PID=""
SNAP_TEST_ADDRESS=""
if command -v foot >/dev/null 2>&1; then
    foot --title "$SNAP_TEST_TITLE" sh -c 'sleep 120' >/dev/null 2>&1 &
    SNAP_TEST_PID=$!
elif command -v alacritty >/dev/null 2>&1; then
    alacritty --title "$SNAP_TEST_TITLE" -e sh -c 'sleep 120' >/dev/null 2>&1 &
    SNAP_TEST_PID=$!
fi

if [ -n "$SNAP_TEST_PID" ]; then
    # Wait for the window to appear
    for _ in {1..30}; do
        SNAP_TEST_ADDRESS="$(hyprctl clients -j | jq -r --arg t "$SNAP_TEST_TITLE" \
            '.[] | select(.title == $t) | .address' | head -n 1)"
        [ -n "$SNAP_TEST_ADDRESS" ] && break
        sleep 0.1
    done
    if [ -z "$SNAP_TEST_ADDRESS" ]; then
        kill "$SNAP_TEST_PID" >/dev/null 2>&1 || true
        fail "Snap test window did not appear"
    fi
    info "Snap test window address: $SNAP_TEST_ADDRESS"
    # New foot windows auto-focus, no explicit focus needed
else
    # Fallback to active window
    SNAP_TEST_ADDRESS="$(hyprctl activewindow -j | jq -r '.address')"
    info "No terminal available, using active window: $SNAP_TEST_ADDRESS"
fi

# Compute expected geometry from the window's monitor
TEST_SNAP_MONITOR="$(hyprctl clients -j | jq -r --arg a "$SNAP_TEST_ADDRESS" \
    '.[] | select(.address == $a) | .monitor')"
MONITOR_JSON="$(hyprctl monitors -j | jq --argjson id "$TEST_SNAP_MONITOR" '.[] | select(.id == $id)')"
read -r WORK_X WORK_Y WORK_WIDTH WORK_HEIGHT < <(
    jq -r '[
        (.x + .reserved[0]),
        (.y + .reserved[1]),
        (((.width / .scale) - .reserved[0] - .reserved[2]) | floor),
        (((.height / .scale) - .reserved[1] - .reserved[3]) | floor)
    ] | @tsv' <<< "$MONITOR_JSON"
)
HALF_WIDTH=$((WORK_WIDTH / 2))
RIGHT_HALF_GEOMETRY="$((WORK_X + HALF_WIDTH)) $WORK_Y $HALF_WIDTH $WORK_HEIGHT"
LEFT_HALF_GEOMETRY="$WORK_X $WORK_Y $HALF_WIDTH $WORK_HEIGHT"

INITIAL_GEOMETRY="$(geometry_for_address "$SNAP_TEST_ADDRESS")"

aethershift snap half-right >/dev/null
wait_geometry "$SNAP_TEST_ADDRESS" "$RIGHT_HALF_GEOMETRY" "half-right" 8
pass "Half-right snap geometry matches expected work area"

aethershift snap half-left >/dev/null
wait_geometry "$SNAP_TEST_ADDRESS" "$LEFT_HALF_GEOMETRY" "half-left" 8
pass "Half-left snap geometry matches expected work area"

aethershift snap restore >/dev/null
wait_geometry "$SNAP_TEST_ADDRESS" "$INITIAL_GEOMETRY" "snap restore" 8
pass "Snap restore returned window to its original geometry"

aethershift snap half-left >/dev/null
wait_geometry "$SNAP_TEST_ADDRESS" "$LEFT_HALF_GEOMETRY" "re-snap half-left" 8
aethershift snap half-left --preview >/dev/null
sleep 1
AFTER_PREVIEW="$(geometry_for_address "$SNAP_TEST_ADDRESS")"
assert_geometry_close "$AFTER_PREVIEW" "$LEFT_HALF_GEOMETRY" "snap preview" 0
pass "Snap preview did not move the window"

# Clean up the snap test window
if [ -n "$SNAP_TEST_ADDRESS" ]; then
    hyprctl eval "hl.dispatch(hl.dsp.window.close({ address = \"$SNAP_TEST_ADDRESS\" }))" >/dev/null 2>&1 || true
fi
if [ -n "$SNAP_TEST_PID" ]; then
    kill "$SNAP_TEST_PID" >/dev/null 2>&1 || true
    wait "$SNAP_TEST_PID" 2>/dev/null || true
fi
sleep 0.3

info "Testing floating and tiled policies for newly created windows..."
TEST_WINDOW_TITLE="aethershift-e2e-window"
if command -v foot >/dev/null 2>&1; then
    TEST_WINDOW_CMD=(foot --title "$TEST_WINDOW_TITLE" sh -c 'sleep 30')
elif command -v alacritty >/dev/null 2>&1; then
    TEST_WINDOW_CMD=(alacritty --title "$TEST_WINDOW_TITLE" -e sh -c 'sleep 30')
else
    TEST_WINDOW_CMD=()
fi

if [ "${#TEST_WINDOW_CMD[@]}" -eq 0 ]; then
    skip "No disposable terminal emulator available"
else
    find_test_window() {
        hyprctl clients -j | jq -r --arg title "$TEST_WINDOW_TITLE" \
            '.[] | select(.title == $title) | [.address, .floating] | @tsv' | head -n 1
    }

    for policy in floating tiled; do
        aethershift window-mode set "$policy" >/dev/null
        "${TEST_WINDOW_CMD[@]}" >/dev/null 2>&1 &
        TEST_WINDOW_PID=$!
        TEST_WINDOW_ADDRESS=""
        for _ in {1..30}; do
            TEST_WINDOW_ADDRESS="$(find_test_window | cut -f1)"
            [ -n "$TEST_WINDOW_ADDRESS" ] && break
            sleep 0.1
        done
        if [ -z "$TEST_WINDOW_ADDRESS" ]; then
            kill "$TEST_WINDOW_PID" >/dev/null 2>&1 || true
            fail "Test window did not appear for $policy policy"
        fi

        EXPECTED_FLOATING=true
        [ "$policy" = tiled ] && EXPECTED_FLOATING=false
        ACTUAL_FLOATING="$(find_test_window | cut -f2)"
        hyprctl eval "hl.dispatch(hl.dsp.window.close({ address = \"$TEST_WINDOW_ADDRESS\" }))" >/dev/null 2>&1 || true
        kill "$TEST_WINDOW_PID" >/dev/null 2>&1 || true
        wait "$TEST_WINDOW_PID" 2>/dev/null || true
        if [ "$ACTUAL_FLOATING" != "$EXPECTED_FLOATING" ]; then
            fail "$policy policy expected floating=$EXPECTED_FLOATING, got $ACTUAL_FLOATING"
        fi
        pass "New windows follow the $policy policy"
    done
fi

info "Testing daemon crash self-heal..."
aethershift restore >/dev/null
RESTORE_DONE=true
if command -v systemctl >/dev/null 2>&1 && systemctl --user is-active --quiet aethershift.service; then
    DAEMON_PID="$(systemctl --user show -p MainPID --value aethershift.service 2>/dev/null || true)"
    if [ -z "$DAEMON_PID" ]; then
        skip "Could not identify aethershift-daemon PID"
    else
        kill -9 "$DAEMON_PID"
        RESTART_OK=false
        for _ in {1..50}; do
            if aethershift status >/dev/null 2>&1; then
                RESTART_OK=true
                break
            fi
            sleep 0.1
        done
        if [ "$RESTART_OK" != true ]; then
            fail "Daemon did not self-heal after SIGKILL"
        fi
        STATUS_JSON="$(aethershift status --json)"
        RESTART_OVERLAYS="$(jq -r '.overlays_count' <<< "$STATUS_JSON")"
        RESTART_PROFILE="$(jq -r '.active_profile' <<< "$STATUS_JSON")"
        if [ "$RESTART_OVERLAYS" != "0" ] || [ "$RESTART_PROFILE" != "native" ]; then
            fail "Unexpected restart state: profile=$RESTART_PROFILE overlays=$RESTART_OVERLAYS"
        fi
        pass "systemd restarted the daemon after SIGKILL"
    fi
else
    skip "aethershift.service is not active in this systemd user session"
fi

echo
pass "ALL LIVE END-TO-END TESTS PASSED"
