use super::ThumbnailPlacement;
use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::thumbnail::NativeThumbnail;
use crate::platform::windows::ui::window::Window;
use crate::platform::windows::ui::window::WindowAttributes;
use crate::platform::windows::ui::window::WindowInput;
use crate::platform::windows::window_server::NativeWindowServer;
use dome_core::WindowId;
use std::rc::Rc;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Dwm::{
    DWM_THUMBNAIL_PROPERTIES, DWM_TNP_RECTDESTINATION, DWM_TNP_RECTSOURCE, DWM_TNP_VISIBLE,
};

/// An overlay over a parked tiling window's on-screen part, where DWM draws that part from the
/// window itself. DWM keeps the thumbnail live, so a frame never passes through Dome.
pub(in crate::platform::windows) struct ThumbnailOverlay {
    thumbnail: NativeThumbnail,
    aux: Window,
}

impl ThumbnailOverlay {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        server: &NativeWindowServer,
        source: WindowId,
        show: &ThumbnailPlacement,
    ) -> anyhow::Result<Self> {
        let aux = Window::new(
            callback_panic,
            &WindowAttributes {
                position: show.frame.position(),
                size: show.frame.size(),
                click_through: false,
                focusable: false,
                dpi_changes: None,
            },
            Some(WindowInput::Thumbnail(source)),
        )?;
        let thumbnail = server.register_thumbnail(&aux, source)?;
        let mut overlay = Self { thumbnail, aux };
        overlay.update(show);
        Ok(overlay)
    }
}

impl ThumbnailOverlay {
    pub(in crate::platform::windows) fn update(&mut self, show: &ThumbnailPlacement) {
        self.aux.set_frame(show.frame.position(), show.frame.size());
        self.aux.set_visible(true);
        let properties = thumbnail_properties(show, self.thumbnail.invisible_border());
        self.thumbnail.update(&properties);
    }
}

fn thumbnail_properties(
    show: &ThumbnailPlacement,
    (border_left, border_top): (i32, i32),
) -> DWM_THUMBNAIL_PROPERTIES {
    let width = show.frame.width().value();
    let height = show.frame.height().value();
    // `rcSource` starts at the invisible resize border reported by `GetWindowRect`,
    // while `source_rect` is relative to the visible frame.
    let source = show.source_rect;
    let left = source.x().value() + border_left;
    let top = source.y().value() + border_top;
    DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_RECTSOURCE | DWM_TNP_VISIBLE,
        rcDestination: RECT {
            left: 0,
            top: 0,
            right: i32::try_from(width).expect("thumbnail width out of range"),
            bottom: i32::try_from(height).expect("thumbnail height out of range"),
        },
        rcSource: RECT {
            left,
            top,
            right: (source.right() + dome_core::Offset::new(i64::from(border_left))).value(),
            bottom: (source.bottom() + dome_core::Offset::new(i64::from(border_top))).value(),
        },
        fVisible: true.into(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dome_core::PixelRect;

    #[test]
    fn crop_thumbnail_offsets_source_border_and_keeps_destination_local() {
        for (frame, source_rect, expected_source, expected_destination) in [
            (
                PixelRect::new(20, 30, 40, 50),
                PixelRect::new(3, 5, 40, 50),
                (10, 16, 50, 66),
                (0, 0, 40, 50),
            ),
            (
                PixelRect::new(-10, 60, 25, 35),
                PixelRect::new(9, 2, 25, 35),
                (16, 13, 41, 48),
                (0, 0, 25, 35),
            ),
        ] {
            let properties =
                thumbnail_properties(&ThumbnailPlacement { frame, source_rect }, (7, 11));
            let source = properties.rcSource;
            assert_eq!(
                (source.left, source.top, source.right, source.bottom),
                expected_source
            );
            let destination = properties.rcDestination;
            assert_eq!(
                (
                    destination.left,
                    destination.top,
                    destination.right,
                    destination.bottom
                ),
                expected_destination
            );
        }
    }
}
