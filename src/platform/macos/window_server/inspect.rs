use super::{
    ApplicationLifetime, ApplicationResult, ExistingWindow, InspectionKind, InspectionOutcome,
    InspectionResult, NativeEvent, Observation, ObserverContext, ObserverSource,
};
use crate::core::{Dimension, Length, PixelRect};
use crate::platform::macos::Wake;
use crate::platform::macos::accessibility::{AXApp, AXWindow};
use crate::platform::macos::corner_radius::corner_radius;
use crate::platform::macos::dispatcher::{
    ApplicationNative, ApplicationOperation, DispatcherMarker,
};
use crate::platform::macos::dome::events::{DiscoveredWindow, FullscreenKind, MacOsMetadata};
use crate::platform::macos::focus::SharedFocus;
use crate::platform::macos::listeners::RetainedObserverRegistration;
use crate::platform::macos::objc2_wrapper::{get_attribute, get_cg_window_id, kAXTitleAttribute};
use crate::platform::macos::running_application::RunningApp;
use dome_auxiliary_window::LoopWaker;
use objc2_core_graphics::CGWindowID;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, mpsc};

pub(in crate::platform::macos) struct ApplicationAccess {
    lifetime: Arc<ApplicationLifetime>,
    app: Arc<AXApp>,
    focus: Arc<SharedFocus>,
    events: mpsc::Sender<NativeEvent>,
    results: mpsc::Sender<ApplicationResult>,
    loop_waker: Arc<OnceLock<LoopWaker<Wake>>>,
    handles: HashMap<CGWindowID, AXWindow>,
    observer: Option<RetainedObserverRegistration>,
}

impl ApplicationAccess {
    pub(in crate::platform::macos) fn new(
        lifetime: Arc<ApplicationLifetime>,
        app: Arc<AXApp>,
        focus: Arc<SharedFocus>,
        events: mpsc::Sender<NativeEvent>,
        results: mpsc::Sender<ApplicationResult>,
        loop_waker: Arc<OnceLock<LoopWaker<Wake>>>,
        handles: HashMap<CGWindowID, AXWindow>,
    ) -> Self {
        Self {
            lifetime,
            app,
            focus,
            events,
            results,
            loop_waker,
            handles,
            observer: None,
        }
    }

    fn publish(&self, result: ApplicationResult) {
        if self.results.send(result).is_ok()
            && let Some(waker) = self.loop_waker.get()
        {
            waker.wake(Wake::WindowServer);
        }
    }

    fn register(&mut self, marker: &DispatcherMarker) {
        self.app.refresh_enhanced_ui(marker);
        let app = self.app.clone();
        self.register_with(marker, move |context, marker| {
            RetainedObserverRegistration::new(app, context, marker)
        });
    }

    pub(super) fn register_with(
        &mut self,
        marker: &DispatcherMarker,
        create: impl FnOnce(
            Arc<ObserverContext>,
            &DispatcherMarker,
        ) -> Result<
            RetainedObserverRegistration,
            crate::platform::macos::objc2_wrapper::AXError,
        >,
    ) {
        if let Some(observer) = &mut self.observer {
            observer.repair(marker);
            return;
        }
        let Some(waker) = self.loop_waker.get() else {
            return;
        };
        let context = Arc::new(ObserverContext {
            lifetime: self.lifetime.clone(),
            events: self.events.clone(),
            loop_waker: waker.clone(),
        });
        let Ok(observer) = create(context, marker) else {
            return;
        };
        let result = ApplicationResult::ObserverReady {
            lifetime: self.lifetime.clone(),
            source: ObserverSource(observer.source()),
            context: observer.context(),
        };
        self.observer = Some(observer);
        self.publish(result);
    }

    fn inspect(&mut self, kind: InspectionKind, marker: &DispatcherMarker) {
        let mut handles = Vec::new();
        let outcome = match kind {
            InspectionKind::AppWindows => self.enumerate(marker, &mut handles),
            InspectionKind::Positions(origin) => {
                self.positions(marker).map(|windows| Observation::Settled {
                    lifetime: self.lifetime.clone(),
                    origin,
                    windows,
                })
            }
            InspectionKind::Space => self.positions(marker).map(|windows| Observation::Space {
                lifetime: self.lifetime.clone(),
                windows,
            }),
            InspectionKind::FocusedWindow => {
                if RunningApp::new(self.lifetime.pid).is_some_and(|app| app.is_active()) {
                    Some(Observation::FocusReported {
                        lifetime: self.lifetime.clone(),
                        window: self
                            .app
                            .clone()
                            .focused_window(marker)
                            .ok()
                            .map(|window| window.cg_id()),
                    })
                } else {
                    None
                }
            }
            InspectionKind::Title(key) => self
                .handles
                .get(&key)
                .and_then(|window| window.read_title(marker))
                .map(|title| Observation::TitleChanged {
                    lifetime: self.lifetime.clone(),
                    key,
                    title,
                }),
        };
        self.publish(ApplicationResult::Inspection(InspectionResult {
            lifetime: self.lifetime.clone(),
            kind,
            outcome: outcome.map_or(InspectionOutcome::Failed, InspectionOutcome::Completed),
            handle_updates: handles,
        }));
    }

    fn enumerate(
        &mut self,
        marker: &DispatcherMarker,
        updates: &mut Vec<AXWindow>,
    ) -> Option<Observation> {
        enumerate_windows(
            &self.lifetime,
            &mut self.handles,
            updates,
            self.app.clone().windows(marker).ok(),
            |window| window.is_valid(marker),
            |window| window.check_unmanageable(marker),
            |window| read_window(window, marker),
        )
    }

    fn positions(&self, marker: &DispatcherMarker) -> Option<Vec<ExistingWindow>> {
        let windows = self.app.clone().windows(marker).ok()?;
        Some(
            windows
                .iter()
                .filter(|window| self.handles.contains_key(&window.cg_id()))
                .filter_map(|window| read_window(window, marker))
                .collect(),
        )
    }
}

impl ApplicationNative for ApplicationAccess {
    fn probe(&mut self, marker: &DispatcherMarker) {
        self.app.refresh_enhanced_ui(marker);
    }

    fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
        use ApplicationOperation::*;
        let result = match operation {
            SetFrame { window, frame } | Recover { window, frame } => {
                window.set_frame(marker, frame)
            }
            MoveOffscreen {
                window,
                position: (x, y),
            } => window.move_offscreen(marker, x, y),
            Minimize(window) => window.minimize(marker),
            Unminimize(window) => window.unminimize(marker),
            Close(window) => window.close(marker),
            Focus(window) => {
                if self.focus.is_submitted(window.cg_id()) {
                    window.focus(marker)
                } else {
                    Ok(())
                }
            }
            Read(kind) => {
                self.inspect(kind, marker);
                Ok(())
            }
            ResolveTitle(element) => {
                let element = element.0;
                if let Ok(key) = get_cg_window_id(marker, &element) {
                    let title = get_attribute::<objc2_core_foundation::CFString>(
                        marker,
                        &element,
                        &kAXTitleAttribute(),
                    )
                    .ok();
                    self.publish(ApplicationResult::Inspection(InspectionResult {
                        lifetime: self.lifetime.clone(),
                        kind: InspectionKind::Title(key),
                        outcome: title.map_or(InspectionOutcome::Failed, |title| {
                            InspectionOutcome::Completed(Observation::TitleChanged {
                                lifetime: self.lifetime.clone(),
                                key,
                                title: title.to_string(),
                            })
                        }),
                        handle_updates: Vec::new(),
                    }));
                }
                Ok(())
            }
            RegisterObserver => {
                self.register(marker);
                Ok(())
            }
            RemoveObserver(observer) => {
                observer.remove(marker);
                Ok(())
            }
            Restore(recovery) => {
                for entry in recovery {
                    if let Some(window) = self.handles.get(&entry.window).cloned() {
                        self.execute(
                            Recover {
                                window,
                                frame: entry.original_rect,
                            },
                            marker,
                        );
                    }
                }
                Ok(())
            }
            Terminate => {
                if let Some(observer) = self.observer.take() {
                    self.execute(RemoveObserver(observer), marker);
                }
                Ok(())
            }
        };
        if let Err(error) = result {
            tracing::trace!(pid = self.lifetime.pid, %error, "Application operation failed");
        }
    }
}

fn read_window(window: &AXWindow, marker: &DispatcherMarker) -> Option<ExistingWindow> {
    read_window_with(
        window.cg_id(),
        || window.is_minimized(marker).ok(),
        || window.is_native_fullscreen(marker).ok(),
        || {
            let (x, y) = window.get_position(marker).ok()?;
            let (width, height) = window.get_size(marker).ok()?;
            Some((
                PixelRect::from_dimension(Dimension::new(x, y, width, height)),
                corner_radius((x, y), window.read_close_button_frame(marker)),
            ))
        },
    )
}

pub(super) fn read_window_with(
    key: CGWindowID,
    minimized: impl FnOnce() -> Option<bool>,
    fullscreen: impl FnOnce() -> Option<bool>,
    geometry: impl FnOnce() -> Option<(
        PixelRect<crate::core::Logical>,
        Length<crate::core::Logical>,
    )>,
) -> Option<ExistingWindow> {
    let minimized = minimized()?;
    let fullscreen = fullscreen()?.then_some(FullscreenKind::Native);
    let (frame, radius) = if !minimized && fullscreen.is_none() {
        let (frame, radius) = geometry()?;
        (Some(frame), radius)
    } else {
        (None, Length::new(0.0))
    };
    Some(ExistingWindow {
        key,
        frame,
        corner_radius: radius,
        minimized,
        fullscreen,
    })
}

pub(super) fn enumerate_windows(
    lifetime: &Arc<ApplicationLifetime>,
    handles: &mut HashMap<CGWindowID, AXWindow>,
    updates: &mut Vec<AXWindow>,
    windows: Option<Vec<AXWindow>>,
    is_valid: impl Fn(&AXWindow) -> bool,
    check_unmanageable: impl Fn(&AXWindow) -> bool,
    read_window: impl Fn(&AXWindow) -> Option<ExistingWindow>,
) -> Option<Observation> {
    let windows = windows?;
    let present: HashSet<_> = windows.iter().map(AXWindow::cg_id).collect();
    let removed: Vec<_> = handles
        .iter()
        .filter_map(|(&key, window)| (!present.contains(&key) && !is_valid(window)).then_some(key))
        .collect();
    for key in &removed {
        handles.remove(key);
    }
    let mut discovered = Vec::new();
    let mut existing = Vec::new();
    for window in windows {
        let key = window.cg_id();
        if !handles.contains_key(&key) && check_unmanageable(&window) {
            continue;
        }
        handles.insert(key, window.clone());
        updates.push(window.clone());
        let Some(observed) = read_window(&window) else {
            continue;
        };
        discovered.push(DiscoveredWindow {
            key,
            process_id: lifetime.pid,
            metadata: MacOsMetadata {
                title: window.title().map(str::to_owned),
                app_name: window.app_name().map(str::to_owned),
                bundle_id: window.bundle_id().map(str::to_owned),
            },
            frame: observed.frame,
            corner_radius: observed.corner_radius,
            minimized: observed.minimized,
            fullscreen: observed.fullscreen,
        });
        existing.push(observed);
    }
    for (&key, window) in handles.iter() {
        if !present.contains(&key)
            && let Some(observed) = read_window(window)
        {
            existing.push(observed);
        }
    }
    Some(Observation::AppWindows {
        lifetime: lifetime.clone(),
        discovered,
        existing,
        removed,
    })
}
