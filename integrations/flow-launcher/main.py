"""Flow Launcher plugin that lists Dome's minimized windows and restores one on selection.

Shells out to the local `dome` CLI. Requires `dome.exe` on PATH.
"""

import json
import shutil
import subprocess
import sys
from pathlib import Path

plugindir = Path(__file__).parent.resolve()
sys.path.insert(0, str(plugindir / "lib"))

from flowlauncher import FlowLauncher


FALLBACK_ICON = str(plugindir / "assets" / "icon.png")


def _error_result(title, subtitle):
    return {
        "Title": title,
        "SubTitle": subtitle,
        "IcoPath": FALLBACK_ICON,
    }


class DomePlugin(FlowLauncher):
    def query(self, query: str = ""):
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
            results.append({
                "Title": title,
                "SubTitle": subtitle,
                "IcoPath": ico,
                "JsonRPCAction": {
                    "method": "restore_window",
                    "parameters": [entry["id"]],
                },
            })
        return results

    def restore_window(self, window_id: int):
        dome = shutil.which("dome") or "dome"
        subprocess.run(
            [dome, "unminimize-window", str(window_id)],
            check=False,
        )


if __name__ == "__main__":
    DomePlugin()
