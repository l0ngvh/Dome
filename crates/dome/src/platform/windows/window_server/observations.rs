use dome_core::{LimitObservation, Physical, PixelRect, WindowId};
use std::time::Instant;

pub(in crate::platform::windows) struct WindowObservation {
    pub(in crate::platform::windows) id: WindowId,
    pub(in crate::platform::windows) pid: u32,
    pub(in crate::platform::windows) metadata: WindowMetadata,
    pub(in crate::platform::windows) constraints: LimitObservation,
    pub(in crate::platform::windows) state: NativeWindowState,
}

pub(in crate::platform::windows) struct NativeWindowState {
    pub(in crate::platform::windows) visible: bool,
    pub(in crate::platform::windows) minimized: bool,
    pub(in crate::platform::windows) cloaked: bool,
    pub(in crate::platform::windows) is_root: bool,
    pub(in crate::platform::windows) has_owner: bool,
    pub(in crate::platform::windows) style: u32,
    pub(in crate::platform::windows) extended_style: u32,
    pub(in crate::platform::windows) elevated: bool,
    pub(in crate::platform::windows) window_rect: PixelRect<Physical>,
    pub(in crate::platform::windows) monitor_work_area: Option<PixelRect<Physical>>,
}

impl std::fmt::Display for WindowObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[pid={}|window_id={}] ", self.pid, self.id)?;
        write!(f, "{}", self.metadata)
    }
}

#[derive(Debug, Clone)]
pub(in crate::platform::windows) struct WindowMetadata {
    pub(in crate::platform::windows) title: Option<String>,
    pub(in crate::platform::windows) process: String,
    pub(in crate::platform::windows) process_path: Option<String>,
    pub(in crate::platform::windows) class: Option<String>,
    pub(in crate::platform::windows) aumid: Option<String>,
    pub(in crate::platform::windows) app_name: Option<String>,
}

impl std::fmt::Display for WindowMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.app_name.as_deref() {
            Some(name) => write!(f, "{name} ({})", self.process)?,
            None => write!(f, "{}", self.process)?,
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        if let Some(class) = &self.class {
            write!(f, " class={class}")?;
        }
        if let Some(aumid) = &self.aumid {
            write!(f, " aumid={aumid}")?;
        }
        Ok(())
    }
}

pub(in crate::platform::windows) enum PlacementObservation {
    Minimized,
    Visible {
        rect: PixelRect<Physical>,
        monitor: isize,
    },
}

pub(in crate::platform::windows) enum WindowNotification {
    WindowObserved {
        window: WindowObservation,
        rect: PixelRect<Physical>,
        monitor: isize,
        dragged_at: Option<Instant>,
    },
    WindowRemoved(WindowId),
    WindowMinimized(WindowId),
    FocusChanged(WindowId),
    FocusSettled(WindowId),
    MovementStarted(WindowId),
    PlacementChanged {
        window_id: WindowId,
        observed_at: Instant,
        placement: Option<PlacementObservation>,
        constraints: Option<LimitObservation>,
    },
    TitleChanged {
        window_id: WindowId,
        title: Option<String>,
    },
    ConstraintsChanged {
        window_id: WindowId,
        constraints: LimitObservation,
    },
    PeriodicTick,
}
