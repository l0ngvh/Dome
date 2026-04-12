# Conventions

Project-wide conventions for contributing to Dome.

## Logging

Dome uses the `tracing` crate. The default log level is `info`, configurable via the `log_level` config field or the `RUST_LOG` environment variable (which takes precedence).

### Log Levels

#### `error` — Something broke and Dome can't recover

Use for panics, unrecoverable failures, and smoke test failure dumps. The user or developer needs to see this immediately.

```rust,ignore
tracing::error!("Application panicked: {panic_info}");
```

#### `warn` — Something went wrong but Dome continues

Use for recoverable failures where Dome falls back to a degraded behavior. Failed OS calls that are retried or skipped, invalid IPC messages, failed config reloads.

```rust,ignore
tracing::warn!("Failed to enumerate screens: {e}");
tracing::warn!(%command, "Failed to exec: {e}");
```

#### `info` — Significant state changes a user would care about

Use for events that change Dome's overall state: startup, shutdown, config reload, monitor added/removed, window managed/unmanaged, fullscreen entered/exited, suspend/resume. These should read like a high-level activity log. A user running with `info` should be able to understand what Dome is doing without being overwhelmed.

```rust,ignore
tracing::info!(%window_id, "New tiling window");
tracing::info!("Config reloaded");
tracing::info!(count = screens.len(), "Screens changed");
```

#### `debug` — Internal decisions useful for debugging

Use for tree mutations, focus changes, layout decisions, and control flow branches that explain *why* Dome did something. A developer investigating a bug should be able to reconstruct the decision chain from `debug` logs.

```rust,ignore
tracing::debug!(%window_id, "Setting focus to window");
tracing::debug!(?target, "Focusing monitor");
tracing::debug!(?focused, ?new_mode, "Toggled spawn mode");
```

#### `trace` — High-frequency or low-level detail

Use for per-frame rendering, individual AX/Win32 call results, drift correction attempts, throttle decisions, and anything that fires on every event or every frame. These are only useful when actively investigating a specific subsystem.

```rust,ignore
tracing::trace!("no drawable available");
tracing::trace!(window = %self, "not manageable: role is not AXWindow");
```

### Rules

#### Use structured fields, not string interpolation

Prefer `tracing::info!(%window_id, "Window removed")` over `tracing::info!("Window {window_id} removed")`. Structured fields are filterable and machine-parseable.

- `%field` for types that implement `Display`
- `?field` for types that implement `Debug`
- `field = value` for computed values

#### Use `#[tracing::instrument]` for functions with meaningful arguments

Annotate Hub operations, window state transitions, and platform dispatch functions. Always `skip(self)` or `skip_all` to avoid dumping large structs.

```rust,ignore
#[tracing::instrument(skip(self))]
pub(crate) fn insert_tiling(&mut self, window_id: WindowId, ...) {
```

Don't instrument trivial getters, pure layout math, or functions called per-frame.

#### Don't log in core test helpers

The `setup()` and `snapshot()` functions in `src/core/tests/mod.rs` initialize a test logger. Don't add log statements to test helper functions — they clutter test output.

#### Don't log noisy data

- No per-frame logs at `debug` or above. Rendering, overlay updates, and layout recomputation that happen every frame must use `trace`.
- No logging inside tight loops unless gated at `trace`.

#### Keep messages concise and grep-friendly

Write messages as short noun phrases or past-tense actions. Include the relevant ID as a structured field, not in the message string.

```rust,ignore
// Good
tracing::info!(%window_id, "Fullscreen set");
tracing::debug!(%container_id, from = ?direction, "Toggled container layout");

// Bad
tracing::info!("The window with id {window_id} has been set to fullscreen mode");
```

### Quick Reference

| Level   | When to use                                    | Example                                      |
|---------|------------------------------------------------|----------------------------------------------|
| `error` | Unrecoverable failure, panic                   | Application panicked, smoke test failure      |
| `warn`  | Recoverable failure, degraded behavior         | Failed OS call, invalid IPC, failed reload    |
| `info`  | User-visible state change                      | Window managed, config reloaded, monitor added|
| `debug` | Internal decision, mutation, control flow       | Focus changed, tree restructured, mode toggled|
| `trace` | Per-frame, per-call, high-frequency detail      | AX call result, render frame, drift attempt   |
