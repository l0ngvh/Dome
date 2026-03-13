use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

use crate::core::Dimension;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ManagedHwnd(isize);

impl ManagedHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }

    pub(crate) fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

unsafe impl Send for ManagedHwnd {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowMode {
    Tiling,
    Float,
    FullscreenBorderless,
    ManagedFullscreen,
    FullscreenExclusive,
}

impl WindowMode {
    pub(crate) fn is_fullscreen(self) -> bool {
        matches!(
            self,
            Self::FullscreenBorderless | Self::ManagedFullscreen | Self::FullscreenExclusive
        )
    }
}

impl std::fmt::Display for WindowMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiling => write!(f, "tiling"),
            Self::Float => write!(f, "float"),
            Self::FullscreenBorderless => write!(f, "fullscreen-borderless"),
            Self::ManagedFullscreen => write!(f, "managed-fullscreen"),
            Self::FullscreenExclusive => write!(f, "fullscreen-exclusive"),
        }
    }
}

pub(crate) fn get_dimension(hwnd: HWND) -> Dimension {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).ok() };
    Dimension {
        x: rect.left as f32,
        y: rect.top as f32,
        width: (rect.right - rect.left) as f32,
        height: (rect.bottom - rect.top) as f32,
    }
}

pub(crate) fn get_invisible_border(hwnd: HWND) -> (i32, i32, i32, i32) {
    let mut window_rect = RECT::default();
    let mut frame_rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return (0, 0, 0, 0);
        }
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame_rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            return (0, 0, 0, 0);
        }
    }
    (
        frame_rect.left - window_rect.left,
        frame_rect.top - window_rect.top,
        window_rect.right - frame_rect.right,
        window_rect.bottom - frame_rect.bottom,
    )
}
