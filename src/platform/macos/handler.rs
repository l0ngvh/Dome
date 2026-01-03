use std::collections::HashSet;

use anyhow::Result;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

use crate::action::{Action, FocusTarget, MoveTarget, ToggleTarget};
use crate::core::{Child, Dimension, Focus};

use super::context::WindowContext;
use super::overlay::collect_overlays;

pub(super) fn execute_action(context: &mut WindowContext, action: &Action) -> Result<()> {
    tracing::debug!(?action, "Executing action");
    match action {
        Action::Exit => {
            let mtm = MainThreadMarker::new().unwrap();
            NSApplication::sharedApplication(mtm).terminate(None);
            return Ok(());
        }
        Action::Focus { target } => match target {
            FocusTarget::Up => context.hub.focus_up(),
            FocusTarget::Down => context.hub.focus_down(),
            FocusTarget::Left => context.hub.focus_left(),
            FocusTarget::Right => context.hub.focus_right(),
            FocusTarget::Parent => context.hub.focus_parent(),
            FocusTarget::Workspace { index } => context.hub.focus_workspace(*index),
            FocusTarget::NextTab => context.hub.focus_next_tab(),
            FocusTarget::PrevTab => context.hub.focus_prev_tab(),
        },
        Action::Move { target } => match target {
            MoveTarget::Workspace { index } => context.hub.move_focused_to_workspace(*index),
            MoveTarget::Up => context.hub.move_up(),
            MoveTarget::Down => context.hub.move_down(),
            MoveTarget::Left => context.hub.move_left(),
            MoveTarget::Right => context.hub.move_right(),
        },
        Action::Toggle { target } => match target {
            ToggleTarget::SpawnDirection => context.hub.toggle_spawn_mode(),
            ToggleTarget::Direction => context.hub.toggle_direction(),
            ToggleTarget::Layout => context.hub.toggle_container_layout(),
            ToggleTarget::Float => {
                if let Some((window_id, float_id)) = context.hub.toggle_float() {
                    context
                        .registry
                        .borrow_mut()
                        .toggle_float(window_id, float_id);
                }
            }
        },
    }

    if let Err(e) = render_workspace(context) {
        tracing::warn!("Failed to render workspace after action: {e:#}");
    }

    Ok(())
}

pub(super) fn render_workspace(context: &mut WindowContext) -> Result<()> {
    apply_layout(context)?;
    focus_window(context)?;
    Ok(())
}

/// Sync window state to actual macOS windows.
/// Some windows report incorrect AX attributes and can't actually be managed.
/// Layout failures for such windows are logged at trace level and ignored.
pub(super) fn apply_layout(context: &mut WindowContext) -> Result<()> {
    let workspace_id = context.hub.current_workspace();
    let workspace = context.hub.get_workspace(workspace_id);
    let registry = context.registry.borrow();

    let mut workspace_windows = HashSet::new();
    let mut tiling_layouts = Vec::new();
    let mut float_layouts = Vec::new();

    let mut stack: Vec<Child> = workspace.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(window_id) => {
                if let Some(os_window) = registry.get_tiling(window_id) {
                    workspace_windows.insert(os_window.cf_hash());
                    let dim = context.hub.get_window(window_id).dimension();
                    tiling_layouts.push((window_id, dim));
                }
            }
            Child::Container(container_id) => {
                let container = context.hub.get_container(container_id);
                if let Some(active_tab) = container.active_tab() {
                    stack.push(active_tab);
                } else {
                    for &c in container.children() {
                        stack.push(c);
                    }
                }
            }
        }
    }
    for &float_id in workspace.float_windows() {
        if let Some(os_window) = registry.get_float(float_id) {
            workspace_windows.insert(os_window.cf_hash());
            let dim = context.hub.get_float(float_id).dimension();
            float_layouts.push((float_id, dim));
        }
    }

    let to_hide: Vec<usize> = context
        .displayed_windows
        .difference(&workspace_windows)
        .copied()
        .collect();

    for cf_hash in to_hide {
        if let Some(window_id) = registry.get_tiling_by_hash(cf_hash) {
            if let Some(os_window) = registry.get_tiling(window_id)
                && let Err(e) = os_window.hide()
            {
                tracing::warn!("Failed to hide tiling window {window_id}: {e:#}");
            }
        } else if let Some(float_id) = registry.get_float_by_hash(cf_hash)
            && let Some(os_window) = registry.get_float(float_id)
            && let Err(e) = os_window.hide()
        {
            tracing::warn!("Failed to hide float window {float_id}: {e:#}");
        }
    }

    for (window_id, dim) in tiling_layouts {
        if let Some(os_window) = registry.get_tiling(window_id) {
            let border = context.config.border_size;
            let inset_dim = Dimension {
                x: dim.x + border,
                y: dim.y + border,
                width: dim.width - 2.0 * border,
                height: dim.height - 2.0 * border,
            };
            if let Err(e) = os_window.set_dimension(inset_dim) {
                tracing::trace!(%window_id, error = %format!("{e:#}"), "Failed to set dimension");
            }
        }
    }
    for (float_id, dim) in float_layouts {
        if let Some(os_window) = registry.get_float(float_id) {
            let border = context.config.border_size;
            let inset_dim = Dimension {
                x: dim.x + border,
                y: dim.y + border,
                width: dim.width - 2.0 * border,
                height: dim.height - 2.0 * border,
            };
            if let Err(e) = os_window.set_dimension(inset_dim) {
                tracing::trace!(%float_id, error = %format!("{e:#}"), "Failed to set dimension");
            }
        }
    }

    let overlays = collect_overlays(&context.hub, &context.config, workspace_id, &registry);

    context
        .tiling_overlay
        .set_rects(overlays.tiling_rects, overlays.tiling_labels);
    context
        .float_overlay
        .set_rects(overlays.float_rects, vec![]);

    context.displayed_windows = workspace_windows;

    Ok(())
}

pub(super) fn focus_window(context: &WindowContext) -> Result<()> {
    let workspace_id = context.hub.current_workspace();
    let workspace = context.hub.get_workspace(workspace_id);

    match workspace.focused() {
        Some(Focus::Tiling(Child::Window(window_id))) => {
            if let Some(os_window) = context.registry.borrow().get_tiling(window_id) {
                os_window.focus()?;
            }
        }
        Some(Focus::Float(float_id)) => {
            if let Some(os_window) = context.registry.borrow().get_float(float_id) {
                os_window.focus()?;
            }
        }
        _ => {}
    }

    Ok(())
}
