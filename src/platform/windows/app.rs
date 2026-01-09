use std::collections::HashSet;
use std::mem::size_of;
use std::pin::Pin;

use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
    ID2D1DeviceContext, ID2D1Factory1,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGISurface, IDXGISwapChain1,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
    GetForegroundWindow, GetWindowLongPtrW, HWND_TOP, RegisterClassW, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, WM_PAINT, WNDCLASSW, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::Interface;

use super::hub::{Frame, OverlayRect, WM_APP_FRAME, WindowHandle};
use super::window::{Taskbar, hide_window, set_window_pos, show_window};
use super::windows_wrapper::set_foreground_window;
use crate::core::Dimension;

pub(super) struct App {
    hwnd: HWND,
    swap_chain: IDXGISwapChain1,
    dc: ID2D1DeviceContext,
    #[expect(
        dead_code,
        reason = "must be kept alive for DirectComposition visual tree"
    )]
    comp_target: IDCompositionTarget,
    comp_device: IDCompositionDevice,
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

        // WS_EX_NOREDIRECTIONBITMAP: enables DirectComposition for per-pixel alpha
        // WS_EX_TRANSPARENT: allows mouse clicks to pass through to windows underneath
        // WS_EX_TOOLWINDOW: hides from taskbar and alt-tab
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
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

        let (swap_chain, dc, comp_device, comp_target) =
            create_composition_resources(hwnd, screen.width as u32, screen.height as u32)?;

        let app = Box::pin(Self {
            hwnd,
            swap_chain,
            dc,
            comp_target,
            comp_device,
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
        let new_displayed: HashSet<WindowHandle> =
            cmd.windows.iter().cloned().map(|(h, _)| h).collect();
        let new_floats: HashSet<WindowHandle> =
            cmd.floats.iter().cloned().map(|(h, _)| h).collect();

        for handle in self.displayed.difference(&new_displayed) {
            hide_window(handle.hwnd());
            self.taskbar.delete_tab(handle.hwnd())?;
        }
        for handle in self.displayed_floats.difference(&new_floats) {
            hide_window(handle.hwnd());
            self.taskbar.delete_tab(handle.hwnd())?;
        }

        for (handle, dim) in &cmd.windows {
            if !self.displayed.contains(handle) {
                show_window(handle.hwnd());
                self.taskbar.add_tab(handle.hwnd())?;
            }
            let inset = Dimension {
                x: dim.x + self.border,
                y: dim.y + self.border,
                width: dim.width - 2.0 * self.border,
                height: dim.height - 2.0 * self.border,
            };
            set_window_pos(handle.hwnd(), &inset)?;
        }

        for (handle, dim) in &cmd.floats {
            if !self.displayed_floats.contains(handle) {
                show_window(handle.hwnd());
                self.taskbar.add_tab(handle.hwnd())?;
            }
            let inset = Dimension {
                x: dim.x + self.border,
                y: dim.y + self.border,
                width: dim.width - 2.0 * self.border,
                height: dim.height - 2.0 * self.border,
            };
            set_window_pos(handle.hwnd(), &inset)?;
        }

        self.displayed = new_displayed;
        self.displayed_floats = new_floats;

        if let Some(ref handle) = cmd.focus
            && let Err(e) = focus_window(handle)
        {
            tracing::warn!("{handle}: {e}");
        }

        self.set_overlays(cmd.overlays)
    }

    fn set_overlays(&mut self, rects: Vec<OverlayRect>) -> anyhow::Result<()> {
        self.rects = rects;
        self.render()?;
        show_window(self.hwnd);
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )?
        };
        Ok(())
    }

    fn render(&self) -> anyhow::Result<()> {
        unsafe {
            let surface: IDXGISurface = self.swap_chain.GetBuffer(0)?;
            let bitmap = self.dc.CreateBitmapFromDxgiSurface(
                &surface,
                Some(&windows::Win32::Graphics::Direct2D::D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    bitmapOptions:
                        windows::Win32::Graphics::Direct2D::D2D1_BITMAP_OPTIONS_TARGET
                            | windows::Win32::Graphics::Direct2D::D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                    ..Default::default()
                }),
            )?;

            self.dc.SetTarget(&bitmap);
            self.dc.BeginDraw();
            self.dc.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));

            for rect in &self.rects {
                let brush = self.dc.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: rect.color.r,
                        g: rect.color.g,
                        b: rect.color.b,
                        a: rect.color.a,
                    },
                    None,
                )?;
                self.dc.FillRectangle(
                    &D2D_RECT_F {
                        left: rect.x,
                        top: rect.y,
                        right: rect.x + rect.width,
                        bottom: rect.y + rect.height,
                    },
                    &brush,
                );
            }

            self.dc.EndDraw(None, None)?;
            self.dc.SetTarget(None);
            self.swap_chain.Present(1, DXGI_PRESENT(0)).ok()?;
            self.comp_device.Commit()?;
        }
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

fn create_composition_resources(
    hwnd: HWND,
    width: u32,
    height: u32,
) -> windows::core::Result<(
    IDXGISwapChain1,
    ID2D1DeviceContext,
    IDCompositionDevice,
    IDCompositionTarget,
)> {
    unsafe {
        let mut d3d_device = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d_device),
            None,
            None,
        )?;
        let d3d_device: ID3D11Device = d3d_device.unwrap();

        let dxgi_device: IDXGIDevice = d3d_device.cast()?;
        let dxgi_adapter = dxgi_device.GetAdapter()?;
        let dxgi_factory: windows::Win32::Graphics::Dxgi::IDXGIFactory2 =
            dxgi_adapter.GetParent()?;

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            ..Default::default()
        };

        let swap_chain =
            dxgi_factory.CreateSwapChainForComposition(&dxgi_device, &swap_chain_desc, None)?;

        let d2d_factory: ID2D1Factory1 =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let d2d_device = d2d_factory.CreateDevice(&dxgi_device)?;
        let dc = d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;

        let comp_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
        let comp_target = comp_device.CreateTargetForHwnd(hwnd, true)?;
        let visual: IDCompositionVisual = comp_device.CreateVisual()?;
        visual.SetContent(&swap_chain)?;
        comp_target.SetRoot(&visual)?;
        comp_device.Commit()?;

        Ok((swap_chain, dc, comp_device, comp_target))
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

fn focus_window(handle: &WindowHandle) -> anyhow::Result<()> {
    let hwnd = handle.hwnd();
    if unsafe { GetForegroundWindow() } == hwnd {
        return Ok(());
    }

    // Simulate ALT key press to bypass SetForegroundWindow restrictions
    // https://gist.github.com/Aetopia/1581b40f00cc0cadc93a0e8ccb65dc8c
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MENU,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MENU,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        },
    ];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };

    set_foreground_window(hwnd)
}
