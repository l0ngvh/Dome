//! Reads window ids, owners, layers, and bounds from CGWindowList.

use std::ffi::c_void;
use std::ptr::null;

use objc2_core_foundation::{
    CFArray, CFDictionary, CFIndex, CFNumber, CFNumberType, CFRetained, CFString, CGPoint, CGRect,
    CGSize,
};
use objc2_core_graphics::{
    CGDisplayBounds, CGMainDisplayID, CGRectMakeWithDictionaryRepresentation,
    CGWindowListCopyWindowInfo, CGWindowListCreateDescriptionFromArray, CGWindowListOption,
    kCGNullWindowID, kCGWindowAlpha, kCGWindowBounds, kCGWindowLayer, kCGWindowNumber,
    kCGWindowOwnerName, kCGWindowOwnerPID,
};

use crate::geometry::Rect;

/// `CGWindowLevelForKey(kCGCursorWindowLevelKey)`.
const CURSOR_LAYER: i64 = 2_147_483_630;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinInfo {
    pub id: u32,
    pub pid: i64,
    pub layer: i64,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Appeared,
    Gone,
    Moved,
    Reordered,
}

impl ChangeKind {
    pub fn name(self) -> &'static str {
        match self {
            ChangeKind::Appeared => "appeared",
            ChangeKind::Gone => "gone",
            ChangeKind::Moved => "moved",
            ChangeKind::Reordered => "reordered",
        }
    }
}

/// Every window that appeared, went, or moved between two lists. Then, as
/// `Reordered`, the first window present in both lists whose place in the
/// front to back order or whose layer changed.
pub fn changes(old: &[WinInfo], new: &[WinInfo]) -> Vec<(WinInfo, ChangeKind)> {
    let mut out = Vec::new();
    for n in new {
        match old.iter().find(|o| o.id == n.id) {
            None => out.push((*n, ChangeKind::Appeared)),
            Some(o) if o.rect != n.rect => out.push((*n, ChangeKind::Moved)),
            Some(_) => {}
        }
    }
    for o in old {
        if !new.iter().any(|n| n.id == o.id) {
            out.push((*o, ChangeKind::Gone));
        }
    }
    let old_common = old.iter().filter(|o| new.iter().any(|n| n.id == o.id));
    let new_common = new.iter().filter(|n| old.iter().any(|o| o.id == n.id));
    if let Some((n, _)) = new_common
        .zip(old_common)
        .find(|(n, o)| n.id != o.id || n.layer != o.layer)
    {
        out.push((*n, ChangeKind::Reordered));
    }
    out
}

/// Index 0 is the frontmost window.
pub fn query_windows(skip_pid: Option<i64>) -> Vec<WinInfo> {
    let options =
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(array) = CGWindowListCopyWindowInfo(options, kCGNullWindowID) else {
        return Vec::new();
    };
    collect(&array, skip_pid, |_| ())
        .into_iter()
        .map(|(w, ())| w)
        .collect()
}

pub fn query_windows_named(skip_pid: Option<i64>) -> Vec<(WinInfo, String)> {
    let options =
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(array) = CGWindowListCopyWindowInfo(options, kCGNullWindowID) else {
        return Vec::new();
    };
    collect(&array, skip_pid, |dict| {
        string(dict, unsafe { kCGWindowOwnerName }).unwrap_or_default()
    })
}

pub fn query_window(id: u32) -> Option<WinInfo> {
    let array = CGWindowListCopyWindowInfo(CGWindowListOption::OptionIncludingWindow, id)?;
    collect(&array, None, |_| ())
        .into_iter()
        .map(|(w, ())| w)
        .find(|w| w.id == id)
}

/// The windows that `ids` names, from one WindowServer call.
pub fn query_windows_by_id(ids: &[u32]) -> Vec<WinInfo> {
    if ids.is_empty() {
        return Vec::new();
    }
    // Apple documents the array values as CGWindowID values, not CFNumbers, so
    // the array holds each id in a pointer and retains nothing.
    let mut values: Vec<*const c_void> =
        ids.iter().map(|id| *id as usize as *const c_void).collect();
    let array = unsafe { CFArray::new(None, values.as_mut_ptr(), values.len() as CFIndex, null()) };
    let Some(array) = array else {
        return Vec::new();
    };
    let Some(descriptions) = (unsafe { CGWindowListCreateDescriptionFromArray(Some(&array)) })
    else {
        return Vec::new();
    };
    collect(&descriptions, None, |_| ())
        .into_iter()
        .map(|(w, ())| w)
        .collect()
}

fn collect<T>(
    array: &CFRetained<CFArray>,
    skip_pid: Option<i64>,
    extra: impl Fn(&CFDictionary) -> T,
) -> Vec<(WinInfo, T)> {
    // The array holds one CFDictionary per window. Its element type is
    // layout-identical to the untyped default.
    let dicts: &CFArray<CFDictionary> =
        unsafe { &*((&**array) as *const CFArray as *const CFArray<CFDictionary>) };

    let mut out = Vec::with_capacity(dicts.len());
    for i in 0..dicts.len() {
        let Some(dict) = dicts.get(i) else { continue };
        let pid = number_i64(&dict, unsafe { kCGWindowOwnerPID }).unwrap_or(-1);
        if Some(pid) == skip_pid {
            continue;
        }
        let alpha = number_f64(&dict, unsafe { kCGWindowAlpha }).unwrap_or(1.0);
        if alpha <= 0.01 {
            continue;
        }
        let Some(rect) = bounds(&dict) else { continue };
        let id = number_i64(&dict, unsafe { kCGWindowNumber }).unwrap_or(0) as u32;
        let layer = number_i64(&dict, unsafe { kCGWindowLayer }).unwrap_or(0);
        // Notification Center can keep a layer 21 window over the whole display
        // that CGWindowList reports on screen with alpha 1. On the test machine
        // the AX hit test under that window returned the app below at every
        // point, so the window drew nothing and took no clicks.
        if layer > 0 && covers_main_display(&rect) && is_notification_center(&dict) {
            continue;
        }
        // The cursor and the screen-recording indicator sit at this layer. The
        // cursor window appears in the list while `screencapture -v` records.
        if layer >= CURSOR_LAYER {
            continue;
        }
        out.push((WinInfo { id, pid, layer, rect }, extra(&dict)));
    }
    out
}

fn covers_main_display(rect: &Rect) -> bool {
    let display = CGDisplayBounds(CGMainDisplayID());
    rect.x <= display.origin.x
        && rect.y <= display.origin.y
        && rect.right() >= display.origin.x + display.size.width
        && rect.bottom() >= display.origin.y + display.size.height
}

fn is_notification_center(dict: &CFDictionary) -> bool {
    string(dict, unsafe { kCGWindowOwnerName }).as_deref() == Some("Notification Center")
}

fn dict_value(dict: &CFDictionary, key: &CFString) -> *const c_void {
    unsafe { dict.value(key as *const CFString as *const c_void) }
}

fn number_i64(dict: &CFDictionary, key: &CFString) -> Option<i64> {
    let v = dict_value(dict, key);
    if v.is_null() {
        return None;
    }
    let num = unsafe { &*(v as *const CFNumber) };
    let mut out: i64 = 0;
    let ok =
        unsafe { num.value(CFNumberType::SInt64Type, (&mut out as *mut i64).cast::<c_void>()) };
    ok.then_some(out)
}

fn number_f64(dict: &CFDictionary, key: &CFString) -> Option<f64> {
    let v = dict_value(dict, key);
    if v.is_null() {
        return None;
    }
    let num = unsafe { &*(v as *const CFNumber) };
    let mut out: f64 = 0.0;
    let ok =
        unsafe { num.value(CFNumberType::Float64Type, (&mut out as *mut f64).cast::<c_void>()) };
    ok.then_some(out)
}

fn string(dict: &CFDictionary, key: &CFString) -> Option<String> {
    let v = dict_value(dict, key);
    if v.is_null() {
        return None;
    }
    let s = unsafe { &*(v as *const CFString) };
    Some(s.to_string())
}

/// Global display coordinates, origin top-left.
fn bounds(dict: &CFDictionary) -> Option<Rect> {
    let v = dict_value(dict, unsafe { kCGWindowBounds });
    if v.is_null() {
        return None;
    }
    let bounds_dict = unsafe { &*(v as *const CFDictionary) };
    let mut cg = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 0.0,
            height: 0.0,
        },
    };
    let ok = unsafe { CGRectMakeWithDictionaryRepresentation(Some(bounds_dict), &mut cg) };
    ok.then_some(Rect::new(
        cg.origin.x,
        cg.origin.y,
        cg.size.width,
        cg.size.height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: u32, x: f64) -> WinInfo {
        WinInfo {
            id,
            pid: 7,
            layer: 0,
            rect: Rect::new(x, 0.0, 100.0, 100.0),
        }
    }

    #[test]
    fn changes_name_each_kind() {
        let old = [win(1, 0.0), win(2, 0.0), win(3, 0.0)];
        let new = [win(1, 5.0), win(3, 0.0), win(4, 0.0)];
        let kinds: Vec<(u32, ChangeKind)> =
            changes(&old, &new).iter().map(|(w, k)| (w.id, *k)).collect();
        assert_eq!(
            kinds,
            vec![
                (1, ChangeKind::Moved),
                (4, ChangeKind::Appeared),
                (2, ChangeKind::Gone)
            ]
        );
    }

    #[test]
    fn a_swap_alone_is_a_reorder_of_the_new_front() {
        let old = [win(1, 0.0), win(2, 0.0)];
        let new = [win(2, 0.0), win(1, 0.0)];
        let found = changes(&old, &new);
        assert_eq!(found, vec![(win(2, 0.0), ChangeKind::Reordered)]);
    }

    #[test]
    fn equal_lists_have_no_changes() {
        let list = [win(1, 0.0), win(2, 0.0)];
        assert!(changes(&list, &list).is_empty());
    }
}
