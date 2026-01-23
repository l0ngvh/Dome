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
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
    GetWindowLongPtrW, HWND_TOPMOST, PostMessageW, RegisterClassW, SW_SHOWNA, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WM_PAINT, WM_QUIT, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use super::dome::{AppHandle, HubEvent, Overlays, WM_APP_FRAME};
use crate::core::Dimension;

pub(super) struct App {
    hwnd: HWND,
    dc_target: ID2D1DCRenderTarget,
    mem_dc: HDC,
    bitmap: HGDIOBJ,
    text_format: IDWriteTextFormat,
    text_format_bold: IDWriteTextFormat,
    screen: Dimension,
    hub_sender: Sender<HubEvent>,
}

impl App {
    pub(super) fn new(
        screen: Dimension,
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

        // WS_EX_LAYERED + WS_EX_TRANSPARENT: enables click-through to other processes
        // WS_EX_TOOLWINDOW: hides from taskbar and alt-tab
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
                class_name,
                windows::core::w!(""),
                WS_POPUP,
                screen.x as i32,
                screen.y as i32,
                screen.width as i32,
                screen.height as i32,
                None,
                None,
                Some(hinstance.into()),
                None,
            )?
        };

        let (dc_target, mem_dc, bitmap) =
            create_render_resources(screen.width as u32, screen.height as u32)?;

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

        let app = Box::new(Self {
            hwnd,
            dc_target,
            mem_dc,
            bitmap,
            text_format,
            text_format_bold,
            screen,
            hub_sender,
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

    fn process_overlays(&mut self, overlays: Overlays) -> anyhow::Result<()> {
        self.render(&overlays)?;
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )?
        };
        Ok(())
    }

    fn render(&self, overlays: &Overlays) -> anyhow::Result<()> {
        let width = self.screen.width as i32;
        let height = self.screen.height as i32;

        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            self.dc_target.BindDC(self.mem_dc, &rect)?;

            self.dc_target.BeginDraw();
            self.dc_target.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));

            for rect in &overlays.rects {
                let brush = self.dc_target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: rect.color.r * rect.color.a, // premultiply
                        g: rect.color.g * rect.color.a,
                        b: rect.color.b * rect.color.a,
                        a: rect.color.a,
                    },
                    None,
                )?;
                self.dc_target.FillRectangle(
                    &D2D_RECT_F {
                        left: rect.x,
                        top: rect.y,
                        right: rect.x + rect.width,
                        bottom: rect.y + rect.height,
                    },
                    &brush,
                );
            }

            for label in &overlays.labels {
                let brush = self.dc_target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: label.color.r,
                        g: label.color.g,
                        b: label.color.b,
                        a: label.color.a,
                    },
                    None,
                )?;
                let text: Vec<u16> = label.text.encode_utf16().collect();
                let format = if label.bold {
                    &self.text_format_bold
                } else {
                    &self.text_format
                };
                self.dc_target.DrawText(
                    &text,
                    format,
                    &D2D_RECT_F {
                        left: label.x,
                        top: label.y,
                        right: label.x + 1000.0,
                        bottom: label.y + 20.0,
                    },
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    Default::default(),
                );
            }

            self.dc_target.EndDraw(None, None)?;

            // Update layered window with alpha blending
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let src_point = POINT { x: 0, y: 0 };
            let dst_point = POINT {
                x: self.screen.x as i32,
                y: self.screen.y as i32,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&dst_point),
                Some(&size),
                Some(self.mem_dc),
                Some(&src_point),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )?;
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.mem_dc);
            let _ = DeleteObject(self.bitmap);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn create_render_resources(
    width: u32,
    height: u32,
) -> windows::core::Result<(ID2D1DCRenderTarget, HDC, HGDIOBJ)> {
    unsafe {
        // Create D2D factory and DC render target
        let d2d_factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

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

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_FRAME => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            let overlays = unsafe { *Box::from_raw(wparam.0 as *mut Overlays) };
            if let Err(e) = unsafe { (*ptr).process_overlays(overlays) } {
                tracing::warn!("process_overlays failed: {e}");
            }
            LRESULT(0)
        }
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
