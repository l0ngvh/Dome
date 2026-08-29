#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod render;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod tab_bar;

use crate::core::{Dimension, Length};

/// Subtract an observed status-bar rect from a monitor's work area so tiled
/// windows reflow around the bar.
///
/// Intrusion is measured against the CURRENT work area, not the bounds, to
/// prevent a double subtract: a bar the OS already excludes (an AppBar, or one
/// inside the macOS menu-bar inset) intrudes by zero and the shrink is a no-op.
/// A bar not at a screen edge (a centered placeholder) leaves the work area
/// unchanged. Non-edge bars are out of scope for v1.
///
/// Does no scaling. The caller owns unit conversion, so macOS feeds logical
/// rects and Windows feeds physical rects.
pub(crate) fn reserve_for_bar<U>(
    monitor_bounds: Dimension<U>,
    work_area: Dimension<U>,
    bar_rect: Dimension<U>,
) -> Dimension<U> {
    enum Edge {
        Top,
        Bottom,
        Left,
        Right,
    }

    // A bar hugging an edge sits within one pixel of it.
    let tolerance = Length::new(1.0);
    let near = |a: Length<U>, b: Length<U>| (a - b).max(b - a) <= tolerance;

    let bar_right = bar_rect.x + bar_rect.width;
    let bar_bottom = bar_rect.y + bar_rect.height;
    let bounds_right = monitor_bounds.x + monitor_bounds.width;
    let bounds_bottom = monitor_bounds.y + monitor_bounds.height;

    let horizontal = bar_rect.width >= bar_rect.height;
    let edge = if horizontal {
        if near(bar_rect.y, monitor_bounds.y) {
            Some(Edge::Top)
        } else if near(bar_bottom, bounds_bottom) {
            Some(Edge::Bottom)
        } else {
            None
        }
    } else if near(bar_rect.x, monitor_bounds.x) {
        Some(Edge::Left)
    } else if near(bar_right, bounds_right) {
        Some(Edge::Right)
    } else {
        None
    };

    match edge {
        None => work_area,
        Some(Edge::Top) => {
            let intrusion = (bar_bottom - work_area.y).max(Length::ZERO);
            let new_y = work_area.y + intrusion;
            let new_height = (work_area.height - intrusion).max(Length::ZERO);
            Dimension::new(work_area.x, new_y, work_area.width, new_height)
        }
        Some(Edge::Bottom) => {
            let work_bottom = work_area.y + work_area.height;
            let intrusion = (work_bottom - bar_rect.y).max(Length::ZERO);
            let new_height = (work_area.height - intrusion).max(Length::ZERO);
            Dimension::new(work_area.x, work_area.y, work_area.width, new_height)
        }
        Some(Edge::Left) => {
            let intrusion = (bar_right - work_area.x).max(Length::ZERO);
            let new_x = work_area.x + intrusion;
            let new_width = (work_area.width - intrusion).max(Length::ZERO);
            Dimension::new(new_x, work_area.y, new_width, work_area.height)
        }
        Some(Edge::Right) => {
            let work_right = work_area.x + work_area.width;
            let intrusion = (work_right - bar_rect.x).max(Length::ZERO);
            let new_width = (work_area.width - intrusion).max(Length::ZERO);
            Dimension::new(work_area.x, work_area.y, new_width, work_area.height)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::reserve_for_bar;
    use crate::core::{Dimension, Length, Logical};

    fn dim(x: f32, y: f32, width: f32, height: f32) -> Dimension<Logical> {
        Dimension::new(
            Length::new(x),
            Length::new(y),
            Length::new(width),
            Length::new(height),
        )
    }

    #[test]
    fn top_bar_shrinks_from_top() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);
        let bar = dim(0.0, 0.0, 1920.0, 30.0);
        assert_eq!(
            reserve_for_bar(bounds, work_area, bar),
            dim(0.0, 30.0, 1920.0, 1050.0)
        );
    }

    #[test]
    fn bottom_bar_shrinks_from_bottom() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);
        let bar = dim(0.0, 1050.0, 1920.0, 30.0);
        assert_eq!(
            reserve_for_bar(bounds, work_area, bar),
            dim(0.0, 0.0, 1920.0, 1050.0)
        );
    }

    #[test]
    fn left_bar_shrinks_from_left() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);
        let bar = dim(0.0, 0.0, 40.0, 1080.0);
        assert_eq!(
            reserve_for_bar(bounds, work_area, bar),
            dim(40.0, 0.0, 1880.0, 1080.0)
        );
    }

    #[test]
    fn right_bar_shrinks_from_right() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);
        let bar = dim(1880.0, 0.0, 40.0, 1080.0);
        assert_eq!(
            reserve_for_bar(bounds, work_area, bar),
            dim(0.0, 0.0, 1880.0, 1080.0)
        );
    }

    #[test]
    fn horizontal_bar_reduces_height_vertical_bar_reduces_width() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);

        let horizontal = reserve_for_bar(bounds, work_area, dim(0.0, 0.0, 1920.0, 30.0));
        assert_eq!(horizontal.width, Length::new(1920.0));
        assert_eq!(horizontal.height, Length::new(1050.0));

        let vertical = reserve_for_bar(bounds, work_area, dim(0.0, 0.0, 40.0, 1080.0));
        assert_eq!(vertical.width, Length::new(1880.0));
        assert_eq!(vertical.height, Length::new(1080.0));
    }

    #[test]
    fn already_excluded_bar_is_a_no_op() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 25.0, 1920.0, 1055.0);
        let bar = dim(0.0, 0.0, 1920.0, 25.0);
        assert_eq!(reserve_for_bar(bounds, work_area, bar), work_area);
    }

    #[test]
    fn non_edge_bar_leaves_work_area_unchanged() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);
        let bar = dim(800.0, 500.0, 320.0, 40.0);
        assert_eq!(reserve_for_bar(bounds, work_area, bar), work_area);
    }

    #[test]
    fn bar_taller_than_work_area_clamps_height_to_zero() {
        let bounds = dim(0.0, 0.0, 1920.0, 1080.0);
        let work_area = dim(0.0, 0.0, 1920.0, 1080.0);
        // Wider than tall so it is inferred as a top bar, but taller than the
        // work area so the height shrink would go negative without the clamp.
        let bar = dim(0.0, 0.0, 1920.0, 1200.0);
        let result = reserve_for_bar(bounds, work_area, bar);
        assert_eq!(result.height, Length::ZERO);
    }
}
