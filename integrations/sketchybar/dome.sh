#!/usr/bin/env bash
set -euo pipefail

# The generator bakes this into the script= of the driver and the watcher.
dome_path="${DOME_PATH:-dome}"

# SketchyBar runs an item's script for every event it subscribes to, and only the
# pills subscribe to the mouse events. Handling them here rather than in a second
# plugin keeps the install to one script. This runs before the argument checks,
# because a pill's script is invoked with no arguments at all.
#
# Hover moves background.border_color on purpose. The per-tick writes below never
# touch it, so it survives, where anything they do write would be reverted inside
# a second. The idle value has to match the one generate.py creates a pill with.
case "${SENDER:-}" in
  mouse.entered)
    sketchybar --set "${NAME:-}" background.border_color=0xffffffff 2>/dev/null || true
    exit 0
    ;;
  mouse.exited)
    sketchybar --set "${NAME:-}" background.border_color=0x66ffffff 2>/dev/null || true
    exit 0
    ;;
  display_change)
    # The item set is fixed when the generator runs, so a display Dome has never
    # reported has no pills and a renamed monitor has stale ones baked in. Only a
    # reload rebuilds them.
    #
    # display_change also fires on a plain active-display focus switch, which
    # happens constantly, so the topology is compared before paying for a reload.
    # Both queries are needed. The display list misses a unique_name change under
    # an unchanged set of displays, and the monitor list misses a display Dome
    # could not identify.
    state="${TMPDIR:-/tmp}/dome-sketchybar-topology"
    now="$(sketchybar --query displays 2>/dev/null)$("$dome_path" query monitors 2>/dev/null)" || true
    # An empty answer means a query failed rather than that the topology emptied,
    # and acting on it would reload now and again once the query recovers.
    [ -n "$now" ] || exit 0
    was="$(cat "$state" 2>/dev/null)" || was=""
    if [ "$now" != "$was" ]; then
      # Stored before the reload, so anything that re-enters here during the
      # rebuild finds no difference and cannot loop.
      printf '%s' "$now" >"$state"
      sketchybar --reload
    fi
    exit 0
    ;;
esac

# A pill's script takes no arguments and exists only for the mouse events above,
# so any other sender reaching it has nothing to do.
[ "$#" -gt 0 ] || exit 0
# The prefix is passed rather than derived, because deriving it would mean
# knowing how the generator slugged every other display's name too.
monitor="${1:?usage: dome.sh <monitor-unique-name> <item-prefix> [workspace-names] [parked-slugs]}"
prefix="${2:?usage: dome.sh <monitor-unique-name> <item-prefix> [workspace-names] [parked-slugs]}"

# The pill names this display owns, space separated, exactly as the generator
# created them. Optional so a plugin copied into place before the next reload
# still runs, since the driver's argv only gains the list once the generator
# has run again.
workspaces="${3:-}"

# The monitor slugs that own popup slots, space separated. Also optional, and for
# the same reason.
parked_slugs="${4:-}"

# A query timeout prints {"error":"query timed out"}, not the array, so bail
# when the output is not an array.

json="$("$dome_path" query workspaces 2>/dev/null || echo '{"error":"unavailable"}')"

if ! echo "$json" | jq -e 'type == "array"' >/dev/null 2>&1; then
  exit 0
fi

# The state test is not redundant with the name test. A parked row's monitor is
# a frozen origin name, and a rerank can later hand that same string to a live
# monitor, which would draw a departed monitor's workspaces on this bar.
rows="$(echo "$json" | jq -r --arg mon "$monitor" '
  .[]
  | select(.monitor == $mon and .state == "Attached")
  | "\(.name) \(.is_focused) \(.is_visible) \(.window_count)"' || true)"

# The popup is global, so this filter is not scoped to this monitor. The slug is
# rebuilt here rather than passed, because the alternative is one argv entry per
# parked item. It must stay identical to _slug in generate.py, which is why both
# spell the same character class. Naming it also lets an origin that slugs to
# nothing drop out, which would otherwise emit a leading space and shift the
# workspace name into the slug field on read.
parked="$(echo "$json" | jq -r '
  .[]
  | select(.state == "Parked" and .window_count > 0)
  | (.monitor | ascii_downcase | gsub("[^a-z0-9]+"; "-") | sub("^-"; "") | sub("-$"; "")) as $slug
  | select($slug != "")
  | "\($slug) \(.name)"' || true)"

# Every property below goes into one sketchybar call, built up in the positional
# parameters. One call per pill would re-lay the bar once per workspace, which
# reads on a live bar as the pills twitching on every tick.
set --

# Names that had a row, delimited on both ends so the sweep can glob for
# " name " without a shorter name matching inside a longer one.
present=" "

while read -r name focused visible count; do
  # An empty read is the here-string's trailing newline, not a workspace.
  [ -n "$name" ] || continue
  present="$present$name "
  item="${prefix}.ws.${name}"

  # Colours, edit to match your theme.
  if [ "$visible" = "false" ] && [ "$count" = "0" ]; then
    set -- "$@" --set "$item" drawing=off
  elif [ "$focused" = "true" ]; then
    set -- "$@" --set "$item" drawing=on label="$name" \
      background.color=0xffffffff label.color=0xff000000
  elif [ "$visible" = "true" ]; then
    # Only reached when the row is not focused, so this display shows the
    # workspace while focus sits elsewhere.
    set -- "$@" --set "$item" drawing=on label="$name" \
      background.color=0xbbffffff label.color=0xff000000
  else
    set -- "$@" --set "$item" drawing=on label="$name" \
      background.color=0x44ffffff label.color=0xffffffff
  fi
done <<< "$rows"

# A workspace that left this monitor keeps no row here, so its pill would stay
# lit until the next reload. Switching off every owned pill that has no row
# covers that. The membership test is a glob against a string, so the sweep
# spawns nothing.
for name in $workspaces; do
  case "$present" in
    *" $name "*) ;;
    *) set -- "$@" --set "${prefix}.ws.${name}" drawing=off ;;
  esac
done

# Unscoped, so it shows on every bar. A parked workspace belongs to a monitor
# that is not present and so to none of the displays. Every display's driver
# reads the same response and computes the same answer, so the repeated writes
# agree with each other. An empty parked workspace hides nothing from view, so
# it does not earn the indicator.
parked_present=" "
headings=" "

while read -r slug name; do
  [ -n "$slug" ] || continue
  parked_present="$parked_present$slug.$name "
  set -- "$@" --set "dome.parked.$slug.$name" drawing=on

  # One heading per monitor, switched on by its first row rather than by every row.
  case "$headings" in
    *" $slug "*) ;;
    *)
      headings="$headings$slug "
      set -- "$@" --set "dome.parked.heading.$slug" drawing=on
      ;;
  esac
done <<< "$parked"

for slug in $parked_slugs; do
  case "$headings" in
    *" $slug "*) ;;
    *) set -- "$@" --set "dome.parked.heading.$slug" drawing=off ;;
  esac
  for name in $workspaces; do
    case "$parked_present" in
      *" $slug.$name "*) ;;
      *) set -- "$@" --set "dome.parked.$slug.$name" drawing=off ;;
    esac
  done
done

# Closing the popup matters when the last parked workspace goes away while the
# popup is open, which would otherwise leave an empty panel over the bar.
if [ "$headings" = " " ]; then
  set -- "$@" --set dome.parked drawing=off popup.drawing=off
else
  set -- "$@" --set dome.parked drawing=on
fi

sketchybar "$@" 2>/dev/null || true
