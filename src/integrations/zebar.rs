use std::path::PathBuf;

use anyhow::{Context, ensure};

const ZPACK_JSON: &str = include_str!("../../resources/integrations/zebar/zpack.json");
const INDEX_HTML: &str = include_str!("../../resources/integrations/zebar/workspaces/index.html");
const STYLES_CSS: &str = include_str!("../../resources/integrations/zebar/workspaces/styles.css");

/// Scaffold the example widget pack to disk. It computes nothing from daemon
/// state, so no dome need be running. It bakes dome's own path into the widget,
/// so the pack calls the binary that generated it, and overwrites its own three
/// files so a re-run refreshes that path. Other files in the pack stay.
pub(crate) fn generate(out: Option<&str>) -> anyhow::Result<()> {
    let dome = std::env::current_exe().context("resolve dome's own path")?;
    let dome = dome.to_string_lossy().into_owned();
    let (zpack, index) = bake(ZPACK_JSON, INDEX_HTML, &dome)?;

    let root = match out {
        Some(p) => PathBuf::from(p),
        None => default_pack_dir()?,
    };
    let workspaces = root.join("workspaces");
    std::fs::create_dir_all(&workspaces)
        .with_context(|| format!("create {}", workspaces.display()))?;

    for (path, body) in [
        (root.join("zpack.json"), zpack.as_str()),
        (workspaces.join("index.html"), index.as_str()),
        (workspaces.join("styles.css"), STYLES_CSS),
    ] {
        std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    }

    eprintln!(
        "dome: wrote the Zebar widget pack to {}. Enable it from the Zebar tray menu.",
        root.display()
    );
    Ok(())
}

/// Bake dome's path into the program the widget execs. Zebar checks each
/// shellExec against the zpack.json allowlist, so the three `shellExec` calls in
/// index.html and the allowlist entry must all name the same program. Refuse on
/// a drifted template rather than ship a pack that still calls a bare `dome`.
fn bake(zpack: &str, index: &str, dome: &str) -> anyhow::Result<(String, String)> {
    let calls = index.matches("shellExec('dome'").count();
    ensure!(
        calls == 3,
        "index.html should hold 3 shellExec('dome') calls, found {calls}"
    );
    let index = index.replace(
        "shellExec('dome'",
        &format!("shellExec({}", js_string(dome)),
    );

    let entry = "\"program\": \"dome\"";
    let hits = zpack.matches(entry).count();
    ensure!(
        hits == 1,
        "zpack.json should hold 1 program allowlist entry, found {hits}"
    );
    let json = serde_json::to_string(dome).context("encode dome's path as JSON")?;
    let zpack = zpack.replace(entry, &format!("\"program\": {json}"));

    Ok((zpack, index))
}

/// A single-quoted JavaScript string. Backslash escapes first, so a Windows
/// path's separators survive, then the apostrophe.
fn js_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn default_pack_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .context("USERPROFILE is not set. Pass --out with the Zebar pack directory.")?;
    Ok(PathBuf::from(home).join(".glzr").join("zebar").join("dome"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dome-zebar-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn bake_replaces_program() {
        let dome = r"C:\Program Files\dome\dome.exe";
        let (zpack, index) = bake(ZPACK_JSON, INDEX_HTML, dome).unwrap();

        assert!(zpack.contains(r#""program": "C:\\Program Files\\dome\\dome.exe""#));
        assert!(!zpack.contains(r#""program": "dome""#));
        assert!(index.contains(r"shellExec('C:\\Program Files\\dome\\dome.exe'"));
        assert!(!index.contains("shellExec('dome'"));
    }

    #[test]
    fn bake_rejects_template_drift() {
        let no_entry = ZPACK_JSON.replace(r#""program": "dome""#, r#""program": "other""#);
        assert!(bake(&no_entry, INDEX_HTML, "dome").is_err());

        let one_call = INDEX_HTML.replacen("shellExec('dome'", "shellExec('x'", 1);
        assert!(bake(ZPACK_JSON, &one_call, "dome").is_err());
    }

    #[test]
    fn generate_overwrites_in_place() {
        let root = scratch("overwrite");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("keep.txt"), "mine").unwrap();
        std::fs::write(root.join("zpack.json"), "stale").unwrap();

        generate(Some(root.to_str().unwrap())).unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("keep.txt")).unwrap(),
            "mine"
        );
        let zpack = std::fs::read_to_string(root.join("zpack.json")).unwrap();
        assert_ne!(zpack, "stale");
        assert!(!zpack.contains(r#""program": "dome""#));
        assert_eq!(
            std::fs::read_to_string(root.join("workspaces").join("styles.css")).unwrap(),
            STYLES_CSS
        );

        std::fs::remove_dir_all(&root).unwrap();
    }
}
