# Integration

Dome currently ships no status bar. It provides basic integration with popular status bars
through its [query interface](cli.md#dome-query-workspaces). You can adjust these to suit
your setup.

## SketchyBar (macOS)

Copy `integrations/sketchybar/` to `~/.config/sketchybar/dome/` and add this to your
`sketchybarrc`:

```bash
eval "$("$CONFIG_DIR/dome/generate.py")"
```

## YASB (Windows)

The formatter below turns the query into styled HTML, run by
a `yasb.custom.CustomWidget` once a second. Save it to
`%USERPROFILE%\.config\yasb\dome_workspaces.ps1`.

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

Copy `integrations/zebar/` to `%USERPROFILE%\.glzr\zebar\dome\` and enable
the pack in the Zebar GUI.
