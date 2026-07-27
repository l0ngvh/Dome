# Dome Raycast extension

A Raycast extension that lists Dome's minimized windows and restores the one you pick. Type `dome` in Raycast to see the list, then hit Enter on a row to restore that window.

## Prerequisites

The Dome daemon must be running and Raycast must be able to find the `dome` binary. Homebrew installs work out of the box. If you installed via `cargo install` or similar, symlink `dome` into `/usr/local/bin`.

## Install from source

The extension is not on the Raycast store yet. Clone the repo and run it in Raycast's developer mode.

```bash
cd integrations/raycast
npm ci
npm run dev
```
