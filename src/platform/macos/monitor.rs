use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID};
use objc2_foundation::{NSNumber, NSString};

use crate::core::Dimension;

#[derive(Clone)]
pub(super) struct MonitorInfo {
    pub(super) display_id: CGDirectDisplayID,
    pub(super) name: String,
    pub(super) dimension: Dimension,
    pub(super) full_height: f32,
    pub(super) is_primary: bool,
    pub(super) scale: f64,
}

impl std::fmt::Display for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, dim={:?}, scale={})",
            self.name, self.display_id, self.dimension, self.scale
        )
    }
}

fn get_display_id(screen: &NSScreen) -> CGDirectDisplayID {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    desc.objectForKey(&key)
        .and_then(|obj| {
            let num: Option<&NSNumber> = obj.downcast_ref();
            num.map(|n| n.unsignedIntValue())
        })
        .unwrap_or(0)
}

pub(super) fn get_all_screens(mtm: MainThreadMarker) -> Vec<MonitorInfo> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen);
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            MonitorInfo {
                display_id,
                name,
                dimension: Dimension {
                    x: bounds.origin.x as f32,
                    y: (bounds.origin.y + top_inset) as f32,
                    width: bounds.size.width as f32,
                    height: (bounds.size.height - top_inset - bottom_inset) as f32,
                },
                full_height: bounds.size.height as f32,
                is_primary: display_id == primary_id,
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}

pub(super) fn primary_full_height_from(monitors: &[MonitorInfo]) -> f32 {
    monitors.iter().find(|s| s.is_primary).unwrap().full_height
}
