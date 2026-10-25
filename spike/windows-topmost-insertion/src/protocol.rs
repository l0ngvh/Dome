pub(crate) const CASES: [&str; 36] = [
    "normal_x_after_topmost_a",
    "notopmost_x_then_after_topmost_a",
    "topmost_x_then_after_topmost_a",
    "topmost_x_after_normal_a",
    "held_async_a_demotion_then_local_x_after_a",
    "normal_x_after_sole_global_topmost_a",
    "normal_x_after_sole_controlled_topmost_a",
    "normal_x_after_clean_topmost_prefix_a_with_b_above_displaced_v6",
    "already_topmost_x_after_topmost_a",
    "demoted_topmost_x_then_after_topmost_a",
    "normal_x_after_normal_reference",
    "repeat_x_after_topmost_a",
    "wait_for_async_a_demotion_then_x_after_a",
    "held_async_a_promotion_then_local_x_after_a",
    "wait_for_async_a_promotion_then_x_after_a",
    "held_c_after_observed_normal_a_then_promote_a",
    "held_c_after_observed_normal_a_keep_a_normal",
    "held_c_after_observed_normal_a_promote_then_demote_a",
    "topmost_a_after_normal_x_below_overlay_control",
    "topmost_a_after_normal_predecessor_of_x",
    "topmost_a_after_topmost_predecessor_of_x",
    "owner_x_promotion_promotes_owned_o",
    "owner_x_demotion_demotes_owned_o",
    "owned_o_promotion_keeps_owner_x_normal",
    "owned_o_demotion_demotes_owner_x",
    "topmost_x_then_after_a_with_x_owned_o",
    "topmost_a_after_x_with_a_owned_o",
    "normal_x_displaced_to_bottom",
    "topmost_x_to_bottom",
    "owner_x_demotion_owner_topmost_before_o_creation",
    "owned_o_demotion_owner_topmost_before_o_creation",
    "topmost_x_after_a_owner_topmost_before_o_creation",
    "owner_x_demotion_owner_then_owned_topmost",
    "owner_x_demotion_owned_then_owner_topmost",
    "owner_x_promotion_noownerzorder",
    "topmost_x_then_after_a_with_x_owned_o_noownerzorder",
];

#[cfg(test)]
fn scheduled_cases(environment: &str) -> Vec<&'static str> {
    CASES
        .iter()
        .enumerate()
        .filter_map(|(index, case)| {
            let number = index + 1;
            let included = match environment {
                "ordinary" => !matches!(number, 6 | 8 | 21 | 23..=25 | 27..=29),
                "private" => matches!(number, 1 | 3 | 4 | 6 | 8 | 11 | 21..=36),
                _ => false,
            };
            included.then_some(*case)
        })
        .collect()
}

pub(crate) fn owned_by(case: &str) -> Option<&'static str> {
    if CASES[29..].contains(&case) {
        return Some("X");
    }
    match case {
        "topmost_a_after_x_with_a_owned_o" => Some("A"),
        "owner_x_promotion_promotes_owned_o"
        | "owner_x_demotion_demotes_owned_o"
        | "owned_o_promotion_keeps_owner_x_normal"
        | "owned_o_demotion_demotes_owner_x"
        | "topmost_x_then_after_a_with_x_owned_o" => Some("X"),
        _ => None,
    }
}

pub(crate) fn owner_before_creation(case: &str) -> bool {
    CASES[29..32].contains(&case)
}

pub(crate) fn both_topmost_setup(case: &str) -> bool {
    CASES[29..34].contains(&case)
}

pub(crate) fn no_owner_order(case: &str) -> bool {
    CASES[34..36].contains(&case)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_coverage_includes_comparison_controls_and_every_scenario() {
        for case in CASES {
            assert!(!owner_before_creation(case) || both_topmost_setup(case));
            assert!(!both_topmost_setup(case) || owned_by(case) == Some("X"));
            assert!(!no_owner_order(case) || !both_topmost_setup(case));
        }
        let ordinary = scheduled_cases("ordinary");
        let private = scheduled_cases("private");
        assert_eq!(ordinary.len(), 27);
        assert_eq!(private.len(), 22);
        let all: std::collections::HashSet<_> = ordinary.iter().chain(&private).collect();
        assert_eq!(all.len(), 36);
        assert_eq!(
            ordinary
                .iter()
                .filter(|case| private.contains(case))
                .count(),
            13
        );
        assert_eq!((ordinary.len() + private.len()) * 5, 245);
        assert_eq!(
            CASES.iter().filter(|case| owned_by(case).is_some()).count(),
            13
        );
    }
}
