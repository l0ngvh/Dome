use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, ensure};

use super::slug;
use crate::DomeClient;
use crate::action::{MonitorDetails, Query};

/// The self-dispatching plugin body. dome bakes its own path and the item setup
/// into it, then writes the result as `dome.sh`.
const DOME_SH: &str = include_str!("../../resources/integrations/sketchybar/dome.sh");

// The number row is pre-created for every connected monitor so it keeps a fixed
// order. The tick adds a cell for any other workspace name on first sight.
const WORKSPACES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// Run the `sketchybar` CLI. A spawn failure with NotFound means SketchyBar is
/// not installed, which is fatal for every path here. A non-zero exit from a
/// running binary is tolerated, because the bar may be mid-reload.
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

/// Query dome state, write `dome.sh`, source it from `sketchybarrc`, then reload
/// if the bar is running. dome does not query SketchyBar, so this works before
/// SketchyBar starts. Every query runs before any write, so a failed query
/// leaves both files untouched.
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

    let monitors: Vec<MonitorDetails> = {
        let json = DomeClient
            .send_query(&Query::Monitors)
            .context("query monitors (is dome running?)")?;
        serde_json::from_str(&json)
            .with_context(|| format!("monitors query did not return an array: {json}"))?
    };
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

/// Build the item setup lines. Pure over the queried data, so the tests pin the
/// emitted items without SketchyBar.
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
             click_script=\"'$DOME' focus workspace '$n' --monitor {name_q}; \
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
/// up once, the way `generate yasb` does, so the pristine original survives a
/// re-run. The source line is idempotent, so a re-run does not duplicate it. A
/// missing sketchybarrc is created holding just the source line.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::MonitorFrame;

    fn mon(unique: &str, id: Option<u32>) -> MonitorDetails {
        MonitorDetails {
            device_name: unique.to_string(),
            unique_name: unique.to_string(),
            cg_display_id: id,
            gdi_device: None,
            work_area: MonitorFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("dome-sketchybar-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn compose_bakes_path_and_wires_scripts() {
        let monitors = vec![mon("DELL SE2416H", Some(3))];
        let s = compose_dome_sh(&monitors, "/opt/dome", "/opt/cfg/dome/dome.sh").unwrap();
        assert!(s.contains("DOME='/opt/dome'"));
        // The plugin path is baked so the tick can set script= on a cell it adds.
        assert!(s.contains("PLUGIN='/opt/cfg/dome/dome.sh'"));
        assert!(s.contains("sketchybar --add event dome_update"));
        assert!(s.contains("script=\"'$PLUGIN'\""));
        assert!(s.contains("--add item dome.driver left"));
        assert!(s.contains("--subscribe dome.driver dome_update"));
    }

    #[test]
    fn compose_setup_uses_color_vars_not_rust_literals() {
        let monitors = vec![mon("DELL SE2416H", Some(3))];
        let setup = build_setup(&monitors);
        assert!(setup.contains("popup.background.color=$POPUP_BG"));
        assert!(setup.contains("label.color=$PARKED_FG"));
        assert!(setup.contains("$WORKSPACE_STYLE"));
        assert!(!setup.contains("0x"), "no color literal comes from Rust");
    }

    #[test]
    fn compose_creates_items_per_monitor_without_baked_display() {
        let monitors = vec![mon("DELL SE2416H", Some(3))];
        let s = build_setup(&monitors);
        assert!(s.contains("for n in 0 1 2 3 4 5 6 7 8 9; do"));
        assert!(s.contains("item=\"dome.dell-se2416h.ws.$n\""));
        assert!(s.contains("--add item \"$item\" left"));
        // display= is not baked. The tick sets it live.
        assert!(!s.contains("display="));

        // A monitor with no name gets no loop.
        let unnamed = vec![mon("", Some(7))];
        let s2 = build_setup(&unnamed);
        assert!(!s2.contains("for n in"));
    }

    #[test]
    fn compose_loop_body_is_one_sketchybar_line() {
        let monitors = vec![mon("DELL SE2416H", Some(3))];
        let s = build_setup(&monitors);
        assert!(s.lines().any(|l| l == "for n in 0 1 2 3 4 5 6 7 8 9; do"));
        assert!(s.lines().any(|l| l == "done"));
        // The whole sketchybar invocation is one physical line, so the semicolon
        // inside click_script stays a literal and does not split it in two.
        let sb = s
            .lines()
            .find(|l| {
                l.trim_start()
                    .starts_with("sketchybar --add item \"$item\"")
            })
            .expect("loop emits a sketchybar line");
        assert!(sb.contains(
            "click_script=\"'$DOME' focus workspace '$n' --monitor 'DELL SE2416H'; \
             sketchybar --trigger dome_update\""
        ));
        assert!(
            sb.trim_end()
                .ends_with("--subscribe \"$item\" mouse.entered mouse.exited")
        );
    }

    #[test]
    fn compose_bakes_parked_container_not_rows() {
        let monitors = vec![mon("DELL SE2416H", Some(3))];
        let s = build_setup(&monitors);
        // The popup container is baked. The tick fills its heading and entry rows.
        assert!(s.contains("--add item dome.parked left"));
        assert!(!s.contains("dome.parked.heading."));
        assert!(!s.contains("--add item dome.parked.dell-se2416h"));
        // A cell click focuses by the monitor's unique name and repaints at once.
        assert!(s.contains("'$DOME' focus workspace '$n' --monitor 'DELL SE2416H'"));
        assert!(s.contains("; sketchybar --trigger dome_update"));
    }

    #[test]
    fn install_writes_dome_sh_and_sources_once() {
        let dir = scratch("install");
        std::fs::create_dir_all(&dir).unwrap();
        let rc = dir.join("sketchybarrc");
        std::fs::write(&rc, "#!/bin/bash\nsketchybar --bar height=30\n").unwrap();
        let dome_sh = dir.join("dome").join("dome.sh");
        let content = compose_dome_sh(&[], "/opt/dome", dome_sh.to_str().unwrap()).unwrap();

        install(&dome_sh, &rc, &content).unwrap();
        let written = std::fs::read_to_string(&dome_sh).unwrap();
        assert!(written.contains("DOME='/opt/dome'"));
        assert!(written.contains("sketchybar --add event dome_update"));
        assert!(dir.join("sketchybarrc.bak").exists(), "backup written");

        let src = format!("source \"{}\"", dome_sh.display());
        let rc_text = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(
            rc_text.matches(&src).count(),
            1,
            "source line inserted once"
        );

        // A re-run overwrites dome.sh and does not duplicate the source line.
        install(&dome_sh, &rc, &content).unwrap();
        let rc_again = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(
            rc_again.matches(&src).count(),
            1,
            "source line stays single"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_backs_up_sketchybarrc_once() {
        let dir = scratch("bak-once");
        std::fs::create_dir_all(&dir).unwrap();
        let rc = dir.join("sketchybarrc");
        std::fs::write(&rc, "# original\n").unwrap();
        let dome_sh = dir.join("dome").join("dome.sh");

        install(&dome_sh, &rc, "PLUGIN").unwrap();
        let bak = dir.join("sketchybarrc.bak");
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), "# original\n");

        // Drop the source line and re-run, so the second install rewrites rc and
        // reaches the backup step. The pristine original must still survive.
        std::fs::write(&rc, "# edited by hand\n").unwrap();
        install(&dome_sh, &rc, "PLUGIN2").unwrap();
        assert_eq!(
            std::fs::read_to_string(&bak).unwrap(),
            "# original\n",
            "backup stays pristine"
        );
        assert!(
            std::fs::read_to_string(&rc)
                .unwrap()
                .contains("# edited by hand"),
            "the hand edit is kept and the source line re-added"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_bootstraps_missing_sketchybarrc() {
        let dir = scratch("bootstrap");
        std::fs::create_dir_all(&dir).unwrap();
        let rc = dir.join("sketchybarrc"); // does not exist
        let dome_sh = dir.join("dome").join("dome.sh");

        install(&dome_sh, &rc, "PLUGIN").unwrap();

        assert!(dome_sh.exists(), "dome.sh written");
        let src = format!("source \"{}\"", dome_sh.display());
        let rc_text = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(
            rc_text.matches(&src).count(),
            1,
            "a fresh sketchybarrc holds the source line"
        );
        assert!(
            !dir.join("sketchybarrc.bak").exists(),
            "no backup, since there was no original"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn insert_source_line_is_idempotent() {
        let abs = "/opt/cfg/dome/dome.sh";
        let once = insert_source_line("# rc\n", abs);
        assert!(once.contains("source \"/opt/cfg/dome/dome.sh\""));
        let twice = insert_source_line(&once, abs);
        assert_eq!(once, twice, "a second insert is a no-op");
        assert_eq!(twice.matches("source \"").count(), 1);
    }
}
