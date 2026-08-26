use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_MOUSE, SendInput};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

/// Activate `hwnd` as the foreground window. No-op when it is already
/// foreground.
///
/// Windows blocks SetForegroundWindow unless the calling process just received
/// input. We synthesize a zero-delta mouse event first, which makes this process
/// the last input source and lifts the block. A synthetic key would clear
/// held-modifier state through our own keyboard hook, but dome installs no mouse
/// hook, so a mouse event has no such side effect.
///
/// We do not attach input queues (AttachThreadInput). The attach makes the
/// outgoing window's deactivation a synchronous send, so a hung foreground app
/// blocks the call forever. Without it, SetForegroundWindow does not wait on the
/// outgoing app, so this runs safely on the dome thread. A grab from a
/// higher-integrity window still fails (UIPI) and is logged.
pub(super) fn force_set_foreground(hwnd: HWND) {
    if unsafe { GetForegroundWindow() } == hwnd {
        return;
    }

    let input = [INPUT {
        r#type: INPUT_MOUSE,
        ..Default::default()
    }];
    unsafe { SendInput(&input, size_of::<INPUT>() as i32) };

    if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        tracing::warn!("SetForegroundWindow failed, another app may have focus lock");
    }
}
