CASES = ["fresh_owner_promotion_default", "fresh_owner_promotion_noownerzorder",
         "fresh_owner_after_a_default", "fresh_owner_after_a_noownerzorder", "fresh_owned_demotion"]
ROLES = ["A", "B", "P", "X", "O"]
MANIFEST = {
    "creation_order": ROLES, "class": "DomeTopmostInsertionSpike",
    "source_geometry": [160, 160, 300, 180], "overlay_geometry": [120, 160, 300, 180],
    "owner": "X", "setup_flags": 19, "initial_fixture_order": ["A", "B", "O", "X", "P"],
    "setup_after_creation": [["P", 1], ["B", -1], ["A", -1]],
    "owner_promotion": {"precreation_calls": [], "owner_topmost_at_creation": False, "treatment": ["X", -1]},
    "owner_after_a": {"precreation_calls": [["X", -1]], "owner_topmost_at_creation": True, "treatment": ["X", "A"]},
    "owned_demotion": {"precreation_calls": [["X", -1]], "owner_topmost_at_creation": True, "treatment": ["O", -2]},
    "freshness": "fresh A/B/P processes and windows, fresh X/O owning thread and windows, collector process and desktop persist",
    "comparison_limit": "matched recorded fixture history, not control of every Windows internal or external desktop condition",
}


def schedule(repetitions):
    members = []
    for environment in ("ordinary", "private"):
        for repetition in range(repetitions):
            indices = [0, 1, 2, 3, 4] if repetition % 2 == 0 else [1, 0, 3, 2, 4]
            for member_order, index in enumerate(indices):
                family = "owner_promotion" if index < 2 else "owner_after_a" if index < 4 else "owned_demotion"
                pair_id = f"{family}/{repetition}" if index == 4 else f"{environment}/{family}/{repetition}"
                members.append(dict(environment=environment, case=CASES[index], repetition=repetition,
                                    family=family, pair_id=pair_id, lifetime_id=f"{environment}/{repetition}/{CASES[index]}",
                                    planned_order=len(members), member_order=member_order,
                                    treatment_flags=531 if index in (1, 3) else 19))
    return members
