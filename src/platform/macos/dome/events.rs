use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use objc2_core_graphics::{CGDirectDisplayID, CGWindowID};

use super::super::MonitorInfo;
use crate::action::{Actions, Query, WorkspaceInfo};
use crate::config::{Appearance, Keystroke};
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, FloatWindowPlacement, Length, LimitObservation,
    Logical, MonitorId, PixelRect, TilingWindowPlacement, WindowId, WindowMatcher, WindowMetadata,
    pattern_matches,
};

pub(in crate::platform::macos) enum HubEvent {
    WindowsDiscovered(Vec<DiscoveredWindow>),
    WindowGone(CGWindowID),
    WindowMinimized(CGWindowID),
    WindowRestored(CGWindowID),
    /// Zero reports unresolved foreground and still clears pending domain intent.
    FocusReported(CGWindowID),
    TitleChanged {
        key: CGWindowID,
        title: String,
    },
    SizeConstraint {
        key: CGWindowID,
        limits: LimitObservation,
    },
    FloatMoved {
        key: CGWindowID,
        frame: PixelRect<Logical>,
    },
    FullscreenChanged {
        key: CGWindowID,
        fullscreen: Option<FullscreenKind>,
        /// Observed window frame, absent for native fullscreen Spaces.
        frame: Option<PixelRect<Logical>>,
    },
    Action(Actions),
    Query {
        query: Query,
        sender: std::sync::mpsc::SyncSender<String>,
    },
    RunBinding {
        keymap: String,
        keystroke: Keystroke,
    },
    ReloadConfig,
    ApplyLayout(String),
    SaveLayout(String),
    MonitorsChanged(Vec<MonitorInfo>),
    MirrorClicked(CGWindowID),
    TabClicked(ContainerId, usize),
    Shutdown,
}

impl fmt::Display for HubEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowsDiscovered(windows) => {
                write!(f, "WindowsDiscovered(count={})", windows.len())
            }
            Self::WindowGone(key) => write!(f, "WindowGone({key})"),
            Self::WindowMinimized(key) => write!(f, "WindowMinimized({key})"),
            Self::WindowRestored(key) => write!(f, "WindowRestored({key})"),
            Self::FocusReported(key) => write!(f, "FocusReported({key})"),
            Self::TitleChanged { key, .. } => write!(f, "TitleChanged({key})"),
            Self::SizeConstraint { key, .. } => write!(f, "SizeConstraint({key})"),
            Self::FloatMoved { key, .. } => write!(f, "FloatMoved({key})"),
            Self::FullscreenChanged {
                key, fullscreen, ..
            } => {
                write!(f, "FullscreenChanged({key}, {fullscreen:?})")
            }
            Self::Action(actions) => write!(f, "Action({actions})"),
            Self::Query { query, .. } => write!(f, "Query({query:?})"),
            Self::RunBinding { keymap, keystroke } => {
                write!(f, "RunBinding(keymap={keymap}, keystroke={keystroke})")
            }
            Self::ReloadConfig => write!(f, "ReloadConfig"),
            Self::ApplyLayout(_) => write!(f, "ApplyLayout"),
            Self::SaveLayout(_) => write!(f, "SaveLayout"),
            Self::MonitorsChanged(monitors) => {
                write!(f, "MonitorsChanged(count={})", monitors.len())
            }
            Self::MirrorClicked(key) => write!(f, "MirrorClicked({key})"),
            Self::TabClicked(container, index) => write!(f, "TabClicked({container}, {index})"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

pub(in crate::platform::macos) enum HubMessage {
    Scene(Scene),
    Command(WindowCommand),
    AppearanceChanged(Appearance),
    Shutdown,
}

#[derive(Clone)]
pub(in crate::platform::macos) struct Scene {
    pub windows: HashMap<CGWindowID, ExpectedWindow>,
    pub monitors: Vec<MonitorScene>,
    pub focused_window: Option<CGWindowID>,
    pub focus_request: Option<FocusTarget>,
    pub workspaces: Vec<WorkspaceInfo>,
    /// Domain display-event time, not the time this scene reaches UI.
    pub displays_changed_at: Instant,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::macos) struct ExpectedWindow {
    pub id: WindowId,
    pub placement: WindowPlacement,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::macos) enum WindowPlacement {
    Tiling {
        monitor: MonitorId,
        placement: TilingWindowPlacement,
    },
    Float {
        monitor: MonitorId,
        placement: FloatWindowPlacement,
    },
    Fullscreen {
        monitor: MonitorId,
    },
    Hidden,
    Minimized,
}

#[derive(Clone, Debug)]
pub(in crate::platform::macos) struct MonitorScene {
    pub id: MonitorId,
    pub native: CGDirectDisplayID,
    pub work_area: PixelRect<Logical>,
    pub system_work_area: PixelRect<Logical>,
    pub bounds: Dimension<Logical>,
    pub is_primary: bool,
    pub full_height: f32,
    /// Apply only to render density, not to logical placements.
    pub scale: f64,
    pub border_thickness: Length<Logical>,
    pub containers: Vec<ContainerPlacement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::platform::macos) enum FocusTarget {
    Window(CGWindowID),
    Monitor(MonitorId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::platform::macos) enum WindowCommand {
    Close(CGWindowID),
    Unminimize(CGWindowID),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::platform::macos) enum FullscreenKind {
    Borderless,
    Native,
}

#[derive(Clone, Debug)]
pub(in crate::platform::macos) struct DiscoveredWindow {
    pub key: CGWindowID,
    pub process_id: i32,
    pub metadata: MacOsMetadata,
    /// Native fullscreen and unreadable minimized windows have no ordinary frame.
    pub frame: Option<PixelRect<Logical>>,
    pub corner_radius: Length<Logical>,
    pub minimized: bool,
    pub fullscreen: Option<FullscreenKind>,
}

#[derive(Debug, Clone)]
pub(in crate::platform::macos) struct MacOsMetadata {
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

impl fmt::Display for MacOsMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.app_name.as_deref().unwrap_or("Unknown"))?;
        if let Some(bundle_id) = &self.bundle_id {
            write!(f, " ({bundle_id})")?;
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        Ok(())
    }
}

impl WindowMetadata for MacOsMetadata {
    fn app_name(&self) -> Option<String> {
        self.app_name.clone()
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn bundle_id(&self) -> Option<String> {
        self.bundle_id.clone()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn WindowMetadata> {
        Box::new(self.clone())
    }
    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool {
        let app = self.app_name.as_deref();
        let bundle_id = self.bundle_id.as_deref();
        let title = self.title.as_deref();
        if let Some(pattern) = matcher.app.as_deref()
            && !app.is_some_and(|value| pattern_matches(pattern, value))
        {
            return false;
        }
        if let Some(pattern) = matcher.bundle_id.as_deref()
            && !bundle_id.is_some_and(|value| pattern_matches(pattern, value))
        {
            return false;
        }
        if let Some(pattern) = matcher.title.as_deref()
            && !title.is_some_and(|value| pattern_matches(pattern, value))
        {
            return false;
        }
        if app.is_none() && bundle_id.is_none() && title.is_none() {
            return false;
        }
        matcher.app.is_some() || matcher.bundle_id.is_some() || matcher.title.is_some()
    }
    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            app: self.app_name.clone(),
            bundle_id: self.bundle_id.clone(),
            title: self.title.clone(),
            ..Default::default()
        }
    }
}
