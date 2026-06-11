use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::platform::windows::external::HwndId;

// The cache only filters trace emission. check_unmanageable is re-run every
// poll so state transitions are observed on the next call. A 1h TTL produces a
// passive heartbeat: log on first occurrence, suppress for ~1h, prune evicts
// the entry, then the next cache miss re-logs. Safe at this length because the
// predicate always runs regardless of cache state.
const TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RejectionReason {
    NotVisible,
    Iconic,
    Cloaked,
    Ancestor,
    WsChild,
    Toolwindow,
    Noactivate,
    Transparent,
    OwnedNoAppWindow,
    ZeroDim,
}

pub(crate) struct RejectionLogFilter {
    entries: RwLock<HashMap<(HwndId, u32), Entry>>,
}

struct Entry {
    reason: RejectionReason,
    last_seen: Instant,
}

impl RejectionLogFilter {
    pub(crate) fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub(crate) fn record_and_should_log(
        &self,
        hwnd: HwndId,
        pid: u32,
        reason: RejectionReason,
        now: Instant,
    ) -> bool {
        if self
            .entries
            .read()
            .unwrap()
            .get(&(hwnd, pid))
            .is_some_and(|e| e.reason == reason)
        {
            return false;
        }
        self.entries.write().unwrap().insert(
            (hwnd, pid),
            Entry {
                reason,
                last_seen: now,
            },
        );
        true
    }

    pub(crate) fn prune(&self, now: Instant) {
        let mut entries = self.entries.write().unwrap();
        entries.retain(|_, e| now.duration_since(e.last_seen) <= TTL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_call_logs() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));
        assert!(!filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));
    }

    #[test]
    fn same_reason_hit_dedups_and_does_not_refresh_last_seen() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));

        let t_half = t0 + Duration::from_secs(30 * 60);
        assert!(!filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t_half));

        // Prune just past TTL from t0. If last_seen had been refreshed to
        // t_half, the entry would survive (30min + 1ms <= 1h). It does not
        // survive because the read-fast-path never writes last_seen.
        let t_expire = t0 + Duration::from_secs(60 * 60) + Duration::from_millis(1);
        filter.prune(t_expire);

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t_expire));
    }

    #[test]
    fn reason_change_relogs() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));
        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Cloaked, t0));
        assert!(!filter.record_and_should_log(h1, 100, RejectionReason::Cloaked, t0));
    }

    #[test]
    fn composite_key_does_not_collide() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));
        assert!(filter.record_and_should_log(h1, 200, RejectionReason::Iconic, t0));

        assert!(!filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));
        assert!(!filter.record_and_should_log(h1, 200, RejectionReason::Iconic, t0));
    }

    #[test]
    fn heartbeat_re_fires_after_prune() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));

        let t_expire = t0 + Duration::from_secs(60 * 60) + Duration::from_millis(1);
        filter.prune(t_expire);

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t_expire));
    }

    #[test]
    fn prune_drops_stale() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));

        let t_expire = t0 + Duration::from_secs(60 * 60) + Duration::from_millis(1);
        filter.prune(t_expire);

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t_expire));
    }

    #[test]
    fn prune_keeps_fresh() {
        let filter = RejectionLogFilter::new();
        let h1 = HwndId::test(1);
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));

        // Boundary: now - last_seen == TTL, which satisfies <= TTL
        let t_boundary = t0 + Duration::from_secs(60 * 60);
        filter.prune(t_boundary);

        assert!(!filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t_boundary));
    }

    #[test]
    fn prune_empty_is_noop() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();
        let h1 = HwndId::test(1);

        filter.prune(t0);

        assert!(filter.record_and_should_log(h1, 100, RejectionReason::Iconic, t0));
    }
}
