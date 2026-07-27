"""Flow Launcher plugin that lists Dome's minimized windows and restores one on selection.

Shells out to the local `dome` CLI. Requires `dome.exe` on PATH.
"""

import json
import shutil
import subprocess
from pathlib import Path

from pyflowlauncher import Plugin, Result


FALLBACK_ICON = str((Path(__file__).parent / "assets" / "icon.png").resolve())

plugin = Plugin()


def _error_result(title, subtitle):
    return Result(Title=title, SubTitle=subtitle, IcoPath=FALLBACK_ICON)


@plugin.on_method
def query(query: str):
    dome = shutil.which("dome")
    if dome is None:
        return [
            _error_result(
                "dome not on PATH",
                "See the Flow Launcher plugin README for install steps.",
            )
        ]

    try:
        completed = subprocess.run(
            [dome, "query", "minimized"],
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
        return [
            _error_result(
                "dome query minimized failed",
                str(e),
            )
        ]

    try:
        entries = json.loads(completed.stdout)
    except json.JSONDecodeError as e:
        return [
            _error_result(
                "dome returned malformed output",
                str(e),
            )
        ]

    results = []
    for entry in entries:
        title = entry.get("title") or entry.get("app_name") or "(untitled)"
        subtitle = entry.get("app_name") or ""
        ico = entry.get("executable_path") or FALLBACK_ICON
        results.append(
            Result(
                Title=title,
                SubTitle=subtitle,
                IcoPath=ico,
                json_rpc_action={
                    "Method": "restore_window",
                    "Parameters": [entry["id"]],
                },
            )
        )
    return results


@plugin.on_method
def restore_window(window_id: int):
    # Flow's Python JSON-RPC v1 shim has no toast API, so a failed unminimize
    # surfaces nothing to the user. Re-open the launcher to see the current
    # window list. Documented in README under Known limitations.
    dome = shutil.which("dome") or "dome"
    subprocess.run(
        [dome, "unminimize-window", str(window_id)],
        check=False,
    )
    return []


if __name__ == "__main__":
    plugin.run()
