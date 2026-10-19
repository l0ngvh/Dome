use crate::platform::windows::external::HwndId;
use crate::platform::windows::handle::get_invisible_border;
use crate::platform::windows::ui::window::Window;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_THUMBNAIL_PROPERTIES, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties,
};

pub(in crate::platform::windows) struct NativeThumbnail {
    registration: isize,
    source: HWND,
}
impl NativeThumbnail {
    pub(in crate::platform::windows) fn invisible_border(&self) -> (i32, i32) {
        let (left, top, _, _) = get_invisible_border(self.source);
        (left, top)
    }
    pub(in crate::platform::windows) fn update(&mut self, properties: &DWM_THUMBNAIL_PROPERTIES) {
        if let Err(error) = unsafe { DwmUpdateThumbnailProperties(self.registration, properties) } {
            tracing::debug!(source = ?self.source, %error, "DwmUpdateThumbnailProperties failed");
        }
    }
}
impl Drop for NativeThumbnail {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DwmUnregisterThumbnail(self.registration) } {
            tracing::debug!(source = ?self.source, %error, "DwmUnregisterThumbnail failed");
        }
    }
}

pub(in crate::platform::windows) fn register_native_thumbnail(
    destination: &Window,
    source: HwndId,
) -> anyhow::Result<NativeThumbnail> {
    let source = HWND::from(source);
    let registration = unsafe { DwmRegisterThumbnail(destination.hwnd(), source)? };
    Ok(NativeThumbnail {
        registration,
        source,
    })
}
