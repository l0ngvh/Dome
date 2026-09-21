use std::sync::{Arc, Mutex};

use crate::platform::windows::external::{HwndId, ZOrder};

struct ZOrderBands {
    topmost: Vec<HwndId>,
    normal: Vec<HwndId>,
}

/// Emulates Win32's z-order stack for test assertions.
#[derive(Clone)]
pub(crate) struct ZOrderStack {
    bands: Arc<Mutex<ZOrderBands>>,
}

impl ZOrderStack {
    pub(crate) fn new() -> Self {
        Self {
            bands: Arc::new(Mutex::new(ZOrderBands {
                topmost: Vec::new(),
                normal: Vec::new(),
            })),
        }
    }

    pub(crate) fn apply(&self, hwnd: HwndId, z: ZOrder) {
        let mut bands = self.bands.lock().unwrap();

        let orig_topmost_pos = bands.topmost.iter().position(|&id| id == hwnd);
        let orig_normal_pos = bands.normal.iter().position(|&id| id == hwnd);

        match z {
            // Win32 self-reference (SetWindowPos(hwnd, hwnd, ...)) is a no-op.
            ZOrder::After(other) if other == hwnd => return,
            _ => {}
        }

        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);

        match z {
            ZOrder::After(other) => {
                // Win32 is not documented to promote `hwnd` into the topmost band here.
                if let Some(pos) = bands.topmost.iter().position(|&id| id == other) {
                    bands.topmost.insert(pos + 1, hwnd);
                } else if let Some(pos) = bands.normal.iter().position(|&id| id == other) {
                    bands.normal.insert(pos + 1, hwnd);
                } else {
                    bands.normal.push(hwnd);
                }
            }
            ZOrder::Topmost => {
                bands.topmost.insert(0, hwnd);
            }
            ZOrder::NotTopmost => {
                let clamped = orig_normal_pos.unwrap_or(0).min(bands.normal.len());
                bands.normal.insert(clamped, hwnd);
            }
            ZOrder::Unchanged => {
                if let Some(pos) = orig_topmost_pos {
                    let clamped = pos.min(bands.topmost.len());
                    bands.topmost.insert(clamped, hwnd);
                } else if let Some(pos) = orig_normal_pos {
                    let clamped = pos.min(bands.normal.len());
                    bands.normal.insert(clamped, hwnd);
                } else {
                    bands.normal.push(hwnd);
                }
            }
        }
    }

    pub(crate) fn move_to_bottom(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);
        bands.normal.push(hwnd);
    }

    /// Models an application raising `hwnd` to the top of the normal band behind Dome's back.
    pub(crate) fn move_to_top(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);
        bands.normal.insert(0, hwnd);
    }

    /// Returns the full z-order stack from top to bottom: topmost band first, then normal.
    pub(crate) fn stack(&self) -> Vec<HwndId> {
        let bands = self.bands.lock().unwrap();
        let mut result = bands.topmost.clone();
        result.extend_from_slice(&bands.normal);
        result
    }

    pub(crate) fn normal_stack(&self) -> Vec<HwndId> {
        self.bands.lock().unwrap().normal.clone()
    }

    pub(crate) fn is_topmost(&self, hwnd: HwndId) -> bool {
        self.bands.lock().unwrap().topmost.contains(&hwnd)
    }

    /// Mirrors Win32 `DestroyWindow`.
    pub(crate) fn remove(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);
    }

    /// Simulate CreateWindowExW: place a freshly-created HWND at the top of
    /// the normal z-order band.
    pub(crate) fn simulate_create(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.normal.retain(|&id| id != hwnd);
        bands.normal.insert(0, hwnd);
    }
}
