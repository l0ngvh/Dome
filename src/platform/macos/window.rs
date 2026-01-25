use anyhow::{Context, Result};
use std::collections::HashSet;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};
use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFEqual, CFNumber, CFRetained, CFString, CFType, CGPoint,
    CGSize, kCFBooleanFalse, kCFBooleanTrue,
};
use objc2_core_graphics::{
    CGSessionCopyCurrentDictionary, CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption,
};

use super::objc2_wrapper::{
    AXError, get_attribute, get_cg_window_id, is_attribute_settable,
    kAXEnhancedUserInterfaceAttribute, kAXFrontmostAttribute, kAXFullScreenAttribute,
    kAXMainAttribute, kAXMinimizedAttribute, kAXParentAttribute, kAXPositionAttribute,
    kAXRoleAttribute, kAXSizeAttribute, kAXStandardWindowSubrole, kAXSubroleAttribute,
    kAXTitleAttribute, kAXWindowRole, kAXWindowsAttribute, kCGWindowNumber, set_attribute_value,
};
use crate::core::{Dimension, Window};

#[derive(Clone)]
pub(super) struct MacWindow {
    element: CFRetained<AXUIElement>,
    app: CFRetained<AXUIElement>,
    cg_id: CGWindowID,
    pid: i32,
    global_bounds: Dimension,
    app_name: String,
    bundle_id: Option<String>,
    title: Option<String>,
    logical_placement: Option<Dimension>,
    physical_placement: Option<Dimension>,
    is_hidden: bool,
}

// Safety: AXUIElement operations are IPC calls to the accessibility server,
// safe to use from any thread for manipulating other applications' windows.
unsafe impl Send for MacWindow {}

impl std::fmt::Display for MacWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.app_name)?;
        if let Some(bundle_id) = &self.bundle_id {
            write!(f, " ({bundle_id})")?;
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        Ok(())
    }
}

impl MacWindow {
    pub(super) fn new(
        element: CFRetained<AXUIElement>,
        cg_id: CGWindowID,
        global_bounds: Dimension,
        app: &NSRunningApplication,
    ) -> Self {
        let pid = app.processIdentifier();
        let ax_app = unsafe { AXUIElement::new_application(pid) };
        let app_name = app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let title = get_attribute::<CFString>(&element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();

        Self {
            element,
            app: ax_app,
            cg_id,
            pid,
            global_bounds,
            app_name,
            bundle_id,
            title,
            logical_placement: None,
            physical_placement: None,
            is_hidden: false,
        }
    }

    pub(super) fn cg_id(&self) -> CGWindowID {
        self.cg_id
    }

    pub(super) fn pid(&self) -> i32 {
        self.pid
    }

    pub(super) fn app_name(&self) -> &str {
        &self.app_name
    }

    pub(super) fn bundle_id(&self) -> Option<&str> {
        self.bundle_id.as_deref()
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn update_title(&mut self) {
        self.title = get_attribute::<CFString>(&self.element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();
    }

    pub(super) fn is_manageable(&self) -> bool {
        let Some(title) = &self.title else {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, "not manageable: window has no title");
            return false;
        };

        let role = get_attribute::<CFString>(&self.element, &kAXRoleAttribute()).ok();
        let is_window = role
            .as_ref()
            .map(|r| CFEqual(Some(&**r), Some(&*kAXWindowRole())))
            .unwrap_or(false);
        if !is_window {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: role is not AXWindow");
            return false;
        }

        let subrole = get_attribute::<CFString>(&self.element, &kAXSubroleAttribute()).ok();
        let is_standard = subrole
            .as_ref()
            .map(|sr| CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole())))
            .unwrap_or(false);
        if !is_standard {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: subrole is not AXStandardWindow");
            return false;
        }

        let is_root = match get_attribute::<AXUIElement>(&self.element, &kAXParentAttribute()) {
            Err(_) => true,
            Ok(parent) => CFEqual(Some(&*parent), Some(&*self.app)),
        };
        if !is_root {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: window is not root");
            return false;
        }

        if !is_attribute_settable(&self.element, &kAXPositionAttribute()) {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: position is not settable");
            return false;
        }

        if !is_attribute_settable(&self.element, &kAXSizeAttribute()) {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: size is not settable");
            return false;
        }

        if !is_attribute_settable(&self.element, &kAXMainAttribute()) {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: main attribute is not settable");
            return false;
        }

        let is_minimized = get_attribute::<CFBoolean>(&self.element, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_minimized {
            tracing::trace!(app = %self.app_name, bundle_id = ?self.bundle_id, %title, "not manageable: window is minimized");
            return false;
        }

        true
    }

    pub(super) fn should_tile(&self) -> bool {
        let is_fullscreen = get_attribute::<CFBoolean>(&self.element, &kAXFullScreenAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        !is_fullscreen
    }

    pub(super) fn get_dimension(&self) -> Dimension {
        let (x, y) = get_attribute::<AXValue>(&self.element, &kAXPositionAttribute())
            .map(|v| {
                let mut pos = CGPoint::new(0.0, 0.0);
                let ptr = NonNull::new(&mut pos as *mut _ as *mut _).unwrap();
                unsafe { v.value(AXValueType::CGPoint, ptr) };
                (pos.x as f32, pos.y as f32)
            })
            .unwrap_or((0.0, 0.0));
        let (width, height) = self.get_size().unwrap_or((0.0, 0.0));
        Dimension {
            x,
            y,
            width,
            height,
        }
    }

    /// As we're tracking windows with CGWindowID, we have to check whether a window is still valid
    /// as macOS can reuse CGWindowID of deleted windows.
    pub(super) fn is_valid(&self) -> bool {
        if is_screen_locked() {
            return true;
        }
        let is_deleted = matches!(
            get_attribute::<CFString>(&self.element, &kAXRoleAttribute()),
            Err(AXError::InvalidUIElement)
        );
        if is_deleted {
            tracing::trace!(app = %self.app_name, title = ?self.title, "not valid: window is deleted");
            return false;
        }
        let is_minimized = get_attribute::<CFBoolean>(&self.element, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_minimized {
            tracing::trace!(app = %self.app_name, title = ?self.title, "not valid: window is minimized");
            return false;
        }
        true
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn set_dimension(&mut self, dim: Dimension) -> Result<()> {
        // Round to avoid floating point comparison issues when checking if window settled at expected position
        let dim = round_dim(dim);
        self.is_hidden = false;
        // Set position/size calls are expensive, and it can trigger a window moved/resize events
        if self.approx_eq(dim) {
            return Ok(());
        }
        self.with_animation_disabled(|| {
            self.set_position(dim.x, dim.y)?;
            self.set_size(dim.width, dim.height)
        })
        .with_context(|| format!("set_dimension for {} {:?}", self.app_name, self.title))
    }

    /// Try to place this physical window on the logical placement. Mac has restrictions on how
    /// windows can be placed, so if we try to put a window above menu bar, or stretch a window
    /// taller than screen height, Mac will instead come up with an alternative placement. For our
    /// use case, the alternative placements are acceptable, albeit they will mess a little with
    /// our border rendering
    pub(super) fn try_placement(&mut self, window: &Window, border: f32) {
        let dim = apply_inset(window.dimension(), border);

        if is_completely_offscreen(dim, self.global_bounds) {
            // TODO: if hide fail to move the window to offscreen position, this window is clearly
            // trying to take focus, so we should pop it to float or something.
            // Exception is full screen window, which, should be handled differently as a first
            // party citizen
            if let Err(e) = self.hide() {
                tracing::trace!("Failed to hide window: {e:#}");
            }
            return;
        }

        let rounded = round_dim(dim);
        if self.logical_placement == Some(rounded) && !self.is_hidden {
            return;
        }

        let mut target = dim;

        // Mac prevents putting windows above menu bar
        if target.y < self.global_bounds.y {
            target.height -= self.global_bounds.y - target.y;
            target.y = self.global_bounds.y;
        }
        // Clip to fit within screen, as Mac sometime snap windows to fit within screen, which
        // might be confused with user setting size manually
        if target.y + target.height > self.global_bounds.y + self.global_bounds.height {
            target.height = self.global_bounds.y + self.global_bounds.height - target.y;
        }
        if target.x < self.global_bounds.x {
            target.width -= self.global_bounds.x - target.x;
            target.x = self.global_bounds.x;
        }
        if target.x + target.width > self.global_bounds.x + self.global_bounds.width {
            target.width = self.global_bounds.x + self.global_bounds.width - target.x;
        }

        if self.set_dimension(target).is_err() {
            return;
        }
        self.logical_placement = Some(rounded);
        self.physical_placement = Some(target);
    }

    /// Check if window settled at expected position and detect constraints
    pub(super) fn check_placement(&self, window: &Window) -> Option<RawConstraint> {
        let expected = self.physical_placement?;
        let actual = self.get_dimension();

        // At least one edge must match on each axis - user resize moves both edges on one axis
        let left = pixel_eq(actual.x, expected.x);
        let right = pixel_eq(actual.x + actual.width, expected.x + expected.width);
        let top = pixel_eq(actual.y, expected.y);
        let bottom = pixel_eq(actual.y + actual.height, expected.y + expected.height);
        tracing::debug!(
            ?actual,
            ?expected,
            left,
            right,
            top,
            bottom,
            global_bounds = ?self.global_bounds,
            "check_placement"
        );
        if !((left || right) && (top || bottom)) {
            return None;
        }

        compute_constraint(actual, expected, window)
    }

    pub(super) fn focus(&self) -> Result<()> {
        let is_frontmost = get_attribute::<CFBoolean>(&self.app, &kAXFrontmostAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_frontmost {
            set_attribute_value(&self.app, &kAXFrontmostAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for {} {:?}", self.app_name, self.title))?;
        }
        let is_main = get_attribute::<CFBoolean>(&self.element, &kAXMainAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_main {
            set_attribute_value(&self.element, &kAXMainAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for {} {:?}", self.app_name, self.title))?;
        }
        Ok(())
    }

    /// Hide the window by moving it offscreen
    /// We don't minimize windows as there is no way to disable minimizing animation. When hiding
    /// multiple windows, it gets triggered in a staggered manner, which is extremely slow, and
    /// causes event tap to be timed out
    pub(super) fn hide(&mut self) -> Result<()> {
        self.is_hidden = true;
        // MacOS doesn't allow completely set windows offscreen, so we need to leave at
        // least one pixel left
        // https://nikitabobko.github.io/AeroSpace/guide#emulation-of-virtual-workspaces
        self.with_animation_disabled(|| {
            self.set_position(
                self.global_bounds.x + self.global_bounds.width - 1.0,
                self.global_bounds.y + self.global_bounds.height - 1.0,
            )
        })
        .with_context(|| format!("hide for {} {:?}", self.app_name, self.title))
    }

    pub(super) fn set_global_bounds(&mut self, bounds: Dimension) {
        self.global_bounds = bounds;
    }

    pub(super) fn get_size(&self) -> Result<(f32, f32)> {
        let size = get_attribute::<AXValue>(&self.element, &kAXSizeAttribute())
            .with_context(|| format!("get_size for {} {:?}", self.app_name, self.title))?;
        let mut cg_size = CGSize::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_size as *mut CGSize).cast()).unwrap();
        unsafe { size.value(AXValueType::CGSize, ptr) };
        Ok((cg_size.width as f32, cg_size.height as f32))
    }

    /// Without this the windows move in a janky way
    /// https://github.com/nikitabobko/AeroSpace/issues/51
    fn with_animation_disabled<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        let was_enabled =
            get_attribute::<CFBoolean>(&self.app, &kAXEnhancedUserInterfaceAttribute()).ok();
        let _ = set_attribute_value(&self.app, &kAXEnhancedUserInterfaceAttribute(), unsafe {
            kCFBooleanFalse.unwrap()
        });
        let result = f();
        if was_enabled.is_some() {
            let _ = set_attribute_value(&self.app, &kAXEnhancedUserInterfaceAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            });
        }
        result
    }

    fn set_position(&self, x: f32, y: f32) -> anyhow::Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x as f64, y as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        Ok(set_attribute_value(
            &self.element,
            &kAXPositionAttribute(),
            &pos_ptr,
        )?)
    }

    fn set_size(&self, width: f32, height: f32) -> anyhow::Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        Ok(set_attribute_value(
            &self.element,
            &kAXSizeAttribute(),
            &size,
        )?)
    }

    fn approx_eq(&self, other: Dimension) -> bool {
        let s = self.get_dimension();
        pixel_eq(s.x, other.x)
            && pixel_eq(s.y, other.y)
            && pixel_eq(s.width, other.width)
            && pixel_eq(s.height, other.height)
    }
}

// For comparing actual window position/size from macOS vs what we requested.
const PIXEL_EPSILON: f32 = 1.0;

fn pixel_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= PIXEL_EPSILON
}

fn exceeds_by_pixel(actual: f32, expected: f32) -> bool {
    actual > expected + PIXEL_EPSILON
}

fn falls_short_by_pixel(actual: f32, expected: f32) -> bool {
    actual < expected - PIXEL_EPSILON
}

fn round_dim(dim: Dimension) -> Dimension {
    Dimension {
        x: dim.x.round(),
        y: dim.y.round(),
        width: dim.width.round(),
        height: dim.height.round(),
    }
}

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

fn is_completely_offscreen(dim: Dimension, screen: Dimension) -> bool {
    dim.x + dim.width <= screen.x
        || dim.x >= screen.x + screen.width
        || dim.y + dim.height <= screen.y
        || dim.y >= screen.y + screen.height
}

/// Constraint on raw window size (min_w, min_h, max_w, max_h).
pub(super) type RawConstraint = (Option<f32>, Option<f32>, Option<f32>, Option<f32>);

fn compute_constraint(
    actual: Dimension,
    expected: Dimension,
    existing: &Window,
) -> Option<RawConstraint> {
    let (cur_min_w, cur_min_h) = existing.min_size();
    let (cur_max_w, cur_max_h) = existing.max_size();
    let min_w = exceeds_by_pixel(actual.width, expected.width)
        .then_some(actual.width)
        .filter(|&w| !pixel_eq(w, cur_min_w));
    let min_h = exceeds_by_pixel(actual.height, expected.height)
        .then_some(actual.height)
        .filter(|&h| !pixel_eq(h, cur_min_h));
    let max_w = falls_short_by_pixel(actual.width, expected.width)
        .then_some(actual.width)
        .filter(|&w| !pixel_eq(w, cur_max_w));
    let max_h = falls_short_by_pixel(actual.height, expected.height)
        .then_some(actual.height)
        .filter(|&h| !pixel_eq(h, cur_max_h));
    if min_w.is_some() || min_h.is_some() || max_w.is_some() || max_h.is_some() {
        Some((min_w, min_h, max_w, max_h))
    } else {
        None
    }
}

fn is_screen_locked() -> bool {
    let Some(dict) = CGSessionCopyCurrentDictionary() else {
        return false;
    };
    let dict: &CFDictionary<CFString, CFType> = unsafe { dict.cast_unchecked() };

    // CGSSessionScreenIsLocked is present when screen is locked
    let locked_key = CFString::from_static_str("CGSSessionScreenIsLocked");
    if dict.contains_key(&locked_key) {
        return true;
    }

    // kCGSSessionOnConsoleKey is false when screen is off/sleeping
    let on_console_key = CFString::from_static_str("kCGSSessionOnConsoleKey");
    if let Some(value) = dict.get(&on_console_key)
        && let Some(b) = value.downcast_ref::<CFBoolean>()
    {
        return !b.as_bool();
    }

    false
}

pub(super) fn list_cg_window_ids() -> HashSet<CGWindowID> {
    let Some(window_list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        tracing::warn!("CGWindowListCopyWindowInfo returned None");
        return HashSet::new();
    };
    let window_list: &CFArray<CFDictionary<CFString, CFType>> =
        unsafe { window_list.cast_unchecked() };

    let mut ids = HashSet::new();
    let key = kCGWindowNumber();
    for dict in window_list {
        // window id is a required attribute
        // https://developer.apple.com/documentation/coregraphics/kcgwindownumber?language=objc
        let id = dict
            .get(&key)
            .unwrap()
            .downcast::<CFNumber>()
            .unwrap()
            .as_i64()
            .unwrap();
        ids.insert(id as CGWindowID);
    }
    ids
}

pub(super) fn running_apps() -> impl Iterator<Item = Retained<NSRunningApplication>> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .into_iter()
        .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .filter(|app| app.processIdentifier() != -1)
}

pub(super) fn get_app_by_pid(pid: i32) -> Option<Retained<NSRunningApplication>> {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
}

pub(super) fn get_ax_windows(pid: i32) -> Vec<(CGWindowID, CFRetained<AXUIElement>)> {
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    let Ok(windows) = get_attribute::<CFArray<AXUIElement>>(&ax_app, &kAXWindowsAttribute()) else {
        return Vec::new();
    };
    windows
        .iter()
        .filter_map(|w| {
            let cg_id = get_cg_window_id(&w)?;
            Some((cg_id, w.clone()))
        })
        .collect()
}
