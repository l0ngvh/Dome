//! Live reload. A callback passed to this module runs on the `notify`
//! watcher's own thread.

use anyhow::Context;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

pub(crate) fn load_or_else<T>(
    path: &str,
    load: impl Fn(&str) -> anyhow::Result<T>,
    fallback: impl Fn() -> T,
) -> T {
    match load(path) {
        Ok(v) => v,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            tracing::info!(%path, "File not found, using defaults");
            fallback()
        }
        Err(e) => {
            tracing::warn!(%path, error = %format!("{e:#}"), "Failed to load, using defaults");
            fallback()
        }
    }
}

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

pub(crate) fn start_config_watcher<T: Send + 'static>(
    path: &str,
    load_fn: impl Fn(&str) -> anyhow::Result<T> + Send + 'static,
    on_change: impl Fn(T) + Send + 'static,
) -> anyhow::Result<RecommendedWatcher> {
    // The loader gets the path as configured, the same string the initial load
    // used, so a symlinked config directory does not name the file two ways.
    let target = path.to_owned();
    start_file_watcher(path, move || match load_fn(&target) {
        Ok(v) => {
            tracing::info!(path = %target, "File reloaded");
            on_change(v);
        }
        Err(e) => tracing::warn!(path = %target, error = %format!("{e:#}"), "Failed to reload"),
    })
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
