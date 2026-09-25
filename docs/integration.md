# Status bar integrations

## YASB

1. Copy `dome_workspaces.ps1` (<https://github.com/l0ngvh/Dome/blob/main/integrations/yasb/dome_workspaces.ps1>) to `%USERPROFILE%\.config\yasb\dome_workspaces.ps1`, beside the YASB config (assuming the config file is at `%USERPROFILE%\.config\yasb\config.yaml`).
2. Query the available monitors. Note each monitor's `device_name` and `gdi_device`.

   ```powershell
   dome query monitors | ConvertFrom-Json | Format-Table device_name, gdi_device
   ```

3. Add a bar and a widget to `%USERPROFILE%\.config\yasb\config.yaml`, then reload YASB.

```yaml
bars:
  dome-bar-1:
    enabled: true
    window_flags:
      windows_app_bar: true
    # Substitute with the `device_name` from `dome query monitors`
    screens: ['<DEVICE_NAME>']
    widgets:
      left: ['dome_workspaces_1']

widgets:
  dome_workspaces_1:
    type: 'yasb.custom.CustomWidget'
    options:
      label: '{data}'
      label_alt: '{data}'
      class_name: 'dome-workspaces-widget'
      exec_options:
        # Substitute with the `gdi_device` from `dome query monitors`
        run_cmd: 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\yasb\dome_workspaces.ps1 <GDI_DEVICE>'
        run_interval: 1000
        return_format: 'string'
```

Repeat for each additional monitor, for example:

```yaml
bars:
  dome-bar-1:
    enabled: true
    window_flags:
      windows_app_bar: true
    screens: ['DELL P2419H']
    widgets:
      left: ['dome_workspaces_1']
  dome-bar-2:
    enabled: true
    window_flags:
      windows_app_bar: true
    screens: ['LG HDR 4K']
    widgets:
      left: ['dome_workspaces_2']

widgets:
  dome_workspaces_1:
    type: 'yasb.custom.CustomWidget'
    options:
      label: '{data}'
      label_alt: '{data}'
      class_name: 'dome-workspaces-widget'
      exec_options:
        run_cmd: 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\yasb\dome_workspaces.ps1 \\.\DISPLAY1'
        run_interval: 1000
        return_format: 'string'
  dome_workspaces_2:
    type: 'yasb.custom.CustomWidget'
    options:
      label: '{data}'
      label_alt: '{data}'
      class_name: 'dome-workspaces-widget'
      exec_options:
        run_cmd: 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\yasb\dome_workspaces.ps1 \\.\DISPLAY2'
        run_interval: 1000
        return_format: 'string'
```

Note that the above examples does not work when the user profile path contains a space. In that case, keep the workspace script at a path with no space, for example `C:\yasb\dome_workspaces.ps1`. Then update each widget's `run_cmd` to reference that path.

## SketchyBar

Requires `jq`:

```bash
dome generate sketchybar
```

## Zebar

1. Copy the `integrations/zebar` folder (<https://github.com/l0ngvh/Dome/tree/main/integrations/zebar>) to `%USERPROFILE%\.glzr\zebar\`.
2. Enable the widget pack from the Zebar tray menu.
