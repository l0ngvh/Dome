#!/usr/bin/env bash
# Headless udev/DRM integration suite for the Dome Linux Wayland compositor.
#
# Boots the compositor on a real DRM node, drives it over IPC, and asserts hub state,
# scanout, and session behaviour after each step. Only the startup phase is a smoke
# check. virtio-gpu-gl (virgl) is unreachable on the macOS dev host, so vkms plus Mesa
# software GL is the portable path. See plans/linux-testing/02-vm-validation.md.
#
# Run as root: it opens DRM through the libseat builtin backend. Build dome first
# and pass the binary path (default target/debug/dome).
#
#   KMS=vkms|virtio     DRM backend. vkms (default) loads the module and exposes the
#                       CRTC CRC. virtio takes an existing virtio-gpu node, no CRC.
#   GALLIUM_DRIVER=...  Software GL driver, llvmpipe (default) or softpipe.
#   TEST_PHASES=...     Phases to run, comma-separated. "all" (default) runs every
#                       phase. "driver" runs the phases whose result depends on
#                       GALLIUM_DRIVER, for the later passes of a driver matrix.
#                       Phases: render, windows, clipboard, xwayland,
#                       xwayland_absent, vt, hotplug.
#
# The startup phase always runs, because every other phase asserts against the
# compositor it brings up. Each phase starts the clients it needs and leaves the
# window count back at 0, so any subset runs on its own.
#
# A selected phase whose environment is missing prints a reason, skips, and leaves
# the run green. render needs the vkms CRC node. xwayland needs xwayland-satellite,
# Xwayland, and xeyes. vt needs a VT subsystem, absent in a CI container. hotplug
# needs vkms configfs, a kernel around 6.11+ such as the Ubuntu HWE kernel.
set -euo pipefail

DOME="${1:-target/debug/dome}"
CRC=/sys/kernel/debug/dri/0/crtc-0/crc
LOG=/tmp/dome-udev.log

export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/dome-xdg}"
export DOME_BACKEND=udev
export LIBSEAT_BACKEND=builtin
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER="${GALLIUM_DRIVER:-llvmpipe}"
KMS="${KMS:-vkms}"

ALL_PHASES=(render windows clipboard xwayland xwayland_absent shutdown vt hotplug)
DRIVER_PHASES=(render windows)

case "${TEST_PHASES:-all}" in
  all)    PHASES=("${ALL_PHASES[@]}") ;;
  driver) PHASES=("${DRIVER_PHASES[@]}") ;;
  *)      IFS=', ' read -r -a PHASES <<< "${TEST_PHASES}" ;;
esac
for p in "${PHASES[@]}"; do
  case " ${ALL_PHASES[*]} " in
    *" $p "*) ;;
    *) echo "unknown phase '$p' (valid: ${ALL_PHASES[*]}, all, driver)" >&2; exit 1 ;;
  esac
done

RAN=()
SKIPPED=()
OFF=()
selected() { case " ${PHASES[*]} " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }
skip() { SKIPPED+=("$1 ($2)"); echo "skip $1: $2"; }
pass() { RAN+=("$1"); echo "phase $1 ok"; }
fail() { echo "$1" >&2; tail -20 "${2:-$LOG}" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1; }

[ -x "$DOME" ] || { echo "dome binary not found at $DOME (build it first)" >&2; exit 1; }
if selected render || selected windows; then
  need weston-simple-egl || { echo "weston-simple-egl missing (install weston)" >&2; exit 1; }
  need weston-terminal || { echo "weston-terminal missing (install weston)" >&2; exit 1; }
fi
if selected clipboard; then
  need wl-copy || { echo "wl-copy missing (install wl-clipboard)" >&2; exit 1; }
fi

if [ "$KMS" = vkms ]; then
  modprobe vkms enable_writeback=1
  [ -e "$CRC/data" ] || { echo "no vkms CRC node at $CRC" >&2; exit 1; }
fi

# dome spawns xwayland-satellite at startup, so the software-Xwayland knob has to be
# set before the compositor comes up. Headless software GL crashes the default glamor
# renderer, hence -glamor none here.
XWAYLAND_SKIP=""
if selected xwayland; then
  if need xwayland-satellite && need Xwayland && need xeyes; then
    export DOME_XWAYLAND_GLAMOR="${DOME_XWAYLAND_GLAMOR:-none}"
  else
    XWAYLAND_SKIP="needs xwayland-satellite, Xwayland, and xeyes"
  fi
fi

mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

cleanup() { kill "${CLIENT_PID:-}" "${TERM_PID:-}" "${DOME_PID:-}" "${HP_PID:-}" "${XEYES_PID:-}" "${AB_PID:-}" "${SD_PID:-}" >/dev/null 2>&1 || true; }
trap cleanup EXIT

wc_count() { "$DOME" query workspaces 2>/dev/null | grep -o '"window_count":[0-9]*' | awk -F: '{s+=$2} END{print s+0}'; }
wait_count() {
  local want=$1
  for _ in $(seq 1 20); do
    [ "$(wc_count)" = "$want" ] && return 0
    sleep 0.5
  done
  return 1
}
check_panic() {
  local log=$1 where=$2
  grep -q panicked "$log" || return 0
  echo "compositor panicked during $where:" >&2
  grep -A5 panicked "$log" >&2
  exit 1
}
# A client leaked by an earlier phase surfaces here, rather than as a count mismatch
# attributed to the phase that follows it.
expect_empty() {
  wait_count 0 || fail "phase $1 started with $(wc_count) windows, expected 0"
}

: > "$LOG"
"$DOME" > "$LOG" 2>&1 &
DOME_PID=$!

sock=""
for _ in $(seq 1 40); do
  sock=$(ls "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null | grep -v '\.lock$' | head -1 || true)
  [ -n "$sock" ] && break
  kill -0 "$DOME_PID" 2>/dev/null || fail "dome exited early:"
  sleep 0.5
done
[ -n "$sock" ] || fail "no wayland socket appeared:"
wd=$(basename "$sock")
echo "dome up on $wd (GALLIUM_DRIVER=$GALLIUM_DRIVER)"

# vkms exposes one connector, so a monitor in the response proves the query reaches
# real hub state.
monitors=""
for _ in $(seq 1 10); do
  monitors=$("$DOME" query monitors 2>&1 || true)
  printf '%s' "$monitors" | grep -q '"device_name"' && break
  sleep 0.5
done
echo "query monitors -> $monitors"
printf '%s' "$monitors" | grep -q '"device_name"' \
  || fail "dome query monitors returned no monitor (IPC query round-trip failed)"
echo "IPC query round-trip ok"

# A dome query client speaks IPC, not Wayland, so it is not a toplevel and the
# baseline window count stays 0.
wait_count 0 || fail "expected 0 windows at baseline, got $(wc_count)"
echo "window count baseline 0 ok"
pass startup

# A growing set of distinct CRTC CRCs proves new frames scan out. The CRC node is
# vkms-only, so virtio-gpu cannot assert this.
phase_render() {
  [ -e "$CRC/data" ] || { skip render "no CRC node, KMS=$KMS"; return 0; }
  expect_empty render
  echo auto > "$CRC/control"
  local before after
  before=$( (timeout 2 cat "$CRC/data" 2>/dev/null || true) | awk 'NR>1{print $NF}' | sort -u | wc -l)
  WAYLAND_DISPLAY="$wd" weston-simple-egl > /tmp/dome-client.log 2>&1 &
  CLIENT_PID=$!
  sleep 3
  check_panic "$LOG" "the render phase"
  after=$( (timeout 2 cat "$CRC/data" 2>/dev/null || true) | awk 'NR>1{print $NF}' | sort -u | wc -l)
  echo "unique CRCs  before(no client)=$before  after(animated client)=$after"
  if [ "$after" -lt 2 ] || [ "$after" -le "$before" ]; then
    fail "page-flip loop did not advance (after=$after before=$before):"
  fi
  kill "$CLIENT_PID" >/dev/null 2>&1 || true
  CLIENT_PID=""
  wait_count 0 || fail "window count did not drop to 0 after the render client exited, got $(wc_count)"
  pass render
}

phase_windows() {
  expect_empty windows
  WAYLAND_DISPLAY="$wd" weston-simple-egl > /tmp/dome-client.log 2>&1 &
  CLIENT_PID=$!
  wait_count 1 || fail "expected 1 window after client map, got $(wc_count)"
  echo "window count 1 (client mapped) ok"
  WAYLAND_DISPLAY="$wd" weston-terminal > /tmp/dome-term.log 2>&1 &
  TERM_PID=$!
  wait_count 2 || fail "expected 2 windows after second client, got $(wc_count)"
  echo "window count 2 (second client) ok"
  "$DOME" close
  wait_count 1 || fail "dome close did not drop the count to 1, got $(wc_count)"
  echo "window count 1 (after dome close) ok"
  check_panic "$LOG" "the windows phase"
  kill "${CLIENT_PID:-}" "${TERM_PID:-}" >/dev/null 2>&1 || true
  CLIENT_PID=""; TERM_PID=""
  wait_count 0 || fail "window count did not drop to 0 after both clients exited, got $(wc_count)"
  pass windows
}

# wl-clipboard needs no keyboard focus here, because dome serves wlr-data-control.
phase_clipboard() {
  local clip got=""
  clip="dome-clip-$$-$RANDOM"
  WAYLAND_DISPLAY="$wd" wl-copy "$clip"
  for _ in $(seq 1 10); do
    got=$(WAYLAND_DISPLAY="$wd" wl-paste -n 2>/dev/null || true)
    [ "$got" = "$clip" ] && break
    sleep 0.5
  done
  [ "$got" = "$clip" ] || fail "clipboard round-trip failed: wrote '$clip' read '$got'"
  echo "clipboard round-trip ok"
  pass clipboard
}

# dome picks the X display, so read it back from the log rather than assuming one.
phase_xwayland() {
  [ -z "$XWAYLAND_SKIP" ] || { skip xwayland "$XWAYLAND_SKIP"; return 0; }
  expect_empty xwayland
  local xdisp=""
  for _ in $(seq 1 20); do
    xdisp=$(grep -o 'DISPLAY=:[0-9]*' "$LOG" | head -1 | cut -d= -f2 || true)
    [ -n "$xdisp" ] && break
    sleep 0.5
  done
  [ -n "$xdisp" ] || { skip xwayland "dome reported no X display, satellite may be missing"; return 0; }
  DISPLAY="$xdisp" xeyes > /tmp/dome-xeyes.log 2>&1 &
  XEYES_PID=$!
  local xok=0
  for _ in $(seq 1 24); do [ "$(wc_count)" = 1 ] && { xok=1; break; }; sleep 0.5; done
  check_panic "$LOG" "the xwayland phase"
  [ "$xok" = 1 ] || fail "XWayland: X client on $xdisp did not map (count $(wc_count), expected 1):"
  echo "X client mapped as a managed window on $xdisp ok"
  kill "$XEYES_PID" >/dev/null 2>&1 || true
  XEYES_PID=""
  wait_count 0 || fail "window count did not drop to 0 after the X client exited, got $(wc_count)"
  pass xwayland
}

stop_fixture() {
  kill "${CLIENT_PID:-}" "${TERM_PID:-}" "${DOME_PID:-}" >/dev/null 2>&1 || true
  wait "${DOME_PID:-}" 2>/dev/null || true
  CLIENT_PID=""; TERM_PID=""; DOME_PID=""
  pkill -x dome >/dev/null 2>&1 || true
  # A signalled dome exits asynchronously, and while one still answers an IPC ping the
  # next instance refuses to start with "dome is already running" (src/ipc.rs).
  for _ in $(seq 1 20); do
    pgrep -x dome >/dev/null 2>&1 || break
    sleep 0.25
  done
  rm -f "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null || true
}

# XWayland is optional, so dome must warn and carry on when the satellite is not
# installed. It once panicked instead: a process-wide SIGCHLD=SIG_IGN turned the
# failed exec into a panic inside std and made dome's own warn arm unreachable.
phase_xwayland_absent() {
  stop_fixture
  local nopath=/tmp/dome-nopath alog=/tmp/dome-xwl-absent.log up=0
  rm -rf "$nopath"; mkdir -p "$nopath"
  : > "$alog"
  # An empty PATH is the strongest form of absent, whichever directory installed the
  # satellite. dome resolves nothing else through PATH before its main loop.
  env PATH="$nopath" "$DOME" > "$alog" 2>&1 &
  AB_PID=$!
  for _ in $(seq 1 40); do
    ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && { up=1; break; }
    kill -0 "$AB_PID" 2>/dev/null || break
    sleep 0.5
  done
  check_panic "$alog" "the xwayland_absent phase"
  [ "$up" = 1 ] || fail "dome did not come up with xwayland-satellite absent:" "$alog"
  kill -0 "$AB_PID" 2>/dev/null || fail "dome exited with xwayland-satellite absent:" "$alog"
  grep -q "xwayland-satellite not available" "$alog" \
    || fail "dome did not report the satellite as unavailable:" "$alog"
  echo "dome stayed up and warned with xwayland-satellite absent"
  kill "$AB_PID" >/dev/null 2>&1 || true
  wait "$AB_PID" 2>/dev/null || true
  AB_PID=""
  rm -f "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null || true
  rmdir "$nopath" 2>/dev/null || true
  pass xwayland_absent
}

# A terminate signal has to reach dome's own shutdown path. The default disposition
# kills the process outright, which leaves the satellite child, its X11 lock, and the
# IPC socket behind for the next start to clean up.
phase_shutdown() {
  stop_fixture
  local slog=/tmp/dome-shutdown.log ipc="${TMPDIR:-/tmp}/dome.sock" up=0 gone=0 lock="" xsocket=""
  : > "$slog"
  "$DOME" > "$slog" 2>&1 &
  SD_PID=$!
  for _ in $(seq 1 40); do
    ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && { up=1; break; }
    kill -0 "$SD_PID" 2>/dev/null || break
    sleep 0.5
  done
  [ "$up" = 1 ] || fail "dome did not come up for the shutdown phase:" "$slog"
  [ -S "$ipc" ] || fail "no IPC socket at $ipc while dome was running:" "$slog"
  # dome picks a free X display number, so read the one it took rather than guessing.
  # A stale lock from an earlier run sits in /tmp under a different number.
  local display
  display=$(grep -o 'DISPLAY=:[0-9]*' "$slog" | head -1 | cut -d: -f2 || true)
  if [ -n "$display" ]; then
    lock="/tmp/.X${display}-lock"
    xsocket="/tmp/.X11-unix/X${display}"
  fi

  kill -TERM "$SD_PID"
  for _ in $(seq 1 40); do
    kill -0 "$SD_PID" 2>/dev/null || { gone=1; break; }
    sleep 0.25
  done
  [ "$gone" = 1 ] || fail "dome did not exit within 10s of SIGTERM:" "$slog"
  wait "$SD_PID" 2>/dev/null || true
  SD_PID=""
  check_panic "$slog" "the shutdown phase"
  grep -q "Shutting down on a terminate signal" "$slog" \
    || fail "dome exited on SIGTERM without reaching the shutdown path:" "$slog"
  [ -e "$ipc" ] && fail "the IPC socket $ipc outlived dome:" "$slog"
  pgrep -f xwayland-satellite >/dev/null 2>&1 && fail "xwayland-satellite outlived dome:" "$slog"
  if [ -n "$lock" ] && [ -e "$lock" ]; then
    fail "the X11 lock $lock outlived dome:" "$slog"
  fi
  if [ -n "$xsocket" ] && [ -e "$xsocket" ]; then
    fail "the X11 socket $xsocket outlived dome:" "$slog"
  fi
  echo "SIGTERM ran the shutdown path, removing the IPC socket and the X11 files"
  rm -f "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null || true
  pass shutdown
}

# Launching via openvt -ls makes dome the VT session leader, so the libseat builtin
# backend takes VT_PROCESS and receives the switch signals.
phase_vt() {
  if ! (need openvt && need chvt && need fgconsole && [ -e /dev/tty0 ] && fgconsole >/dev/null 2>&1); then
    skip vt "needs a VT subsystem (openvt, chvt, fgconsole, an active VT)"
    return 0
  fi
  stop_fixture
  local dome_abs vtlog vt_before vt_dome
  dome_abs=$(readlink -f "$DOME")
  vtlog=/tmp/dome-vt.log; : > "$vtlog"
  vt_before=$(fgconsole)
  openvt -ls -- bash -c "export XDG_RUNTIME_DIR='$XDG_RUNTIME_DIR' DOME_BACKEND=udev LIBSEAT_BACKEND=builtin LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER='$GALLIUM_DRIVER'; exec '$dome_abs' >> '$vtlog' 2>&1"
  sleep 1
  vt_dome=$(fgconsole)
  for _ in $(seq 1 40); do ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && break; sleep 0.25; done
  sleep 1
  chvt "$vt_before"; sleep 2
  chvt "$vt_dome"; sleep 2
  if ! (grep -q "Session paused" "$vtlog" && grep -q "Session resumed" "$vtlog"); then
    pkill -x dome >/dev/null 2>&1 || true
    chvt "$vt_before" 2>/dev/null || true
    fail "VT switch did not pause/resume (vt_before=$vt_before vt_dome=$vt_dome):" "$vtlog"
  fi
  echo "VT switch pause/resume ok"
  pkill -x dome >/dev/null 2>&1 || true
  chvt "$vt_before" 2>/dev/null || true
  rm -f "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null || true
  pass vt
}

# vkms configfs lets us build a 2-connector card and force the primary connector off,
# so dome must re-home the primary onto the survivor instead of removing it
# (udev_backend.rs). The sysfs force-status is sticky and emits no uevent, so each
# toggle is paired with a udevadm trigger.
phase_hotplug() {
  local configfs=/sys/kernel/config/vkms
  if [ "$KMS" = vkms ] && need udevadm; then
    mountpoint -q /sys/kernel/config 2>/dev/null || mount -t configfs none /sys/kernel/config 2>/dev/null || true
  fi
  if [ "$KMS" != vkms ] || ! need udevadm || [ ! -d "$configfs" ]; then
    skip hotplug "needs vkms configfs and KMS=vkms, got KMS=$KMS"
    return 0
  fi

  # Reload vkms with no default card so the configfs card we build becomes card0, the
  # one dome picks. The running compositor holds the DRM node, so drop it first.
  stop_fixture
  sleep 1
  local demo="$configfs/demo"
  hp_teardown() {
    [ -d "$demo" ] || return 0
    echo 0 > "$demo/enabled" 2>/dev/null || true
    find "$demo" -type l -delete 2>/dev/null || true
    for d in "$demo"/connectors/* "$demo"/encoders/* "$demo"/crtcs/* "$demo"/planes/*; do rmdir "$d" 2>/dev/null || true; done
    rmdir "$demo" 2>/dev/null || true
  }
  hp_teardown
  if ! (modprobe -r vkms 2>/dev/null && modprobe vkms create_default_dev=0 2>/dev/null); then
    skip hotplug "could not reload vkms with create_default_dev=0"
    modprobe vkms enable_writeback=1 2>/dev/null || true
    return 0
  fi
  mountpoint -q /sys/kernel/config 2>/dev/null || mount -t configfs none /sys/kernel/config 2>/dev/null || true
  mkdir "$demo"
  local n
  for n in 0 1; do
    mkdir "$demo/planes/plane$n" "$demo/crtcs/crtc$n" "$demo/encoders/enc$n" "$demo/connectors/conn$n"
    echo 1 > "$demo/planes/plane$n/type"
    ln -s "$demo/crtcs/crtc$n" "$demo/planes/plane$n/possible_crtcs/crtc$n"
    ln -s "$demo/crtcs/crtc$n" "$demo/encoders/enc$n/possible_crtcs/crtc$n"
    ln -s "$demo/encoders/enc$n" "$demo/connectors/conn$n/possible_encoders/enc$n"
  done
  echo 1 > "$demo/enabled"
  sleep 1

  local hplog=/tmp/dome-hotplug.log; : > "$hplog"
  "$DOME" > "$hplog" 2>&1 &
  HP_PID=$!
  for _ in $(seq 1 40); do
    ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && break
    kill -0 "$HP_PID" 2>/dev/null || fail "dome exited early on the 2-connector card:" "$hplog"
    sleep 0.5
  done
  mon_count() { "$DOME" query monitors 2>/dev/null | grep -o '"device_name"' | wc -l | tr -d ' '; }
  wait_mon() { local want=$1; for _ in $(seq 1 40); do [ "$(mon_count)" = "$want" ] && return 0; sleep 0.5; done; return 1; }

  # First-frame latency on 2 software-GL outputs means the IPC query answers only
  # after a few seconds, so wait_mon polls rather than querying once.
  wait_mon 2 || fail "expected 2 monitors on the 2-connector card, got $(mon_count):" "$hplog"
  echo "hotplug: 2 monitors up"

  # The focused workspace sits on the primary monitor.
  local primary secondary
  primary=$("$DOME" query workspaces 2>/dev/null | grep -o '"monitor":"[^"]*"[^}]*"is_focused":true' | head -1 | sed 's/.*"monitor":"\([^"]*\)".*/\1/')
  [ -n "$primary" ] || primary=Virtual-1
  case "$primary" in Virtual-1) secondary=Virtual-2 ;; *) secondary=Virtual-1 ;; esac

  echo off > "/sys/class/drm/card0-$primary/status"; udevadm trigger -c change /sys/class/drm/card0
  wait_mon 1 || fail "primary ($primary) unplug: expected 1 monitor, got $(mon_count):" "$hplog"
  check_panic "$hplog" "the hotplug phase"
  echo "hotplug: primary ($primary) unplug re-homed to 1 monitor, no panic"

  echo on > "/sys/class/drm/card0-$primary/status"; udevadm trigger -c change /sys/class/drm/card0
  wait_mon 2 || fail "primary replug: expected 2 monitors, got $(mon_count):" "$hplog"
  check_panic "$hplog" "the hotplug phase"
  echo "hotplug: primary replug restored 2 monitors"

  echo off > "/sys/class/drm/card0-$secondary/status"; udevadm trigger -c change /sys/class/drm/card0
  wait_mon 1 || fail "secondary ($secondary) unplug: expected 1 monitor, got $(mon_count):" "$hplog"
  check_panic "$hplog" "the hotplug phase"
  echo "hotplug: secondary ($secondary) unplug dropped to 1 monitor, no panic"

  echo on > "/sys/class/drm/card0-$secondary/status"; udevadm trigger -c change /sys/class/drm/card0
  wait_mon 2 || fail "secondary replug: expected 2 monitors, got $(mon_count):" "$hplog"
  check_panic "$hplog" "the hotplug phase"
  echo "hotplug: connector hotplug and primary re-home ok"

  # Restore the default single-connector card so a re-run (the softpipe pass) finds it.
  # dome holds the DRM node, so wait for it to fully exit before tearing the card down,
  # or configfs refuses to remove the still-active device and vkms stays pinned. The
  # card release can lag the process exit, so retry the teardown and unload.
  kill "$HP_PID" >/dev/null 2>&1 || true
  wait "$HP_PID" 2>/dev/null || true
  HP_PID=""
  pkill -x dome >/dev/null 2>&1 || true
  rm -f "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null || true
  sleep 2
  for _ in 1 2 3; do
    hp_teardown
    modprobe -r vkms 2>/dev/null && break
    sleep 1
  done
  modprobe vkms enable_writeback=1 2>/dev/null || true
  pass hotplug
}

for phase in "${ALL_PHASES[@]}"; do
  if selected "$phase"; then
    "phase_$phase"
  else
    OFF+=("$phase")
  fi
done

echo "udev integration suite passed (KMS=$KMS, GALLIUM_DRIVER=$GALLIUM_DRIVER)"
echo "  ran:     ${RAN[*]:-none}"
echo "  skipped: ${SKIPPED[*]:-none}"
echo "  off:     ${OFF[*]:-none}"
