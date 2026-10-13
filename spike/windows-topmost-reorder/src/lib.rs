use serde::Serialize;

mod boundary;
pub use boundary::{LocalBoundary, Neighbor};

#[derive(Clone, Debug, Serialize)]
pub struct Reorder {
    pub sequence: u64,
    pub native_tick: u32,
    pub expanded_tick: Option<u64>,
    pub delivered_tick: u64,
    pub hwnd: usize,
    pub object: i32,
    pub child: i32,
    pub thread: u32,
}

/// Expands a native 32-bit tick near delivery, assuming less than 2^31 ms of delay.
pub fn expand_tick(native: u32, delivered: u64) -> Option<u64> {
    let difference = native.wrapping_sub(delivered as u32) as i32;
    delivered.checked_add_signed(i64::from(difference))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    ObservedTransition,
    ApiFailed,
    NoTransition,
    TargetClosed,
    SamplingFailed,
    Interrupted,
    Deadline,
    FixtureFailed,
}

/// Uses only the last collection cutoff, never time spent writing trial records.
pub fn manual_settlement(
    settled_tick: u64,
    collected_through: u64,
    stop: Option<Completion>,
) -> (Completion, u64) {
    if collected_through >= settled_tick && stop.is_none_or(|reason| reason == Completion::Deadline)
    {
        (Completion::ObservedTransition, settled_tick)
    } else {
        (stop.unwrap_or(Completion::Deadline), collected_through)
    }
}

pub fn case_adjacency_eligible(
    case: &str,
    before: (Option<u64>, Option<u64>),
    after: (Option<u64>, Option<u64>),
    before_topmost: Option<bool>,
    after_topmost: Option<bool>,
) -> bool {
    case != "lowest_topmost_unchanged_adjacency"
        || (before_topmost == Some(true)
            && after_topmost == Some(false)
            && before.0.is_some()
            && before.1.is_some()
            && before == after)
}

pub struct CaseBoundary<'a> {
    pub complete: bool,
    pub local: LocalBoundary,
    pub above: &'a [usize],
    pub ordinary: &'a [usize],
    pub ordinary_above: &'a [usize],
    pub other_topmost: &'a [usize],
}

impl CaseBoundary<'_> {
    pub fn eligible(
        &self,
        case: &str,
        desired_topmost: bool,
        before: bool,
        peer: usize,
        band_peer: usize,
    ) -> bool {
        if !self.complete {
            return false;
        }
        let target_topmost = if before {
            !desired_topmost
        } else {
            desired_topmost
        };
        match case {
            "below_ordinary_peer" => {
                let peer_above = before && desired_topmost;
                self.ordinary.contains(&peer) && self.above.contains(&peer) == peer_above
            }
            "highest_ordinary_no_other_topmost" => {
                self.ordinary_above.is_empty() && self.other_topmost.is_empty()
            }
            "alongside_fixture_topmost" => {
                self.other_topmost.contains(&band_peer)
                    && (target_topmost || self.above.contains(&band_peer))
            }
            "lowest_topmost_unchanged_adjacency" => {
                !desired_topmost && self.local.eligible(target_topmost)
            }
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "classification", rename_all = "snake_case")]
pub enum Evidence {
    NotEvaluable { reason: Completion },
    GlobalCandidatesAmbiguous { event_sequences: Vec<u64> },
    NoEventCounterexampleCandidate,
}

/// Classifies temporal evidence without asserting causality or coverage.
/// Start at the previous sample for manual edges, or the command tick for controlled edges.
pub fn correlate(
    completion: Completion,
    earliest_tick: u64,
    settled_tick: u64,
    events: &[Reorder],
) -> Evidence {
    if completion != Completion::ObservedTransition {
        return Evidence::NotEvaluable { reason: completion };
    }
    let event_sequences: Vec<_> = events
        .iter()
        .filter(|event| {
            event.expanded_tick.is_some_and(|tick| {
                tick >= earliest_tick
                    && tick <= settled_tick
                    && event.delivered_tick >= earliest_tick
                    && event.delivered_tick <= settled_tick
            })
        })
        .map(|event| event.sequence)
        .collect();
    if event_sequences.is_empty() {
        Evidence::NoEventCounterexampleCandidate
    } else {
        Evidence::GlobalCandidatesAmbiguous { event_sequences }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(native: u32, delivered: u64) -> Reorder {
        Reorder {
            sequence: 7,
            native_tick: native,
            expanded_tick: expand_tick(native, delivered),
            delivered_tick: delivered,
            hwnd: 99,
            object: -4,
            child: 0,
            thread: 12,
        }
    }

    fn ambiguous() -> Evidence {
        Evidence::GlobalCandidatesAmbiguous {
            event_sequences: vec![7],
        }
    }

    #[test]
    fn shutdown_keeps_pending_edges_at_last_collection_cutoff() {
        let collected_through = 140;
        let stop = Some(Completion::Deadline);
        assert_eq!(
            manual_settlement(120, collected_through, stop),
            (Completion::ObservedTransition, 120)
        );
        for settled_tick in [150, 180] {
            let (completion, end) = manual_settlement(settled_tick, collected_through, stop);
            assert_eq!(end, collected_through);
            assert_eq!(
                correlate(completion, 100, end, &[]),
                Evidence::NotEvaluable {
                    reason: Completion::Deadline
                }
            );
        }
        assert_eq!(
            manual_settlement(150, 150, stop),
            (Completion::ObservedTransition, 150)
        );
    }

    #[test]
    fn settle_requires_collection_and_preserves_failure_reasons() {
        assert_eq!(
            manual_settlement(150, 140, None),
            (Completion::Deadline, 140)
        );
        assert_eq!(
            manual_settlement(150, 150, None),
            (Completion::ObservedTransition, 150)
        );
        for reason in [
            Completion::Interrupted,
            Completion::TargetClosed,
            Completion::SamplingFailed,
        ] {
            assert_eq!(manual_settlement(150, 160, Some(reason)), (reason, 160));
        }
    }

    #[test]
    fn check_ordinary_peer_at_each_boundary_rejects_disappearance_band_change_or_reordering() {
        for (desired, before) in [(true, true), (true, false), (false, true), (false, false)] {
            let boundary = CaseBoundary {
                complete: true,
                local: LocalBoundary::default(),
                above: if desired && before { &[2] } else { &[] },
                ordinary: &[2],
                ordinary_above: if desired && before { &[2] } else { &[] },
                other_topmost: &[],
            };
            assert!(boundary.eligible("below_ordinary_peer", desired, before, 2, 3));
            for other_topmost in [&[][..], &[2][..]] {
                let changed = CaseBoundary {
                    ordinary: &[],
                    other_topmost,
                    ..boundary
                };
                assert!(!changed.eligible("below_ordinary_peer", desired, before, 2, 3));
            }
            let reordered = CaseBoundary {
                above: if desired && before { &[] } else { &[2] },
                ..boundary
            };
            assert!(!reordered.eligible("below_ordinary_peer", desired, before, 2, 3));
        }
    }

    #[test]
    fn check_topmost_peer_at_each_boundary_rejects_disappearance_or_band_change() {
        for (desired, before) in [(true, true), (true, false), (false, true), (false, false)] {
            let boundary = CaseBoundary {
                complete: true,
                local: LocalBoundary::default(),
                above: &[3],
                ordinary: &[2],
                ordinary_above: &[],
                other_topmost: &[3],
            };
            assert!(boundary.eligible("alongside_fixture_topmost", desired, before, 2, 3));
            for ordinary in [&[2][..], &[2, 3][..]] {
                let changed = CaseBoundary {
                    ordinary,
                    other_topmost: &[],
                    ..boundary
                };
                assert!(!changed.eligible("alongside_fixture_topmost", desired, before, 2, 3));
            }
            let peer_below = CaseBoundary {
                above: &[],
                ..boundary
            };
            assert_eq!(
                peer_below.eligible("alongside_fixture_topmost", desired, before, 2, 3),
                desired != before
            );
        }
    }

    #[test]
    fn check_highest_ordinary_at_each_boundary_rejects_competing_windows() {
        for (desired, before) in [(true, true), (true, false), (false, true), (false, false)] {
            let boundary = CaseBoundary {
                complete: true,
                local: LocalBoundary::default(),
                above: &[],
                ordinary: &[2, 3],
                ordinary_above: &[],
                other_topmost: &[],
            };
            assert!(boundary.eligible("highest_ordinary_no_other_topmost", desired, before, 2, 3));
            for changed in [
                CaseBoundary {
                    above: &[2],
                    ordinary_above: &[2],
                    ..boundary
                },
                CaseBoundary {
                    above: &[3],
                    ordinary: &[2],
                    other_topmost: &[3],
                    ..boundary
                },
            ] {
                assert!(!changed.eligible(
                    "highest_ordinary_no_other_topmost",
                    desired,
                    before,
                    2,
                    3
                ));
            }
        }
    }

    #[test]
    fn check_incomplete_boundary_rejects_case_preconditions() {
        for case in [
            "below_ordinary_peer",
            "highest_ordinary_no_other_topmost",
            "alongside_fixture_topmost",
        ] {
            let boundary = CaseBoundary {
                complete: true,
                local: LocalBoundary::default(),
                above: if case == "highest_ordinary_no_other_topmost" {
                    &[]
                } else {
                    &[2, 3]
                },
                ordinary: &[2],
                ordinary_above: &[],
                other_topmost: if case == "alongside_fixture_topmost" {
                    &[3]
                } else {
                    &[]
                },
            };
            assert!(boundary.eligible(case, true, true, 2, 3));
            let incomplete = CaseBoundary {
                complete: false,
                ..boundary
            };
            assert!(!incomplete.eligible(case, true, true, 2, 3));
        }
    }

    #[test]
    fn check_unchanged_adjacency_requires_both_real_neighbors_to_survive() {
        let case = "lowest_topmost_unchanged_adjacency";
        let original = (Some(3), Some(2));
        assert!(case_adjacency_eligible(
            case,
            original,
            original,
            Some(true),
            Some(false)
        ));
        for changed in [
            (Some(4), Some(2)),
            (Some(3), Some(5)),
            (None, Some(2)),
            (Some(3), None),
        ] {
            assert!(!case_adjacency_eligible(
                case,
                original,
                changed,
                Some(true),
                Some(false)
            ));
        }
        for missing in [(None, None), (None, Some(2)), (Some(3), None)] {
            assert!(!case_adjacency_eligible(
                case,
                missing,
                missing,
                Some(true),
                Some(false)
            ));
        }
        for (before, after) in [
            (None, Some(false)),
            (Some(true), None),
            (Some(false), Some(false)),
            (Some(true), Some(true)),
        ] {
            assert!(!case_adjacency_eligible(
                case, original, original, before, after
            ));
        }
        assert!(case_adjacency_eligible(
            "below_ordinary_peer",
            original,
            (Some(4), Some(5)),
            Some(false),
            Some(true)
        ));
    }

    #[test]
    fn correlate_before_sampled_edge_keeps_candidate() {
        assert_eq!(
            correlate(Completion::ObservedTransition, 100, 150, &[event(105, 110)]),
            ambiguous()
        );
    }

    #[test]
    fn correlate_delayed_delivery_inside_settle_keeps_candidate() {
        assert_eq!(
            correlate(Completion::ObservedTransition, 100, 150, &[event(105, 149)]),
            ambiguous()
        );
    }

    #[test]
    fn correlate_old_native_event_delivered_during_trial_excludes_background() {
        assert_eq!(
            correlate(Completion::ObservedTransition, 100, 150, &[event(90, 120)]),
            Evidence::NoEventCounterexampleCandidate
        );
    }

    #[test]
    fn correlate_future_or_late_event_excludes_background() {
        let events = [event(151, 151), event(120, 151), event(99, 99)];
        assert_eq!(
            correlate(Completion::ObservedTransition, 100, 150, &events),
            Evidence::NoEventCounterexampleCandidate
        );
    }

    #[test]
    fn correlate_no_notification_after_observation_reports_counterexample_candidate() {
        assert_eq!(
            correlate(Completion::ObservedTransition, 100, 150, &[]),
            Evidence::NoEventCounterexampleCandidate
        );
    }

    #[test]
    fn correlate_failures_with_or_without_traffic_reports_not_evaluable() {
        for reason in [
            Completion::ApiFailed,
            Completion::NoTransition,
            Completion::TargetClosed,
            Completion::SamplingFailed,
            Completion::Interrupted,
            Completion::Deadline,
            Completion::FixtureFailed,
        ] {
            assert_eq!(
                correlate(reason, 100, 150, &[]),
                Evidence::NotEvaluable { reason }
            );
            assert_eq!(
                correlate(reason, 100, 150, &[event(120, 125)]),
                Evidence::NotEvaluable { reason }
            );
        }
    }

    #[test]
    fn correlate_wraparound_keeps_pre_wrap_event_in_interval() {
        let wrap = 1_u64 << 32;
        assert_eq!(expand_tick(u32::MAX - 2, wrap + 8), Some(wrap - 3));
        assert_eq!(
            correlate(
                Completion::ObservedTransition,
                wrap - 10,
                wrap + 20,
                &[event(u32::MAX - 2, wrap + 8)]
            ),
            ambiguous()
        );
    }

    #[test]
    fn correlate_unrelated_handle_and_client_object_remains_ambiguous() {
        let mut unrelated = event(110, 120);
        unrelated.hwnd = 0;
        unrelated.object = -4;
        unrelated.child = 5;
        assert_eq!(
            correlate(Completion::ObservedTransition, 100, 150, &[unrelated]),
            ambiguous()
        );
    }
}
