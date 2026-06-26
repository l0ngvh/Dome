use std::collections::HashMap;
use std::sync::Arc;

use egui::TextureHandle;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, SWP_NOACTIVATE, SWP_NOZORDER,
    SetWindowLongPtrW, SetWindowPos, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT,
};
use windows::core::PCWSTR;

fn configure_picker_dwm(hwnd: HWND) {
    let preference = DWMWCP_ROUND;
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const DWM_WINDOW_CORNER_PREFERENCE as *const _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    };
    if let Err(e) = result {
        // Windows 11 22000+ only. Older versions return E_INVALIDARG; fall through without
        // rounded corners and log at debug so we don't spam the console.
        tracing::debug!("DWM corner preference not supported: {e:#}");
    }
}

use super::HubEvent;
use super::overlay::{OwnedHwnd, PickerApi, Renderer};
use crate::action::{Action, Actions};
use crate::config::Config;
use crate::core::{Dimension, Physical, WindowId};
use crate::picker::{PickerEntry, PickerResult};
use crate::platform::windows::HubSender;
use crate::platform::windows::external::HwndId;
use crate::theme::Theme;

const PICKER_WIDTH_LOGICAL: f32 = 400.0;
const PICKER_HEIGHT_LOGICAL: f32 = 300.0;

/// Compute centred physical-pixel rect for the picker window.
/// Scales the logical 400x300 picker size to physical at entry, then clamps
/// and centres within the (already physical) monitor rect.
/// The `.max(1)` floor on dimensions prevents wgpu surface validation failure
/// at degenerate scales.
fn picker_physical_rect(scale: f32, monitor_physical: Dimension<Physical>) -> (i32, i32, u32, u32) {
    let picker_w = PICKER_WIDTH_LOGICAL * scale;
    let picker_h = PICKER_HEIGHT_LOGICAL * scale;
    let w = picker_w.min(monitor_physical.width.value());
    let h = picker_h.min(monitor_physical.height.value());
    let x = monitor_physical.x.value() + (monitor_physical.width.value() - w) / 2.0;
    let y = monitor_physical.y.value() + (monitor_physical.height.value() - h) / 2.0;
    (
        x.round() as i32,
        y.round() as i32,
        w.round().max(1.0) as u32,
        h.round().max(1.0) as u32,
    )
}

pub(in crate::platform::windows) const PICKER_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomePickerOverlay");

/// Returns `(app_id, hwnd_id)` pairs for entries that need icon loading.
/// Skips entries with no `app_id` and entries already present in `icon_textures`
/// (either loaded or in-flight).
pub(super) fn collect_icons_to_load(
    entries: &[PickerEntry],
    icon_textures: &HashMap<String, Option<TextureHandle>>,
    lookup_hwnd: impl Fn(WindowId) -> Option<HwndId>,
) -> Vec<(String, HwndId)> {
    entries
        .iter()
        .filter_map(|entry| {
            let app_id = entry.app_id.as_ref()?;
            if icon_textures.contains_key(app_id) {
                return None;
            }
            let hwnd_id = lookup_hwnd(entry.id)?;
            Some((app_id.clone(), hwnd_id))
        })
        .collect()
}

/// Opaque picker popup window for browsing and restoring minimized windows.
/// `renderer` is declared before `window` so it drops first (renderer cleanup before HWND destruction).
pub(in crate::platform::windows) struct PickerWindow {
    renderer: Renderer,
    events: Vec<egui::Event>,
    entries: Vec<PickerEntry>,
    selected_index: usize,
    hub_sender: HubSender,
    window: OwnedHwnd,
    width_phys: u32,
    height_phys: u32,
    pixels_per_point: f32,
    icon_textures: HashMap<String, Option<TextureHandle>>,
    /// Background threads cannot create TextureHandle (requires egui Context
    /// during render). Raw ColorImage results are staged here until the next
    /// render converts them.
    pending_icons: Vec<(String, egui::ColorImage)>,
    config: Config,
}

impl PickerWindow {
    /// Creates the picker window hidden. Call [`show`](PickerWindow::show) to
    /// position, size, and reveal it with entries.
    pub(in crate::platform::windows) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        hub_sender: HubSender,
        config: Config,
    ) -> anyhow::Result<Box<Self>> {
        // Placeholder size; show() repositions and resizes to the correct
        // monitor-relative rect on first reveal.
        let w_phys: u32 = 400;
        let h_phys: u32 = 300;

        let window = OwnedHwnd::new(
            PICKER_OVERLAY_CLASS,
            windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW
                | windows::Win32::UI::WindowsAndMessaging::WS_EX_TOPMOST,
            0,
            0,
            w_phys,
            h_phys,
        )?;
        let hwnd = window.hwnd();
        configure_picker_dwm(hwnd);
        let renderer = Renderer::new(
            instance,
            device,
            queue,
            hwnd,
            w_phys,
            h_phys,
            config.theme,
            &config.font,
        )?;

        let mut boxed = Box::new(Self {
            renderer,
            events: Vec::new(),
            entries: Vec::new(),
            selected_index: 0,
            hub_sender,
            window,
            width_phys: w_phys,
            height_phys: h_phys,
            pixels_per_point: 1.0,
            icon_textures: HashMap::new(),
            pending_icons: Vec::new(),
            config,
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };
        Ok(boxed)
    }
}

/// Delegates to inherent methods via fully-qualified syntax (`PickerWindow::show`)
/// to avoid infinite recursion, since the trait methods have the same names.
impl PickerApi for PickerWindow {
    fn show(&mut self, entries: Vec<PickerEntry>, monitor_dim: Dimension, scale: f32) {
        let (x, y, w_phys, h_phys) = picker_physical_rect(scale, monitor_dim);
        if let Err(e) = unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                x,
                y,
                w_phys as i32,
                h_phys as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        } {
            tracing::trace!("picker SetWindowPos failed: {e}");
        }
        // Clear cached icon textures when the monitor scale changes so icons
        // are re-captured at the new physical density.
        if self.pixels_per_point != scale {
            self.icon_textures.clear();
        }
        if self.width_phys != w_phys || self.height_phys != h_phys {
            self.renderer.resize(w_phys, h_phys);
            self.width_phys = w_phys;
            self.height_phys = h_phys;
        }
        self.entries = entries;
        self.selected_index = 0;
        self.pixels_per_point = scale;
        self.pending_icons.clear();
        self.window.show();
        crate::platform::windows::handle::force_set_foreground(self.window.hwnd());
        self.rerender();
    }

    fn hide(&mut self) {
        self.window.hide();
    }

    fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    fn icons_to_load(
        &mut self,
        lookup_hwnd: &dyn Fn(WindowId) -> Option<HwndId>,
    ) -> Vec<(String, HwndId)> {
        let to_load = collect_icons_to_load(&self.entries, &self.icon_textures, lookup_hwnd);
        for (app_id, _) in &to_load {
            self.icon_textures.insert(app_id.clone(), None);
        }
        to_load
    }

    fn receive_icon(&mut self, app_id: String, image: egui::ColorImage) {
        self.pending_icons.push((app_id, image));
    }

    fn rerender(&mut self) {
        let pending: Vec<_> = self.pending_icons.drain(..).collect();
        let events = std::mem::take(&mut self.events);
        let entries = &self.entries;
        let selected_index = self.selected_index;
        let icon_textures = &self.icon_textures;
        let flavor = self.config.theme;
        let mut pending_opt = Some(pending);
        let (result, new_textures) = self.renderer.render(
            self.width_phys,
            self.height_phys,
            self.pixels_per_point,
            events,
            |ctx| {
                // Take pending out of the Option so it's only consumed on the first call.
                // Renderer::render calls the closure exactly once.
                let pending = pending_opt.take().unwrap_or_default();
                let new_handles: Vec<(String, TextureHandle)> = pending
                    .into_iter()
                    .map(|(app_id, image)| {
                        let handle = ctx.load_texture(
                            "icon",
                            image,
                            Default::default(), // TextureOptions default is fine for icon textures
                        );
                        (app_id, handle)
                    })
                    .collect();
                let mut all_icons: HashMap<String, Option<TextureHandle>> = icon_textures.clone();
                for (id, handle) in &new_handles {
                    all_icons.insert(id.clone(), Some(handle.clone()));
                }
                let result = crate::picker::paint_picker(
                    ctx,
                    entries,
                    selected_index,
                    &all_icons,
                    &Theme::from_flavor(flavor),
                );
                (result, new_handles)
            },
        );
        for (id, handle) in new_textures {
            self.icon_textures.insert(id, Some(handle));
        }
        if let PickerResult::Selected(id) = result {
            let actions = Actions::new(vec![Action::UnminimizeWindow(id)]);
            self.hub_sender.send(HubEvent::Action(actions));
            self.window.hide();
        }
    }

    fn set_config(&mut self, config: &Config) {
        if self.config.theme != config.theme {
            self.renderer.apply_theme(config.theme);
        }
        if self.config.font != config.font {
            if self.config.font.family != config.font.family {
                self.reinstall_fonts(config.font.family.as_deref());
            }
            self.renderer.apply_font(&config.font);
        }
        self.config = config.clone();
        if self.is_visible() {
            self.rerender();
        }
    }
}

impl PickerWindow {
    fn reinstall_fonts(&mut self, family: Option<&str>) {
        if let Some(family) = family {
            match crate::platform::windows::font::resolve_system_font(family) {
                Ok(bytes) => crate::font::install_fonts(bytes, self.renderer.egui_ctx()),
                Err(e) => tracing::warn!(
                    family = %family,
                    error = %e,
                    "font reload failed"
                ),
            }
        }
    }
}

impl Drop for PickerWindow {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
    }
}

pub(in crate::platform::windows) unsafe extern "system" fn picker_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(lr) = crate::platform::windows::dome_wnd_proc_common(hwnd, msg, wparam, lparam) {
        return lr;
    }
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut PickerWindow;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let picker = unsafe { &mut *ptr };
    match msg {
        WM_KEYDOWN => {
            let vk = VIRTUAL_KEY(wparam.0 as u16);
            match vk {
                VK_UP => {
                    if picker.selected_index > 0 {
                        picker.selected_index -= 1;
                        picker.rerender();
                    }
                    return LRESULT(0);
                }
                VK_DOWN => {
                    let max = picker.entries.len().saturating_sub(1);
                    if picker.selected_index < max {
                        picker.selected_index += 1;
                        picker.rerender();
                    }
                    return LRESULT(0);
                }
                VK_RETURN => {
                    if let Some(entry) = picker.entries.get(picker.selected_index) {
                        let actions = Actions::new(vec![Action::UnminimizeWindow(entry.id)]);
                        picker.hub_sender.send(HubEvent::Action(actions));
                    }
                    picker.window.hide();
                    return LRESULT(0);
                }
                VK_ESCAPE => {
                    picker.window.hide();
                    return LRESULT(0);
                }
                _ => {}
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            picker
                .events
                .push(egui::Event::PointerMoved(egui::pos2(x, y)));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            picker.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            picker.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            picker.rerender();
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            // No event sent. OwnedHwnd::hide() is a no-op if already hidden
            // (e.g. after VK_ESCAPE/VK_RETURN already hid the window).
            picker.window.hide();
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps).ok().ok();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LayoutConfig;
    use crate::core::{Dimension, Hub, Length, Physical};
    use crate::platform::windows::external::HwndId;

    fn test_hub() -> Hub {
        Hub::new(
            Dimension::new(
                Length::new(0.0),
                Length::new(0.0),
                Length::new(100.0),
                Length::new(100.0),
            ),
            1.0,
            LayoutConfig::default(),
        )
    }

    #[test]
    fn icons_to_load_filters_dispatched_and_loaded() {
        let mut hub = test_hub();
        let w1 = hub.insert_tiling(hub.current_workspace());
        let w2 = hub.insert_tiling(hub.current_workspace());
        let w3 = hub.insert_tiling(hub.current_workspace());
        let w4 = hub.insert_tiling(hub.current_workspace());
        let entries = vec![
            PickerEntry {
                id: w1,
                title: "Win A".to_string(),
                app_id: Some("a".to_string()),
                app_name: None,
            },
            PickerEntry {
                id: w2,
                title: "Win B".to_string(),
                app_id: Some("b".to_string()),
                app_name: None,
            },
            PickerEntry {
                id: w3,
                title: "Win C".to_string(),
                app_id: None,
                app_name: None,
            },
            PickerEntry {
                id: w4,
                title: "Win D".to_string(),
                app_id: Some("c".to_string()),
                app_name: None,
            },
        ];

        // "a" is already in-flight (None sentinel)
        let mut icon_textures: HashMap<String, Option<TextureHandle>> = HashMap::new();
        icon_textures.insert("a".to_string(), None);

        let lookup_hwnd = |wid: WindowId| {
            // Map each test WindowId to a distinct HwndId
            let n = match wid {
                w if w == w1 => 1,
                w if w == w2 => 2,
                w if w == w3 => 3,
                w if w == w4 => 4,
                _ => return None,
            };
            Some(HwndId::test(n))
        };

        let mut result = collect_icons_to_load(&entries, &icon_textures, lookup_hwnd);
        result.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "b");
        assert_eq!(result[1].0, "c");

        // Simulate inserting returned app_ids as None (in-flight)
        for (app_id, _) in &result {
            icon_textures.insert(app_id.clone(), None);
        }

        // Second call should return empty
        let result2 = collect_icons_to_load(&entries, &icon_textures, lookup_hwnd);
        assert!(result2.is_empty());
    }

    #[test]
    fn picker_physical_rect_scale_table() {
        let cases = [
            (1.0, 1920.0, 1080.0, (760, 390, 400, 300)),
            (1.5, 2880.0, 1620.0, (1140, 585, 600, 450)),
            (2.0, 3840.0, 2160.0, (1520, 780, 800, 600)),
        ];
        for (scale, w, h, expected) in cases {
            let monitor = Dimension::<Physical>::new(
                Length::new(0.0),
                Length::new(0.0),
                Length::new(w),
                Length::new(h),
            );
            assert_eq!(
                picker_physical_rect(scale, monitor),
                expected,
                "scale={scale}"
            );
        }
    }

    #[test]
    fn picker_physical_rect_centers_offset_origin() {
        let monitor = Dimension::<Physical>::new(
            Length::new(200.0),
            Length::new(100.0),
            Length::new(3840.0),
            Length::new(2160.0),
        );
        assert_eq!(picker_physical_rect(2.0, monitor), (1720, 880, 800, 600));
    }

    #[test]
    fn picker_physical_rect_clamped_to_monitor() {
        let monitor = Dimension::<Physical>::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(400.0),
            Length::new(200.0),
        );
        assert_eq!(picker_physical_rect(2.0, monitor), (0, 0, 400, 200));
    }

    #[test]
    fn picker_physical_rect_exact_monitor_match() {
        let monitor = Dimension::<Physical>::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(400.0),
            Length::new(300.0),
        );
        assert_eq!(picker_physical_rect(1.0, monitor), (0, 0, 400, 300));
    }
}
