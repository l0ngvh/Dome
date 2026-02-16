use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::ptr;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory,
    ID2D1DCRenderTarget, ID2D1Factory,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWriteCreateFactory, IDWriteFactory,
    IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HDC, HGDIOBJ,
    SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDPREV,
    GWLP_USERDATA, GetWindow, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST,
    PostMessageW, RegisterClassW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WM_DISPLAYCHANGE, WM_PAINT, WM_QUIT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};

use super::dome::{AppHandle, HubEvent, OverlayFrame, TabBarInfo, WM_APP_OVERLAY};
use super::get_all_screens;
use crate::config::Color;
use crate::core::{ContainerId, Dimension, WindowId};

struct OverlayWindow {
    hwnd: HWND,
    managed_hwnd: HWND,
    dc_target: ID2D1DCRenderTarget,
    mem_dc: HDC,
    bitmap: HGDIOBJ,
    width: u32,
    height: u32,
    is_float: bool,
    is_visible: bool,
    text_format: Option<IDWriteTextFormat>,
    text_format_bold: Option<IDWriteTextFormat>,
}

pub(super) struct App {
    hwnd: HWND,
    d2d_factory: ID2D1Factory,
    text_format: IDWriteTextFormat,
    text_format_bold: IDWriteTextFormat,
    dwrite_factory: IDWriteFactory,
    hub_sender: Sender<HubEvent>,
    window_overlays: HashMap<WindowId, OverlayWindow>,
    container_overlays: HashMap<ContainerId, OverlayWindow>,
}

impl App {
    pub(super) fn new(
        hub_sender: Sender<HubEvent>,
    ) -> windows::core::Result<Box<Self>> {
        let class_name = windows::core::w!("DomeApp");
        let hinstance = unsafe { GetModuleHandleW(None)? };

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name,
                windows::core::w!(""),
                WS_POPUP,
                0, 0, 1, 1,
                None,
                None,
                Some(hinstance.into()),
                None,
            )?
        };

        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let text_format = unsafe {
            dwrite_factory.CreateTextFormat(
                windows::core::w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                windows::core::w!(""),
            )?
        };
        let text_format_bold = unsafe {
            dwrite_factory.CreateTextFormat(
                windows::core::w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                windows::core::w!(""),
            )?
        };

        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

        let app = Box::new(Self {
            hwnd,
            d2d_factory,
            text_format,
            text_format_bold,
            dwrite_factory,
            hub_sender,
            window_overlays: HashMap::new(),
            container_overlays: HashMap::new(),
        });

        let ptr = &*app as *const _ as *mut App;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize) };

        app.send_event(HubEvent::AppInitialized(AppHandle::new(hwnd)));

        Ok(app)
    }

    fn send_event(&self, event: HubEvent) {
        if self.hub_sender.send(event).is_err() {
            tracing::error!("Hub thread died, shutting down");
            unsafe { PostMessageW(Some(self.hwnd), WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
        }
    }

    fn handle_overlay_frame(&mut self, frame: OverlayFrame) {
        // Create new window overlays
        for create in frame.creates {
            if let Ok(overlay) = OverlayWindow::new(&self.d2d_factory, create.hwnd, create.is_float) {
                self.window_overlays.insert(create.window_id, overlay);
            }
        }

        // Delete window overlays
        for id in frame.deletes {
            self.window_overlays.remove(&id);
        }

        let mut seen_windows: HashSet<WindowId> = HashSet::new();

        // Update window overlays
        for window in &frame.windows {
            seen_windows.insert(window.window_id);

            if let Some(overlay) = self.window_overlays.get_mut(&window.window_id) {
                // Handle float toggle
                if overlay.is_float != window.is_float {
                    overlay.is_float = window.is_float;
                    unsafe {
                        if window.is_float {
                            SetWindowPos(overlay.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                            SetWindowPos(overlay.managed_hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                        } else {
                            SetWindowPos(overlay.hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                            SetWindowPos(overlay.managed_hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                        }
                        // Re-establish window behind border
                        SetWindowPos(overlay.managed_hwnd, Some(overlay.hwnd), 0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                    }
                }
                overlay.update(&self.d2d_factory, &window.frame, &window.edges);
                overlay.show();
            }
        }

        // Update container overlays
        let frame_container_ids: HashSet<_> = frame.containers.iter()
            .map(|c| c.container_id)
            .collect();
        self.container_overlays.retain(|id, _| frame_container_ids.contains(id));

        for container in &frame.containers {
            let overlay = self.container_overlays
                .entry(container.container_id)
                .or_insert_with(|| OverlayWindow::new_container(
                    &self.d2d_factory,
                    self.text_format.clone(),
                    self.text_format_bold.clone(),
                ).unwrap());

            overlay.update_container(&self.d2d_factory, &self.dwrite_factory, &container.frame, &container.edges, container.tab_bar.as_ref());
            overlay.show();
            unsafe {
                SetWindowPos(overlay.hwnd, Some(HWND_TOP), 0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
            }
        }

        // Hide unseen window overlays (workspace switch)
        for (id, overlay) in &mut self.window_overlays {
            if !seen_windows.contains(id) {
                overlay.hide();
            }
        }

        // Bring focused window's border above its window
        if let Some(id) = frame.focused
            && let Some(overlay) = self.window_overlays.get(&id)
        {
            unsafe {
                let above = GetWindow(overlay.managed_hwnd, GW_HWNDPREV);
                match above {
                    Ok(hwnd) if hwnd != overlay.hwnd => {
                        SetWindowPos(overlay.hwnd, Some(hwnd), 0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                    }
                    _ => {
                        let top = if overlay.is_float { Some(HWND_TOPMOST) } else { Some(HWND_TOP) };
                            SetWindowPos(overlay.hwnd, top, 0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
                    }
                }
            }
        }
    }
}
fn create_render_resources(
    d2d_factory: &ID2D1Factory,
    width: u32,
    height: u32,
) -> windows::core::Result<(ID2D1DCRenderTarget, HDC, HGDIOBJ)> {
    unsafe {
        let render_props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 0.0,
            dpiY: 0.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: Default::default(),
        };

        let dc_target = d2d_factory.CreateDCRenderTarget(&render_props)?;

        // Create memory DC and 32bpp DIB
        let mem_dc = CreateCompatibleDC(None);

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits = ptr::null_mut();
        let bitmap = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
        SelectObject(mem_dc, bitmap.into());

        Ok((dc_target, mem_dc, bitmap.into()))
    }
}

fn color_to_d2d(color: &Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r * color.a,
        g: color.g * color.a,
        b: color.b * color.a,
        a: color.a,
    }
}

impl OverlayWindow {
    fn new(d2d_factory: &ID2D1Factory, managed_hwnd: HWND, is_float: bool) -> windows::core::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                windows::core::w!("DomeApp"),
                windows::core::w!(""),
                WS_POPUP,
                0, 0, 1, 1,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };

        if is_float {
            unsafe {
                SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
            }
        }

        // Position managed window behind border
        unsafe {
            SetWindowPos(managed_hwnd, Some(hwnd), 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
        }

        let (dc_target, mem_dc, bitmap) = create_render_resources(d2d_factory, 1, 1)?;

        Ok(Self {
            hwnd,
            managed_hwnd,
            dc_target,
            mem_dc,
            bitmap,
            width: 1,
            height: 1,
            is_float,
            is_visible: false,
            text_format: None,
            text_format_bold: None,
        })
    }

    fn new_container(
        d2d_factory: &ID2D1Factory,
        text_format: IDWriteTextFormat,
        text_format_bold: IDWriteTextFormat,
    ) -> windows::core::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                windows::core::w!("DomeApp"),
                windows::core::w!(""),
                WS_POPUP,
                0, 0, 1, 1,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };

        let (dc_target, mem_dc, bitmap) = create_render_resources(d2d_factory, 1, 1)?;

        Ok(Self {
            hwnd,
            managed_hwnd: HWND::default(),
            dc_target,
            mem_dc,
            bitmap,
            width: 1,
            height: 1,
            is_float: false,
            is_visible: false,
            text_format: Some(text_format),
            text_format_bold: Some(text_format_bold),
        })
    }

    fn update(&mut self, d2d_factory: &ID2D1Factory, frame: &Dimension, edges: &[(Dimension, Color)]) {
        let w = frame.width.max(1.0) as u32;
        let h = frame.height.max(1.0) as u32;

        if (self.width != w || self.height != h)
            && let Ok((dc_target, mem_dc, bitmap)) = create_render_resources(d2d_factory, w, h)
        {
            unsafe {
                let _ = DeleteDC(self.mem_dc);
                let _ = DeleteObject(self.bitmap);
            }
            self.dc_target = dc_target;
            self.mem_dc = mem_dc;
            self.bitmap = bitmap;
            self.width = w;
            self.height = h;
        }

        self.render_edges(edges, frame);

        unsafe {
            SetWindowPos(self.hwnd, None,
                frame.x as i32, frame.y as i32, w as i32, h as i32,
                SWP_NOZORDER | SWP_NOACTIVATE).ok();
        }
    }

    fn render_edges(&self, edges: &[(Dimension, Color)], frame: &Dimension) {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            };
            if self.dc_target.BindDC(self.mem_dc, &rect).is_err() {
                return;
            }
            self.dc_target.BeginDraw();
            self.dc_target.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));

            for (edge, color) in edges {
                if let Ok(brush) = self.dc_target.CreateSolidColorBrush(&color_to_d2d(color), None) {
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F {
                            left: edge.x - frame.x,
                            top: edge.y - frame.y,
                            right: edge.x - frame.x + edge.width,
                            bottom: edge.y - frame.y + edge.height,
                        },
                        &brush,
                    );
                }
            }

            self.dc_target.EndDraw(None, None).ok();

            let size = SIZE { cx: self.width as i32, cy: self.height as i32 };
            let src_point = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(self.hwnd, None, None, Some(&size), Some(self.mem_dc),
                Some(&src_point), COLORREF(0), Some(&blend), ULW_ALPHA).ok();
        }
    }

    fn update_container(
        &mut self,
        d2d_factory: &ID2D1Factory,
        dwrite_factory: &IDWriteFactory,
        frame: &Dimension,
        edges: &[(Dimension, Color)],
        tab_bar: Option<&TabBarInfo>,
    ) {
        let w = frame.width.max(1.0) as u32;
        let h = frame.height.max(1.0) as u32;

        if (self.width != w || self.height != h)
            && let Ok((dc_target, mem_dc, bitmap)) = create_render_resources(d2d_factory, w, h)
        {
            unsafe {
                let _ = DeleteDC(self.mem_dc);
                let _ = DeleteObject(self.bitmap);
            }
            self.dc_target = dc_target;
            self.mem_dc = mem_dc;
            self.bitmap = bitmap;
            self.width = w;
            self.height = h;
        }

        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            };
            if self.dc_target.BindDC(self.mem_dc, &rect).is_err() {
                return;
            }
            self.dc_target.BeginDraw();
            self.dc_target.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));

            // Render border edges
            for (edge, color) in edges {
                if let Ok(brush) = self.dc_target.CreateSolidColorBrush(&color_to_d2d(color), None) {
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F {
                            left: edge.x - frame.x,
                            top: edge.y - frame.y,
                            right: edge.x - frame.x + edge.width,
                            bottom: edge.y - frame.y + edge.height,
                        },
                        &brush,
                    );
                }
            }

            // Render tab bar
            if let Some(tb) = tab_bar {
                // Background
                if let Ok(bg_brush) = self.dc_target.CreateSolidColorBrush(&color_to_d2d(&tb.background_color), None) {
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F { left: 0.0, top: 0.0, right: frame.width, bottom: tb.height },
                        &bg_brush,
                    );
                }

                // Border
                if let Ok(border_brush) = self.dc_target.CreateSolidColorBrush(&color_to_d2d(&tb.border_color), None) {
                    let b = tb.border;
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F { left: 0.0, top: 0.0, right: frame.width, bottom: b },
                        &border_brush,
                    );
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F { left: 0.0, top: tb.height - b, right: frame.width, bottom: tb.height },
                        &border_brush,
                    );
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F { left: 0.0, top: b, right: b, bottom: tb.height - b },
                        &border_brush,
                    );
                    self.dc_target.FillRectangle(
                        &D2D_RECT_F { left: frame.width - b, top: b, right: frame.width, bottom: tb.height - b },
                        &border_brush,
                    );

                    for (i, tab) in tb.tabs.iter().enumerate() {
                        if i > 0 {
                            self.dc_target.FillRectangle(
                                &D2D_RECT_F {
                                    left: tab.x - frame.x - b / 2.0,
                                    top: 0.0,
                                    right: tab.x - frame.x + b / 2.0,
                                    bottom: tb.height,
                                },
                                &border_brush,
                            );
                        }
                    }
                }

                // Active tab background
                if let Ok(active_brush) = self.dc_target.CreateSolidColorBrush(&color_to_d2d(&tb.active_background_color), None) {
                    for tab in &tb.tabs {
                        if tab.is_active {
                            self.dc_target.FillRectangle(
                                &D2D_RECT_F {
                                    left: tab.x - frame.x,
                                    top: 0.0,
                                    right: tab.x - frame.x + tab.width,
                                    bottom: tb.height,
                                },
                                &active_brush,
                            );
                        }
                    }
                }

                // Tab labels
                if let Ok(text_brush) = self.dc_target.CreateSolidColorBrush(
                    &D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
                    None,
                ) {
                    for tab in &tb.tabs {
                        let format = if tab.is_active {
                            self.text_format_bold.as_ref()
                        } else {
                            self.text_format.as_ref()
                        };
                        if let Some(fmt) = format {
                            let text: Vec<u16> = tab.title.encode_utf16().collect();
                            let tab_left = tab.x - frame.x;
                            let tab_center = tab_left + tab.width / 2.0;

                            // Measure text width
                            let text_width = dwrite_factory
                                .CreateTextLayout(&text, fmt, tab.width, tb.height)
                                .ok()
                                .and_then(|layout| {
                                    let mut metrics = Default::default();
                                    layout.GetMetrics(&mut metrics).ok()?;
                                    Some(metrics.width)
                                })
                                .unwrap_or(0.0);

                            self.dc_target.DrawText(
                                &text,
                                fmt,
                                &D2D_RECT_F {
                                    left: tab_center - text_width / 2.0,
                                    top: tb.height / 2.0 - 6.0,
                                    right: tab_left + tab.width,
                                    bottom: tb.height,
                                },
                                &text_brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                                Default::default(),
                            );
                        }
                    }
                }
            }

            self.dc_target.EndDraw(None, None).ok();

            let size = SIZE { cx: self.width as i32, cy: self.height as i32 };
            let src_point = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(self.hwnd, None, None, Some(&size), Some(self.mem_dc),
                Some(&src_point), COLORREF(0), Some(&blend), ULW_ALPHA).ok();

            SetWindowPos(self.hwnd, None,
                frame.x as i32, frame.y as i32, w as i32, h as i32,
                SWP_NOZORDER | SWP_NOACTIVATE).ok();
        }
    }

    fn show(&mut self) {
        if !self.is_visible {
            unsafe { let _ = ShowWindow(self.hwnd, SW_SHOWNA); }
            self.is_visible = true;
        }
    }

    fn hide(&mut self) {
        if self.is_visible {
            unsafe { let _ = ShowWindow(self.hwnd, SW_HIDE); }
            self.is_visible = false;
        }
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.mem_dc);
            let _ = DeleteObject(self.bitmap);
            DestroyWindow(self.hwnd).ok();
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_OVERLAY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            let frame = unsafe { *Box::from_raw(wparam.0 as *mut OverlayFrame) };
            unsafe { (*ptr).handle_overlay_frame(frame) };
            LRESULT(0)
        }
        // Don't know if this is still relevant
        // https://stackoverflow.com/questions/33762140/what-is-the-notification-when-the-number-of-monitors-changes
        WM_DISPLAYCHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            if !ptr.is_null() {
                match get_all_screens() {
                    Ok(screens) => unsafe { (*ptr).send_event(HubEvent::ScreensChanged(screens)) },
                    Err(e) => tracing::warn!("Failed to enumerate screens: {e}"),
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
