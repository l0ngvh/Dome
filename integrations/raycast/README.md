# Dome Raycast extension

A Raycast extension that lists Dome's minimized windows and restores the one
you pick. It shells out to the local Dome daemon via the `dome` CLI, so both
of the following must be true before it will work:

- The Dome daemon is running.
- The `dome` binary is on `PATH`. Raycast spawns it via `execFile`, so shell
  aliases do not count. Symlink `dome` into `/usr/local/bin` or add its
  install directory to your global `PATH`.

## Install from source

The extension is not published to the Raycast store. Install it locally:

```bash
cd integrations/raycast
npm ci
npm run dev
```

`npm run dev` launches Raycast in developer mode and registers the command
"List Minimized Windows" inside the running Raycast instance. Stop the dev
server with Ctrl-C when done. The command stays installed until you uninstall
it from Raycast's Extensions preferences.

## Commands

### List Minimized Windows

Renders one row per minimized window with its title, app name, and icon. The
primary action restores the selected window and closes Raycast. If the daemon
is unreachable, the command shows a failure toast rather than an empty list.

Icons are resolved by Raycast from the app's bundle identifier, which Dome
supplies via `dome query minimized`.
