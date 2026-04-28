use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageRep, NSRunningApplication};
use objc2_foundation::NSString;

const ICON_PIXELS: usize = 48;

/// Loads the application icon for a given bundle identifier as a 48x48 RGBA image.
/// Runs synchronously on the main thread. NSImage is thread-unsafe per Apple docs,
/// so this must not be called from background threads.
pub(super) fn load_app_icon(bundle_id: &str) -> Option<egui::ColorImage> {
    let ns_bundle_id = NSString::from_str(bundle_id);
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&ns_bundle_id);
    let app = apps.firstObject()?;
    let icon = app.icon()?;

    // Force the NSImage to render at our target size
    icon.setSize(objc2_foundation::NSSize::new(
        ICON_PIXELS as f64,
        ICON_PIXELS as f64,
    ));

    let tiff_data = icon.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff_data)?;

    let width = bitmap.pixelsWide() as usize;
    let height = bitmap.pixelsHigh() as usize;
    if width == 0 || height == 0 {
        return None;
    }

    let ptr = bitmap.bitmapData();
    if ptr.is_null() {
        return None;
    }

    let samples = bitmap.samplesPerPixel() as usize;
    let bytes_per_row = bitmap.bytesPerRow() as usize;
    let has_alpha = bitmap.hasAlpha();

    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let row_start = y * bytes_per_row;
        for x in 0..width {
            let offset = row_start + x * samples;
            let (r, g, b, a) = unsafe {
                let r = *ptr.add(offset);
                let g = *ptr.add(offset + 1);
                let b = *ptr.add(offset + 2);
                let a = if has_alpha && samples >= 4 {
                    *ptr.add(offset + 3)
                } else {
                    255
                };
                (r, g, b, a)
            };
            pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
        }
    }

    Some(egui::ColorImage::new([width, height], pixels))
}
