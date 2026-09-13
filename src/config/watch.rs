//! Live reload. A reload runs on the `notify` watcher's own thread, never on
//! the hub thread.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub(crate) fn load_or_default<T: Default>(
    path: &str,
    load: impl Fn(&str) -> anyhow::Result<T>,
) -> T {
    match load(path) {
        Ok(v) => v,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            tracing::info!(%path, "File not found, using defaults");
            T::default()
        }
        Err(e) => {
            tracing::warn!(%path, error = %format!("{e:#}"), "Failed to load, using defaults");
            T::default()
        }
    }
}

pub(crate) fn start_file_watcher(
    path: &str,
    on_event: impl Fn() + Send + 'static,
) -> anyhow::Result<RecommendedWatcher> {
    let path_buf = Path::new(path).canonicalize()?;
    let watch_dir = path_buf
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent dir"))?
        .to_owned();
    let target = path_buf.clone();
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
    let target = Path::new(path)
        .canonicalize()?
        .to_string_lossy()
        .into_owned();
    start_file_watcher(path, move || match load_fn(&target) {
        Ok(v) => {
            tracing::info!(path = %target, "File reloaded");
            on_change(v);
        }
        Err(e) => tracing::warn!(path = %target, error = %e, "Failed to reload"),
    })
}
