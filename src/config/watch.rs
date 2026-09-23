//! Live reload. A callback passed to this module runs on the `notify`
//! watcher's own thread.

use anyhow::Context;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

pub(crate) fn start_file_watcher(
    path: &str,
    on_event: impl Fn() + Send + 'static,
) -> anyhow::Result<RecommendedWatcher> {
    let (watch_dir, target) = watch_target(path)?;
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res
            && matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
            && event.paths.iter().any(|p| p == &target)
        {
            on_event();
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    tracing::info!(%path, "File watcher started");
    Ok(watcher)
}

/// Returns the directory to watch and the absolute file to match events
/// against. Only the directory is canonicalized, so a watcher starts for a file
/// that does not exist yet and fires on its creation.
fn watch_target(path: &str) -> anyhow::Result<(PathBuf, PathBuf)> {
    let raw = Path::new(path);
    let file_name = raw
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("{path} names no file"))?;
    let dir = match raw.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let dir = dir
        .canonicalize()
        .with_context(|| format!("resolve the directory holding {path}"))?;
    let target = dir.join(file_name);
    Ok((dir, target))
}
