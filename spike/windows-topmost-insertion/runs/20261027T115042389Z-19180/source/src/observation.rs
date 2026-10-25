use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct Observation {
    pub handle: i64,
    pub valid: bool,
    pub process: u32,
    pub thread: u32,
    pub extended_style: isize,
    pub style_read_error: Option<u32>,
    pub topmost: bool,
    pub visible: bool,
    pub above: i64,
    pub below: i64,
    pub owner: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct Neighbor {
    pub handle: i64,
    pub valid: bool,
    pub process: u32,
    pub thread: u32,
    pub above: i64,
    pub below: i64,
}

impl From<&Observation> for Neighbor {
    fn from(item: &Observation) -> Self {
        Self {
            handle: item.handle,
            valid: item.valid,
            process: item.process,
            thread: item.thread,
            above: item.above,
            below: item.below,
        }
    }
}

pub(crate) fn capture_local_neighbors(
    fixtures: &[Observation],
    mut read: impl FnMut(i64) -> Neighbor,
) -> Vec<Neighbor> {
    let mut links = HashMap::new();
    let fixture_handles: HashSet<_> = fixtures.iter().map(|item| item.handle).collect();
    for fixture in fixtures {
        for adjacent in [fixture.handle, fixture.above, fixture.below] {
            if adjacent != 0 {
                links.entry(adjacent).or_insert_with(|| read(adjacent));
            }
        }
        let mut current = fixture.below;
        for _ in 0..32 {
            if current == 0 || fixture_handles.contains(&current) {
                break;
            }
            let item = links.entry(current).or_insert_with(|| read(current));
            current = item.below;
        }
    }
    let mut neighbors: Vec<_> = links.into_values().collect();
    neighbors.sort_by_key(|item| item.handle);
    neighbors
}

pub(crate) fn validated_local(
    started_us: u128,
    finished_us: u128,
    neighbors: &[Neighbor],
    fixtures: &[Observation],
    fixture_rereads: &[Observation],
) -> Result<(), &'static str> {
    validated_local_with_ownership(
        started_us,
        finished_us,
        neighbors,
        fixtures,
        fixture_rereads,
        None,
    )
}

pub(crate) fn validated_local_with_ownership(
    started_us: u128,
    finished_us: u128,
    neighbors: &[Neighbor],
    fixtures: &[Observation],
    fixture_rereads: &[Observation],
    ownership: Option<(i64, i64)>,
) -> Result<(), &'static str> {
    if started_us > finished_us || !matches!(fixtures.len(), 4..=6) {
        return Err("invalid local time or fixture count");
    }
    if fixtures != fixture_rereads {
        return Err("fixture changed after local traversal");
    }
    let mut by_handle = HashMap::new();
    for item in neighbors {
        if item.handle == 0 || by_handle.insert(item.handle, item).is_some() {
            return Err("zero or duplicate local handle");
        }
        if !item.valid || item.process == 0 || item.thread == 0 {
            return Err("invalid neighbor identity");
        }
    }
    if neighbors.len() > fixtures.len() * 35 {
        return Err("oversized local graph");
    }
    for item in neighbors {
        if let Some(above) = by_handle.get(&item.above)
            && above.below != item.handle
        {
            return Err("nonreciprocal local links");
        }
        if let Some(below) = by_handle.get(&item.below)
            && below.above != item.handle
        {
            return Err("nonreciprocal local links");
        }
        let mut visited = HashSet::new();
        let mut current = item.handle;
        for _ in 0..=neighbors.len() {
            let Some(node) = by_handle.get(&current) else {
                break;
            };
            if !visited.insert(current) {
                return Err("cycle in local links");
            }
            current = node.below;
        }
    }
    validate_fixture_relationships(fixtures, ownership)?;
    for item in fixtures {
        if !item.valid
            || !item.visible
            || item.style_read_error.is_some()
            || item.topmost != (item.extended_style & 8 != 0)
        {
            return Err("invalid fixture state");
        }
        if by_handle.get(&item.handle).copied() != Some(&Neighbor::from(item)) {
            return Err("fixture changed during local read");
        }
        for adjacent in [item.above, item.below] {
            if adjacent != 0 && !by_handle.contains_key(&adjacent) {
                return Err("missing immediate neighbor");
            }
        }
    }
    Ok(())
}

pub(crate) fn local_above(neighbors: &[Neighbor], upper: i64, lower: i64) -> Option<bool> {
    if upper == lower {
        return Some(false);
    }
    let by_handle: HashMap<_, _> = neighbors.iter().map(|item| (item.handle, item)).collect();
    for (start, end, answer) in [(upper, lower, true), (lower, upper, false)] {
        let mut current = start;
        for _ in 0..neighbors.len() {
            let item = by_handle.get(&current)?;
            let Some(next) = by_handle.get(&item.below) else {
                break;
            };
            if next.above != current {
                return None;
            }
            if next.handle == end {
                return Some(answer);
            }
            current = next.handle;
        }
    }
    None
}

pub(crate) fn validated_order(
    enumeration_ok: bool,
    started_us: u128,
    finished_us: u128,
    raw_enumeration: &[Observation],
    fixtures: &[Observation],
) -> Result<Vec<Observation>, &'static str> {
    validated_order_with_ownership(
        enumeration_ok,
        started_us,
        finished_us,
        raw_enumeration,
        fixtures,
        None,
    )
}

pub(crate) fn validated_order_with_ownership(
    enumeration_ok: bool,
    started_us: u128,
    finished_us: u128,
    raw_enumeration: &[Observation],
    fixtures: &[Observation],
    ownership: Option<(i64, i64)>,
) -> Result<Vec<Observation>, &'static str> {
    if !enumeration_ok {
        return Err("enumeration failed");
    }
    if started_us > finished_us {
        return Err("reversed snapshot time");
    }
    if raw_enumeration.is_empty() || raw_enumeration.len() > 8192 {
        return Err("empty or oversized enumeration");
    }
    let mut by_handle = HashMap::new();
    for item in raw_enumeration {
        if item.handle == 0 || by_handle.insert(item.handle, item).is_some() {
            return Err("zero or duplicate global handle");
        }
        if !item.valid || item.style_read_error.is_some() {
            return Err("invalid window or style read");
        }
        if item.process == 0 || item.thread == 0 {
            return Err("missing window identity");
        }
        if item.topmost != (item.extended_style & 8 != 0) {
            return Err("style decoding disagrees");
        }
    }
    let heads: Vec<_> = raw_enumeration
        .iter()
        .filter(|item| item.above == 0)
        .collect();
    if heads.len() != 1
        || raw_enumeration
            .iter()
            .filter(|item| item.below == 0)
            .count()
            != 1
    {
        return Err("chain needs a single head and tail");
    }
    let mut current = heads[0].handle;
    let mut previous = 0;
    let mut visited = HashSet::new();
    let mut order = Vec::with_capacity(raw_enumeration.len());
    for _ in 0..raw_enumeration.len() {
        if current == 0 {
            break;
        }
        if !visited.insert(current) {
            return Err("cycle in neighbor links");
        }
        let item = by_handle
            .get(&current)
            .ok_or("neighbor missing from enumeration")?;
        if item.above != previous {
            return Err("nonreciprocal neighbor links");
        }
        order.push((*item).clone());
        previous = current;
        current = item.below;
    }
    if current != 0 || visited.len() != raw_enumeration.len() {
        return Err("cycle or incomplete neighbor coverage");
    }
    if !matches!(fixtures.len(), 4..=6) {
        return Err("missing fixtures");
    }
    validate_fixture_relationships(fixtures, ownership)?;
    for fixture in fixtures {
        if by_handle.get(&fixture.handle).copied() != Some(fixture) {
            return Err("fixture changed during read");
        }
    }
    Ok(order)
}

fn validate_fixture_relationships(
    fixtures: &[Observation],
    ownership: Option<(i64, i64)>,
) -> Result<(), &'static str> {
    let mut handles = HashSet::new();
    let mut processes = HashSet::new();
    let mut threads = HashSet::new();
    for item in fixtures {
        if !handles.insert(item.handle) || !item.visible {
            return Err("fixture hidden or duplicate");
        }
        if let Some((owned, owner)) = ownership.filter(|(owned, _)| *owned == item.handle) {
            let parent = fixtures
                .iter()
                .find(|entry| entry.handle == owner)
                .ok_or("missing owner")?;
            if owned == owner
                || item.owner != owner
                || item.process != parent.process
                || item.thread != parent.thread
            {
                return Err("scheduled ownership relationship differs");
            }
        } else if item.owner != 0 || !processes.insert(item.process) || !threads.insert(item.thread)
        {
            return Err("baseline fixtures not independent and unowned");
        }
    }
    if let Some((owned, _)) = ownership
        && !handles.contains(&owned)
    {
        return Err("missing scheduled owned fixture");
    }
    Ok(())
}

pub(crate) fn both_topmost_owned_pair(fixtures: &[Observation], owner: i64, owned: i64) -> bool {
    let Some(parent) = fixtures.iter().find(|item| item.handle == owner) else {
        return false;
    };
    let Some(child) = fixtures.iter().find(|item| item.handle == owned) else {
        return false;
    };
    owner != owned
        && parent.owner == 0
        && child.owner == owner
        && parent.process == child.process
        && parent.thread == child.thread
        && [parent, child].iter().all(|item| {
            item.valid
                && item.visible
                && item.style_read_error.is_none()
                && item.topmost
                && item.extended_style & 8 != 0
        })
}

pub(crate) fn clean_topmost_prefix(order: &[Observation], last: i64) -> bool {
    let Some(index) = order.iter().position(|item| item.handle == last) else {
        return false;
    };
    order[..=index].iter().all(|item| item.topmost)
        && order[index + 1..].iter().all(|item| !item.topmost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn recorded() -> (Vec<Observation>, Vec<Observation>) {
        let snapshot: Value =
            serde_json::from_str(include_str!("../tests/scrambled-snapshot.json")).unwrap();
        (
            serde_json::from_value(snapshot["global_order"].clone()).unwrap(),
            serde_json::from_value(snapshot["fixtures"].clone()).unwrap(),
        )
    }

    #[test]
    fn validate_owned_pair_with_wrong_flags_or_owner_identity_rejects() {
        let (_, fixtures) = recorded();
        let mut owner = fixtures[3].clone();
        owner.topmost = true;
        owner.extended_style |= 8;
        let mut owned = owner.clone();
        owned.handle += 1;
        owned.owner = owner.handle;
        assert!(both_topmost_owned_pair(
            &[owner.clone(), owned.clone()],
            owner.handle,
            owned.handle
        ));
        for corruption in 0..6 {
            let mut changed = owned.clone();
            match corruption {
                0 => changed.topmost = false,
                1 => changed.extended_style &= !8,
                2 => changed.owner = 0,
                3 => changed.thread += 1,
                4 => changed.process += 1,
                5 => changed.style_read_error = Some(5),
                _ => unreachable!(),
            }
            assert!(!both_topmost_owned_pair(
                &[owner.clone(), changed],
                owner.handle,
                owned.handle
            ));
        }
        owner.topmost = false;
        owner.extended_style &= !8;
        assert!(!both_topmost_owned_pair(
            &[owner.clone(), owned.clone()],
            owner.handle,
            owned.handle
        ));
    }

    #[test]
    fn check_prefix_with_normal_above_or_hidden_flag_below_rejects() {
        let (raw, fixtures) = recorded();
        let mut order = validated_order(true, 1, 2, &raw, &fixtures).unwrap();
        for (index, item) in order.iter_mut().enumerate() {
            item.topmost = index < 2;
        }
        let boundary = order[1].handle;
        assert!(clean_topmost_prefix(&order, boundary));
        order[0].topmost = false;
        assert!(!clean_topmost_prefix(&order, boundary));
        order[0].topmost = true;
        order[2].visible = false;
        order[2].topmost = true;
        assert!(!clean_topmost_prefix(&order, boundary));
    }

    #[test]
    fn validate_scheduled_owner_sharing_keeps_baseline_strict() {
        let (mut raw, mut fixtures) = recorded();
        let owner = fixtures[3].clone();
        let candidate = raw
            .iter_mut()
            .find(|item| !fixtures.iter().any(|fixture| fixture.handle == item.handle))
            .unwrap();
        candidate.owner = owner.handle;
        candidate.process = owner.process;
        candidate.thread = owner.thread;
        candidate.visible = true;
        let owned_handle = candidate.handle;
        fixtures.push(candidate.clone());
        assert!(validated_order(true, 1, 2, &raw, &fixtures).is_err());
        assert!(
            validated_order_with_ownership(
                true,
                1,
                2,
                &raw,
                &fixtures,
                Some((owned_handle, owner.handle))
            )
            .is_ok()
        );
        assert!(
            validated_order_with_ownership(
                true,
                1,
                2,
                &raw,
                &fixtures,
                Some((owned_handle, fixtures[0].handle))
            )
            .is_err()
        );
    }

    #[test]
    fn derive_order_from_recorded_neighbors_not_enumeration() {
        let (raw, fixtures) = recorded();
        let order = validated_order(true, 1, 2, &raw, &fixtures).unwrap();
        assert_ne!(raw, order);
        assert_eq!(order.len(), raw.len());
        let position = |handle| order.iter().position(|item| item.handle == handle).unwrap();
        assert_eq!(position(1770198) + 1, position(327832));
        assert_eq!(position(327832) + 1, position(328988));
        assert!(position(fixtures[0].handle) < position(fixtures[1].handle));
        assert!(fixtures[0].topmost && fixtures[1].topmost && !fixtures[3].topmost);
    }

    #[test]
    fn validate_order_with_broken_graphs_or_partial_reads_returns_error() {
        let (raw, fixtures) = recorded();
        let order = validated_order(true, 1, 2, &raw, &fixtures).unwrap();
        for corruption in 0..10 {
            let mut broken = order.clone();
            match corruption {
                0 => broken.push(broken[0].clone()),
                1 => {
                    broken.remove(1);
                }
                2 => broken[1].above = 0,
                3 => broken[1].above = broken[2].handle,
                4 => broken[1].below = broken[0].handle,
                5 => broken[1].valid = false,
                6 => broken[1].style_read_error = Some(5),
                7 => broken[1].process = 0,
                8 => broken[1].topmost = !broken[1].topmost,
                9 => {
                    let last = broken.len() - 1;
                    broken[last].below = broken[0].handle;
                    broken[0].above = broken[last].handle;
                }
                _ => unreachable!(),
            }
            assert!(
                validated_order(true, 1, 2, &broken, &fixtures).is_err(),
                "corruption {corruption}"
            );
        }
        assert!(validated_order(false, 1, 2, &raw, &fixtures).is_err());
        assert!(validated_order(true, 3, 2, &raw, &fixtures).is_err());
    }

    #[test]
    fn validate_order_with_detached_cycle_returns_error() {
        let (raw, fixtures) = recorded();
        let mut order = validated_order(true, 1, 2, &raw, &fixtures).unwrap();
        let first = order[1].handle;
        let second = order[2].handle;
        order[0].below = order[3].handle;
        order[3].above = order[0].handle;
        order[1].above = second;
        order[1].below = second;
        order[2].above = first;
        order[2].below = first;
        assert_eq!(
            validated_order(true, 1, 2, &order, &fixtures).unwrap_err(),
            "cycle or incomplete neighbor coverage"
        );
    }

    fn recorded_local() -> (Vec<Neighbor>, Vec<Observation>) {
        let (raw, fixtures) = recorded();
        let by_handle: HashMap<_, _> = raw.iter().map(|item| (item.handle, item)).collect();
        let neighbors =
            capture_local_neighbors(&fixtures, |handle| Neighbor::from(by_handle[&handle]));
        (neighbors, fixtures)
    }

    #[test]
    fn validate_local_with_recorded_paths_roundtrips_without_claiming_global_coverage() {
        let (neighbors, fixtures) = recorded_local();
        let bytes = serde_json::to_vec(&neighbors).unwrap();
        let restored: Vec<Neighbor> = serde_json::from_slice(&bytes).unwrap();
        validated_local(1, 2, &restored, &fixtures, &fixtures).unwrap();
        assert_eq!(
            local_above(&restored, fixtures[0].handle, fixtures[1].handle),
            Some(true)
        );
        assert_eq!(
            local_above(&restored, fixtures[1].handle, fixtures[0].handle),
            Some(false)
        );
        let (raw, _) = recorded();
        let subset: Vec<_> = raw
            .into_iter()
            .filter(|item| restored.iter().any(|node| node.handle == item.handle))
            .collect();
        assert!(validated_order(true, 1, 2, &subset, &fixtures).is_err());
        assert_eq!(local_above(&restored, fixtures[0].handle, i64::MAX), None);
    }

    #[test]
    fn validate_local_with_bad_identity_style_cycles_and_nonreciprocal_links_rejects() {
        let (neighbors, fixtures) = recorded_local();
        for corruption in 0..7 {
            let mut broken = neighbors.clone();
            match corruption {
                0 => broken.push(broken[0].clone()),
                1 => broken[0].valid = false,
                2 => broken[0].thread = 0,
                3 => broken[0].process = 0,
                4 => {
                    broken
                        .iter_mut()
                        .find(|item| item.handle == fixtures[0].handle)
                        .unwrap()
                        .below = -99
                }
                5 => {
                    broken.retain(|item| item.handle != fixtures[0].above);
                }
                6 => {
                    let mut first = broken[0].clone();
                    first.handle = -10;
                    first.above = -11;
                    first.below = -11;
                    let mut second = first.clone();
                    second.handle = -11;
                    second.above = -10;
                    second.below = -10;
                    broken.extend([first, second]);
                }
                _ => unreachable!(),
            }
            assert!(
                validated_local(1, 2, &broken, &fixtures, &fixtures).is_err(),
                "corruption {corruption}"
            );
        }
        for corruption in 0..7 {
            let mut changed = fixtures.clone();
            match corruption {
                0 => changed[0].style_read_error = Some(5),
                1 => changed[0].topmost = !changed[0].topmost,
                2 => changed[0].visible = false,
                3 => changed[0].owner = 3,
                4 => changed[0].process += 1,
                5 => changed[1] = changed[0].clone(),
                6 => changed[0].valid = false,
                _ => unreachable!(),
            }
            assert!(
                validated_local(1, 2, &neighbors, &changed, &changed).is_err(),
                "corruption {corruption}"
            );
        }
        assert!(validated_local(3, 2, &neighbors, &fixtures, &fixtures).is_err());
    }

    #[test]
    fn validate_local_with_full_fixture_field_changed_after_traversal_rejects() {
        let (neighbors, fixtures) = recorded_local();
        for index in 0..fixtures.len() {
            for field in 0..12 {
                let mut rereads = fixtures.clone();
                let item = &mut rereads[index];
                match field {
                    0 => item.handle += 1,
                    1 => item.valid = !item.valid,
                    2 => item.process += 1,
                    3 => item.thread += 1,
                    4 => item.extended_style ^= 16,
                    5 => item.style_read_error = Some(5),
                    6 => item.topmost = !item.topmost,
                    7 => item.visible = !item.visible,
                    8 => item.above += 1,
                    9 => item.below += 1,
                    10 => item.owner += 1,
                    11 => {
                        item.extended_style ^= 8;
                        item.topmost = !item.topmost;
                    }
                    _ => unreachable!(),
                }
                assert_eq!(
                    validated_local(1, 2, &neighbors, &fixtures, &rereads),
                    Err("fixture changed after local traversal"),
                    "fixture {index}, field {field}"
                );
            }
        }
        let mut missing = fixtures.clone();
        missing.pop();
        assert!(validated_local(1, 2, &neighbors, &fixtures, &missing).is_err());
        let mut duplicate = fixtures.clone();
        duplicate[1] = duplicate[0].clone();
        assert!(validated_local(1, 2, &neighbors, &fixtures, &duplicate).is_err());
    }

    #[test]
    fn validate_order_with_changed_or_nonindependent_fixtures_returns_error() {
        let (raw, fixtures) = recorded();
        for corruption in 0..5 {
            let mut changed = fixtures.clone();
            match corruption {
                0 => changed[0].below = 0,
                1 => changed[0].thread += 1,
                2 => changed[0].valid = false,
                3 => changed[0].style_read_error = Some(5),
                4 => changed[1] = changed[0].clone(),
                _ => unreachable!(),
            }
            assert!(validated_order(true, 1, 2, &raw, &changed).is_err());
        }
    }
}
