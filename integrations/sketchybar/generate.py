#!/usr/bin/env python3
"""Emit sketchybar commands that scope Dome's workspace pills to each display.

Joins `sketchybar --query displays` to `dome query monitors`, so the bar on a
display draws only the workspaces of the monitor Dome has on it. Pills and parked
entries carry the click that switches to them. Requires `sketchybar` and `dome`
on PATH, unless `--dome-path` is given.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

# Workspace names to emit a pill for, on every display. Edit to match your Dome
# config. Pills are emitted drawing=off and only dome.sh turns one on, so a name
# listed here that does not exist stays invisible, while a name missing from here
# can never appear on the bar at all.
WORKSPACES = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]

# An item script runs from the daemon's cwd, not this one, so the path baked into
# every item has to be absolute.
PLUGIN = str(Path(__file__).resolve().parent / "dome.sh")


def _parse_args():
    parser = argparse.ArgumentParser(description="Emit sketchybar commands for Dome.")
    parser.add_argument(
        "--dome-path",
        default="dome",
        metavar="PATH",
        help="dome binary to bake into every generated item (default: found on PATH)",
    )
    args = parser.parse_args()
    if "/" in args.dome_path:
        args.dome_path = str(Path(args.dome_path).expanduser().resolve())
    return args


def _query(argv):
    """Run a JSON query and return its array, or exit without printing anything."""
    try:
        result = subprocess.run(argv, capture_output=True, text=True, check=True)
    except OSError as exc:
        sys.exit(f"{argv[0]}: {exc}")
    except subprocess.CalledProcessError as exc:
        sys.exit(f"{' '.join(argv)} failed: {exc.stderr.strip() or exc.returncode}")

    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError:
        sys.exit(f"{' '.join(argv)} did not return JSON")

    # A query timeout answers {"error":"query timed out"} rather than the array.
    if not isinstance(value, list):
        sys.exit(f"{' '.join(argv)} did not return an array")
    return value


def _slug(name):
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


# The item script reaches execvp("/usr/bin/env", {"env","sh","-c",command}), so a
# quoted argument is handled by a real shell and only the quote itself escapes.
# shlex.quote is wrong here, because it escapes a quote as '"'"' and those double
# quotes would close the script="..." property early.
def _shell_quote(value):
    return "'" + value.replace("'", "'\\''") + "'"


def _pill(item, arrangement, workspace, monitor, dome_path):
    # Triggering the event repaints the bar at once. Without it the clicked pill
    # keeps its old colour until the next tick, which reads as a click that did
    # nothing.
    click = (
        f"{_shell_quote(dome_path)} focus workspace {_shell_quote(workspace)} --monitor {_shell_quote(monitor)}"
        "; sketchybar --trigger dome_update"
    )
    print(f"sketchybar --add item {item} left \\")
    print(f"  --set {item} display={arrangement} drawing=off \\")
    print(f'    script="{_shell_quote(PLUGIN)}" \\')
    print(f'    click_script="{click}" \\')
    print("    background.drawing=on background.corner_radius=5 background.height=22 \\")
    # The idle border colour has to match the one dome.sh restores on mouse.exited.
    print("    background.border_width=1 background.border_color=0x66ffffff \\")
    print("    label.padding_left=8 label.padding_right=8 \\")
    print("    padding_left=4 padding_right=4 \\")
    print(f"  --subscribe {item} mouse.entered mouse.exited")


# Popup children, one heading plus one entry per workspace name for every monitor
# that could own a parked workspace. They are created empty because the parked set
# changes while the bar runs, and only dome.sh turns one on.
def _parked_heading(item, monitor):
    print(f"sketchybar --add item {item} popup.dome.parked \\")
    print(f"  --set {item} drawing=off label={_shell_quote(monitor)} \\")
    print("    label.color=0xff888888 label.padding_left=8 label.padding_right=8 \\")
    print("    background.drawing=off")


def _parked_entry(item, workspace, origin, dome_path):
    # Closes the popup itself, because focusing leaves it open over the bar.
    click = (
        f"{_shell_quote(dome_path)} focus workspace {_shell_quote(workspace)} --monitor {_shell_quote(origin)}"
        "; sketchybar --set dome.parked popup.drawing=off"
        "; sketchybar --trigger dome_update"
    )
    print(f"sketchybar --add item {item} popup.dome.parked \\")
    print(f"  --set {item} drawing=off label={_shell_quote(workspace)} \\")
    print(f'    click_script="{click}" \\')
    print("    label.color=0xffffbb00 label.padding_left=24 label.padding_right=8 \\")
    print("    background.drawing=off")


def main():
    args = _parse_args()
    dome_path = args.dome_path

    # Every query runs before the first print, so a failed run emits nothing and
    # the caller's eval becomes a no-op instead of applying half an item set.
    displays = _query(["sketchybar", "--query", "displays"])
    monitors = _query([dome_path, "query", "monitors"])
    rows = _query([dome_path, "query", "workspaces"])

    # Join on DirectDisplayID alone. Both sides also report a rectangle and the
    # two disagree by design, since sketchybar reports the full display while
    # dome reports the work area, so matching on frames would reject a correct
    # pairing. A monitor Dome could not identify carries a null id, which must
    # not match a display that reports none.
    by_id = {}
    for monitor in monitors:
        display_id = monitor.get("cg_display_id")
        if display_id is not None:
            by_id.setdefault(display_id, monitor.get("unique_name") or "")

    print("sketchybar --add event dome_update\n")

    # One unscoped watcher, because a display arriving or a monitor being renamed
    # changes the item set itself, which only a reload can rebuild.
    print("sketchybar --add item dome.watcher left \\")
    print(
        f'  --set dome.watcher drawing=off script="DOME_PATH={_shell_quote(dome_path)}'
        f' {_shell_quote(PLUGIN)}" \\'
    )
    print("  --subscribe dome.watcher display_change\n")

    # Every monitor that could own a parked workspace needs popup slots. A live
    # monitor qualifies because it can be unplugged before the next reload, and a
    # monitor already gone qualifies because its rows are parked right now, which
    # is the only place its name still appears.
    origins = []
    for monitor in monitors:
        name = monitor.get("unique_name") or ""
        if name and name not in origins:
            origins.append(name)
    for row in rows:
        origin = row.get("monitor") or ""
        if row.get("state") == "Parked" and origin and origin not in origins:
            origins.append(origin)

    # The plugin gets the same list the pills are created from, so it can switch
    # off a pill whose workspace has left that monitor. Both sides come from this
    # one run, so they cannot disagree about which pills exist. The parked slugs
    # do the same for the popup, and the plugin rebuilds each item name from a
    # slug plus a workspace name rather than being handed every name.
    pill_names = _shell_quote(" ".join(WORKSPACES))
    parked_slugs = _shell_quote(" ".join(_slug(origin) for origin in origins))

    print("sketchybar --add item dome.parked left \\")
    print("  --set dome.parked drawing=off label=parked \\")
    print('    click_script="sketchybar --set dome.parked popup.drawing=toggle" \\')
    print("    popup.align=center popup.background.drawing=on \\")
    print("    popup.background.color=0xee1e1e2e popup.background.corner_radius=5 \\")
    print("    popup.background.border_width=1 popup.background.border_color=0x66ffbb00 \\")
    print("    background.drawing=on background.corner_radius=5 background.height=22 \\")
    print("    background.border_width=1 background.border_color=0x66ffbb00 \\")
    print("    label.color=0xffffbb00 label.padding_left=8 label.padding_right=8 \\")
    print("    padding_left=4 padding_right=4\n")

    # One shared popup, not one per bar, because the indicator is one unscoped
    # item. Emitted after its parent so the parent exists to attach to, and in
    # heading then entries order because a popup renders items as they were added.
    for origin in origins:
        slug = _slug(origin)
        _parked_heading(f"dome.parked.heading.{slug}", origin)
        for workspace in WORKSPACES:
            _parked_entry(f"dome.parked.{slug}.{workspace}", workspace, origin, dome_path)
        print()

    for display in displays:
        arrangement = display.get("arrangement-id")
        if arrangement is None:
            continue

        name = by_id.get(display.get("DirectDisplayID"), "")
        if not name:
            print(f"# dome: display {arrangement} has no monitor registered with dome, skipped\n")
            continue

        prefix = f"dome.{_slug(name)}"
        print(f"# dome: display {arrangement} = {name}")

        for workspace in WORKSPACES:
            _pill(f"{prefix}.ws.{workspace}", arrangement, workspace, name, dome_path)

        # No display= on the driver. That property governs where an item draws,
        # the driver draws nothing, and script firing ignores it. Its scope is
        # its argv.
        print(f"sketchybar --add item {prefix}.driver left \\")
        print(f"  --set {prefix}.driver drawing=off update_freq=1 \\")
        print(
            f'    script="DOME_PATH={_shell_quote(dome_path)} {_shell_quote(PLUGIN)}'
            f' {_shell_quote(name)} {prefix}'
            f' {pill_names} {parked_slugs}" \\'
        )
        print(f"  --subscribe {prefix}.driver dome_update\n")


if __name__ == "__main__":
    main()
