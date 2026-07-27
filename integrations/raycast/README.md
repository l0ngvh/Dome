# Dome Raycast extension

A Raycast extension that lists Dome's minimized windows and restores the one you pick. It shells out to the local Dome daemon via the `dome` CLI, so both of the following must be true before it will work.

- The Dome daemon is running.
- The `dome` binary is on `PATH`. Raycast spawns it via `execFile`, so shell aliases do not count. Symlink `dome` into `/usr/local/bin` or add its install directory to your global `PATH`.

## Install from release

1. Download `dome-<version>-raycast-extension.zip` from the latest Dome release at <https://github.com/l0ngvh/Dome/releases>. Use `dome-nightly-raycast-extension.zip` from the `nightly` release for the nightly channel.
2. Unzip. Move the resulting `dist/` folder to `~/.config/raycast/extensions/dome/`, renaming as you go. The final path must be `~/.config/raycast/extensions/dome/package.json`.
3. Restart Raycast. The command "List Minimized Windows" appears under the `dome` action keyword.

## Install from source

For local iteration on the extension itself:

```bash
cd integrations/raycast
npm ci
npm run dev
```

`npm run dev` launches Raycast in developer mode and registers the command inside the running Raycast instance. Stop the dev server with Ctrl-C when done. The command stays installed until you uninstall it from Raycast's Extensions preferences.

For a persistent local build without a running dev process, run `npm run build` then symlink the output into Raycast's extensions folder:

```bash
npm run build
ln -s "$(pwd)/dist" ~/.config/raycast/extensions/dome
```

## Commands

### List Minimized Windows

Renders one row per minimized window with its title, app name, and icon. The primary action restores the selected window and closes Raycast. If the daemon is unreachable, the command shows a failure toast rather than an empty list.

Icons are resolved by Raycast from the app's bundle identifier, which Dome supplies via `dome query minimized`.
