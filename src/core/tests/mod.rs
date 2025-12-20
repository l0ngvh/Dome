mod border;
mod delete_window;
mod focus_direction;
mod focus_parent;
mod focus_workspace;
mod insert_window;
mod move_to_workspace;
mod toggle_new_window_direction;

use crate::core::hub::Hub;
use crate::core::node::Child;

pub(super) fn snapshot(hub: &Hub) -> String {
    let mut s = format!(
        "Hub(focused={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
        hub.current_workspace(),
        hub.screen().x,
        hub.screen().y,
        hub.screen().width,
        hub.screen().height
    );
    for (workspace_id, workspace) in hub.all_workspaces() {
        let focused = if let Some(current) = workspace.focused {
            format!(", focused={}", current)
        } else {
            String::new()
        };
        if workspace.root().is_none() {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{})\n",
                workspace_id, workspace.name, focused
            ));
        } else {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{},\n",
                workspace_id, workspace.name, focused
            ));
            fmt_child_str(hub, &mut s, workspace.root().unwrap(), 2);
            s.push_str("  )\n");
        }
    }
    s.push_str(")\n");
    s
}

fn fmt_child_str(hub: &Hub, s: &mut String, child: Child, indent: usize) {
    let prefix = "  ".repeat(indent);
    match child {
        Child::Window(id) => {
            let w = hub.get_window(id);
            let dim = w.dimension();
            s.push_str(&format!(
                "{}Window(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2})\n",
                prefix, id, w.parent, dim.x, dim.y, dim.width, dim.height
            ));
        }
        Child::Container(id) => {
            let c = hub.get_container(id);
            s.push_str(&format!(
                    "{}Container(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}, direction={:?},\n",
                    prefix,
                    id,
                    c.parent,
                    c.dimension.x,
                    c.dimension.y,
                    c.dimension.width,
                    c.dimension.height,
                    c.direction,
                ));
            for &child in c.children() {
                fmt_child_str(hub, s, child, indent + 1);
            }
            s.push_str(&format!("{})\n", prefix));
        }
    }
}

pub(super) fn setup_logger() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
}
