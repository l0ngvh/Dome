pub(crate) fn should_log_once<K: Hash + ?Sized>(file: &'static str, line: u32, key: &K) -> bool {
    let mut hasher = DefaultHasher::new();
    file.hash(&mut hasher);
    line.hash(&mut hasher);
    key.hash(&mut hasher);
    SEEN.lock().unwrap().insert(hasher.finish())
}

macro_rules! trace_once {
    (key: $key:expr, $($rest:tt)*) => {{
        if $crate::log_dedup::should_log_once(file!(), line!(), &$key) {
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
        if $crate::log_dedup::should_log_once(file!(), line!(), &$key) {
            tracing::debug!($($rest)*);
        }
    }};
}
pub(crate) use debug_once;

#[expect(
    unused_macros,
    reason = "reserved for future callers, symmetric with trace_once"
)]
macro_rules! warn_once {
    (key: $key:expr, $($rest:tt)*) => {{
        if $crate::log_dedup::should_log_once(file!(), line!(), &$key) {
            tracing::warn!($($rest)*);
        }
    }};
}
pub(crate) use warn_once;

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

static SEEN: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_log_once_first_call_returns_true() {
        assert!(should_log_once("test_a.rs", 1, &(1u32, 2u32)));
    }

    #[test]
    fn should_log_once_repeat_returns_false() {
        assert!(should_log_once("test_b.rs", 1, &(1u32, 2u32)));
        assert!(!should_log_once("test_b.rs", 1, &(1u32, 2u32)));
    }

    #[test]
    fn should_log_once_distinct_lines_independent() {
        assert!(should_log_once("test_c.rs", 1, &42u32));
        assert!(should_log_once("test_c.rs", 2, &42u32));
    }

    #[test]
    fn should_log_once_distinct_keys_independent() {
        assert!(should_log_once("test_d.rs", 1, &(1u32, 2u32)));
        assert!(should_log_once("test_d.rs", 1, &(1u32, 3u32)));
        assert!(!should_log_once("test_d.rs", 1, &(1u32, 2u32)));
    }
}
