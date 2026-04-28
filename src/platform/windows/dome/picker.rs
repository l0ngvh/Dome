use std::collections::HashMap;
use std::sync::Arc;

use egui::TextureHandle;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, SWP_NOACTIVATE, SWP_NOZORDER,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
};
use windows::core::PCWSTR;

use super::HubEvent;
use super::overlay::{OwnedHwnd, PickerApi, Renderer};
use crate::action::{Action, Actions};
use crate::core::{Dimension, WindowId};
use crate::picker::{PickerEntry, PickerResult};
use crate::platform::windows::HubSender;
use crate::platform::windows::external::HwndId;

pub(in crate::platform::windows) const PICKER_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomePickerOverlay");

const PICKER_WIDTH: u32 = 400;
const PICKER_HEIGHT: u32 = 300;

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
    width: u32,
    height: u32,
    pixels_per_point: f32,
    icon_textures: HashMap<String, Option<TextureHandle>>,
    /// Background threads cannot create TextureHandle (requires egui Context
    /// during render). Raw ColorImage results are staged here until the next
    /// render converts them.
    pending_icons: Vec<(String, egui::ColorImage)>,
}

impl PickerWindow {
    pub(in crate::platform::windows) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        hub_sender: HubSender,
    ) -> anyhow::Result<Box<Self>> {
        let w = PICKER_WIDTH.min(monitor_dim.width as u32);
        let h = PICKER_HEIGHT.min(monitor_dim.height as u32);
        let x = monitor_dim.x as i32 + (monitor_dim.width as i32 - w as i32) / 2;
        let y = monitor_dim.y as i32 + (monitor_dim.height as i32 - h as i32) / 2;

        let window = OwnedHwnd::new(
            PICKER_OVERLAY_CLASS,
            windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW
                | windows::Win32::UI::WindowsAndMessaging::WS_EX_TOPMOST,
        )?;
        let hwnd = window.hwnd();
        let renderer = Renderer::new(instance, device, queue, hwnd, w, h, true)?;
        renderer.set_visuals(egui::Visuals::dark());

        let mut boxed = Box::new(Self {
            renderer,
            events: Vec::new(),
            entries,
            selected_index: 0,
            hub_sender,
            window,
            width: w,
            height: h,
            pixels_per_point: 1.0,
            icon_textures: HashMap::new(),
            pending_icons: Vec::new(),
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                x,
                y,
                w as i32,
                h as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
            .ok();
        }
        boxed.window.show();
        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed for picker window");
        }
        boxed.rerender();
        Ok(boxed)
    }

    fn show(&mut self, entries: Vec<PickerEntry>, monitor_dim: Dimension) {
        let w = PICKER_WIDTH.min(monitor_dim.width as u32);
        let h = PICKER_HEIGHT.min(monitor_dim.height as u32);
        let x = monitor_dim.x as i32 + (monitor_dim.width as i32 - w as i32) / 2;
        let y = monitor_dim.y as i32 + (monitor_dim.height as i32 - h as i32) / 2;
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                x,
                y,
                w as i32,
                h as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
            .ok();
        }
        if self.width != w || self.height != h {
            self.renderer.resize(w, h);
            self.width = w;
            self.height = h;
        }
        self.entries = entries;
        self.selected_index = 0;
        self.pending_icons.clear();
        self.window.show();
        if !unsafe { SetForegroundWindow(self.window.hwnd()) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed for picker window");
        }
        self.rerender();
    }

    fn hide(&mut self) {
        self.window.hide();
    }

    fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}

/// Delegates to inherent methods via fully-qualified syntax (`PickerWindow::show`)
/// to avoid infinite recursion, since the trait methods have the same names.
impl PickerApi for PickerWindow {
    fn show(&mut self, entries: Vec<PickerEntry>, monitor_dim: Dimension) {
        PickerWindow::show(self, entries, monitor_dim);
    }

    fn hide(&mut self) {
        PickerWindow::hide(self);
    }

    fn is_visible(&self) -> bool {
        PickerWindow::is_visible(self)
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
        let mut pending_opt = Some(pending);
        let (result, new_textures) = self.renderer.render(
            self.width,
            self.height,
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
                let result = crate::picker::paint_picker(ctx, entries, selected_index, &all_icons);
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
    if msg == WM_ERASEBKGND {
        return LRESULT(1);
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
    use crate::config::Config;
    use crate::core::{Dimension, Hub};
    use crate::platform::windows::external::HwndId;

    fn test_hub() -> Hub {
        Hub::new(
            Dimension {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            Config::default().into(),
        )
    }

    #[test]
    fn icons_to_load_filters_dispatched_and_loaded() {
        let mut hub = test_hub();
        let w1 = hub.insert_tiling();
        let w2 = hub.insert_tiling();
        let w3 = hub.insert_tiling();
        let w4 = hub.insert_tiling();
        let entries = vec![
            PickerEntry {
                id: w1,
                title: "Win A".to_string(),
                app_id: Some("a".to_string()),
            },
            PickerEntry {
                id: w2,
                title: "Win B".to_string(),
                app_id: Some("b".to_string()),
            },
            PickerEntry {
                id: w3,
                title: "Win C".to_string(),
                app_id: None,
            },
            PickerEntry {
                id: w4,
                title: "Win D".to_string(),
                app_id: Some("c".to_string()),
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
}
