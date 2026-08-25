use std::sync::{Arc, Mutex};
use std::thread;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
};

const MAX_WORKERS: usize = 8;

/// Activates external windows as the foreground window, off the dome thread.
///
/// SetForegroundWindow can block forever on a hung foreground app, and no Win32
/// call can cancel it, so each activation runs on its own disposable thread that
/// a hung app parks instead of the dome loop. In-flight activations are tracked
/// so a window already being activated is skipped, and the live worker count is
/// capped. Clones share one in-flight set, so every clone enforces the same cap.
#[derive(Clone)]
pub(super) struct ForegroundActivator {
    /// Outgoing foreground windows with a live activation worker. An entry is
    /// added before its worker spawns and removed when the worker exits, so an
    /// entry still present on a later request means its worker is parked on a
    /// hung window. Shared with each worker's `ActivationSlot` by clone.
    in_flight: Arc<Mutex<Vec<isize>>>,
}

impl ForegroundActivator {
    pub(super) fn new() -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Request that `hwnd` becomes the foreground window. Returns immediately
    /// and runs the blocking work on a spawned thread.
    pub(super) fn activate(&self, hwnd: HWND) {
        let foreground = unsafe { GetForegroundWindow() };
        if foreground == hwnd {
            return;
        }
        let outgoing = foreground.0 as isize;

        {
            let mut in_flight = self.in_flight.lock().unwrap();
            if in_flight.contains(&outgoing) {
                tracing::warn!(
                    ?foreground,
                    "Foreground window still activating, likely hung, skipping"
                );
                return;
            }
            if in_flight.len() >= MAX_WORKERS {
                tracing::warn!(
                    count = in_flight.len(),
                    "Too many pending foreground activations, skipping"
                );
                return;
            }
            in_flight.push(outgoing);
        }

        let slot = ActivationSlot {
            in_flight: Arc::clone(&self.in_flight),
            outgoing,
        };
        let target = hwnd.0 as isize;
        if let Err(e) = thread::Builder::new()
            .name("dome-foreground".to_owned())
            .spawn(move || activate_on_worker(target, slot))
        {
            // A failed spawn drops the closure and its `slot`, releasing the reservation.
            tracing::warn!("Failed to spawn foreground worker: {e}");
        }
    }
}

/// Attach to the outgoing foreground thread and activate `target`.
///
/// SetForegroundWindow only succeeds when the caller owns the foreground, so we
/// attach this thread's input queue to the foreground thread to lift the lock
/// (see `InputAttach`). We do not use synthetic input, because its keyup would
/// clear held-modifier state through our own keyboard hook and it is fragile
/// across resume. Attaching cannot cross an integrity boundary, so a grab from
/// an elevated window still fails and is logged.
///
/// `slot` is taken by value so its Drop releases the reservation when this
/// worker returns, unwinds, or unblocks after a hung app recovers.
fn activate_on_worker(target: isize, slot: ActivationSlot) {
    let target = HWND(target as *mut _);
    let outgoing = HWND(slot.outgoing as *mut _);
    let outgoing_thread = unsafe { GetWindowThreadProcessId(outgoing, None) };
    let this_thread = unsafe { GetCurrentThreadId() };

    let _attach = InputAttach::new(this_thread, outgoing_thread);
    if !unsafe { SetForegroundWindow(target) }.as_bool() {
        tracing::warn!("SetForegroundWindow failed, another app may have focus lock");
    }
}

/// Removes the outgoing-window entry on worker exit, including on panic, so a
/// worker parked on a hung window frees its slot as soon as it unblocks.
struct ActivationSlot {
    in_flight: Arc<Mutex<Vec<isize>>>,
    outgoing: isize,
}

impl Drop for ActivationSlot {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = self.in_flight.lock()
            && let Some(pos) = in_flight.iter().position(|&h| h == self.outgoing)
        {
            in_flight.swap_remove(pos);
        }
    }
}

/// Attaches `owner`'s input queue to `other` for the guard's lifetime and
/// detaches on drop, so the queues never stay coupled. Skips attach when the
/// threads match or `other` is zero, which AttachThreadInput rejects.
struct InputAttach {
    owner: u32,
    other: u32,
    attached: bool,
}

impl InputAttach {
    fn new(owner: u32, other: u32) -> Self {
        let attached = other != 0
            && other != owner
            && unsafe { AttachThreadInput(owner, other, true) }.as_bool();
        Self {
            owner,
            other,
            attached,
        }
    }
}

impl Drop for InputAttach {
    fn drop(&mut self) {
        if self.attached {
            let _ = unsafe { AttachThreadInput(self.owner, self.other, false) };
        }
    }
}
