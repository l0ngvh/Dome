use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::WindowId;
use crate::core::{
    ColumnConfig, PixelRect, ScrollingConfig, SizeConstraint, Strategy, WindowMatcher,
};

use super::{LayoutWorkspaceConfigBuilder, TestHubBuilder, TilingConfigBuilder};

mod focus;
mod placement;
mod scroll;

pub(super) fn scrolling_hub() -> Hub {
    scrolling_hub_with(ScrollingConfig::default())
}

pub(super) fn scrolling_hub_with(scrolling: ScrollingConfig) -> Hub {
    TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Scrolling)
                .with_scrolling_config(scrolling)
                .build(),
        )
        .build()
}

/// Focus stays on workspace `0`, so a test focuses `dev` before it inserts there.
pub(in crate::core::tests) fn scrolling_layout_hub(columns: Vec<ColumnConfig>) -> Hub {
    TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Scrolling)
                .with_scrolling_config(ScrollingConfig {
                    default_column_width: SizeConstraint::Percent(20.0),
                })
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_strategy(Strategy::Scrolling)
                .with_columns(columns)
                .build(),
        ])
        .build()
}

pub(in crate::core::tests) fn process_matcher(process: &str) -> WindowMatcher {
    WindowMatcher {
        process: Some(process.into()),
        ..Default::default()
    }
}

/// Tiling border boxes for the primary monitor, sorted by window id.
pub(super) fn border_boxes(hub: &Hub) -> Vec<PixelRect> {
    border_boxes_by_window(hub)
        .into_iter()
        .map(|(_, b)| b)
        .collect()
}

pub(super) fn border_boxes_by_window(hub: &Hub) -> Vec<(WindowId, PixelRect)> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { tiling_windows, .. } = &placements.monitors[0].layout else {
        panic!("expected a normal layout");
    };
    let mut boxes: Vec<(WindowId, PixelRect)> = tiling_windows
        .iter()
        .map(|w| (w.id, w.border_box))
        .collect();
    boxes.sort_by_key(|(id, _)| id.get());
    boxes
}
