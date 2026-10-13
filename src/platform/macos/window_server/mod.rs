pub(super) mod inspect;
pub(super) mod schedule;
mod server;
pub(super) use server::MacOsWindowServer;
#[cfg(feature = "native-ui-test")]
pub(super) use server::native_observer_test::run as run_native_observer_test;

use std::sync::{Arc, mpsc};
use std::time::Instant;

use dome_auxiliary_window::LoopWaker;
use objc2_application_services::AXUIElement;
use objc2_core_foundation::{CFRetained, CFRunLoopSource};
use objc2_core_graphics::CGWindowID;

use super::Wake;
use super::accessibility::AXWindow;
use super::dome::events::{DiscoveredWindow, FullscreenKind};
use crate::core::{Length, Logical, PixelRect};

pub(super) trait WindowServer {
    fn ingest_native_events(&mut self, now: Instant);
    fn ingest_read_results(&mut self, now: Instant);
    fn has_observations(&self) -> bool;
    /// Drains accepted facts without polling native state.
    fn take_observations(&mut self) -> Vec<Observation>;
    fn next_deadline(&self) -> Option<Instant>;
    fn refresh_applications(&mut self, now: Instant);
    fn set_frame(&mut self, window: CGWindowID, frame: PixelRect<Logical>);
    fn move_offscreen(&mut self, window: CGWindowID, position: (Length<Logical>, Length<Logical>));
    fn minimize(&mut self, window: CGWindowID);
    fn unminimize(&mut self, window: CGWindowID);
    fn close(&mut self, window: CGWindowID);
    /// Queues a submitted target without creating another focus request.
    fn focus(&mut self, window: CGWindowID);
    #[expect(
        dead_code,
        reason = "The server contract retains individual recovery. Shutdown uses one terminal command with recovery records instead."
    )]
    fn recover(&mut self, window: CGWindowID, frame: PixelRect<Logical>);
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>);
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RecoveryEntry {
    pub window: CGWindowID,
    pub original_rect: PixelRect<Logical>,
}

/// Compare Arc identity, not pid, when accepting events or results.
#[derive(Debug)]
pub(super) struct ApplicationLifetime {
    pub pid: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DebounceBurst {
    pub first: Instant,
    pub last: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PositionReadOrigin {
    Settle(DebounceBurst),
    Periodic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InspectionKind {
    AppWindows,
    Positions(PositionReadOrigin),
    Space,
    FocusedWindow,
    Title(CGWindowID),
}

#[derive(Clone, Debug)]
pub(super) struct ExistingWindow {
    pub key: CGWindowID,
    pub frame: Option<PixelRect<Logical>>,
    pub corner_radius: Length<Logical>,
    pub minimized: bool,
    pub fullscreen: Option<FullscreenKind>,
}

#[derive(Clone, Debug)]
pub(super) enum Observation {
    AppWindows {
        lifetime: Arc<ApplicationLifetime>,
        discovered: Vec<DiscoveredWindow>,
        existing: Vec<ExistingWindow>,
        /// Proven absent after successful enumeration and handle validation.
        removed: Vec<CGWindowID>,
    },
    Settled {
        lifetime: Arc<ApplicationLifetime>,
        origin: PositionReadOrigin,
        windows: Vec<ExistingWindow>,
    },
    Space {
        lifetime: Arc<ApplicationLifetime>,
        windows: Vec<ExistingWindow>,
    },
    FocusReported {
        lifetime: Arc<ApplicationLifetime>,
        /// None clears domain intent when the foreground has no resolvable window.
        window: Option<CGWindowID>,
    },
    TitleChanged {
        lifetime: Arc<ApplicationLifetime>,
        key: CGWindowID,
        title: String,
    },
    AppHidden {
        lifetime: Arc<ApplicationLifetime>,
    },
    AppGone {
        lifetime: Arc<ApplicationLifetime>,
    },
}

pub(super) struct InspectionResult {
    pub lifetime: Arc<ApplicationLifetime>,
    pub kind: InspectionKind,
    pub outcome: InspectionOutcome,
    /// Installed by native result acceptance, never exposed to UI policy or Hub.
    pub handle_updates: Vec<AXWindow>,
}

pub(super) enum InspectionOutcome {
    Completed(Observation),
    Failed,
}

#[derive(Clone, Debug)]
pub(super) enum ApplicationEvent {
    MovedOrResized {
        observed_at: Instant,
    },
    Hidden,
    Shown,
    WindowsChanged,
    FocusChanged,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Native title callbacks forward retained elements. Keyed title events remain part of the application event contract."
        )
    )]
    TitleChanged(CGWindowID),
    TitleElementChanged(TitleElement),
    SpaceChanged,
    Suspended(bool),
    Refresh,
}

pub(super) enum NativeEvent {
    Application {
        lifetime: Arc<ApplicationLifetime>,
        event: ApplicationEvent,
    },
    ApplicationActivated {
        process_id: i32,
    },
    ApplicationLaunched {
        process_id: i32,
    },
    ApplicationTerminated {
        process_id: i32,
    },
    SpaceChanged,
    Suspended(bool),
}

/// Retained through callback quiescence.
pub(super) struct ObserverContext {
    pub lifetime: Arc<ApplicationLifetime>,
    pub events: mpsc::Sender<NativeEvent>,
    pub loop_waker: LoopWaker<Wake>,
}

/// A retained remote identity, messaged only by the receiving app worker.
#[derive(Clone, Debug)]
pub(super) struct TitleElement(pub CFRetained<AXUIElement>);

// UI may retain/release this opaque identity but must not message it.
// Attribute access must stay serial on the receiving app worker.
unsafe impl Send for TitleElement {}

/// Retained source created by a worker for attachment on the UI run loop.
pub(super) struct ObserverSource(pub CFRetained<CFRunLoopSource>);

// CFRunLoop synchronizes source attachment and removal. UI must detach this
// source before worker notification removal, retaining context through callbacks.
unsafe impl Send for ObserverSource {}

pub(super) enum ApplicationResult {
    Inspection(InspectionResult),
    ObserverReady {
        lifetime: Arc<ApplicationLifetime>,
        source: ObserverSource,
        context: Arc<ObserverContext>,
    },
}
