# Integration

Dome ships no status bar. These snippets read live state from `dome query workspaces`
(see [cli.md](cli.md#dome-query-workspaces)). They are built from the public APIs of
the status bars and of Dome, and they have not been tested against a live install yet,
so you may need to adjust them for your setup.

## SketchyBar (macOS)

```bash
#!/usr/bin/env bash
# Save to ~/.config/sketchybar/plugins/dome.sh and chmod +x
set -euo pipefail

# A query timeout prints {"error":"query timed out"}, not the array, so bail
# when the output is not an array.

json="$(dome query workspaces 2>/dev/null || echo '{"error":"unavailable"}')"

if ! echo "$json" | jq -e 'type == "array"' >/dev/null 2>&1; then
  exit 0
fi

echo "$json" | jq -r '.[] | "\(.name) \(.is_focused) \(.is_visible) \(.window_count)"' \
  | while read -r name focused visible count; do
      item="dome.ws.${name}"

      if [ "$visible" = "false" ] && [ "$count" = "0" ]; then
        sketchybar --set "$item" drawing=off 2>/dev/null || true
        continue
      fi

      if [ "$focused" = "true" ]; then
        sketchybar --set "$item" drawing=on label="$name" \
          background.color=0xffffffff label.color=0xff000000 2>/dev/null || true
      else
        sketchybar --set "$item" drawing=on label="$name" \
          background.color=0x44ffffff label.color=0xffffffff 2>/dev/null || true
      fi
    done
```

```bash
# Add to your sketchybarrc, then: sketchybar --reload
PLUGIN_DIR="$HOME/.config/sketchybar/plugins"

sketchybar --add event dome_update

# Range 0-9 = default workspaces, change to match yours
for i in 0 1 2 3 4 5 6 7 8 9; do
  sketchybar --add item "dome.ws.$i" left \
    --set "dome.ws.$i" drawing=off \
      background.drawing=on background.corner_radius=5 background.height=22 \
      background.border_width=1 background.border_color=0x66ffffff \
      label.padding_left=8 label.padding_right=8 \
      padding_left=4 padding_right=4 \
    --subscribe "dome.ws.$i" dome_update
done

# hidden driver: runs the plugin once a second
sketchybar --add item dome.driver left \
  --set dome.driver drawing=off \
    script="$PLUGIN_DIR/dome.sh" \
    update_freq=1
```

## YASB (Windows)

The formatter below turns the query into styled HTML, run by
a `yasb.custom.CustomWidget` once a second. Save it to
`%USERPROFILE%\.config\yasb\dome_workspaces.ps1`.

The script renders a single-row `<table>` so each workspace pill gets
proper padding and vertical centering. Adjust the hex values at the
top of the script to match your YASB theme.

```powershell
$focused_bg  = '#cba6f7'
$focused_fg  = '#1e1e2e'
$visible_bg  = '#45475a'
$visible_fg  = '#cdd6f4'
$separator_fg = '#585b70'
$pill_style   = 'padding:2px 8px;text-align:center;vertical-align:middle'
$sep_style    = 'padding:2px 3px;vertical-align:middle'

try {
    $json = dome query workspaces 2>$null | Out-String
    $data = $json | ConvertFrom-Json
} catch {
    Write-Output ""
    exit 0
}

if ($data -isnot [array]) {
    Write-Output ""
    exit 0
}

$data = $data | Sort-Object { [int]$_.name }

$cells = @()
foreach ($ws in $data) {
    if ($ws.is_focused) {
        $cells += "<td style='background:$focused_bg;color:$focused_fg;$pill_style'>$($ws.name)</td>"
    } elseif ($ws.is_visible -or $ws.window_count -gt 0) {
        $cells += "<td style='background:$visible_bg;color:$visible_fg;$pill_style'>$($ws.name)</td>"
    } else {
        continue
    }
    $cells += "<td style='color:$separator_fg;$sep_style'>|</td>"
}
# Drop the trailing separator.
if ($cells.Count -gt 0) { $cells = $cells[0..($cells.Count - 2)] }

if ($cells.Count -eq 0) {
    Write-Output ""
} else {
    Write-Output "<table cellspacing=0 cellpadding=0><tr>$($cells -join '')</tr></table>"
}
```

Merge the widget below into your `config.yaml` under `widgets`,
then add `"dome_workspaces"` to a bar's `widgets` list.

```yaml
widgets:
  dome_workspaces:
    type: "yasb.custom.CustomWidget"
    options:
      label: "{data}"
      label_alt: "{data}"
      class_name: "dome-workspaces-widget"
      exec_options:
        run_cmd: "powershell -ExecutionPolicy Bypass -File %USERPROFILE%\\.config\\yasb\\dome_workspaces.ps1"
        run_interval: 1000
        return_format: "string"
```

## Zebar (Windows)

The widget below calls the CLI through Zebar's `shellExec`
(requires Zebar 2.7.0 or later) and draws one pill per workspace.

```html
<!-- Save to %USERPROFILE%\.glzr\zebar\dome\dome-workspaces.html with a zpack.json -->
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <style>
      body {
        margin: 0;
        font-family: sans-serif;
      }
      .ws {
        display: inline-block;
        padding: 2px 6px;
        margin: 0 2px;
        border-radius: 4px;
        background: #ffffff44;
        color: #fff;
      }
      .ws.focused {
        background: #fff;
        color: #000;
      }
    </style>
  </head>
  <body>
    <div id="workspaces"></div>
    <script type="module">
      import { shellExec } from 'https://esm.sh/zebar@2';

      const container = document.getElementById('workspaces');

      async function refresh() {
        let text;
        try {
          const res = await shellExec('dome', ['query', 'workspaces']);
          text = res.stdout.trim();
        } catch {
          return;
        }

        let data;
        try {
          data = JSON.parse(text);
        } catch {
          return;
        }

        // A query timeout prints {"error":"query timed out"}, not the array.
        if (!Array.isArray(data)) return;

        container.innerHTML = '';
        for (const ws of data) {
          if (!ws.is_focused && !ws.is_visible && ws.window_count === 0) continue;
          const el = document.createElement('span');
          el.className = ws.is_focused ? 'ws focused' : 'ws';
          el.textContent =
            ws.window_count > 0 ? `${ws.name}·${ws.window_count}` : ws.name;
          container.appendChild(el);
        }
      }

      refresh();
      setInterval(refresh, 1000);
    </script>
  </body>
</html>
```
