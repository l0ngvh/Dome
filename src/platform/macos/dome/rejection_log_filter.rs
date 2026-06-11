use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use objc2_core_graphics::CGWindowID;

use crate::platform::macos::accessibility::RejectionReason;

// The cache only filters trace emission. check_unmanageable runs every poll so
// state transitions are observed on the next call. A 1h TTL produces a passive
// heartbeat: log on first occurrence, suppress for ~1h, prune evicts the entry,
// then the next cache miss re-logs.
const TTL: Duration = Duration::from_secs(60 * 60);

struct Entry {
    reason: RejectionReason,
    last_seen: Instant,
}

pub(in crate::platform::macos) struct RejectionLogFilter {
    entries: RwLock<HashMap<(CGWindowID, i32), Entry>>,
}

impl RejectionLogFilter {
    pub(in crate::platform::macos) fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub(in crate::platform::macos) fn record_and_should_log(
        &self,
        cg: CGWindowID,
        pid: i32,
        reason: RejectionReason,
        now: Instant,
    ) -> bool {
        if self
            .entries
            .read()
            .unwrap()
            .get(&(cg, pid))
            .is_some_and(|e| e.reason == reason)
        {
            return false;
        }
        self.entries.write().unwrap().insert(
            (cg, pid),
            Entry {
                reason,
                last_seen: now,
            },
        );
        true
    }

    pub(in crate::platform::macos) fn prune(&self, now: Instant) {
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
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));
        assert!(!filter.record_and_should_log(1, 10, RejectionReason::Role, t0));
    }

    #[test]
    fn same_reason_hit_dedups_and_does_not_refresh_last_seen() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));

        let t_half = t0 + Duration::from_secs(30 * 60);
        assert!(!filter.record_and_should_log(1, 10, RejectionReason::Role, t_half));

        // Prune just past TTL from t0. If last_seen had been refreshed to
        // t_half, the entry would survive (30min + 1ms <= 1h). It does not
        // survive because the read-fast-path never writes last_seen.
        let t_expire = t0 + Duration::from_secs(60 * 60) + Duration::from_millis(1);
        filter.prune(t_expire);

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t_expire));
    }

    #[test]
    fn reason_change_relogs() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));
        assert!(filter.record_and_should_log(1, 10, RejectionReason::Subrole, t0));
        assert!(!filter.record_and_should_log(1, 10, RejectionReason::Subrole, t0));
    }

    #[test]
    fn composite_key_does_not_collide() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));
        assert!(filter.record_and_should_log(1, 11, RejectionReason::Role, t0));

        assert!(!filter.record_and_should_log(1, 10, RejectionReason::Role, t0));
        assert!(!filter.record_and_should_log(1, 11, RejectionReason::Role, t0));
    }

    #[test]
    fn prune_drops_stale() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));

        let t_expire = t0 + Duration::from_secs(60 * 60) + Duration::from_millis(1);
        filter.prune(t_expire);

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t_expire));
    }

    #[test]
    fn prune_keeps_fresh() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));

        let t_boundary = t0 + Duration::from_secs(60 * 60);
        filter.prune(t_boundary);

        assert!(!filter.record_and_should_log(1, 10, RejectionReason::Role, t_boundary));
    }

    #[test]
    fn prune_empty_is_noop() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        filter.prune(t0);

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));
    }

    #[test]
    fn prune_drops_only_expired_subset() {
        let filter = RejectionLogFilter::new();
        let t0 = Instant::now();

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t0));

        let t_mid = t0 + Duration::from_secs(30 * 60);
        assert!(filter.record_and_should_log(2, 20, RejectionReason::Role, t_mid));

        let t_expire = t0 + Duration::from_secs(60 * 60) + Duration::from_millis(1);
        filter.prune(t_expire);

        assert!(filter.record_and_should_log(1, 10, RejectionReason::Role, t_expire));
        assert!(!filter.record_and_should_log(2, 20, RejectionReason::Role, t_expire));
    }
}
