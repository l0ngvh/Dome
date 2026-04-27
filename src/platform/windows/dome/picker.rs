use std::sync::Arc;

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
use crate::picker::PickerResult;
use crate::platform::windows::HubSender;

pub(in crate::platform::windows) const PICKER_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomePickerOverlay");

const PICKER_WIDTH: u32 = 400;
const PICKER_HEIGHT: u32 = 300;

/// Opaque picker popup window for browsing and restoring minimized windows.
/// `renderer` is declared before `window` so it drops first (renderer cleanup before HWND destruction).
pub(in crate::platform::windows) struct PickerWindow {
    renderer: Renderer,
    events: Vec<egui::Event>,
    entries: Vec<(WindowId, String)>,
    selected_index: usize,
    hub_sender: HubSender,
    window: OwnedHwnd,
    width: u32,
    height: u32,
    pixels_per_point: f32,
}

impl PickerWindow {
    pub(in crate::platform::windows) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        entries: Vec<(WindowId, String)>,
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

    fn show(&mut self, entries: Vec<(WindowId, String)>, monitor_dim: Dimension) {
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

    fn rerender(&mut self) {
        let events = std::mem::take(&mut self.events);
        let entries = &self.entries;
        let selected_index = self.selected_index;
        let result = self.renderer.render(
            self.width,
            self.height,
            self.pixels_per_point,
            events,
            |ctx| crate::picker::paint_picker(ctx, entries, selected_index),
        );
        if let PickerResult::Selected(id) = result {
            let actions = Actions::new(vec![Action::UnminimizeWindow(id)]);
            self.hub_sender.send(HubEvent::Action(actions));
            self.window.hide();
        }
    }
}

/// Delegates to inherent methods via fully-qualified syntax (`PickerWindow::show`)
/// to avoid infinite recursion, since the trait methods have the same names.
impl PickerApi for PickerWindow {
    fn show(&mut self, entries: Vec<(WindowId, String)>, monitor_dim: Dimension) {
        PickerWindow::show(self, entries, monitor_dim);
    }

    fn hide(&mut self) {
        PickerWindow::hide(self);
    }

    fn is_visible(&self) -> bool {
        PickerWindow::is_visible(self)
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
                    if let Some(&(id, _)) = picker.entries.get(picker.selected_index) {
                        let actions = Actions::new(vec![Action::UnminimizeWindow(id)]);
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
