#!/usr/bin/env bash
# Headless winit-backend checks for the Dome Linux Wayland compositor.
# Runs inside the dome-linux-test container. Boots the compositor nested in Xvfb
# with software OpenGL, drives it over IPC, and asserts hub state after each step.
# Also captures a screenshot of the Xvfb root as an artifact for a human to look at.
#
# The udev suite (ci/udev-integration.sh) covers the same ground on real KMS. This one
# needs no VM and no DRM node, so it is the fast local check for the winit path.
set -euo pipefail

cargo build --bin dome
DOME=target/debug/dome
LOG=/tmp/dome.log

export DISPLAY=:99
export DOME_BACKEND=winit
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export XDG_RUNTIME_DIR=/tmp/xdg-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

Xvfb :99 -screen 0 1920x1080x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!

cleanup() {
  "$DOME" exit >/dev/null 2>&1 || true
  pkill -x foot >/dev/null 2>&1 || true
  kill "${FIXTURE_PID:-}" "${DOME_PID:-}" "$XVFB_PID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

fail() { echo "$1" >&2; tail -20 "$LOG" >&2; exit 1; }
check_panic() {
  grep -q panicked "$LOG" || return 0
  echo "compositor panicked during $1:" >&2
  grep -A5 panicked "$LOG" >&2
  exit 1
}
wc_count() { "$DOME" query workspaces 2>/dev/null | grep -o '"window_count":[0-9]*' | awk -F: '{s+=$2} END{print s+0}'; }
wait_count() {
  local want=$1
  for _ in $(seq 1 40); do
    [ "$(wc_count)" = "$want" ] && return 0
    sleep 0.5
  done
  return 1
}

sleep 1
"$DOME" >"$LOG" 2>&1 &
DOME_PID=$!

up=0
for _ in $(seq 1 50); do
  if grep -q "Starting Dome on Linux (winit backend)" "$LOG"; then up=1; break; fi
  if ! kill -0 "$DOME_PID" 2>/dev/null; then break; fi
  sleep 0.2
done
[ "$up" = 1 ] || fail "compositor did not reach the winit event loop:"
echo "winit event loop reached ok"

# The winit window becomes dome's single output, so the hub must report a monitor.
monitors=""
for _ in $(seq 1 20); do
  monitors=$("$DOME" query monitors 2>&1 || true)
  printf '%s' "$monitors" | grep -q '"device_name"' && break
  sleep 0.5
done
printf '%s' "$monitors" | grep -q '"device_name"' \
  || fail "query monitors returned no monitor: $monitors"
echo "query monitors -> $monitors"
echo "IPC query round-trip ok"

wait_count 0 || fail "expected 0 windows at baseline, got $(wc_count)"
echo "window count baseline 0 ok"

"$DOME" exec foot
wait_count 1 || fail "expected 1 window after the first client, got $(wc_count)"
echo "window count 1 (first client mapped) ok"

"$DOME" exec foot
wait_count 2 || fail "expected 2 windows after the second client, got $(wc_count)"
echo "window count 2 (second client) ok"

mkdir -p /out
import -window root /out/winit.png
test -s /out/winit.png || fail "screenshot of the Xvfb root is empty:"
echo "screenshot written to test-out/winit.png (artifact, not an assertion)"

"$DOME" close
wait_count 1 || fail "dome close did not drop the count to 1, got $(wc_count)"
echo "window count 1 (after dome close) ok"

pkill -x foot >/dev/null 2>&1 || true
wait_count 0 || fail "window count did not drop to 0 after both clients exited, got $(wc_count)"
echo "window count 0 (after both clients exited) ok"

# A capture on this backend reads the window framebuffer back, so it takes a
# different path from the udev one and can arrive upside down on its own.
wd=$(grep -o 'Wayland socket: "[^"]*"' "$LOG" | head -1 | cut -d'"' -f2)
[ -n "$wd" ] || fail "could not read the wayland socket name from the log:"
expected=$("$DOME" query monitors | grep -o '"width":[0-9]*,"height":[0-9]*' | head -1 \
  | sed 's/"width"://; s/,"height":/x/')

# The fixture paints red rows at the top of its window, so red below means flipped.
WAYLAND_DISPLAY="$wd" foot sh -c \
  'printf "\033[41m"; for _ in 1 2 3 4 5 6 7 8; do printf "%120s\n" " "; done; printf "\033[0m"; sleep 600' \
  >/tmp/dome-fixture.log 2>&1 &
FIXTURE_PID=$!
wait_count 1 || fail "the capture fixture did not map, got $(wc_count)"
sleep 1

shot=/tmp/winit-shot.png region=/tmp/winit-region.png crop=/tmp/winit-crop.png
WAYLAND_DISPLAY="$wd" grim "$shot" || fail "grim could not capture the output:"
[ "$(identify -format '%wx%h' "$shot")" = "$expected" ] \
  || fail "the capture is $(identify -format '%wx%h' "$shot"), not the output size $expected:"
[ "$(identify -format '%k' "$shot")" -gt 1 ] \
  || fail "the capture holds one colour, so it missed the client:"
top=$(convert "$shot" -gravity North -crop 100%x40% +repage -format '%[fx:mean.r]' info:)
bottom=$(convert "$shot" -gravity South -crop 100%x40% +repage -format '%[fx:mean.r]' info:)
awk -v t="$top" -v b="$bottom" 'BEGIN{exit !(t > b + 0.01)}' \
  || fail "the capture holds more red below than above (top=$top bottom=$bottom), so it is upside down:"
echo "capture is $expected, holds the client, and is not flipped (top=$top bottom=$bottom)"

WAYLAND_DISPLAY="$wd" grim -g "40,30 160x120" "$region" || fail "grim could not capture a region:"
[ "$(identify -format '%wx%h' "$region")" = 160x120 ] \
  || fail "the region capture is $(identify -format '%wx%h' "$region"), not 160x120:"
convert "$shot" -crop 160x120+40+30 +repage "$crop"
[ "$(compare -metric AE "$region" "$crop" null: 2>&1)" = 0 ] \
  || fail "the region capture differs from the same crop of the whole output:"
echo "region capture matches the same crop of the whole output"

kill "$FIXTURE_PID" >/dev/null 2>&1 || true
FIXTURE_PID=""
wait_count 0 || fail "the capture fixture did not exit, got $(wc_count)"

check_panic "the winit checks"
echo "winit checks passed"
