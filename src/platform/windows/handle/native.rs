use super::*;

pub(super) fn outer(window: HWND) -> Option<RECT> {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "outer") {
        return read.outer;
    }
    let mut rectangle = RECT::default();
    unsafe { GetWindowRect(window, &mut rectangle) }.ok()?;
    Some(rectangle)
}

pub(super) fn extended_frame(window: HWND) -> Option<RECT> {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "extended_frame") {
        return read.frame;
    }
    let mut rectangle = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            std::ptr::from_mut(&mut rectangle).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .ok()?;
    Some(rectangle)
}

pub(super) fn valid(window: HWND) -> bool {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "valid") {
        return read.valid;
    }
    unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(window)) }.as_bool()
}

pub(super) fn visible(window: HWND) -> bool {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "visible") {
        return read.visible;
    }
    unsafe { IsWindowVisible(window) }.as_bool()
}

pub(super) fn root_owner(window: HWND) -> HWND {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "root_owner") {
        return read.owner.unwrap_or_else(|| window.into()).into();
    }
    unsafe { GetAncestor(window, GA_ROOTOWNER) }
}

pub(super) fn thread(window: HWND) -> u32 {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "thread") {
        return read.thread;
    }
    unsafe { GetWindowThreadProcessId(window, None) }
}

pub(super) fn root(window: HWND) -> HWND {
    native_query();
    #[cfg(test)]
    if fixtures::read(window, "root").is_some() {
        return window;
    }
    unsafe { GetAncestor(window, GA_ROOT) }
}

pub(super) fn style(
    window: HWND,
    index: windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX,
) -> u32 {
    native_query();
    #[cfg(test)]
    if fixtures::read(window, "style").is_some() {
        return if index == GWL_STYLE {
            WS_CAPTION.0 | WS_THICKFRAME.0
        } else {
            0
        };
    }
    (unsafe { GetWindowLongW(window, index) }) as u32
}

pub(super) fn owner(window: HWND) -> Option<HWND> {
    native_query();
    #[cfg(test)]
    if let Some(read) = fixtures::read(window, "owner") {
        return read.owner.map(Into::into);
    }
    unsafe { GetWindow(window, GW_OWNER) }
        .ok()
        .filter(|owner| !owner.is_invalid())
}

pub(super) fn enumerate_thread(thread: u32, mut callback: impl FnMut(HWND)) -> bool {
    native_query();
    #[cfg(test)]
    if let Some(windows) = fixtures::thread_windows(thread) {
        let Some(windows) = windows else {
            return false;
        };
        for window in windows {
            callback(window.into());
        }
        return true;
    }
    unsafe extern "system" fn enumerate<F: FnMut(HWND)>(window: HWND, parameter: LPARAM) -> BOOL {
        let callback = unsafe { &mut *(parameter.0 as *mut F) };
        callback(window);
        BOOL(1)
    }
    fn run<F: FnMut(HWND)>(thread: u32, callback: &mut F) -> bool {
        unsafe {
            EnumThreadWindows(
                thread,
                Some(enumerate::<F>),
                LPARAM(callback as *mut F as isize),
            )
            .as_bool()
        }
    }
    run(thread, &mut callback)
}

pub(super) fn foreground() -> HWND {
    native_query();
    #[cfg(test)]
    if let Some(window) = fixtures::foreground() {
        return window.map(Into::into).unwrap_or_default();
    }
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }
}

pub(super) fn current_process() -> u32 {
    native_query();
    #[cfg(test)]
    if fixtures::active() {
        return u32::MAX;
    }
    unsafe { windows::Win32::System::Threading::GetCurrentProcessId() }
}

pub(super) fn exclusive_fullscreen() -> bool {
    native_query();
    #[cfg(test)]
    if fixtures::active() {
        return false;
    }
    use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
    unsafe { SHQueryUserNotificationState() }.ok() == Some(QUNS_RUNNING_D3D_FULL_SCREEN)
}

#[cfg(test)]
pub(in crate::platform::windows) mod fixtures {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    pub(in crate::platform::windows) struct Window {
        pub outer: Option<RECT>,
        pub frame: Option<RECT>,
        pub owner: Option<HwndId>,
        pub thread: u32,
        pub valid: bool,
        pub visible: bool,
        pub unaware: bool,
        pub minimized: bool,
        pub maximized: bool,
        pub monitor: isize,
    }

    pub(in crate::platform::windows) type ReadObserver =
        Arc<dyn Fn(HwndId, &'static str) + Send + Sync>;

    #[derive(Clone, Default)]
    pub(in crate::platform::windows) struct Reads {
        pub before_read: Option<ReadObserver>,
        pub enumeration_failed: Arc<Mutex<bool>>,
        pub windows: Arc<Mutex<HashMap<HwndId, Window>>>,
        pub calls: Arc<Mutex<Vec<(HwndId, &'static str)>>>,
        pub foreground: Arc<Mutex<Option<HwndId>>>,
    }

    thread_local! {
        static READS: RefCell<Option<Reads>> = const { RefCell::new(None) };
    }

    pub(in crate::platform::windows) fn with<R>(reads: Reads, work: impl FnOnce() -> R) -> R {
        struct Guard(Option<Reads>);
        impl Drop for Guard {
            fn drop(&mut self) {
                READS.with_borrow_mut(|reads| *reads = self.0.take());
            }
        }
        let _guard = Guard(READS.replace(Some(reads)));
        work()
    }

    pub(in crate::platform::windows) fn read(
        window: HWND,
        operation: &'static str,
    ) -> Option<Window> {
        READS.with_borrow(|reads| {
            let reads = reads.as_ref()?;
            let key = window.into();
            reads.calls.lock().unwrap().push((key, operation));
            if let Some(before_read) = &reads.before_read {
                before_read(key, operation);
            }
            Some(
                reads
                    .windows
                    .lock()
                    .unwrap()
                    .get(&key)
                    .expect("unconfigured native read")
                    .clone(),
            )
        })
    }

    pub(in crate::platform::windows::handle) fn process_elevated(process: u32) -> Option<bool> {
        READS.with_borrow(|reads| {
            let reads = reads.as_ref()?;
            assert!(
                reads
                    .windows
                    .lock()
                    .unwrap()
                    .values()
                    .any(|window| window.thread == process)
            );
            Some(false)
        })
    }

    pub(super) fn active() -> bool {
        READS.with_borrow(|reads| reads.is_some())
    }

    pub(super) fn foreground() -> Option<Option<HwndId>> {
        READS.with_borrow(|reads| Some(*reads.as_ref()?.foreground.lock().unwrap()))
    }

    pub(super) fn thread_windows(thread: u32) -> Option<Option<Vec<HwndId>>> {
        READS.with_borrow(|reads| {
            let reads = reads.as_ref()?;
            if *reads.enumeration_failed.lock().unwrap() {
                return Some(None);
            }
            Some(Some(
                reads
                    .windows
                    .lock()
                    .unwrap()
                    .iter()
                    .filter_map(|(&key, window)| (window.thread == thread).then_some(key))
                    .collect(),
            ))
        })
    }
}
