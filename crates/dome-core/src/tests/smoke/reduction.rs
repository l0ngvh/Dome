use super::*;

pub(super) fn log_reduced(window: &[RecordedOp]) {
    tracing::error!("=== REDUCED WINDOW OPS ({}) ===", window.len());
    for (i, op) in window.iter().enumerate() {
        tracing::error!("  {i}: {op:?}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FailureSignature {
    pub(super) file: String,
    pub(super) line: u32,
    pub(super) normalized_payload: String,
}

pub(super) fn capture_panic<F: FnOnce()>(f: F) -> Option<FailureSignature> {
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex};

    static HOOK_LOCK: Mutex<()> = Mutex::new(());
    let _guard = HOOK_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let owner = std::thread::current().id();

    thread_local! {
        static SIGNATURE: RefCell<Option<FailureSignature>> = const { RefCell::new(None) };
    }

    SIGNATURE.with(|cell| *cell.borrow_mut() = None);

    let previous_hook = Arc::new(std::panic::take_hook());
    let forwarded_hook = previous_hook.clone();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() != owner {
            forwarded_hook(info);
            return;
        }
        let file = info
            .location()
            .map(|loc| loc.file().to_owned())
            .unwrap_or_default();
        let line = info.location().map(|loc| loc.line()).unwrap_or(0);

        let raw_payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or_default();

        let sig = FailureSignature {
            file,
            line,
            normalized_payload: normalize_digits(raw_payload),
        };
        SIGNATURE.with(|cell| *cell.borrow_mut() = Some(sig));
    }));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    drop(std::panic::take_hook());
    let previous_hook =
        Arc::try_unwrap(previous_hook).unwrap_or_else(|_| unreachable!("capture hook released"));
    std::panic::set_hook(previous_hook);

    match result {
        Err(_) => SIGNATURE.with(|cell| cell.borrow().clone()),
        Ok(()) => None,
    }
}

pub(super) fn normalize_digits(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_digit_run = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            if !in_digit_run {
                out.push('#');
                in_digit_run = true;
            }
        } else {
            in_digit_run = false;
            out.push(ch);
        }
    }
    out
}

#[expect(
    clippy::mut_range_bound,
    reason = "granularity is modified then we break immediately, so the loop bound is unaffected"
)]
pub(super) fn ddmin<T: Clone, P>(mut ops: Vec<T>, mut reproduces: P) -> Vec<T>
where
    P: FnMut(&[T]) -> bool,
{
    let mut granularity: usize = 2;
    let max_outer_iterations = ops.len().saturating_mul(2).saturating_add(8);
    for _ in 0..max_outer_iterations {
        if granularity > ops.len() {
            break;
        }
        let chunk = ops.len() / granularity;
        if chunk == 0 {
            break;
        }
        let mut reduced = false;
        for c in 0..granularity {
            let start = c * chunk;
            let end = if c + 1 == granularity {
                ops.len()
            } else {
                start + chunk
            };
            let candidate: Vec<T> = ops[..start]
                .iter()
                .chain(ops[end..].iter())
                .cloned()
                .collect();
            if reproduces(&candidate) {
                ops = candidate;
                granularity = (granularity - 1).max(2);
                reduced = true;
                break;
            }
        }
        if !reduced {
            granularity = granularity.saturating_mul(2);
        }
    }
    ops
}

pub(super) fn record(
    seed: u64,
    ops_per_run: usize,
    strategy: SmokeStrategy,
) -> (Vec<RecordedOp>, FailureSignature) {
    let abort = AtomicBool::new(false);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let workload = Workload::for_seed(seed, ops_per_run);
    let mut dome = strategy.build_dome();
    let mut ops: Vec<RecordedOp> = Vec::new();
    let mut current_tiling = initial_tiling(strategy);
    let signature = capture_panic(|| {
        run_iteration(
            &mut dome,
            &abort,
            |op| ops.push(op.clone()),
            &mut rng,
            &workload,
            &mut current_tiling,
        );
    });
    (
        ops,
        signature.expect("seed did not panic, nothing to reduce"),
    )
}

pub(super) fn replay(
    ops: &[RecordedOp],
    make_dome: impl FnOnce() -> TestDome,
) -> Option<FailureSignature> {
    capture_panic(|| replay_without_capture(ops, make_dome))
}

/// Shrink the recorded operations, then the embedded preferred-tree payloads of
/// retained operations, accepting only reductions that reproduce the `target`
/// failure signature.
pub(super) fn shrink_ops(
    window_ops: Vec<RecordedOp>,
    target: &FailureSignature,
    make_dome: impl Fn() -> TestDome + Copy,
) -> Vec<RecordedOp> {
    let mut window = window_ops;
    let target = target.clone();
    let reproduces =
        |w: &[RecordedOp]| matches!(replay(w, make_dome), Some(ref sig) if *sig == target);
    loop {
        let new_window = ddmin(window.clone(), |w| reproduces(w));
        let payload_shrunk = shrink_op_payloads(&new_window, |w| reproduces(w));

        if new_window.len() == window.len() && payload_shrunk.is_none() {
            return new_window;
        }
        window = payload_shrunk.unwrap_or(new_window);
        tracing::info!(window = window.len(), "shrink iteration");
    }
}

/// Shrinks the payload of the first op whose payload is reducible, returning
/// `None` when no payload could be reduced. Op-level `ddmin` can only delete a
/// whole op, so an op the failure needs keeps whatever payload it was recorded
/// with unless it is shrunk here.
pub(super) fn shrink_op_payloads(
    window: &[RecordedOp],
    mut reproduces: impl FnMut(&[RecordedOp]) -> bool,
) -> Option<Vec<RecordedOp>> {
    for i in 0..window.len() {
        let Some(tree_ops) = sync_tree_ops(&window[i]) else {
            continue;
        };
        let reduced_tree = ddmin(tree_ops.clone(), |t| {
            let mut candidate = window.to_vec();
            set_sync_tree_ops(&mut candidate[i], t.to_vec());
            reproduces(&candidate)
        });
        if reduced_tree.len() < tree_ops.len() {
            let mut shrunk = window.to_vec();
            set_sync_tree_ops(&mut shrunk[i], reduced_tree);
            return Some(shrunk);
        }
    }
    None
}
