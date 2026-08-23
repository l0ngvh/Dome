# Dome · YASB workspaces

Per-monitor workspaces for [Dome](https://github.com/l0ngvh/Dome).

## Install

1. Copy `dome_workspaces.ps1` to `%USERPROFILE%\.config\yasb\`.
2. Generate the bars and paste the output into `config.yaml`:

   ```powershell
   .\generate.ps1 | Set-Clipboard
   ```
  
  You config should look something like:
```
  bars:
  dome-bar-aw2725dm:
    enabled: true
    alignment:
      position: top
      align: center
    animation:
      enabled: true
      duration: 300
    blur_effect:
      enabled: true
      round_corners: true
      round_corners_type: normal
      border_color: system
    window_flags:
      always_on_top: false
      windows_app_bar: true
    alignment:
      position: top
      align: center
    screens: ['AW2725DM']
    widgets:
      left: ['dome_workspaces_aw2725dm']
      center:
      - home
      right:
      - volume
      - notifications
      - power_menu
      - clock
  dome-bar-dell-se2416h:
    enabled: true
    alignment:
      position: top
      align: center
    animation:
      enabled: true
      duration: 300
    blur_effect:
      enabled: true
      round_corners: true
      round_corners_type: normal
      border_color: system
    window_flags:
      always_on_top: false
      windows_app_bar: true
    screens: ['DELL SE2416H']
    widgets:
      left: ['dome_workspaces_dell_se2416h']
      center:
      - home
      right:
      - volume
      - notifications
      - power_menu
      - clock
widgets:
  home:
    type: yasb.home.HomeWidget
    options:
      label: "<span>\uE8A9</span>"
      menu_list:
      - title: User Home
        path: '~'
      - title: Download
        path: ~\Downloads
      - title: Documents
        path: ~\Documents
      - title: Pictures
        path: ~\Pictures
      system_menu: true
      power_menu: true
      blur: true
      round_corners: true
      round_corners_type: normal
      border_color: System
      alignment: left
      offset_left: -12
  clock:
    type: yasb.clock.ClockWidget
    options:
      label: '{%H:%M}<span>{alarm}</span>'
      label_alt: '{%a, %d %b %H:%M}'
      timezones: []
      calendar:
        blur: true
        round_corners: true
        alignment: center
        direction: down
        extended: false
        show_years: true
        show_holidays: false
        show_week_numbers: true
      callbacks:
        on_left: toggle_calendar
        on_middle: toggle_label
        on_right: context_menu
  volume:
    type: yasb.volume.VolumeWidget
    options:
      label: <span>{icon}</span>
      label_alt: <span>{icon}</span>{level}
      tooltip: true
      icons:
        muted: "\uE74F"
        '10': "\uE992"
        '30': "\uE993"
        '60': "\uE994"
        '100': "\uE995"
      callbacks:
        on_left: toggle_volume_menu
        on_right: toggle_mute
      audio_menu:
        blur: true
        round_corners: true
        round_corners_type: normal
        border_color: system
        alignment: right
        direction: down
        show_apps: true
        show_app_labels: false
        show_app_icons: true
        show_apps_expanded: false
        app_icons:
          toggle_down: "\uE972"
          toggle_up: "\uE971"
  notifications:
    type: yasb.notifications.NotificationsWidget
    options:
      label: "<span>\uF2A5</span>"
      label_alt: '{count} notifications'
      hide_empty: true
      tooltip: false
      callbacks:
        on_left: toggle_notification
        on_right: do_nothing
        on_middle: do_nothing
  power_menu:
    type: yasb.power_menu.PowerMenuWidget
    options:
      label: "<span>\uE712</span>"
      uptime: true
      show_user: true
      menu_style: popup
      popup:
        blur: true
        round_corners: true
        round_corners_type: normal
        border_color: System
        alignment: right
        offset_left: 12
      profile_image_size: 64
      buttons:
        lock:
        - "\uDB80\uDF41"
        - Lock
        signout:
        - "\uDB80\uDF43"
        - Sign out
        sleep:
        - "\uDB82\uDD04"
        - Sleep
        hibernate:
        - "\uDB82\uDD01"
        - Hibernate
        restart:
        - "\uDB81\uDC53"
        - Restart
        shutdown:
        - "\uDB82\uDD06"
        - Shut Down
        cancel:
        - ''
        - Cancel
  dome_workspaces_aw2725dm:
    type: 'yasb.custom.CustomWidget'
    options:
      label: '{data}'
      label_alt: '{data}'
      class_name: 'dome-workspaces-widget'
      exec_options:
        run_cmd: 'powershell -ExecutionPolicy Bypass -File C:\Users\mamlo\.config\yasb\dome_workspaces.ps1 -Monitor aw2725dm -DomePath C:\Users\mamlo\oss\dome\target\debug\dome.exe'
        run_interval: 1000
        return_format: 'string'
  dome_workspaces_dell_se2416h:
    type: 'yasb.custom.CustomWidget'
    options:
      label: '{data}'
      label_alt: '{data}'
      class_name: 'dome-workspaces-widget'
      exec_options:
        run_cmd: 'powershell -ExecutionPolicy Bypass -File C:\Users\mamlo\.config\yasb\dome_workspaces.ps1 -Monitor dell-se2416h -DomePath C:\Users\mamlo\oss\dome\target\debug\dome.exe'
        run_interval: 1000
        return_format: 'string'
```

3. Reload YASB.

## Run from source

```powershell
$dome_src = "$env:USERPROFILE\src\dome"
& "$dome_src\integrations\yasb\generate.ps1" -DomePath "$dome_src\target\debug\dome.exe" | Set-Clipboard
```
