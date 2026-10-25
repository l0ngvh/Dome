use serde::Serialize;

pub(crate) const CASES: [&str; 5] = [
    "fresh_owner_promotion_default",
    "fresh_owner_promotion_noownerzorder",
    "fresh_owner_after_a_default",
    "fresh_owner_after_a_noownerzorder",
    "fresh_owned_demotion",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Member {
    pub environment: &'static str,
    pub case: &'static str,
    pub repetition: u32,
    pub family: &'static str,
    pub pair_id: String,
    pub lifetime_id: String,
    pub planned_order: usize,
    pub member_order: usize,
    pub treatment_flags: u32,
}

pub(crate) fn schedule(repetitions: u32) -> Vec<Member> {
    let mut members = Vec::new();
    for environment in ["ordinary", "private"] {
        for repetition in 0..repetitions {
            let indices = if repetition % 2 == 0 {
                [0, 1, 2, 3, 4]
            } else {
                [1, 0, 3, 2, 4]
            };
            for (member_order, index) in indices.into_iter().enumerate() {
                let family = match index {
                    0 | 1 => "owner_promotion",
                    2 | 3 => "owner_after_a",
                    _ => "owned_demotion",
                };
                let pair_id = if index == 4 {
                    format!("{family}/{repetition}")
                } else {
                    format!("{environment}/{family}/{repetition}")
                };
                members.push(Member {
                    environment,
                    case: CASES[index],
                    repetition,
                    family,
                    lifetime_id: format!("{environment}/{repetition}/{}", CASES[index]),
                    pair_id,
                    planned_order: members.len(),
                    member_order,
                    treatment_flags: if matches!(index, 1 | 3) { 531 } else { 19 },
                });
            }
        }
    }
    members
}

pub(crate) fn initialize_owned<Resource, Output, Error>(
    slot: &mut Option<Resource>,
    acquire: impl FnOnce() -> Result<Resource, Error>,
    inspect: impl FnOnce(&mut Resource) -> Result<Output, Error>,
) -> Result<Output, Error> {
    assert!(
        slot.is_none(),
        "initialization cannot replace an owned resource"
    );
    *slot = Some(acquire()?);
    inspect(slot.as_mut().expect("acquired resource retained"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_owned_failed_inspection_keeps_resource_for_explicit_retirement() {
        let mut resource = None;
        let outcome = initialize_owned(
            &mut resource,
            || Ok::<_, &str>(vec!["acquired"]),
            |owned| {
                owned.push("inspection failed");
                Err::<(), _>("native metadata unavailable")
            },
        );
        assert_eq!(outcome, Err("native metadata unavailable"));
        let mut owned = resource
            .take()
            .expect("failed initialization still owns resource");
        owned.push("explicit retirement");
        assert_eq!(
            owned,
            ["acquired", "inspection failed", "explicit retirement"]
        );
    }

    #[test]
    fn initialize_owned_failed_acquisition_leaves_no_resource_and_never_inspects() {
        let mut resource = None::<()>;
        let outcome = initialize_owned(
            &mut resource,
            || Err::<(), _>("create failed"),
            |_| -> Result<(), &str> { panic!("inspection cannot run after failed acquisition") },
        );
        assert_eq!(outcome, Err("create failed"));
        assert!(resource.is_none());
    }

    #[test]
    fn initialize_owned_successful_inspection_retains_resource_until_explicit_retirement() {
        let mut resource = None;
        let metadata =
            initialize_owned(&mut resource, || Ok::<_, &str>(42), |owned| Ok(*owned + 1));
        assert_eq!(metadata, Ok(43));
        assert_eq!(resource.take(), Some(42));
    }

    #[test]
    fn schedule_repetitions_covers_fresh_pairs_and_alternates_flag_order() {
        for repetitions in 1..=20 {
            let members = schedule(repetitions);
            assert_eq!(members.len(), 10 * repetitions as usize);
            let mut pairs = std::collections::BTreeMap::<_, Vec<_>>::new();
            let mut lifetimes = std::collections::HashSet::new();
            for (index, member) in members.iter().enumerate() {
                assert_eq!(member.planned_order, index);
                assert!(lifetimes.insert(&member.lifetime_id));
                pairs.entry(&member.pair_id).or_default().push(member);
            }
            assert_eq!(pairs.len(), 5 * repetitions as usize);
            for pair in pairs.values() {
                assert_eq!(pair.len(), 2);
                assert_eq!(pair[0].repetition, pair[1].repetition);
                if pair[0].family == "owned_demotion" {
                    assert_ne!(pair[0].environment, pair[1].environment);
                    assert_eq!(pair[0].case, pair[1].case);
                } else {
                    assert_eq!(pair[0].environment, pair[1].environment);
                    assert_eq!(
                        (pair[0].treatment_flags, pair[1].treatment_flags),
                        if pair[0].repetition % 2 == 0 {
                            (19, 531)
                        } else {
                            (531, 19)
                        }
                    );
                }
            }
        }
    }
}
