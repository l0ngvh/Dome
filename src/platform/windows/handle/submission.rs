use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT, SetThreadDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    SET_WINDOW_POS_FLAGS, SHOW_WINDOW_CMD, SetWindowPos, ShowWindowAsync,
};

pub(super) fn context(context: DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS_CONTEXT {
    #[cfg(test)]
    if record(Submission::Context(context.0 as isize)) {
        return DPI_AWARENESS_CONTEXT(CONTEXT_RESULT.get() as *mut _);
    }
    unsafe { SetThreadDpiAwarenessContext(context) }
}

pub(super) fn close(window: HWND) -> windows::core::Result<()> {
    #[cfg(test)]
    if record(Submission::Close(window.into())) {
        return Ok(());
    }
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) }
}

pub(super) fn foreground(window: HWND) {
    #[cfg(test)]
    if record(Submission::Foreground(window.into())) {
        return;
    }
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_MOUSE, SendInput};
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    let input = [INPUT {
        r#type: INPUT_MOUSE,
        ..Default::default()
    }];
    unsafe { SendInput(&input, size_of::<INPUT>() as i32) };
    if !unsafe { SetForegroundWindow(window) }.as_bool() {
        tracing::warn!(?window, "SetForegroundWindow failed");
    }
}

pub(super) fn position(
    window: HWND,
    after: Option<HWND>,
    rectangle: (i32, i32, i32, i32),
    flags: SET_WINDOW_POS_FLAGS,
) -> windows::core::Result<()> {
    #[cfg(test)]
    if record(Submission::Position {
        window: window.into(),
        after: after.map(Into::into),
        rectangle,
        flags,
    }) {
        return Ok(());
    }
    let (x, y, width, height) = rectangle;
    unsafe { SetWindowPos(window, after, x, y, width, height, flags) }
}

pub(super) fn show(window: HWND, command: SHOW_WINDOW_CMD) -> windows::core::Result<()> {
    #[cfg(test)]
    if record(Submission::Show {
        window: window.into(),
        command,
    }) {
        return Ok(());
    }
    unsafe { ShowWindowAsync(window, command) }.ok()
}

#[cfg(test)]
use crate::platform::windows::external::HwndId;

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(in crate::platform::windows) enum Submission {
    Context(isize),
    Close(HwndId),
    Foreground(HwndId),
    Position {
        window: HwndId,
        after: Option<HwndId>,
        rectangle: (i32, i32, i32, i32),
        flags: SET_WINDOW_POS_FLAGS,
    },
    Show {
        window: HwndId,
        command: SHOW_WINDOW_CMD,
    },
}

#[cfg(test)]
type SubmissionObserver = Box<dyn Fn(&Submission)>;

#[cfg(test)]
thread_local! {
    static OBSERVER: std::cell::RefCell<Option<SubmissionObserver>> = const { std::cell::RefCell::new(None) };
    static CONTEXT_RESULT: std::cell::Cell<isize> = const { std::cell::Cell::new(-4) };
    static QUERIES_FORBIDDEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static QUERY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SUBMISSIONS: std::cell::RefCell<Option<Vec<Submission>>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
pub(in crate::platform::windows) fn record_query() {
    assert!(
        !QUERIES_FORBIDDEN.get(),
        "native query in external write path"
    );
    QUERY_COUNT.set(QUERY_COUNT.get() + 1);
}

#[cfg(test)]
pub(in crate::platform::windows) fn query_count() -> usize {
    QUERY_COUNT.get()
}

#[cfg(test)]
pub(in crate::platform::windows) fn without_queries(work: impl FnOnce()) {
    struct Guard(bool);
    impl Drop for Guard {
        fn drop(&mut self) {
            QUERIES_FORBIDDEN.set(self.0);
        }
    }
    let _guard = Guard(QUERIES_FORBIDDEN.replace(true));
    work();
}

#[cfg(test)]
pub(in crate::platform::windows) fn scene_queries<Result>(work: impl FnOnce() -> Result) -> Result {
    struct Guard(bool);
    impl Drop for Guard {
        fn drop(&mut self) {
            QUERIES_FORBIDDEN.set(self.0);
        }
    }
    let _guard = Guard(QUERIES_FORBIDDEN.replace(false));
    work()
}

#[cfg(test)]
pub(in crate::platform::windows) fn with_context_result(result: isize, work: impl FnOnce()) {
    struct Guard(isize);
    impl Drop for Guard {
        fn drop(&mut self) {
            CONTEXT_RESULT.set(self.0);
        }
    }
    let _guard = Guard(CONTEXT_RESULT.replace(result));
    work();
}

#[cfg(test)]
fn record(submission: Submission) -> bool {
    OBSERVER.with_borrow(|observer| {
        if let Some(observer) = observer {
            observer(&submission);
        }
    });
    SUBMISSIONS.with_borrow_mut(|recording| {
        let Some(recording) = recording else {
            return false;
        };
        recording.push(submission);
        true
    })
}

#[cfg(test)]
pub(in crate::platform::windows) fn observe_native_submissions(
    observer: impl Fn(&Submission) + 'static,
) {
    OBSERVER.with_borrow_mut(|slot| {
        assert!(slot.is_none());
        *slot = Some(Box::new(observer));
    });
}

#[cfg(test)]
pub(in crate::platform::windows) fn record_submissions(work: impl FnOnce()) -> Vec<Submission> {
    struct Recording;
    impl Drop for Recording {
        fn drop(&mut self) {
            SUBMISSIONS.with_borrow_mut(|recording| *recording = None);
        }
    }
    SUBMISSIONS.with_borrow_mut(|recording| {
        assert!(
            recording.is_none(),
            "native submission recording is not reentrant"
        );
        *recording = Some(Vec::new());
    });
    let _recording = Recording;
    work();
    SUBMISSIONS.with_borrow_mut(|recording| recording.take().unwrap())
}
