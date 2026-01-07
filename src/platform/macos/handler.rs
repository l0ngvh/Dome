use std::collections::HashSet;

use anyhow::Result;
use objc2::{DefinedClass, MainThreadMarker};
use objc2_app_kit::NSApplication;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::core::{Child, Dimension, Focus, Hub};

use super::app::AppDelegate;
use super::context::WindowRegistry;
use super::overlay::collect_overlays;
use super::window::WindowType;

#[tracing::instrument(skip(hub, registry), fields(actions = %actions))]
pub(super) fn execute_actions(hub: &mut Hub, registry: &mut WindowRegistry, actions: &Actions) {
    for action in actions {
        match action {
            Action::Exit => {
                let mtm = MainThreadMarker::new().unwrap();
                NSApplication::sharedApplication(mtm).terminate(None);
            }
            Action::Focus { target } => match target {
                FocusTarget::Up => hub.focus_up(),
                FocusTarget::Down => hub.focus_down(),
                FocusTarget::Left => hub.focus_left(),
                FocusTarget::Right => hub.focus_right(),
                FocusTarget::Parent => hub.focus_parent(),
                FocusTarget::Workspace { index } => hub.focus_workspace(*index),
                FocusTarget::NextTab => hub.focus_next_tab(),
                FocusTarget::PrevTab => hub.focus_prev_tab(),
            },
            Action::Move { target } => match target {
                MoveTarget::Workspace { index } => hub.move_focused_to_workspace(*index),
                MoveTarget::Up => hub.move_up(),
                MoveTarget::Down => hub.move_down(),
                MoveTarget::Left => hub.move_left(),
                MoveTarget::Right => hub.move_right(),
            },
            Action::Toggle { target } => match target {
                ToggleTarget::SpawnDirection => hub.toggle_spawn_mode(),
                ToggleTarget::Direction => hub.toggle_direction(),
                ToggleTarget::Layout => hub.toggle_container_layout(),
                ToggleTarget::Float => {
                    if let Some((window_id, float_id)) = hub.toggle_float() {
                        registry.toggle_float(window_id, float_id);
                    }
                }
            },
        }
    }
}

/// Sync hub state to actual macOS windows.
/// Some windows report incorrect AX attributes and can't actually be managed.
/// Layout failures for such windows are logged at trace level and ignored.
pub(super) fn render_workspace(delegate: &'static AppDelegate) -> Result<()> {
    let hub = delegate.ivars().hub.borrow();
    let registry = delegate.ivars().registry.borrow();
    let config = delegate.ivars().config.borrow();
    let mut displayed_windows = delegate.ivars().displayed_windows.borrow_mut();
    let tiling_overlay = delegate.ivars().tiling_overlay.get().unwrap();
    let float_overlay = delegate.ivars().float_overlay.get().unwrap();

    let workspace_id = hub.current_workspace();
    let workspace = hub.get_workspace(workspace_id);

    let mut workspace_windows = HashSet::new();
    let mut tiling_layouts = Vec::new();
    let mut float_layouts = Vec::new();

    let mut stack: Vec<Child> = workspace.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(window_id) => {
                if let Some(os_window) = registry.get_by_tiling_id(window_id) {
                    workspace_windows.insert(os_window.window_id());
                    let dim = hub.get_window(window_id).dimension();
                    tiling_layouts.push((window_id, dim));
                }
            }
            Child::Container(container_id) => {
                let container = hub.get_container(container_id);
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
        if let Some(os_window) = registry.get_by_float_id(float_id) {
            workspace_windows.insert(os_window.window_id());
            let dim = hub.get_float(float_id).dimension();
            float_layouts.push((float_id, dim));
        }
    }

    let to_hide: Vec<_> = displayed_windows
        .difference(&workspace_windows)
        .copied()
        .collect();

    for cg_id in to_hide {
        if let Some(os_window) = registry.get(cg_id) {
            match os_window.window_type() {
                WindowType::Tiling(id) => {
                    if let Err(e) = os_window.hide() {
                        tracing::warn!("Failed to hide tiling window {id}: {e:#}");
                    }
                }
                WindowType::Float(id) => {
                    if let Err(e) = os_window.hide() {
                        tracing::warn!("Failed to hide float window {id}: {e:#}");
                    }
                }
                WindowType::Popup => {}
            }
        }
    }

    for (window_id, dim) in tiling_layouts {
        if let Some(os_window) = registry.get_by_tiling_id(window_id) {
            let border = config.border_size;
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
        if let Some(os_window) = registry.get_by_float_id(float_id) {
            let border = config.border_size;
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

    let overlays = collect_overlays(&hub, &config, workspace_id, &registry);

    tiling_overlay.set_rects(overlays.tiling_rects, overlays.tiling_labels);
    float_overlay.set_rects(overlays.float_rects, vec![]);

    *displayed_windows = workspace_windows;

    match workspace.focused() {
        Some(Focus::Tiling(Child::Window(window_id))) => {
            let os_window = registry.get_by_tiling_id(window_id).unwrap();
            os_window.focus()?;
        }
        Some(Focus::Float(float_id)) => {
            let os_window = registry.get_by_float_id(float_id).unwrap();
            os_window.focus()?;
        }
        _ => {}
    }
    Ok(())
}
