# FAQ

## Why didn't Dome move my window into place?

Some apps fight back when Dome tries to position them, snapping their windows
back to a different spot. To avoid an infinite tug-of-war, Dome stops retrying
after 5 placement attempts and leaves the window wherever it ended up.
Switching to another workspace and back resets the retry counter, giving Dome a
fresh chance.

## Why are some windows minimized instead of moving offscreen

Some fullscreen apps are really aggressive about staying fullscreen, so to be
safe Dome minimizes them when they go out of view instead of moving them
offscreen.

## My keybindings stopped working while gaming.

When a game is running in exclusive fullscreen on Windows, even small things
like repositioning the window can break it (the game can crash, lose
fullscreen, or get stuck trying to grab fullscreen back). To play it safe, Dome
detects exclusive fullscreen and skips any action that would touch the game's
window, including most keybindings. Tab out (Alt+Tab) or switch to borderless
fullscreen, and the keybindings will work again.

## My config changes didn't take effect

Likely a syntax error in the TOML, an unknown action name, or an unknown
modifier name. Check `dome.log` for the exact parse error.

## A random window got focused when the focused window closed

When you close a window on macOS, the system sometimes picks the next window
from the same app to focus. Dome doesn't override macOS's focus pick, so the
window that ends up focused is the one the app picked.

## Will there be Linux/Wayland support?

This is something I really want to do, but haven't been able to get into yet,
due to the sheer amount of work to build an actual Wayland compositor. But I'm
really too lazy to configure my own hyprland/Sway for my Fedora box, so I'd
better get going.
