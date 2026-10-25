pub(crate) const CASES: [&str; 29] = [
    "normal_x_after_topmost_a",
    "notopmost_x_then_after_topmost_a",
    "topmost_x_then_after_topmost_a",
    "topmost_x_after_normal_a",
    "held_async_a_demotion_then_local_x_after_a",
    "normal_x_after_sole_global_topmost_a",
    "normal_x_after_sole_controlled_topmost_a",
    "normal_x_after_clean_topmost_prefix_a_with_b_above",
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
    "normal_x_to_bottom",
    "topmost_x_to_bottom",
];

pub(crate) fn scheduled_cases(environment: &str) -> Vec<&'static str> {
    CASES
        .iter()
        .enumerate()
        .filter_map(|(index, case)| {
            let number = index + 1;
            let included = match environment {
                "ordinary" => !matches!(number, 6 | 8 | 21..=29),
                "private" => matches!(number, 1 | 3 | 4 | 6 | 8 | 11 | 21..=29),
                _ => false,
            };
            included.then_some(*case)
        })
        .collect()
}

pub(crate) fn owned_by(case: &str) -> Option<&'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_coverage_includes_comparison_controls_and_every_scenario() {
        let ordinary = scheduled_cases("ordinary");
        let private = scheduled_cases("private");
        assert_eq!(ordinary.len(), 18);
        assert_eq!(private.len(), 15);
        let all: std::collections::HashSet<_> = ordinary.iter().chain(&private).collect();
        assert_eq!(all.len(), 29);
        assert_eq!(
            ordinary
                .iter()
                .filter(|case| private.contains(case))
                .count(),
            4
        );
        assert_eq!((ordinary.len() + private.len()) * 5, 165);
        assert_eq!(
            CASES.iter().filter(|case| owned_by(case).is_some()).count(),
            6
        );
    }
}
