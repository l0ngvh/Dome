#!/bin/bash
# dome SketchyBar plugin. dome writes this file. It runs two ways. Sourced from
# sketchybarrc it adds the parked popup, the numeric cells, and the driver.
# Spawned by the bar as an item script it creates named cells and paints every
# cell. Edit the colors below. A regenerate overwrites the whole file.
#
# Keep this file parseable by POSIX sh. sketchybar sources a shebang-less
# sketchybarrc under /bin/sh, which parses this whole file, so a bash array or a
# process substitution anywhere aborts sourcing with no error. No bash-only
# syntax, even inside the tick branch.

# Colors. Defaults are the Catppuccin Mocha palette.
FOCUSED_BG=0xffcba6f7
FOCUSED_FG=0xff1e1e2e
VISIBLE_BG=0xff45475a
VISIBLE_FG=0xffcdd6f4
OCCUPIED_BG=0xff313244
OCCUPIED_FG=0xffbac2de
PARKED_FG=0xfff9e2af
HEADING_FG=0xff7f849c
IDLE_BORDER=0x66585b70
HOVER_BORDER=0xffcdd6f4
POPUP_BG=0xee1e1e2e
POPUP_BORDER=0x66f9e2af

# Shared workspace-cell styling in one place. Both the baked setup and the tick
# apply it, so a cell looks the same however it was created.
WORKSPACE_STYLE="background.drawing=on background.corner_radius=5 background.height=22 background.border_width=1 background.border_color=$IDLE_BORDER label.padding_left=8 label.padding_right=8 padding_left=4 padding_right=4"

# SketchyBar spawns this script with the daemon's PATH, which usually lacks
# Homebrew, so the tick cannot find sketchybar or jq without this. Edit it if
# yours live elsewhere.
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

DOME=__DOME__
PLUGIN=__PLUGIN__

if [ -n "$SENDER" ]; then
  case "$SENDER" in
    mouse.entered)
      if [ -n "$NAME" ]; then
        sketchybar --set "$NAME" background.border_color=$HOVER_BORDER
      fi
      ;;
    mouse.exited)
      if [ -n "$NAME" ]; then
        sketchybar --set "$NAME" background.border_color=$IDLE_BORDER
      fi
      ;;
    *)
      # Tick. Create a cell the first time a workspace appears, paint every cell,
      # and hide a cell whose workspace is gone. display= is resolved live, so a
      # monitor reorder or a new monitor needs no regenerate.
      ws=$("$DOME" query workspaces 2>/dev/null)
      mons=$("$DOME" query monitors 2>/dev/null)
      disps=$(sketchybar --query displays 2>/dev/null)
      bar=$(sketchybar --query bar 2>/dev/null)
      # Each dome and display source must be a JSON array and the bar a JSON
      # object. A bad or empty result keeps prior state.
      if printf '%s' "$ws" | jq -e 'type == "array"' >/dev/null 2>&1 &&
        printf '%s' "$mons" | jq -e 'type == "array"' >/dev/null 2>&1 &&
        printf '%s' "$disps" | jq -e 'type == "array"' >/dev/null 2>&1 &&
        printf '%s' "$bar" | jq -e 'type == "object"' >/dev/null 2>&1; then
        cmd=$(jq -rn \
          --argjson ws "$ws" \
          --argjson mons "$mons" \
          --argjson disps "$disps" \
          --argjson bar "$bar" \
          --arg dome "$DOME" \
          --arg plugin "$PLUGIN" \
          --arg style "$WORKSPACE_STYLE" \
          --arg focused_bg "$FOCUSED_BG" --arg focused_fg "$FOCUSED_FG" \
          --arg visible_bg "$VISIBLE_BG" --arg visible_fg "$VISIBLE_FG" \
          --arg occupied_bg "$OCCUPIED_BG" --arg occupied_fg "$OCCUPIED_FG" \
          --arg parked_fg "$PARKED_FG" --arg heading_fg "$HEADING_FG" '
            def slug: ascii_downcase | gsub("[^a-z0-9]+"; "-") | sub("^-+"; "") | sub("-+$"; "");
            # Escape a single quote so a name is safe inside a single-quoted sh
            # word in a click_script. The program itself holds no literal quote.
            def q: gsub("\u0027"; "\u0027\\\u0027\u0027");
            ($style | split(" ")) as $style_tokens
            | ([ $bar.items[] | select(startswith("dome.") and (startswith("dome.parked.") | not) and (contains(".ws."))) ]) as $cells
            | ([ $bar.items[] | select(startswith("dome.parked.") and (startswith("dome.parked.heading.") | not)) ]) as $entries
            | ([ $bar.items[] | select(startswith("dome.parked.heading.")) ]) as $head_items
            | ( [ $disps[] | select(.["arrangement-id"] != null and .["DirectDisplayID"] != null) | { (.["DirectDisplayID"] | tostring): .["arrangement-id"] } ] | add // {} ) as $arr_by_did
            | ( reduce ( $mons[] | select(.cg_display_id != null and (.unique_name | length) > 0) ) as $m
                  ({}; ($m.unique_name | slug) as $sl | .[$sl] = $arr_by_did[$m.cg_display_id | tostring]) ) as $arr_by_slug
            | ( [ $ws[] | select(.state == "Attached" and (.monitor | slug) != "")
                  | { item: ("dome." + (.monitor | slug) + ".ws." + .name), sl: (.monitor | slug), name: .name, w: . } ] ) as $live
            | ( $live | map(.item) ) as $live_items
            | ( [ $ws[] | select(.state == "Parked" and .window_count > 0 and (.monitor | slug) != "")
                  | { o: (.monitor | slug), n: .name, origin: .monitor } ] ) as $parked
            | ( $parked | map(.o) | unique ) as $origins
            | ( $parked | map("dome.parked." + .o + "." + .n) ) as $parked_items
            | ( $origins | map("dome.parked.heading." + .) ) as $head_present
            | ( reduce $parked[] as $p ({}; .[$p.o] = $p.origin) ) as $origin_name_by_slug
            | [
                # Create each live cell the first time it appears.
                ( $live[]
                  | . as $l
                  | select(($cells | index($l.item)) | not)
                  | ( "--add", "item", $l.item, "left",
                      "--set", $l.item, ("script=\u0027" + ($plugin | q) + "\u0027"),
                      ("click_script=\u0027" + ($dome | q) + "\u0027 focus workspace \u0027" + ($l.name | q) + "\u0027 --monitor \u0027" + ($l.w.monitor | q) + "\u0027; sketchybar --trigger dome_update"),
                      $style_tokens[],
                      "--subscribe", $l.item, "mouse.entered", "mouse.exited" ) ),
                # Paint every live cell.
                ( $live[]
                  | .item as $it | .sl as $sl | .name as $nm | .w as $w
                  | ($arr_by_slug[$sl]) as $arr
                  | if $arr != null and ($w.is_visible or $w.window_count > 0)
                    then ( ( if $w.is_focused then [$focused_bg, $focused_fg]
                             elif $w.is_visible then [$visible_bg, $visible_fg]
                             else [$occupied_bg, $occupied_fg] end ) as $c
                           | ( "--set", $it, ("display=" + ($arr | tostring)), "drawing=on", ("label=" + $nm), ("background.color=" + $c[0]), ("label.color=" + $c[1]) ) )
                    else ( "--set", $it, "drawing=off" ) end ),
                # Hide a cell whose workspace is gone.
                ( $cells[] | . as $c | select(($live_items | index($c)) | not) | ( "--set", $c, "drawing=off" ) ),
                # Create and show each parked heading.
                ( $origins[] as $o
                  | ("dome.parked.heading." + $o) as $it
                  | ($origin_name_by_slug[$o] // $o) as $label
                  | ( if ($head_items | index($it)) then empty
                      else ( "--add", "item", $it, "popup.dome.parked",
                             "--set", $it, ("label=" + $label), ("label.color=" + $heading_fg), "label.padding_left=8", "label.padding_right=8", "background.drawing=off" ) end ),
                    ( "--set", $it, "drawing=on" ) ),
                # Create and show each parked entry.
                ( $parked[] as $p
                  | ("dome.parked." + $p.o + "." + $p.n) as $it
                  | ( if ($entries | index($it)) then empty
                      else ( "--add", "item", $it, "popup.dome.parked",
                             "--set", $it, ("label=" + $p.n),
                             ("click_script=\u0027" + ($dome | q) + "\u0027 focus workspace \u0027" + ($p.n | q) + "\u0027 --monitor \u0027" + ($p.origin | q) + "\u0027; sketchybar --set dome.parked popup.drawing=off; sketchybar --trigger dome_update"),
                             ("label.color=" + $parked_fg), "label.padding_left=24", "label.padding_right=8", "background.drawing=off" ) end ),
                    ( "--set", $it, "drawing=on" ) ),
                # Hide parked entries and headings no longer present.
                ( $entries[] | . as $e | select(($parked_items | index($e)) | not) | ( "--set", $e, "drawing=off" ) ),
                ( $head_items[] | . as $h | select(($head_present | index($h)) | not) | ( "--set", $h, "drawing=off" ) ),
                # Toggle the parked indicator itself.
                ( if ($origins | length) == 0 then ( "--set", "dome.parked", "drawing=off", "popup.drawing=off" ) else ( "--set", "dome.parked", "drawing=on" ) end )
              ] | .[]
          ')
        # One sketchybar call, each line one argument. A token may hold spaces (a
        # click_script), but none holds a newline, so newline to NUL is safe.
        if [ -n "$cmd" ]; then
          printf '%s' "$cmd" | tr '\n' '\0' | xargs -0 sketchybar
        fi
      fi
      ;;
  esac
  exit 0
fi

__SETUP__
