//! Per-run op distribution for the smoke test. A run splits its ops into
//! stretches. Each stretch draws its op kinds from the weights of one usage
//! pattern.

use std::fmt;

use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::{ALL_OP_KINDS, OpKind};

const MIN_MEAN_STRETCH_LENGTH: f64 = 20.0;
const MAX_MEAN_STRETCH_LENGTH: f64 = 2000.0;
const MIN_WEIGHT_MULTIPLIER: f64 = 0.25;
const MAX_WEIGHT_MULTIPLIER: f64 = 4.0;

/// `ChaCha8Rng::seed_from_u64` starts on stream 0. Any other stream of the same
/// seed gives an unrelated sequence.
const WORKLOAD_STREAM: u64 = 1;

/// Extra inserts give the new config rules new windows to act on.
const CONFIG_CHANGES_INSERT_TILING_SHARE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pattern {
    NormalUse,
    ConfigChanges,
    DockAndUndock,
    WindowStorm,
    Random,
}

const ALL_PATTERNS: [Pattern; 5] = [
    Pattern::NormalUse,
    Pattern::ConfigChanges,
    Pattern::DockAndUndock,
    Pattern::WindowStorm,
    Pattern::Random,
];

impl Pattern {
    fn name(self) -> &'static str {
        match self {
            Pattern::NormalUse => "normal use",
            Pattern::ConfigChanges => "config changes",
            Pattern::DockAndUndock => "dock and undock",
            Pattern::WindowStorm => "window storm",
            Pattern::Random => "random",
        }
    }
}

pub(super) struct Workload {
    mean_stretch_length: f64,
    stretches: Vec<Stretch>,
    /// One entry per pattern, in `ALL_PATTERNS` order, over `ALL_OP_KINDS`.
    distributions: Vec<WeightedIndex<f64>>,
}

impl Workload {
    pub(super) fn for_seed(seed: u64, ops_per_run: usize) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(WORKLOAD_STREAM);
        let mean_stretch_length =
            log_uniform(&mut rng, MIN_MEAN_STRETCH_LENGTH, MAX_MEAN_STRETCH_LENGTH);
        let distributions = ALL_PATTERNS
            .iter()
            .map(|&pattern| pattern_distribution(&mut rng, pattern))
            .collect();
        let stretches = draw_stretches(&mut rng, ops_per_run, mean_stretch_length);
        Self {
            mean_stretch_length,
            stretches,
            distributions,
        }
    }

    /// Yields exactly `ops_per_run` patterns, one for each op of the run.
    pub(super) fn op_patterns(&self) -> impl Iterator<Item = Pattern> + '_ {
        self.stretches
            .iter()
            .flat_map(|stretch| std::iter::repeat_n(stretch.pattern, stretch.op_count))
    }

    pub(super) fn draw_kind(&self, pattern: Pattern, rng: &mut ChaCha8Rng) -> OpKind {
        ALL_OP_KINDS[self.distribution(pattern).sample(rng)]
    }

    fn distribution(&self, pattern: Pattern) -> &WeightedIndex<f64> {
        let index = ALL_PATTERNS
            .iter()
            .position(|&candidate| candidate == pattern)
            .expect("ALL_PATTERNS lists every pattern");
        &self.distributions[index]
    }
}

impl fmt::Display for Workload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mean stretch length {:.0}, {} stretches",
            self.mean_stretch_length,
            self.stretches.len()
        )?;
        for pattern in ALL_PATTERNS {
            let (stretch_count, op_count) = self
                .stretches
                .iter()
                .filter(|stretch| stretch.pattern == pattern)
                .fold((0, 0), |(stretches, ops), stretch| {
                    (stretches + 1, ops + stretch.op_count)
                });
            write!(
                f,
                ", {}: {op_count} ops in {stretch_count} stretches",
                pattern.name()
            )?;
        }
        Ok(())
    }
}

struct Stretch {
    pattern: Pattern,
    op_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    NavigationAndLayout,
    WindowLifecycle,
    Workspace,
    Monitor,
    Config,
}

impl Category {
    fn of(kind: OpKind) -> Self {
        match kind {
            OpKind::FocusLeft
            | OpKind::FocusRight
            | OpKind::FocusUp
            | OpKind::FocusDown
            | OpKind::MoveLeft
            | OpKind::MoveRight
            | OpKind::MoveUp
            | OpKind::MoveDown
            | OpKind::FocusParent
            | OpKind::FocusNextTab
            | OpKind::FocusPrevTab
            | OpKind::SetFocus
            | OpKind::ToggleSpawnMode
            | OpKind::ToggleDirection
            | OpKind::ToggleContainerLayout
            | OpKind::IncreaseMasterRatio
            | OpKind::DecreaseMasterRatio
            | OpKind::IncrementMasterCount
            | OpKind::DecrementMasterCount
            | OpKind::ToggleFloat
            | OpKind::ToggleFullscreen
            | OpKind::SetFullscreen
            | OpKind::UnsetFullscreen => Category::NavigationAndLayout,
            OpKind::InsertTiling
            | OpKind::InsertFullscreen
            | OpKind::DeleteWindow
            | OpKind::MinimizeWindow
            | OpKind::UnminimizeWindow
            | OpKind::SetWindowTitle
            | OpKind::SetWindowConstraint => Category::WindowLifecycle,
            OpKind::MoveToWorkspace | OpKind::FocusWorkspace | OpKind::QueryWorkspaces => {
                Category::Workspace
            }
            OpKind::AddMonitor
            | OpKind::RemoveMonitor
            | OpKind::FocusMonitor
            | OpKind::MoveToMonitor => Category::Monitor,
            OpKind::ConfigReload | OpKind::SyncPreferredLayout => Category::Config,
        }
    }

    fn kind_count(self) -> usize {
        ALL_OP_KINDS
            .iter()
            .filter(|&&kind| Category::of(kind) == self)
            .count()
    }
}

/// Percent of a pattern's ops that each category gets, before the per-run
/// multipliers.
struct CategoryShares {
    navigation_and_layout: f64,
    window_lifecycle: f64,
    workspace: f64,
    monitor: f64,
    config: f64,
}

impl CategoryShares {
    /// `None` for random, which gives every kind the same weight.
    fn of(pattern: Pattern) -> Option<Self> {
        let shares = match pattern {
            Pattern::NormalUse => CategoryShares {
                navigation_and_layout: 60.0,
                window_lifecycle: 25.0,
                workspace: 10.0,
                monitor: 3.0,
                config: 2.0,
            },
            Pattern::ConfigChanges => CategoryShares {
                navigation_and_layout: 20.0,
                window_lifecycle: 15.0,
                workspace: 3.0,
                monitor: 2.0,
                config: 60.0,
            },
            Pattern::DockAndUndock => CategoryShares {
                navigation_and_layout: 20.0,
                window_lifecycle: 15.0,
                workspace: 3.0,
                monitor: 60.0,
                config: 2.0,
            },
            Pattern::WindowStorm => CategoryShares {
                navigation_and_layout: 20.0,
                window_lifecycle: 70.0,
                workspace: 5.0,
                monitor: 3.0,
                config: 2.0,
            },
            Pattern::Random => return None,
        };
        Some(shares)
    }

    fn share(&self, category: Category) -> f64 {
        match category {
            Category::NavigationAndLayout => self.navigation_and_layout,
            Category::WindowLifecycle => self.window_lifecycle,
            Category::Workspace => self.workspace,
            Category::Monitor => self.monitor,
            Category::Config => self.config,
        }
    }
}

/// A category splits its share evenly over its kinds, except `InsertTiling` in
/// config changes.
fn base_weight(pattern: Pattern, kind: OpKind) -> f64 {
    let Some(shares) = CategoryShares::of(pattern) else {
        return 1.0;
    };
    let category = Category::of(kind);
    if pattern == Pattern::ConfigChanges && category == Category::WindowLifecycle {
        if kind == OpKind::InsertTiling {
            return CONFIG_CHANGES_INSERT_TILING_SHARE;
        }
        let other_lifecycle_kinds = Category::WindowLifecycle.kind_count() - 1;
        return (shares.window_lifecycle - CONFIG_CHANGES_INSERT_TILING_SHARE)
            / other_lifecycle_kinds as f64;
    }
    shares.share(category) / category.kind_count() as f64
}

fn pattern_distribution(rng: &mut ChaCha8Rng, pattern: Pattern) -> WeightedIndex<f64> {
    let weights: Vec<f64> = ALL_OP_KINDS
        .iter()
        .map(|&kind| {
            let base = base_weight(pattern, kind);
            if pattern == Pattern::Random {
                base
            } else {
                base * log_uniform(rng, MIN_WEIGHT_MULTIPLIER, MAX_WEIGHT_MULTIPLIER)
            }
        })
        .collect();
    WeightedIndex::new(weights).expect("every weight is positive and finite")
}

/// After each op, the stretch ends with a chance of one in `mean_stretch_length`.
fn draw_stretches(
    rng: &mut ChaCha8Rng,
    ops_per_run: usize,
    mean_stretch_length: f64,
) -> Vec<Stretch> {
    let end_chance = 1.0 / mean_stretch_length;
    let mut stretches = Vec::new();
    let mut remaining = ops_per_run;
    while remaining > 0 {
        let pattern = ALL_PATTERNS[rng.random_range(0..ALL_PATTERNS.len())];
        let mut op_count = 1;
        while op_count < remaining && !rng.random_bool(end_chance) {
            op_count += 1;
        }
        stretches.push(Stretch { pattern, op_count });
        remaining -= op_count;
    }
    stretches
}

fn log_uniform(rng: &mut ChaCha8Rng, min: f64, max: f64) -> f64 {
    rng.random_range(min.ln()..=max.ln()).exp()
}

mod tests {
    use super::*;

    const SEEDS: std::ops::Range<u64> = 0..200;
    const OPS_PER_RUN: usize = 10_000;

    fn weights(workload: &Workload, pattern: Pattern) -> Vec<f64> {
        let distribution = workload.distribution(pattern);
        (0..ALL_OP_KINDS.len())
            .map(|index| distribution.weight(index).expect("one weight per op kind"))
            .collect()
    }

    #[test]
    fn same_seed_builds_the_same_workload() {
        let first = Workload::for_seed(7, OPS_PER_RUN);
        let second = Workload::for_seed(7, OPS_PER_RUN);

        assert_eq!(
            first.op_patterns().collect::<Vec<_>>(),
            second.op_patterns().collect::<Vec<_>>()
        );
        for pattern in ALL_PATTERNS {
            assert_eq!(weights(&first, pattern), weights(&second, pattern));
        }
    }

    #[test]
    fn stretches_cover_every_op_of_the_run() {
        for seed in SEEDS {
            let workload = Workload::for_seed(seed, OPS_PER_RUN);
            assert_eq!(workload.op_patterns().count(), OPS_PER_RUN, "seed {seed}");
        }
    }

    #[test]
    fn first_stretch_can_use_every_pattern() {
        let first_patterns: Vec<Pattern> = SEEDS
            .map(|seed| {
                Workload::for_seed(seed, OPS_PER_RUN)
                    .op_patterns()
                    .next()
                    .expect("a run with ops has a first stretch")
            })
            .collect();

        for pattern in ALL_PATTERNS {
            assert!(first_patterns.contains(&pattern), "{pattern:?}");
        }
    }

    #[test]
    fn every_pattern_gives_every_kind_a_positive_weight() {
        for seed in SEEDS {
            let workload = Workload::for_seed(seed, OPS_PER_RUN);
            for pattern in ALL_PATTERNS {
                for (kind, weight) in ALL_OP_KINDS.iter().zip(weights(&workload, pattern)) {
                    assert!(weight > 0.0, "seed {seed}, {pattern:?}, {kind:?}");
                }
            }
        }
    }

    #[test]
    fn random_gives_every_kind_the_same_weight() {
        let workload = Workload::for_seed(7, OPS_PER_RUN);

        let random = weights(&workload, Pattern::Random);

        assert!(random.iter().all(|&weight| weight == random[0]));
    }

    #[test]
    fn category_shares_sum_to_one_hundred() {
        for pattern in ALL_PATTERNS {
            if pattern == Pattern::Random {
                continue;
            }
            let total: f64 = ALL_OP_KINDS
                .iter()
                .map(|&kind| base_weight(pattern, kind))
                .sum();
            assert!((total - 100.0).abs() < 1e-9, "{pattern:?} sums to {total}");
        }
    }
}
