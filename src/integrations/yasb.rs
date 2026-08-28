use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail, ensure};

use crate::DomeClient;
use crate::action::{MonitorDetails, Query};

/// The workspace plugin YASB runs each tick. dome bakes its own path in and
/// writes the result beside the config.
const DOME_WORKSPACES_PS1: &str =
    include_str!("../../resources/integrations/yasb/dome_workspaces.ps1");

/// Bake dome's path into the plugin. Refuse a drifted template rather than write
/// a plugin that still holds a placeholder.
fn bake_plugin(template: &str, dome_path: &str) -> anyhow::Result<String> {
    let hits = template.matches("__DOME__").count();
    ensure!(
        hits == 1,
        "dome_workspaces.ps1 template should hold 1 __DOME__ placeholder, found {hits}"
    );
    Ok(template.replace("__DOME__", &ps_string(dome_path)))
}

/// A single-quoted PowerShell string. The one escape is `''` for a literal
/// apostrophe, so a Windows path's backslashes pass through as written.
fn ps_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

struct DomeEntries {
    bar_lines: Vec<String>,
    widget_lines: Vec<String>,
}

/// Query monitors, write the plugin, and edit the YASB config in place. Query
/// and validate before touching disk, so a query failure never writes a partial
/// config.
pub(crate) fn generate(config_path: Option<&str>) -> anyhow::Result<()> {
    let dome = std::env::current_exe().context("resolve dome's own path")?;
    let dome = dome.to_string_lossy().into_owned();

    let config = match config_path {
        Some(p) => PathBuf::from(p),
        None => default_yasb_config()?,
    };
    let plugin = config
        .parent()
        .context("YASB config path has no parent directory")?
        .join("dome_workspaces.ps1");
    let plugin_str = plugin.to_string_lossy().into_owned();
    // YASB splits run_cmd on spaces with no quoting, so run_cmd names the plugin
    // by path. A space in that path arrives truncated at the widget. Refuse
    // rather than write a config that fails silently.
    ensure!(
        !plugin_str.contains(' '),
        "the plugin path {plugin_str:?} contains a space, which YASB run_cmd cannot handle. \
         Pass --config with a space-free directory."
    );

    let json = DomeClient
        .send_query(&Query::Monitors)
        .context("query monitors (is dome running?)")?;
    let monitors: Vec<MonitorDetails> = serde_json::from_str(&json)
        .with_context(|| format!("dome query monitors did not return a monitor array: {json}"))?;

    let generated = generate_yaml(&monitors, &plugin_str)?;
    let plugin_body = bake_plugin(DOME_WORKSPACES_PS1, &dome)?;
    let existing = std::fs::read_to_string(&config)
        .with_context(|| format!("read YASB config at {}", config.display()))?;
    let updated = splice_config(&existing, &generated);

    install(&config, &plugin, &updated, &plugin_body)?;

    eprintln!(
        "dome: rewrote {c}. Backup at {c}.bak. Wrote {p}. Copy your center and right \
         widgets from the commented-out bars into the new dome bars, then reload YASB.",
        c = config.display(),
        p = plugin.display()
    );
    Ok(())
}

/// Write the config and the plugin. Back the config up once, so the pristine
/// original survives a re-run.
fn install(
    config: &Path,
    plugin: &Path,
    config_body: &str,
    plugin_body: &str,
) -> anyhow::Result<()> {
    let mut bak = config.as_os_str().to_owned();
    bak.push(".bak");
    let bak = PathBuf::from(bak);
    // Back up once. A re-run would otherwise overwrite the pristine original
    // with an already-spliced config, losing the only recoverable copy.
    // Back up once. A re-run would otherwise overwrite the pristine original
    // with an already-spliced config, losing the only recoverable copy.
    if !bak.exists() {
        std::fs::copy(config, &bak).with_context(|| format!("back up {}", config.display()))?;
    }
    std::fs::write(config, config_body).with_context(|| format!("write {}", config.display()))?;

    if let Some(parent) = plugin.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(plugin, plugin_body).with_context(|| format!("write {}", plugin.display()))?;
    Ok(())
}

fn default_yasb_config() -> anyhow::Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .context("USERPROFILE is not set. Pass --config with the YASB config path.")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("yasb")
        .join("config.yaml"))
}

/// A single-quoted YAML scalar. The one escape is `''` for a literal apostrophe,
/// and no character inside is an indicator, so a Windows path's backslashes pass
/// through as written.
fn yaml_scalar(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn generate_yaml(monitors: &[MonitorDetails], plugin_path: &str) -> anyhow::Result<DomeEntries> {
    // Refuse a duplicate or empty slug rather than emit two bars with the same
    // key, which YAML collapses to the last, silently dropping a monitor's bar.
    let mut seen: HashMap<String, String> = HashMap::new();
    for m in monitors {
        let s = crate::integrations::slug(&m.unique_name);
        if s.is_empty() {
            bail!("monitor {:?} slugs to an empty token", m.unique_name);
        }
        if let Some(prev) = seen.get(&s) {
            bail!(
                "{:?} and {:?} both slug to {:?}. Rename one monitor.",
                m.unique_name,
                prev,
                s
            );
        }
        seen.insert(s, m.unique_name.clone());
    }

    // device_name repeats across identical panels. A repeated screens: value
    // needs Qt's (N) suffix, which YASB logs but dome cannot know here.
    let mut device_counts: HashMap<&str, u32> = HashMap::new();
    for m in monitors {
        *device_counts.entry(m.device_name.as_str()).or_insert(0) += 1;
    }

    let mut bar_lines = Vec::new();
    let mut widget_lines = Vec::new();
    for m in monitors {
        let s = crate::integrations::slug(&m.unique_name);
        let widget = format!("dome_workspaces_{}", s.replace('-', "_"));

        bar_lines.push(format!("  dome-bar-{s}:"));
        bar_lines.push("    enabled: true".to_string());
        bar_lines.push(
            "    # windows_app_bar registers the bar as a Windows appbar, so the OS reserves"
                .to_string(),
        );
        bar_lines.push(
            "    # its screen space and tiled windows do not draw under it. Each per-monitor"
                .to_string(),
        );
        bar_lines.push("    # bar needs it so every monitor reserves its own space.".to_string());
        bar_lines.push("    window_flags:".to_string());
        bar_lines.push("      windows_app_bar: true".to_string());
        bar_lines.push(format!("    screens: [{}]", yaml_scalar(&m.device_name)));
        if device_counts[m.device_name.as_str()] > 1 {
            bar_lines.push(format!(
                "    # '{}' repeats. Append Qt's (N) suffix to this screens: value",
                m.device_name
            ));
            bar_lines.push(
                "    # by hand. YASB logs the real names in a \"screen not found\" warning."
                    .to_string(),
            );
        }
        bar_lines.push("    widgets:".to_string());
        bar_lines.push(format!("      left: [{}]", yaml_scalar(&widget)));

        let run_cmd =
            format!("powershell -NoProfile -ExecutionPolicy Bypass -File {plugin_path} {s}");
        widget_lines.push(format!("  {widget}:"));
        widget_lines.push("    type: 'yasb.custom.CustomWidget'".to_string());
        widget_lines.push("    options:".to_string());
        widget_lines.push("      label: '{data}'".to_string());
        widget_lines.push("      label_alt: '{data}'".to_string());
        widget_lines.push("      class_name: 'dome-workspaces-widget'".to_string());
        widget_lines.push("      exec_options:".to_string());
        widget_lines.push(format!("        run_cmd: {}", yaml_scalar(&run_cmd)));
        widget_lines.push("        run_interval: 1000".to_string());
        widget_lines.push("        return_format: 'string'".to_string());
    }

    Ok(DomeEntries {
        bar_lines,
        widget_lines,
    })
}

/// Edit the config text. dome owns the whole bars: block, so every active bar is
/// commented out (a re-run re-exposes a center or right widget the user copied
/// in), and the fresh dome bars go in after the bars: line. The
/// dome_workspaces_* widgets carry no user customization, so they are deleted
/// and regenerated rather than commented. The edit is text-level, so every other
/// line, comments included, stays byte-for-byte.
fn splice_config(existing: &str, generated: &DomeEntries) -> String {
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines: Vec<String> = existing.split(newline).map(str::to_string).collect();

    apply_bars(&mut lines, &generated.bar_lines);
    apply_widgets(&mut lines, &generated.widget_lines);

    lines.join(newline)
}

fn apply_bars(lines: &mut Vec<String>, bar_lines: &[String]) {
    match find_top_level_key(lines, "bars") {
        Some(idx) => {
            let end = block_end(lines, idx);
            let target = block_indent(lines, idx + 1, end);
            // Drop commented dome bars left by earlier runs, so re-runs do not
            // stack them. Commenting the active bars next keeps the most recent
            // copy, which re-exposes a center or right widget the user copied in.
            let end = delete_commented_dome_bars(lines, idx + 1, end);
            comment_active_lines(lines, idx + 1, end);
            insert_lines(lines, idx + 1, &reindent(bar_lines, target));
        }
        None => append_block(lines, "bars:", bar_lines),
    }
}

fn apply_widgets(lines: &mut Vec<String>, widget_lines: &[String]) {
    match find_top_level_key(lines, "widgets") {
        Some(idx) => {
            let end = block_end(lines, idx);
            let target = block_indent(lines, idx + 1, end);
            let end = delete_dome_widget_entries(lines, idx + 1, end);
            insert_lines(lines, end, &reindent(widget_lines, target));
        }
        None => append_block(lines, "widgets:", widget_lines),
    }
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_top_level_key(line: &str) -> bool {
    !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') && !line.starts_with('#')
}

fn find_top_level_key(lines: &[String], key: &str) -> Option<usize> {
    lines
        .iter()
        .position(|l| is_top_level_key(l) && l.split(':').next() == Some(key))
}

/// The index of the first top-level key after `start`, or the line count.
fn block_end(lines: &[String], start: usize) -> usize {
    ((start + 1)..lines.len())
        .find(|&i| is_top_level_key(&lines[i]))
        .unwrap_or(lines.len())
}

/// The indent of the first active (non-blank, non-comment) line in [start, end),
/// or 2 when the block has none. Detect this before commenting or deleting.
fn block_indent(lines: &[String], start: usize, end: usize) -> usize {
    lines[start..end]
        .iter()
        .find(|l| {
            let t = l.trim_start();
            !t.is_empty() && !t.starts_with('#')
        })
        .map_or(2, |l| indent(l))
}

/// Shift each line of a generated block right from its 2-space base to `target`,
/// so dome's entries line up with the block's existing entries.
fn reindent(block: &[String], target: usize) -> Vec<String> {
    let pad = " ".repeat(target.saturating_sub(2));
    block.iter().map(|line| format!("{pad}{line}")).collect()
}

/// Comment every active (uncommented, non-blank) line in [start, end).
fn comment_active_lines(lines: &mut [String], start: usize, end: usize) {
    for line in &mut lines[start..end] {
        let trimmed = line.trim_start();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            line.insert(0, '#');
        }
    }
}

/// Delete every `dome_workspaces_*` entry in [start, end) at any indent.
/// Returns the block end shifted by the removals.
fn delete_dome_widget_entries(lines: &mut Vec<String>, start: usize, end: usize) -> usize {
    let mut i = start;
    let mut end = end;
    while i < end {
        if lines[i].trim_start().starts_with("dome_workspaces_") {
            let entry_indent = indent(&lines[i]);
            let mut j = i + 1;
            while j < end && (lines[j].trim().is_empty() || indent(&lines[j]) > entry_indent) {
                j += 1;
            }
            lines.drain(i..j);
            end -= j - i;
        } else {
            i += 1;
        }
    }
    end
}

/// Delete every commented `dome-bar-*` block in [start, end) left by an earlier
/// run, so re-runs do not accumulate stale commented bars. A commented block
/// keeps its leading `#`, so it is matched and measured after that marker.
/// Returns the block end shifted by the removals.
fn delete_commented_dome_bars(lines: &mut Vec<String>, start: usize, end: usize) -> usize {
    let mut i = start;
    let mut end = end;
    while i < end {
        let bare = lines[i].trim_start_matches('#');
        if lines[i].starts_with('#') && bare.trim_start().starts_with("dome-bar-") {
            let entry_indent = indent(bare);
            let mut j = i + 1;
            while j < end
                && (lines[j].trim().is_empty()
                    || (lines[j].starts_with('#')
                        && indent(lines[j].trim_start_matches('#')) > entry_indent))
            {
                j += 1;
            }
            lines.drain(i..j);
            end -= j - i;
        } else {
            i += 1;
        }
    }
    end
}

fn insert_lines(lines: &mut Vec<String>, at: usize, new: &[String]) {
    for (k, line) in new.iter().enumerate() {
        lines.insert(at + k, line.clone());
    }
}

fn append_block(lines: &mut Vec<String>, header: &str, entry_lines: &[String]) {
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.push(header.to_string());
    lines.extend(entry_lines.iter().cloned());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dome-yasb-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn bake_plugin_bakes_path() {
        let baked = bake_plugin(DOME_WORKSPACES_PS1, "C:\\Program Files\\dome\\dome.exe").unwrap();
        assert!(baked.contains("$Dome = 'C:\\Program Files\\dome\\dome.exe'"));
        assert!(!baked.contains("__DOME__"));
    }

    #[test]
    fn bake_plugin_rejects_template_drift() {
        assert!(bake_plugin("no placeholder here", "dome").is_err());
        assert!(bake_plugin("__DOME__ __DOME__", "dome").is_err());
    }

    fn mon(device: &str, unique: &str) -> MonitorDetails {
        MonitorDetails {
            device_name: device.to_string(),
            unique_name: unique.to_string(),
            cg_display_id: None,
            gdi_device: Some(device.to_string()),
            work_area: crate::action::MonitorFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    #[test]
    fn generate_yaml_bare_bar() {
        let g = generate_yaml(
            &[mon("DELL SE2416H", "DELL SE2416H")],
            "C:\\dome\\dome_workspaces.ps1",
        )
        .unwrap();
        let bars = g.bar_lines.join("\n");
        assert!(bars.contains("  dome-bar-dell-se2416h:"));
        assert!(bars.contains("      windows_app_bar: true"));
        assert!(bars.contains("windows_app_bar registers the bar as a Windows appbar"));
        assert!(bars.contains("    screens: ['DELL SE2416H']"));
        let widgets = g.widget_lines.join("\n");
        assert!(widgets.contains("  dome_workspaces_dell_se2416h:"));
        assert!(widgets.contains(
            "run_cmd: 'powershell -NoProfile -ExecutionPolicy Bypass -File \
             C:\\dome\\dome_workspaces.ps1 dell-se2416h'"
        ));
    }

    #[test]
    fn install_writes_config_and_plugin() {
        let dir = scratch("install");
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.yaml");
        std::fs::write(&config, "bars:\n  status-bar:\n    enabled: true\n").unwrap();
        let plugin = dir.join("dome_workspaces.ps1");
        let body = bake_plugin(DOME_WORKSPACES_PS1, "C:\\dome\\dome.exe").unwrap();

        install(&config, &plugin, "widgets: {}\n", &body).unwrap();

        assert!(dir.join("config.yaml.bak").exists(), "config backed up");
        assert_eq!(std::fs::read_to_string(&config).unwrap(), "widgets: {}\n");
        let written = std::fs::read_to_string(&plugin).unwrap();
        assert!(written.contains("$Dome = 'C:\\dome\\dome.exe'"));
        assert!(!written.contains("__DOME__"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_backs_up_original_once() {
        let dir = scratch("bak-once");
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.yaml");
        std::fs::write(&config, "ORIGINAL\n").unwrap();
        let plugin = dir.join("dome_workspaces.ps1");
        let body = bake_plugin(DOME_WORKSPACES_PS1, "C:\\dome\\dome.exe").unwrap();

        install(&config, &plugin, "FIRST\n", &body).unwrap();
        install(&config, &plugin, "SECOND\n", &body).unwrap();

        // The backup is the pristine original, not the first run's output.
        assert_eq!(
            std::fs::read_to_string(dir.join("config.yaml.bak")).unwrap(),
            "ORIGINAL\n"
        );
        assert_eq!(std::fs::read_to_string(&config).unwrap(), "SECOND\n");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn generate_yaml_duplicate_slug_errors() {
        // "DELL #1" and "dell-1" both slug to "dell-1".
        let err = generate_yaml(&[mon("A", "DELL #1"), mon("B", "dell-1")], "dome");
        assert!(err.is_err(), "colliding slugs must error");
    }

    #[test]
    fn splice_comments_existing_bars() {
        let existing = "\
bars:
  status-bar:
    enabled: true
    screens: ['*']
widgets:
  clock:
    type: yasb.clock.ClockWidget
";
        let g = generate_yaml(&[mon("DELL SE2416H", "DELL SE2416H")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert!(out.contains("#  status-bar:"), "user bar commented");
        assert!(
            out.contains("#    enabled: true"),
            "user bar body commented"
        );
        assert!(
            out.contains("  dome-bar-dell-se2416h:"),
            "dome bar inserted"
        );
        let dome_pos = out.find("dome-bar-dell-se2416h").unwrap();
        let commented_pos = out.find("#  status-bar:").unwrap();
        assert!(
            dome_pos < commented_pos,
            "dome bar sits before the commented original"
        );
    }

    #[test]
    fn splice_preserves_other_content() {
        let existing = "\
# my top comment
komorebi:
  start_menu: false
bars:
  status-bar:
    enabled: true
widgets:
  clock:
    type: yasb.clock.ClockWidget
";
        let g = generate_yaml(&[mon("AW2725DM", "AW2725DM")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert!(
            out.contains("# my top comment\nkomorebi:\n  start_menu: false\n"),
            "content outside the blocks is byte-for-byte"
        );
        assert!(
            out.contains("  clock:\n    type: yasb.clock.ClockWidget"),
            "the user's widget survives"
        );
    }

    #[test]
    fn splice_replaces_previous_dome_widgets() {
        let existing = "\
bars:
  dome-bar-old:
    enabled: true
widgets:
  clock:
    type: yasb.clock.ClockWidget
  dome_workspaces_old:
    type: 'yasb.custom.CustomWidget'
    options:
      label: '{data}'
";
        let g = generate_yaml(&[mon("AW2725DM", "AW2725DM")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert!(
            !out.contains("dome_workspaces_old"),
            "stale dome widget removed"
        );
        assert!(
            out.contains("  dome_workspaces_aw2725dm:"),
            "fresh dome widget present"
        );
        assert!(out.contains("  clock:"), "the user's widget survives");
        assert!(
            out.contains("#  dome-bar-old:"),
            "previous dome bar commented out"
        );
    }

    #[test]
    fn splice_drops_stale_commented_dome_bars() {
        // An earlier run left a commented dome bar. A re-run must delete it
        // rather than stack another commented copy.
        let existing = "\
bars:
  dome-bar-aw2725dm:
    enabled: true
    widgets:
      left: ['dome_workspaces_aw2725dm']
      right: ['clock']
#  dome-bar-stale:
#    enabled: true
#    widgets:
#      left: ['dome_workspaces_stale']
widgets:
  clock:
    type: yasb.clock.ClockWidget
";
        let g = generate_yaml(&[mon("AW2725DM", "AW2725DM")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert!(
            !out.contains("dome-bar-stale"),
            "stale commented dome bar deleted"
        );
        // One fresh active bar plus one commented copy of the just-active bar.
        assert_eq!(
            out.matches("dome-bar-aw2725dm:").count(),
            2,
            "the previous active bar is commented once, not stacked"
        );
        // The commented copy keeps the user's right widget for re-copying.
        assert!(
            out.contains("#      right: ['clock']"),
            "re-copy source preserved"
        );
    }

    #[test]
    fn splice_matches_bar_indent() {
        let existing = "\
bars:
    status-bar:
        enabled: true
widgets:
    clock:
        type: yasb.clock.ClockWidget
";
        let g = generate_yaml(&[mon("DELL SE2416H", "DELL SE2416H")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert!(
            out.contains("\n    dome-bar-dell-se2416h:"),
            "dome bar key matches the block's 4-space indent"
        );
        assert!(out.contains("#    status-bar:"), "user bar commented");
    }

    #[test]
    fn splice_matches_widget_indent() {
        let existing = "\
bars:
    status-bar:
        enabled: true
widgets:
    clock:
        type: yasb.clock.ClockWidget
";
        let g = generate_yaml(&[mon("AW2725DM", "AW2725DM")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert!(
            out.contains("\n    dome_workspaces_aw2725dm:"),
            "dome widget key matches the user's 4-space indent"
        );
        assert!(
            out.contains("\n    clock:\n        type: yasb.clock.ClockWidget"),
            "the user's widget stays active and unchanged"
        );
    }

    #[test]
    fn splice_deletes_dome_widgets_at_any_indent() {
        let existing = "\
bars:
    status-bar:
        enabled: true
widgets:
    clock:
        type: yasb.clock.ClockWidget
    dome_workspaces_aw2725dm:
        type: 'yasb.custom.CustomWidget'
        options:
            label: '{data}'
";
        let g = generate_yaml(&[mon("AW2725DM", "AW2725DM")], "dome.exe").unwrap();
        let out = splice_config(existing, &g);
        assert_eq!(
            out.matches("dome_workspaces_aw2725dm:").count(),
            1,
            "the prior 4-space dome widget is replaced, not duplicated"
        );
        assert!(
            out.contains("\n    dome_workspaces_aw2725dm:"),
            "reinserted at the matched 4-space indent"
        );
        assert!(out.contains("\n    clock:"), "the user's widget survives");
    }
}
