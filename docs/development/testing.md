# Testing

## Writing Core Tests

Tests live in `src/core/tests/`, one file per feature area. Each test creates a Hub with `setup()`, performs operations, and snapshots the result with `insta::assert_snapshot!`. The snapshot contains both a text tree and an ASCII visualization of window positions.

```rust,ignore
use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn two_windows_split_evenly() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @"");
}
```

`setup()` creates a Hub with a 150×30 screen. `snapshot()` validates tree invariants, then renders the tree as text + ASCII art. Leave the snapshot string empty (`@""`) — run the test once, review the output, then confirm with a human before accepting.

**Validators.** `snapshot()` runs a set of validators that check structural invariants on every workspace. Structural validators (container tree checks: direction invariance, focus chain correctness, parent-child consistency, dimension validation, children count >= 2) live inside `PartitionTreeStrategy::validate_tree`, called via `hub.validate_tree()`. Each strategy implements its own invariant checks. The workspace focus validator checks:

- `is_float_focused` is false when `float_windows` is empty.
- `focused_tiling` points to a `Tiling`-mode window (not float or fullscreen).
- `focused_tiling` is reachable from the workspace root via the focus chain.
- If root exists, `focused_tiling` is `Some`.

Generic validators (float/fullscreen/minimized consistency, monitor validity, visible placement correctness) stay in `src/core/tests/mod.rs`. The minimized validator checks that every window in `minimized_windows` has mode `Minimized` and does not appear in any workspace's float or fullscreen lists. Snapshot and format functions work with `WindowPlacement` and `ContainerPlacement` from `get_visible_placements()`. `snapshot()` reads `cp.titles` for tab bar labels instead of accessing container internals directly. Use `hub.insert_tiling_titled()` in tests that snapshot a tabbed container so the ASCII tab bar carries readable `W<id>` labels; other tests should keep calling `hub.insert_tiling()`. If any minimized windows exist, the snapshot text appends a sorted `Minimized: [...]` line.

These run on every snapshot, so invariant violations are caught immediately rather than causing subtle downstream bugs.

The smoke test (`smoke.rs`) runs thousands of random operation sequences to catch panics and invariant violations. Update it when adding new Hub operations.

## Platform Tests

Both macOS and Windows have test directories (`src/platform/macos/tests/`, `src/platform/windows/tests/`) with mock implementations of all platform traits. Mock traits are used because platform tests need to exercise the Dome struct's logic (state machine transitions, placement, drift correction) without making real OS API calls. Mocks let tests control what the "OS" reports and verify what Dome asks the "OS" to do, making tests fast, deterministic, and runnable in CI without a display server.

**macOS**: `MockAXWindow` implements `AXWindowApi`, `TestSender` implements `FrameSender` (captures the latest `HubMessage::Frame` into a shared `FrameState`). Tests create a `Dome` with mock windows and a `TestSender`, then call Dome methods and assert on window state transitions and move logs.

**Windows**: `MockExternalHwnd` implements `ManageExternalHwnd`, `MockDisplay` implements `QueryDisplay`, `NoopTaskbar` implements `ManageTaskbar`, `NoopOverlays` implements `CreateOverlay` (with `NoopTilingOverlay` and `NoopFloatOverlay`); `NoopKeyboardSink` implements `KeyboardSinkApi`. `TestEnv` wraps everything for convenient test setup. `NoopFloatOverlay` increments a shared `overlay_update_count` on each `update()` call; `TestEnv::overlay_update_count()` returns the current count, letting tests verify overlay re-renders happen (or don't) without real GL contexts.

Test files are organized by concern:
- `lifecycle.rs` -- window add/remove (both platforms)
- `transitions.rs` -- WindowState transition tests (both platforms)
- `placement.rs` -- drift correction, constraint detection (both platforms)
- `uncooperative.rs` -- windows that resist placement (macOS only)
- `drift.rs` -- drift retry logic (Windows only)
- `zorder.rs` -- z-order chain correctness (Windows only)

Platform tests don't use snapshots -- they assert on concrete state values and mock call counts.

**ZOrderModel.** Windows z-order tests use a `ZOrderModel` that emulates Win32's z-order stack. It tracks two bands (topmost and normal) and processes `ZOrder` variants the same way `SetWindowPos` does: `Top` inserts at the front of the normal band, `After(other)` inserts behind `other`, `Topmost` inserts at the front of the topmost band, `Unchanged` preserves the original position. `MockExternalHwnd` feeds z-order changes into the shared `ZOrderModel` via `set_position`. The overlay sentinel (`HwndId::test(9999)`) is seeded once at creation in `NoopOverlays::create_tiling_overlay`; `NoopTilingOverlay::update` is a no-op that does not touch the z-model. `env.tiling_z_order()` returns the full normal-band stack including the overlay sentinel. The older per-window `z_state` field (`ZOrderState`) is kept alongside `ZOrderModel` because existing tests use `is_topmost()` and `is_bottom()` for per-window assertions where multiple windows can independently be "bottom" (e.g., after workspace switch hides them all).

## E2E Tests

`tests/e2e.rs` contains end-to-end tests. Run via `cargo make e2e`.
