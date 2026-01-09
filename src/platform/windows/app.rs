use std::collections::HashSet;
use std::pin::Pin;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_IMMEDIATELY, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow,
    GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, HWND_TOP, RegisterClassW, SWP_NOACTIVATE,
    SetWindowLongPtrW, SetWindowPos, WM_PAINT, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};

use super::hub::{Frame, OverlayRect, WM_APP_FRAME, WindowHandle};
use super::window::{Taskbar, hide_window, set_window_pos, show_window};
use super::windows_wrapper::set_foreground_window;
use crate::core::Dimension;

pub(super) struct App {
    hwnd: HWND,
    factory: ID2D1Factory,
    render_target: ID2D1HwndRenderTarget,
    rects: Vec<OverlayRect>,
    taskbar: Taskbar,
    displayed: HashSet<WindowHandle>,
    displayed_floats: HashSet<WindowHandle>,
    screen: Dimension,
    border: f32,
}

impl App {
    pub(super) fn new(
        taskbar: Taskbar,
        screen: Dimension,
        border: f32,
    ) -> windows::core::Result<Pin<Box<Self>>> {
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

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
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
                class_name,
                windows::core::w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(hinstance.into()),
                None,
            )?
        };

        let render_target = create_render_target(&factory, hwnd)?;

        let app = Box::pin(Self {
            hwnd,
            factory,
            render_target,
            rects: Vec::new(),
            taskbar,
            displayed: HashSet::new(),
            displayed_floats: HashSet::new(),
            screen,
            border,
        });

        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*app as *const _ as isize) };

        Ok(app)
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn process_frame(&mut self, cmd: Frame) -> anyhow::Result<()> {
        let new_displayed: HashSet<WindowHandle> = cmd.windows.iter().map(|(h, _)| *h).collect();
        let new_floats: HashSet<WindowHandle> = cmd.floats.iter().map(|(h, _)| *h).collect();

        for handle in self.displayed.difference(&new_displayed) {
            hide_window(handle.0);
            self.taskbar.delete_tab(handle.0)?;
        }
        for handle in self.displayed_floats.difference(&new_floats) {
            hide_window(handle.0);
            self.taskbar.delete_tab(handle.0)?;
        }

        for (handle, dim) in &cmd.windows {
            if !self.displayed.contains(handle) {
                show_window(handle.0);
                self.taskbar.add_tab(handle.0)?;
            }
            let inset = Dimension {
                x: dim.x + self.border,
                y: dim.y + self.border,
                width: dim.width - 2.0 * self.border,
                height: dim.height - 2.0 * self.border,
            };
            set_window_pos(handle.0, &inset)?;
        }

        for (handle, dim) in &cmd.floats {
            if !self.displayed_floats.contains(handle) {
                show_window(handle.0);
                self.taskbar.add_tab(handle.0)?;
            }
            let inset = Dimension {
                x: dim.x + self.border,
                y: dim.y + self.border,
                width: dim.width - 2.0 * self.border,
                height: dim.height - 2.0 * self.border,
            };
            set_window_pos(handle.0, &inset)?;
        }

        self.displayed = new_displayed;
        self.displayed_floats = new_floats;

        if let Some(handle) = cmd.focus
            && let Err(e) = focus_window(handle.0)
        {
            tracing::warn!("{e}");
        }

        self.set_overlays(cmd.overlays)
    }

    fn set_overlays(&mut self, rects: Vec<OverlayRect>) -> anyhow::Result<()> {
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                self.screen.x as i32,
                self.screen.y as i32,
                self.screen.width as i32,
                self.screen.height as i32,
                SWP_NOACTIVATE,
            )?
        };

        self.render_target = create_render_target(&self.factory, self.hwnd)?;
        self.rects = rects;
        self.render()?;
        show_window(self.hwnd);
        Ok(())
    }

    fn render(&self) -> anyhow::Result<()> {
        let rt = &self.render_target;

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));

            for rect in &self.rects {
                if let Ok(brush) = rt.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: rect.color.r,
                        g: rect.color.g,
                        b: rect.color.b,
                        a: rect.color.a,
                    },
                    None,
                ) {
                    rt.FillRectangle(
                        &D2D_RECT_F {
                            left: rect.x,
                            top: rect.y,
                            right: rect.x + rect.width,
                            bottom: rect.y + rect.height,
                        },
                        &brush,
                    );
                }
            }

            rt.EndDraw(None, None)?
        };
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Err(e) = unsafe { DestroyWindow(self.hwnd) } {
            tracing::warn!("DestroyWindow failed: {e}");
        }
    }
}

fn create_render_target(
    factory: &ID2D1Factory,
    hwnd: HWND,
) -> windows::core::Result<ID2D1HwndRenderTarget> {
    let mut rc = windows::Win32::Foundation::RECT::default();
    unsafe { GetClientRect(hwnd, &mut rc)? };

    let size = D2D_SIZE_U {
        width: (rc.right - rc.left) as u32,
        height: (rc.bottom - rc.top) as u32,
    };

    let render_props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        ..Default::default()
    };

    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: size,
        presentOptions: D2D1_PRESENT_OPTIONS_IMMEDIATELY,
    };

    unsafe { factory.CreateHwndRenderTarget(&render_props, &hwnd_props) }
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
            let cmd = unsafe { *Box::from_raw(wparam.0 as *mut Frame) };
            if let Err(e) = unsafe { (*ptr).process_frame(cmd) } {
                tracing::warn!("process_frame failed: {e}");
            }
            LRESULT(0)
        }
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn focus_window(hwnd: HWND) -> anyhow::Result<()> {
    if unsafe { GetForegroundWindow() } == hwnd {
        return Ok(());
    }
    set_foreground_window(hwnd)
}
