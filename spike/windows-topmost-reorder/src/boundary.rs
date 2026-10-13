use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Neighbor {
    pub handle: usize,
    pub topmost: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct LocalBoundary {
    pub complete: bool,
    pub target_topmost: Option<bool>,
    pub previous: Option<Neighbor>,
    pub next: Option<Neighbor>,
}

impl LocalBoundary {
    pub fn eligible(&self, target_topmost: bool) -> bool {
        self.complete
            && self.target_topmost == Some(target_topmost)
            && self
                .previous
                .is_some_and(|neighbor| neighbor.handle != 0 && neighbor.topmost == Some(true))
            && self
                .next
                .is_some_and(|neighbor| neighbor.handle != 0 && neighbor.topmost == Some(false))
            && self.previous.map(|neighbor| neighbor.handle)
                != self.next.map(|neighbor| neighbor.handle)
    }

    pub fn at_reference(
        &self,
        reference: (Option<u64>, Option<u64>),
        target_topmost: bool,
    ) -> bool {
        self.eligible(target_topmost) && self.neighbors() == reference
    }

    pub fn neighbors(&self) -> (Option<u64>, Option<u64>) {
        (
            self.previous.map(|neighbor| neighbor.handle as u64),
            self.next.map(|neighbor| neighbor.handle as u64),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaseBoundary, case_adjacency_eligible};

    fn local(target_topmost: bool) -> LocalBoundary {
        LocalBoundary {
            complete: true,
            target_topmost: Some(target_topmost),
            previous: Some(Neighbor {
                handle: 3,
                topmost: Some(true),
            }),
            next: Some(Neighbor {
                handle: 2,
                topmost: Some(false),
            }),
        }
    }

    #[test]
    fn check_eligibility_with_hidden_topmost_below_accepts_local_boundary() {
        for before in [true, false] {
            let boundary = CaseBoundary {
                complete: true,
                above: &[8, 3],
                ordinary: &[8, 2],
                ordinary_above: &[8],
                other_topmost: &[3, 65796],
                local: local(before),
            };
            assert!(boundary.eligible("lowest_topmost_unchanged_adjacency", false, before, 2, 3));
            assert!(!boundary.eligible("lowest_topmost_unchanged_adjacency", true, before, 2, 3));
            assert!(!boundary.eligible("highest_ordinary_no_other_topmost", false, before, 2, 3));
            let incomplete = CaseBoundary {
                complete: false,
                ..boundary
            };
            assert!(!incomplete.eligible(
                "lowest_topmost_unchanged_adjacency",
                false,
                before,
                2,
                3
            ));
        }
    }

    #[test]
    fn check_eligibility_with_wrong_or_missing_neighbor_bands_rejects_boundary() {
        for target_topmost in [true, false] {
            let boundary = local(target_topmost);
            for state in [None, Some(false)] {
                let changed = LocalBoundary {
                    previous: Some(Neighbor {
                        handle: 3,
                        topmost: state,
                    }),
                    ..boundary
                };
                assert!(!changed.eligible(target_topmost));
            }
            for state in [None, Some(true)] {
                let changed = LocalBoundary {
                    next: Some(Neighbor {
                        handle: 2,
                        topmost: state,
                    }),
                    ..boundary
                };
                assert!(!changed.eligible(target_topmost));
            }
        }
    }

    #[test]
    fn check_eligibility_with_invalid_boundary_observations_rejects_boundary() {
        for target_topmost in [true, false] {
            let boundary = local(target_topmost);
            for changed in [
                LocalBoundary {
                    previous: None,
                    ..boundary
                },
                LocalBoundary {
                    next: None,
                    ..boundary
                },
                LocalBoundary {
                    target_topmost: None,
                    ..boundary
                },
                LocalBoundary {
                    target_topmost: Some(!target_topmost),
                    ..boundary
                },
                LocalBoundary {
                    complete: false,
                    ..boundary
                },
                LocalBoundary {
                    previous: Some(Neighbor {
                        handle: 0,
                        topmost: Some(true),
                    }),
                    ..boundary
                },
                LocalBoundary {
                    next: Some(Neighbor {
                        handle: 3,
                        topmost: Some(false),
                    }),
                    ..boundary
                },
            ] {
                assert!(!changed.eligible(target_topmost));
            }
        }
    }

    #[test]
    fn check_reference_after_promotion_rejects_global_last_topmost_placement() {
        let reference = local(false).neighbors();
        assert!(local(true).at_reference(reference, true));
        let misplaced = LocalBoundary {
            previous: Some(Neighbor {
                handle: 65796,
                topmost: Some(true),
            }),
            next: Some(Neighbor {
                handle: 131942,
                topmost: Some(false),
            }),
            ..local(true)
        };
        assert!(misplaced.eligible(true));
        assert!(!misplaced.at_reference(reference, true));
        assert!(!local(false).at_reference(reference, true));
        assert!(!local(true).at_reference((None, None), true));
    }

    #[test]
    fn check_adjacency_with_neighbor_movement_or_disappearance_rejects_boundary() {
        let before = local(true);
        let after = local(false);
        let eligible = |after: LocalBoundary| {
            before.eligible(true)
                && after.eligible(false)
                && case_adjacency_eligible(
                    "lowest_topmost_unchanged_adjacency",
                    before.neighbors(),
                    after.neighbors(),
                    before.target_topmost,
                    after.target_topmost,
                )
        };
        assert!(eligible(after));
        for changed in [
            LocalBoundary {
                previous: Some(Neighbor {
                    handle: 4,
                    topmost: Some(true),
                }),
                ..after
            },
            LocalBoundary {
                next: Some(Neighbor {
                    handle: 5,
                    topmost: Some(false),
                }),
                ..after
            },
            LocalBoundary {
                previous: None,
                ..after
            },
            LocalBoundary {
                next: None,
                ..after
            },
        ] {
            assert!(!eligible(changed));
        }
    }
}
