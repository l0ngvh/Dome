use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::config::{Color, Config};
use crate::core::{Container, ContainerId, Dimension, SpawnMode, Window};

/// Computed border data ready for sending to the app thread.
pub(super) struct BorderEdges {
    pub(super) frame: NSRect,
    pub(super) edges: Vec<(NSRect, Color)>,
}

pub(super) struct ContainerBorder {
    pub(super) key: ContainerId,
    pub(super) frame: NSRect,
    pub(super) edges: Vec<(NSRect, Color)>,
}

pub(super) struct TabInfo {
    pub(super) title: String,
    pub(super) x: f32,
    pub(super) width: f32,
    pub(super) is_active: bool,
}

pub(super) struct TabBarOverlay {
    pub(super) key: ContainerId,
    pub(super) frame: NSRect,
    pub(super) tabs: Vec<TabInfo>,
    pub(super) background_color: Color,
    pub(super) active_background_color: Color,
}

/// Compute border edges for a window, clipped to monitor bounds.
/// Returns frame and edges in Cocoa coordinates (bottom-left origin), ready for rendering.
pub(super) fn compute_window_border(
    window: &Window,
    bounds: Dimension,
    focused: bool,
    config: &Config,
    primary_full_height: f32,
) -> Option<BorderEdges> {
    let colors = if !focused {
        [config.border_color; 4]
    } else if window.is_float() {
        [config.focused_color; 4]
    } else {
        spawn_colors(window.spawn_mode(), config)
    };
    compute_border_edges(window.dimension(), bounds, colors, config.border_size, primary_full_height)
}

/// Compute border edges for a container, clipped to monitor bounds.
/// Returns frame and edges in Cocoa coordinates (bottom-left origin), ready for rendering.
pub(super) fn compute_container_border(
    container: &Container,
    bounds: Dimension,
    config: &Config,
    primary_full_height: f32,
) -> Option<BorderEdges> {
    let colors = spawn_colors(container.spawn_mode(), config);
    compute_border_edges(container.dimension(), bounds, colors, config.border_size, primary_full_height)
}

/// colors: [top, right, bottom, left]
fn compute_border_edges(
    frame: Dimension,
    bounds: Dimension,
    colors: [Color; 4],
    border_size: f32,
    primary_full_height: f32,
) -> Option<BorderEdges> {
    let frame = to_cocoa(frame, primary_full_height);
    let bounds = to_cocoa(bounds, primary_full_height);
    let clipped = clip_to_bounds(frame, bounds)?;

    let offset_x = clipped.x - frame.x;
    let offset_y = clipped.y - frame.y;
    let clip_local = Dimension {
        x: offset_x,
        y: offset_y,
        width: clipped.width,
        height: clipped.height,
    };

    let w = frame.width;
    let h = frame.height;
    let b = border_size;
    let mut edges = Vec::new();

    // top (y = h - b in Cocoa, at the top)
    let top = Dimension { x: 0.0, y: h - b, width: w, height: b };
    if let Some(r) = clip_to_bounds(top, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[0]));
    }

    // right (exclude corners)
    let right = Dimension { x: w - b, y: b, width: b, height: h - 2.0 * b };
    if let Some(r) = clip_to_bounds(right, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[1]));
    }

    // bottom (y = 0 in Cocoa, at the bottom)
    let bottom = Dimension { x: 0.0, y: 0.0, width: w, height: b };
    if let Some(r) = clip_to_bounds(bottom, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[2]));
    }

    // left (exclude corners)
    let left = Dimension { x: 0.0, y: b, width: b, height: h - 2.0 * b };
    if let Some(r) = clip_to_bounds(left, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[3]));
    }

    if edges.is_empty() {
        return None;
    }

    Some(BorderEdges {
        frame: dim_to_ns_rect(clipped),
        edges: edges.into_iter().map(|(r, c)| (dim_to_ns_rect(r), c)).collect(),
    })
}

fn spawn_colors(spawn: SpawnMode, config: &Config) -> [Color; 4] {
    let f = config.focused_color;
    let s = config.spawn_indicator_color;
    // [top, right, bottom, left] to match BorderView draw order
    [
        if spawn.is_tab() { s } else { f },
        if spawn.is_horizontal() { s } else { f },
        if spawn.is_vertical() { s } else { f },
        f,
    ]
}

/// Clip rect to bounds. Returns None if fully outside.
pub(super) fn clip_to_bounds(rect: Dimension, bounds: Dimension) -> Option<Dimension> {
    if rect.x >= bounds.x + bounds.width
        || rect.y >= bounds.y + bounds.height
        || rect.x + rect.width <= bounds.x
        || rect.y + rect.height <= bounds.y
    {
        return None;
    }
    let x = rect.x.max(bounds.x);
    let y = rect.y.max(bounds.y);
    let right = (rect.x + rect.width).min(bounds.x + bounds.width);
    let bottom = (rect.y + rect.height).min(bounds.y + bounds.height);
    Some(Dimension {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

fn translate_dim(dim: Dimension, dx: f32, dy: f32) -> Dimension {
    Dimension {
        x: dim.x + dx,
        y: dim.y + dy,
        width: dim.width,
        height: dim.height,
    }
}

fn to_cocoa(dim: Dimension, primary_full_height: f32) -> Dimension {
    Dimension {
        x: dim.x,
        y: primary_full_height - dim.y - dim.height,
        width: dim.width,
        height: dim.height,
    }
}

fn dim_to_ns_rect(dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(dim.x as f64, dim.y as f64),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}

pub(super) fn build_tab_bar(
    container_dim: Dimension,
    bounds: Dimension,
    id: ContainerId,
    titles: &[String],
    active_tab: usize,
    config: &Config,
    primary_full_height: f32,
) -> Option<TabBarOverlay> {
    let tab_bar_dim = Dimension {
        x: container_dim.x,
        y: container_dim.y,
        width: container_dim.width,
        height: config.tab_bar_height,
    };

    // Convert to Cocoa coords upfront
    let tab_bar = to_cocoa(tab_bar_dim, primary_full_height);
    let bounds = to_cocoa(bounds, primary_full_height);
    let clipped = clip_to_bounds(tab_bar, bounds)?;

    let tab_width = if titles.is_empty() {
        0.0
    } else {
        container_dim.width / titles.len() as f32
    };

    let tabs = titles
        .iter()
        .enumerate()
        .map(|(i, title)| TabInfo {
            title: title.clone(),
            x: i as f32 * tab_width,
            width: tab_width,
            is_active: i == active_tab,
        })
        .collect();

    Some(TabBarOverlay {
        key: id,
        frame: dim_to_ns_rect(clipped),
        tabs,
        background_color: config.tab_bar_background_color,
        active_background_color: config.active_tab_background_color,
    })
}

// Quartz uses top-left origin, Cocoa uses bottom-left origin
pub(super) fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}
