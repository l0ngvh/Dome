// Nothing a generated op is built from may read the iteration order of a
// `HashMap` or `HashSet`, because that order changes between processes.
#![deny(clippy::iter_over_hash_type)]

mod execution;
mod operations;
mod preferred_layouts;
mod reduction;
mod self_tests;
mod support;
mod workload;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::action::{Action, FocusTarget, MonitorTarget, MoveTarget};
use crate::core::{
    ColumnConfig, FractionalLength, Length, LimitObservation, LimitUpdate, MasterConfig, MonitorId,
    PartitionTreeConfig, PixelRect, PreferredLayouts, PreferredMaster, PreferredScrolling,
    PreferredTiling, PreferredWorkspace, ScrollingConfig, SizeConstraint, SizeConstraints,
    SplitMode, Strategy, TilingConfig, TreeLayoutNode, WindowId, WindowMatcher, WindowRestrictions,
};
use crate::tests::fixtures::{TestDome, monitor, window};
use execution::*;
use operations::*;
use preferred_layouts::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use reduction::*;
use support::{export_then_apply, tiling_source, title_matchers};
use workload::Workload;

const RUNS: usize = 200;
const OPS_PER_RUN: usize = 10000;
const SEED: u64 = 42u64;
const PREF_TREE_MAX_LEAVES: usize = 30;
const MAX_MONITORS: usize = 8;

#[test]
fn smoke_test() {
    crate::tests::fixtures::logging::initialize();

    let completed = AtomicUsize::new(0);
    let abort = AtomicBool::new(false);

    (0..RUNS).into_par_iter().for_each(|run| {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let strategy_seed = SEED.wrapping_add(run as u64);
        let strategy = strategy_for_seed(strategy_seed);
        run_smoke_iteration(strategy_seed, OPS_PER_RUN, strategy, &abort);
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        if done.is_multiple_of(10) {
            tracing::info!("Completed {done}/{RUNS}");
        }
    });
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reproduce_smoke_failure() {
    crate::tests::fixtures::logging::initialize();
    let seed = smoke_seed_from_env();
    let strategy = strategy_for_seed(seed);
    let abort = AtomicBool::new(false);
    run_smoke_iteration(seed, OPS_PER_RUN, strategy, &abort);
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reduce_smoke_failure() {
    crate::tests::fixtures::logging::initialize();
    let seed = smoke_seed_from_env();
    let strategy = strategy_for_seed(seed);
    let (window_ops, signature) = record(seed, OPS_PER_RUN, strategy);

    tracing::info!(
        window_ops = window_ops.len(),
        ?signature,
        "captured failure"
    );
    let reduced = shrink_ops(window_ops, &signature, || strategy.build_dome());
    log_reduced(&reduced);
}

fn smoke_seed_from_env() -> u64 {
    match std::env::var("DOME_SMOKE_SEED") {
        Ok(value) => match value.parse::<u64>() {
            Ok(seed) => seed,
            Err(_) => panic!(
                "DOME_SMOKE_SEED='{value}' is not a valid u64.\n\
                 example: DOME_SMOKE_SEED=167 cargo test -p dome-core --lib reproduce_smoke_failure \
                 -- --ignored --nocapture"
            ),
        },
        Err(_) => panic!(
            "DOME_SMOKE_SEED not set.\n\
             example: DOME_SMOKE_SEED=167 cargo test -p dome-core --lib reproduce_smoke_failure \
             -- --ignored --nocapture"
        ),
    }
}
