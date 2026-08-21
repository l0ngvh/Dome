# Dome · SketchyBar items

Per-display workspaces for [Dome](https://github.com/l0ngvh/Dome). Each display shows
only the workspaces of the monitor Dome has on it.

## Install

1. Copy this directory to `~/.config/sketchybar/dome/`.
2. Add one line to your `sketchybarrc`:

   ```bash
   eval "$("$CONFIG_DIR/dome/generate.py")"
   ```

3. Run `sketchybar --reload` to take effect.

Read and modify `generate.py` and `dome.sh` to customize the workspace display.

Requires `python3`, `jq`, and `dome` on `PATH`.

## Run from source

```bash
dome_src=~/src/dome
eval "$("$dome_src/integrations/sketchybar/generate.py" \
  --dome-path "$dome_src/target/debug/dome")"
```
