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

The smoke test (`smoke.rs`) runs thousands of random operation sequences to catch panics and invariant violations. Update it when adding new Hub operations.

## Platform Tests

Both macOS and Windows have test directories (`src/platform/macos/tests/`, `src/platform/windows/tests/`) with mock implementations of all platform traits. Mock traits are used because platform tests need to exercise the Dome struct's logic (state machine transitions, placement, drift correction) without making real OS API calls. Mocks let tests control what the "OS" reports and verify what Dome asks the "OS" to do, making tests fast, deterministic, and runnable in CI without a display server.

**macOS**: `MockAXWindow` implements `AXWindowApi`, `TestSender` implements `FrameSender` (captures the latest `HubMessage::Frame` into a shared `FrameState`). Tests create a `Dome` with mock windows and a `TestSender`, then call Dome methods and assert on window state transitions and move logs.

**Windows**: `MockExternalHwnd` implements `ManageExternalHwnd`, `MockDisplay` implements `QueryDisplay`, `NoopTaskbar` implements `ManageTaskbar`, `NoopOverlays` implements `CreateOverlay` (with `NoopTilingOverlay` and `NoopFloatOverlay`). `TestEnv` wraps everything for convenient test setup.

Test files are organized by concern:
- `lifecycle.rs` — window add/remove (both platforms)
- `transitions.rs` — WindowState transition tests (both platforms)
- `placement.rs` — drift correction, constraint detection (both platforms)
- `uncooperative.rs` — windows that resist placement (macOS only)
- `drift.rs` — drift retry logic (Windows only)

Platform tests don't use snapshots — they assert on concrete state values and mock call counts.

## E2E Tests

`tests/e2e.rs` contains end-to-end tests. Run via `cargo make e2e`.
