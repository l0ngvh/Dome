use super::*;

#[derive(Debug, Clone)]
pub(super) enum PrefTreeBuildOp {
    InsertLeaf {
        leaf_id: usize,
        title: String,
        anchor: Option<usize>,
        split: SplitMode,
    },
}

pub(super) fn generate_tree_ops_small(
    rng: &mut ChaCha8Rng,
    abort: &AtomicBool,
    max_leaves: usize,
) -> (Vec<PrefTreeBuildOp>, Vec<String>) {
    generate_tree_ops(rng, abort, max_leaves)
}

pub(super) fn random_split(rng: &mut ChaCha8Rng) -> SplitMode {
    match rng.random_range(0..3u8) {
        0 => SplitMode::Horizontal,
        1 => SplitMode::Vertical,
        _ => SplitMode::Tabbed,
    }
}

pub(super) fn generate_tree_ops(
    rng: &mut ChaCha8Rng,
    abort: &AtomicBool,
    max_leaves: usize,
) -> (Vec<PrefTreeBuildOp>, Vec<String>) {
    let mut ops = Vec::new();
    let mut titles = Vec::new();
    let leaf_target = rng.random_range(2..=max_leaves);

    if abort.load(Ordering::Relaxed) {
        return (ops, titles);
    }

    // First leaf is the implicit root.
    let root_id = 0usize;
    let title = pref_title(root_id);
    titles.push(title.clone());
    ops.push(PrefTreeBuildOp::InsertLeaf {
        leaf_id: root_id,
        title,
        anchor: None,
        split: random_split(rng),
    });

    let mut next_leaf_id = 1usize;
    // Stable anchor order is required for deterministic seeded replay.
    let mut container_ids: Vec<usize> = Vec::new();
    let mut container_counter: usize = 0;
    let mut leaves_created = 1usize;

    while leaves_created < leaf_target {
        if abort.load(Ordering::Relaxed) {
            break;
        }

        let leaf_count = next_leaf_id;
        let total_nodes = leaf_count + container_ids.len();
        let pick = rng.random_range(0..total_nodes);
        let anchor = if pick < leaf_count {
            pick
        } else {
            let ci = pick - leaf_count;
            container_ids[ci]
        };

        let leaf_id = next_leaf_id;
        next_leaf_id += 1;
        let title = pref_title(leaf_id);
        titles.push(title.clone());

        let split = random_split(rng);
        ops.push(PrefTreeBuildOp::InsertLeaf {
            leaf_id,
            title,
            anchor: Some(anchor),
            split,
        });

        if container_ids.contains(&anchor) {
        } else {
            // Generated container identifiers must stay in step with the
            // containers reconstruction creates by wrapping leaf anchors.
            let container_id = CONTAINER_BASE + container_counter;
            container_counter += 1;
            container_ids.push(container_id);
        }

        leaves_created += 1;
    }

    (ops, titles)
}

pub(super) struct ReconContainer {
    split: SplitMode,
    children: Vec<usize>,
}

pub(super) fn build_node_recursive(
    id: usize,
    leaves: &HashMap<usize, String>,
    containers: &HashMap<usize, ReconContainer>,
) -> Option<TreeLayoutNode> {
    if let Some(title) = leaves.get(&id) {
        return Some(TreeLayoutNode::Leaf(WindowMatcher {
            title: Some(title.clone()),
            ..Default::default()
        }));
    }
    if let Some(c) = containers.get(&id) {
        let children = c
            .children
            .iter()
            .filter_map(|cid| build_node_recursive(*cid, leaves, containers))
            .collect::<Vec<_>>();
        if children.is_empty() {
            return None;
        }
        return Some(TreeLayoutNode::Container {
            split: Some(c.split),
            children,
        });
    }
    None
}

pub(super) fn reconstruct_tree(ops: &[PrefTreeBuildOp]) -> Option<TreeLayoutNode> {
    let mut leaves: HashMap<usize, String> = HashMap::new();
    let mut containers: HashMap<usize, ReconContainer> = HashMap::new();
    let mut parent: HashMap<usize, Option<usize>> = HashMap::new();
    let mut root: Option<usize> = None;
    let mut container_counter: usize = 0;

    for op in ops {
        let PrefTreeBuildOp::InsertLeaf {
            leaf_id,
            title,
            anchor,
            split,
        } = op;

        match anchor {
            None => {
                if root.is_none() {
                    leaves.insert(*leaf_id, title.clone());
                    parent.insert(*leaf_id, None);
                    root = Some(*leaf_id);
                }
            }
            Some(anchor_id) => {
                let aid = *anchor_id;
                if containers.contains_key(&aid) {
                    leaves.insert(*leaf_id, title.clone());
                    containers.get_mut(&aid).unwrap().children.push(*leaf_id);
                    parent.insert(*leaf_id, Some(aid));
                } else if leaves.contains_key(&aid) {
                    let container_id = CONTAINER_BASE + container_counter;
                    container_counter += 1;
                    containers.insert(
                        container_id,
                        ReconContainer {
                            split: *split,
                            children: vec![aid, *leaf_id],
                        },
                    );
                    leaves.insert(*leaf_id, title.clone());
                    let anchor_parent = parent.get(&aid).copied().unwrap();
                    parent.insert(container_id, anchor_parent);
                    match anchor_parent {
                        None => root = Some(container_id),
                        Some(pid) => {
                            if let Some(p) = containers.get_mut(&pid)
                                && let Some(pos) = p.children.iter().position(|c| *c == aid)
                            {
                                p.children[pos] = container_id;
                            }
                        }
                    }
                    parent.insert(aid, Some(container_id));
                    parent.insert(*leaf_id, Some(container_id));
                }
            }
        }
    }

    let root_id = root?;
    build_node_recursive(root_id, &leaves, &containers)
}

pub(super) fn preferred_layout_everywhere(dome: &TestDome, op: &RecordedOp) -> PreferredLayouts {
    let (name, workspace) = preferred_workspace(op);
    let mut layouts = PreferredLayouts::default();
    for monitor in dome.monitor_names() {
        layouts.insert(&monitor, name, workspace.clone());
    }
    layouts
}

pub(super) fn preferred_workspace(op: &RecordedOp) -> (&str, PreferredWorkspace) {
    let RecordedOp::ApplyPreferredLayouts {
        workspace_name,
        strategy,
        tree_ops,
        master,
        secondary,
        columns,
        float,
        fullscreen,
    } = op
    else {
        panic!("preferred_workspace takes an ApplyPreferredLayouts op, got {op:?}");
    };
    let tiling = match strategy {
        Strategy::PartitionTree => PreferredTiling::PartitionTree {
            tree: reconstruct_tree(tree_ops),
        },
        Strategy::Master => {
            let mut layout = PreferredMaster::default();
            layout.master.children = title_matchers(master);
            layout.secondary.children = title_matchers(secondary);
            PreferredTiling::Master(layout)
        }
        Strategy::Scrolling => PreferredTiling::Scrolling(PreferredScrolling {
            columns: columns
                .iter()
                .map(|titles| ColumnConfig {
                    width: None,
                    children: title_matchers(titles),
                })
                .collect(),
            ..PreferredScrolling::default()
        }),
    };
    (
        workspace_name,
        PreferredWorkspace {
            tiling,
            float: title_matchers(float),
            fullscreen: title_matchers(fullscreen),
        },
    )
}

/// Most trees stay small so that `ApplyPreferredLayouts` ops do not take most of the
/// run time of a smoke run.
pub(super) fn sync_tree_max_leaves(rng: &mut ChaCha8Rng) -> usize {
    if rng.random_bool(0.8) {
        rng.random_range(2..=5)
    } else {
        rng.random_range(6..=PREF_TREE_MAX_LEAVES)
    }
}

pub(super) fn sync_tree_ops(op: &RecordedOp) -> Option<Vec<PrefTreeBuildOp>> {
    match op {
        RecordedOp::ApplyPreferredLayouts { tree_ops, .. } => Some(tree_ops.clone()),
        _ => None,
    }
}

pub(super) fn set_sync_tree_ops(op: &mut RecordedOp, ops: Vec<PrefTreeBuildOp>) {
    if let RecordedOp::ApplyPreferredLayouts { tree_ops, .. } = op {
        *tree_ops = ops;
    }
}
