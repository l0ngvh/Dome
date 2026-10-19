use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, ensure};

use super::slug;
use crate::DomeClient;
use crate::action::MonitorDetails;

/// The self-dispatching plugin body. dome bakes its own path and the item setup
/// into it, then writes the result as `dome.sh`.
const DOME_SH: &str = include_str!("../../../../resources/integrations/sketchybar/dome.sh");

const WORKSPACES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// Query dome's connected monitors, compose `dome.sh`, source it from
/// sketchybarrc, and reload SketchyBar.
///
/// dome does not query SketchyBar, so this works before SketchyBar starts. Every
/// query runs before any write, so a failed query leaves both files untouched.
pub(crate) fn generate() -> anyhow::Result<()> {
    let dome = std::env::current_exe().context("resolve dome's own path")?;
    let dome = dome.to_string_lossy().into_owned();

    let rc = default_sketchybarrc()?;
    let dome_sh = rc
        .parent()
        .context("sketchybarrc path has no parent directory")?
        .join("dome")
        .join("dome.sh");
    let dome_sh_str = dome_sh.to_string_lossy().into_owned();

    let monitors = DomeClient
        .monitors()
        .context("query monitors (is dome running?)")?;
    let content = compose_dome_sh(&monitors, &dome, &dome_sh_str)?;
    install(&dome_sh, &rc, &content)?;
    let reloaded = run_sketchybar(&["--reload"])?.status.success();

    eprintln!(
        "dome: wrote {} and sourced it from {}.{}",
        dome_sh.display(),
        rc.display(),
        if reloaded {
            " Reloaded SketchyBar."
        } else {
            " Start SketchyBar to load it."
        }
    );
    Ok(())
}

/// A spawn failure with NotFound means SketchyBar is not installed, which is
/// fatal. A non-zero exit is tolerated, because the bar may be mid-reload.
fn run_sketchybar(args: &[&str]) -> anyhow::Result<std::process::Output> {
    std::process::Command::new("sketchybar")
        .args(args)
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                anyhow!("sketchybar not found on PATH. Install SketchyBar first.")
            }
            _ => anyhow::Error::new(e).context("run sketchybar"),
        })
}

/// Single-quote a value for the `sh -c` that SketchyBar runs a script through.
/// The one escape is `'\''` for a literal apostrophe.
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn default_sketchybarrc() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")
        .context("HOME is not set, so the sketchybarrc location cannot be resolved.")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("sketchybar")
        .join("sketchybarrc"))
}

/// Bake dome's path and the item setup into the plugin body. Refuse a drifted
/// template rather than write a `dome.sh` that still holds a placeholder.
fn compose_dome_sh(
    monitors: &[MonitorDetails],
    dome_path: &str,
    dome_sh_path: &str,
) -> anyhow::Result<String> {
    for placeholder in ["__DOME__", "__PLUGIN__", "__SETUP__"] {
        let hits = DOME_SH.matches(placeholder).count();
        ensure!(
            hits == 1,
            "dome.sh template should hold 1 {placeholder} placeholder, found {hits}"
        );
    }

    let setup = build_setup(monitors);
    Ok(DOME_SH
        .replace("__DOME__", &sh_quote(dome_path))
        .replace("__PLUGIN__", &sh_quote(dome_sh_path))
        .replace("__SETUP__", setup.trim_end()))
}

/// Monitors that get workspace items, as (unique_name, slug) pairs. Skips a
/// monitor with no name, and drops a duplicate slug so the item set stays
/// unique.
fn connected_monitors(monitors: &[MonitorDetails]) -> Vec<(&str, String)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in monitors {
        let name = m.unique_name.as_str();
        if name.is_empty() {
            continue;
        }
        let s = slug(name);
        if s.is_empty() || !seen.insert(s.clone()) {
            continue;
        }
        out.push((name, s));
    }
    out
}

/// Build the sketchybar `--add` / `--set` / `--subscribe` lines for every
/// connected monitor.
fn build_setup(monitors: &[MonitorDetails]) -> String {
    let mut out = String::new();
    out.push_str("sketchybar --add event dome_update\n");

    // The tick fills the parked popup's heading and entry rows as parked
    // workspaces appear, so nothing per-origin is baked here.
    out.push_str(
        "sketchybar --add item dome.parked left --set dome.parked drawing=off label=parked \
         click_script=\"sketchybar --set dome.parked popup.drawing=toggle\" \
         popup.align=center popup.background.drawing=on \
         popup.background.color=$POPUP_BG popup.background.corner_radius=5 \
         popup.background.border_width=1 popup.background.border_color=$POPUP_BORDER \
         background.drawing=on background.corner_radius=5 background.height=22 \
         background.border_width=1 background.border_color=$POPUP_BORDER \
         label.color=$PARKED_FG label.padding_left=8 label.padding_right=8 \
         padding_left=4 padding_right=4\n",
    );

    // Bake the numbered cells so the row keeps a fixed order. The tick adds named
    // cells lazily and resolves display= live, so a monitor reorder needs no
    // regenerate.
    let numbers = WORKSPACES.join(" ");
    for (name, s) in connected_monitors(monitors) {
        // The trailing event trigger repaints the clicked cell at once instead of
        // waiting for the next tick.
        out.push_str(&format!(
            "# dome: {name}\n\
             for n in {numbers}; do\n  \
             item=\"dome.{s}.ws.$n\"\n  \
             sketchybar --add item \"$item\" left --set \"$item\" drawing=off \
             script=\"'$PLUGIN'\" \
             click_script=\"'$DOME' focus-workspace '$n' --monitor {name_q}; \
             sketchybar --trigger dome_update\" $WORKSPACE_STYLE \
             --subscribe \"$item\" mouse.entered mouse.exited\ndone\n",
            name_q = sh_quote(name),
        ));
    }

    // One driver polls dome and repaints every monitor's items each tick. It
    // draws nothing, so it needs no display.
    out.push_str(
        "sketchybar --add item dome.driver left --set dome.driver drawing=off update_freq=1 \
         script=\"'$PLUGIN'\" --subscribe dome.driver dome_update\n",
    );

    out
}

/// Write `dome.sh` and add one `source` line to `sketchybarrc`. Backs the config
/// up once, so the pristine original survives a re-run. The source line is
/// idempotent, so a re-run does not duplicate it. A missing sketchybarrc is
/// created holding just the source line.
fn install(dome_sh: &Path, rc: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = dome_sh.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(dome_sh, content).with_context(|| format!("write {}", dome_sh.display()))?;

    let existing = match std::fs::read_to_string(rc) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("read sketchybarrc at {}", rc.display())),
    };
    let updated = insert_source_line(&existing, &dome_sh.to_string_lossy());
    if updated != existing {
        if rc.exists() {
            let mut bak = rc.as_os_str().to_owned();
            bak.push(".bak");
            let bak = PathBuf::from(bak);
            if !bak.exists() {
                std::fs::copy(rc, &bak).with_context(|| format!("back up {}", rc.display()))?;
            }
        }
        if let Some(parent) = rc.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(rc, updated).with_context(|| format!("write {}", rc.display()))?;
    }
    Ok(())
}

/// Append `source "<path>"` unless the exact line is already present.
fn insert_source_line(existing: &str, dome_sh_abs: &str) -> String {
    let line = format!("source \"{dome_sh_abs}\"");
    if existing.lines().any(|l| l.trim() == line) {
        return existing.to_string();
    }
    let mut out = existing.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&line);
    out.push('\n');
    out
}
