use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::reload::{self, Handle};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt};

use crate::config::{LogLevel, paths};

#[derive(Clone)]
pub(crate) struct FileLogger {
    handle: Option<FilterHandle>,
}

pub(crate) trait Logger {
    fn set_level(&self, level: LogLevel);
}

pub(crate) fn should_log_once<K: Hash + ?Sized>(file: &'static str, line: u32, key: &K) -> bool {
    let mut hasher = DefaultHasher::new();
    file.hash(&mut hasher);
    line.hash(&mut hasher);
    key.hash(&mut hasher);
    SEEN.lock().unwrap().insert(hasher.finish())
}

impl FileLogger {
    // Capture startup events at Info before applying the configured level,
    // unless the environment overrides filtering.
    pub(crate) fn init() -> Self {
        let (filter, handle) = match EnvFilter::try_from_default_env() {
            Ok(f) => (reload::Layer::new(f).0, None),
            Err(_) => {
                let (layer, h) = reload::Layer::new(make_filter(LogLevel::Info));
                (layer, Some(h))
            }
        };
        let log_dir = paths::log_dir();
        let file_layer = std::fs::create_dir_all(&log_dir)
            .and_then(|_| std::fs::File::create(format!("{log_dir}/dome.log")))
            .ok()
            .map(|f| {
                fmt::layer()
                    .fmt_fields(FileFields(DefaultFields::new()))
                    .with_ansi(false)
                    .with_writer(std::sync::Mutex::new(f))
            });
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .with(file_layer)
            .with(ErrorLayer::default())
            .init();
        Self { handle }
    }
}

type FilterHandle = Handle<EnvFilter, Registry>;

impl Logger for FileLogger {
    fn set_level(&self, level: LogLevel) {
        if let Some(h) = &self.handle {
            let _ = h.reload(make_filter(level));
        }
    }
}

macro_rules! trace_once {
    (key: $key:expr, $($rest:tt)*) => {{
        if $crate::logging::should_log_once(file!(), line!(), &$key) {
            tracing::trace!($($rest)*);
        }
    }};
}
pub(crate) use trace_once;

#[expect(
    unused_macros,
    reason = "reserved for future callers, symmetric with trace_once"
)]
macro_rules! debug_once {
    (key: $key:expr, $($rest:tt)*) => {{
        if $crate::logging::should_log_once(file!(), line!(), &$key) {
            tracing::debug!($($rest)*);
        }
    }};
}
pub(crate) use debug_once;

macro_rules! warn_once {
    (key: $key:expr, $($rest:tt)*) => {{
        if $crate::logging::should_log_once(file!(), line!(), &$key) {
            tracing::warn!($($rest)*);
        }
    }};
}
pub(crate) use warn_once;

static SEEN: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn make_filter(level: LogLevel) -> EnvFilter {
    EnvFilter::new(format!("off,dome={}", level.as_str()))
}

// Newtype so the file layer gets its own FormattedFields span extension,
// avoiding ANSI bleed from the stdout layer.
struct FileFields(DefaultFields);

impl<'writer> fmt::FormatFields<'writer> for FileFields {
    fn format_fields<R: tracing_subscriber::field::RecordFields>(
        &self,
        writer: fmt::format::Writer<'writer>,
        fields: R,
    ) -> std::fmt::Result {
        self.0.format_fields(writer, fields)
    }
}

#[cfg(test)]
mod tests {
    use super::should_log_once;

    #[test]
    fn log_once_suppresses_repeats_independently_by_line_and_key() {
        let file = "logging::tests::log_once_suppresses_repeats_independently_by_line_and_key";
        for (line, key) in [(1, "first"), (2, "first"), (1, "second")] {
            assert!(should_log_once(file, line, key));
            assert!(!should_log_once(file, line, key));
        }
    }
}
