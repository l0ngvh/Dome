use std::time::{Duration, Instant};

use objc2_core_graphics::CGWindowID;

use super::registry::ManagedWindow;
use crate::core::{Length, LimitObservation, LimitUpdate, PixelRect, Pixels};
use crate::platform::macos::dome::events::{FullscreenKind, HubEvent, MonitorScene};
use crate::platform::macos::window_server::{PositionReadOrigin, WindowServer};

const MAX_DRIFT_RETRIES: u8 = 5;
const MOVE_SETTLE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlacementTarget {
    Tiling(PixelRect),
    Float(PixelRect),
    Fullscreen(PixelRect),
    Hidden(Pixels, Pixels),
    Minimized,
}

#[derive(Clone, Copy)]
pub(super) enum WindowState {
    Unplaced,
    Offscreen {
        actual: PixelRect,
        retries: u8,
    },
    Tiling(TilingPlacement),
    Float {
        target: PixelRect,
        placed_at: Instant,
    },
    NativeFullscreen,
    BorderlessFullscreen,
    BorderlessMinimized {
        retries: u8,
    },
}

#[derive(Clone, Copy)]
pub(super) struct TilingPlacement {
    target: PixelRect,
    actual: PixelRect,
    retries: u8,
    placed_at: Instant,
    suppress_detect_until: Option<Instant>,
}

impl TilingPlacement {
    fn new(actual: PixelRect, target: PixelRect, monitors: &[MonitorScene], now: Instant) -> Self {
        Self {
            target,
            actual,
            retries: 0,
            placed_at: now,
            suppress_detect_until: crosses_monitor(monitors, actual, target)
                .then_some(now + MOVE_SETTLE),
        }
    }

    fn set_target(&mut self, target: PixelRect, monitors: &[MonitorScene], now: Instant) -> bool {
        if self.target == target {
            return false;
        }
        self.suppress_detect_until =
            crosses_monitor(monitors, self.actual, target).then_some(now + MOVE_SETTLE);
        self.target = target;
        self.retries = 0;
        self.placed_at = now;
        true
    }

    fn has_drifted(&self, actual: PixelRect) -> bool {
        let target = self.target;
        !((actual.x() == target.x() || actual.right() == target.right())
            && (actual.y() == target.y() || actual.bottom() == target.bottom()))
    }

    fn correct(&mut self, key: CGWindowID, actual: PixelRect, windows: &mut dyn WindowServer) {
        self.actual = actual;
        self.retries = self.retries.saturating_add(1);
        if self.retries <= MAX_DRIFT_RETRIES {
            windows.set_frame(key, self.target);
        }
    }

    fn constraint(&self) -> Option<LimitObservation> {
        let (actual, target) = (self.actual, self.target);
        if actual.width() == target.width() && actual.height() == target.height() {
            return None;
        }
        let observed = |value: Option<Pixels>| {
            value.map_or(LimitUpdate::Unchanged, |v| {
                LimitUpdate::Set(Length::from_pixels(v))
            })
        };
        Some(LimitObservation {
            min_width: observed((actual.width() > target.width()).then_some(actual.width())),
            min_height: observed((actual.height() > target.height()).then_some(actual.height())),
            max_width: observed((actual.width() < target.width()).then_some(actual.width())),
            max_height: observed((actual.height() < target.height()).then_some(actual.height())),
        })
    }
}

pub(super) fn hidden_position(monitors: &[MonitorScene]) -> Option<(Pixels, Pixels)> {
    let monitor = monitors
        .iter()
        .max_by_key(|m| m.system_work_area.right() + m.system_work_area.bottom())?;
    Some((
        monitor.system_work_area.right() - Pixels::new(1),
        monitor.system_work_area.bottom() - Pixels::new(1),
    ))
}

fn move_offscreen(
    key: CGWindowID,
    actual: PixelRect,
    position: (Pixels, Pixels),
    windows: &mut dyn WindowServer,
) {
    if actual.x() != position.0 && actual.y() != position.1 {
        windows.move_offscreen(
            key,
            (
                Length::from_pixels(position.0),
                Length::from_pixels(position.1),
            ),
        );
    }
}

pub(super) fn apply_target(
    entry: &mut ManagedWindow,
    key: CGWindowID,
    target: PlacementTarget,
    monitors: &[MonitorScene],
    now: Instant,
    windows: &mut dyn WindowServer,
) {
    if entry.target == Some(target) {
        return;
    }
    if entry.target == Some(PlacementTarget::Minimized) {
        entry.state = WindowState::Unplaced;
    }
    if entry.target.is_none()
        && entry.fullscreen.is_none()
        && entry
            .actual
            .is_some_and(|frame| is_borderless_fullscreen(monitors, frame))
    {
        entry.fullscreen = Some(FullscreenKind::Borderless);
    }
    entry.target = Some(target);
    entry.awaiting_response = false;
    match target {
        PlacementTarget::Hidden(x, y) => {
            if entry.minimized {
                return;
            }
            match entry.fullscreen {
                Some(FullscreenKind::Native) => entry.state = WindowState::NativeFullscreen,
                Some(FullscreenKind::Borderless) => {
                    if !matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
                        windows.minimize(key);
                        entry.state = WindowState::BorderlessMinimized { retries: 0 };
                    }
                }
                None => {
                    if matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
                        windows.unminimize(key);
                    }
                    if let Some(actual) = entry.actual {
                        move_offscreen(key, actual, (x, y), windows);
                        entry.state = WindowState::Offscreen { actual, retries: 0 };
                    }
                }
            }
        }
        PlacementTarget::Minimized => {
            if !entry.minimized {
                windows.minimize(key);
            }
        }
        PlacementTarget::Fullscreen(target) => {
            show_fullscreen_window(entry, key, target, monitors, now, windows)
        }
        PlacementTarget::Tiling(target) | PlacementTarget::Float(target) => {
            if entry.minimized || matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
                windows.unminimize(key);
            }
            if entry.fullscreen == Some(FullscreenKind::Native) {
                return;
            }
            if target.is_empty() {
                return;
            }
            if matches!(entry.target, Some(PlacementTarget::Tiling(_))) {
                match &mut entry.state {
                    WindowState::Tiling(placement) => {
                        if placement.set_target(target, monitors, now) {
                            windows.set_frame(key, target);
                        }
                    }
                    _ => {
                        entry.state = WindowState::Tiling(TilingPlacement::new(
                            entry.actual.unwrap_or(target),
                            target,
                            monitors,
                            now,
                        ));
                        windows.set_frame(key, target);
                    }
                }
            } else {
                let changed = !matches!(entry.state, WindowState::Float { target: previous, .. } if previous == target);
                if changed {
                    entry.state = WindowState::Float {
                        target,
                        placed_at: now,
                    };
                    windows.set_frame(key, target);
                }
            }
        }
    }
}

fn show_fullscreen_window(
    entry: &mut ManagedWindow,
    key: CGWindowID,
    target: PixelRect,
    monitors: &[MonitorScene],
    now: Instant,
    windows: &mut dyn WindowServer,
) {
    if matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
        windows.unminimize(key);
        entry.state = WindowState::BorderlessFullscreen;
        return;
    }
    if entry.fullscreen == Some(FullscreenKind::Native) {
        entry.state = WindowState::NativeFullscreen;
        return;
    }
    if entry.fullscreen == Some(FullscreenKind::Borderless) {
        entry.state = WindowState::BorderlessFullscreen;
        return;
    }
    if entry.minimized {
        windows.unminimize(key);
    }
    match &mut entry.state {
        WindowState::Tiling(placement) => {
            if placement.set_target(target, monitors, now) {
                windows.set_frame(key, target);
            }
        }
        WindowState::Float {
            target: previous,
            placed_at,
        } => {
            if *previous != target {
                *previous = target;
                *placed_at = now;
                windows.set_frame(key, target);
            }
        }
        _ => {
            entry.state = WindowState::Tiling(TilingPlacement::new(
                entry.actual.unwrap_or(target),
                target,
                monitors,
                now,
            ));
            windows.set_frame(key, target);
        }
    }
}

pub(super) fn is_stale(entry: &ManagedWindow, origin: PositionReadOrigin) -> bool {
    let placed_at = match entry.state {
        WindowState::Tiling(placement) => Some(placement.placed_at),
        WindowState::Float { placed_at, .. } => Some(placed_at),
        _ => None,
    };
    matches!((origin, placed_at), (PositionReadOrigin::Settle(burst), Some(at)) if burst.last < at)
}

pub(super) struct PlacementContext<'a> {
    pub(super) monitors: &'a [MonitorScene],
    pub(super) display_changed_at: Option<Instant>,
    pub(super) now: Instant,
}

pub(super) fn observe_frame(
    entry: &mut ManagedWindow,
    key: CGWindowID,
    frame: PixelRect,
    origin: PositionReadOrigin,
    context: PlacementContext<'_>,
    windows: &mut dyn WindowServer,
    reports: &mut Vec<HubEvent>,
) {
    let PlacementContext {
        monitors,
        display_changed_at,
        now,
    } = context;
    if is_stale(entry, origin) {
        return;
    }
    let previous_actual = entry.actual;
    entry.actual = Some(frame);
    let target_matches = match entry.state {
        WindowState::Tiling(placement) => placement.target == frame,
        WindowState::Float { target, .. } => target == frame,
        _ => false,
    };
    let fullscreen = !target_matches && is_borderless_fullscreen(monitors, frame);
    let hidden = matches!(entry.target, Some(PlacementTarget::Hidden(..)));
    if fullscreen && entry.fullscreen != Some(FullscreenKind::Borderless) {
        entry.fullscreen = Some(FullscreenKind::Borderless);
        entry.awaiting_response = true;
        reports.push(HubEvent::FullscreenChanged {
            key,
            fullscreen: entry.fullscreen,
            frame: Some(frame),
        });
        if hidden {
            entry.state = WindowState::BorderlessMinimized { retries: 0 };
            windows.minimize(key);
        } else {
            entry.state = WindowState::BorderlessFullscreen;
        }
        return;
    }
    if !fullscreen && entry.fullscreen.is_some() {
        entry.fullscreen = None;
        entry.awaiting_response = true;
        reports.push(HubEvent::FullscreenChanged {
            key,
            fullscreen: None,
            frame: Some(frame),
        });
        if hidden {
            if matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
                windows.unminimize(key);
            }
            if let Some(position) = hidden_position(monitors) {
                move_offscreen(key, frame, position, windows);
            }
            entry.state = WindowState::Offscreen {
                actual: frame,
                retries: 0,
            };
        } else {
            entry.state = WindowState::Unplaced;
        }
        return;
    }
    match &mut entry.state {
        WindowState::BorderlessMinimized { retries } if hidden && fullscreen => {
            *retries = retries.saturating_add(1);
            if *retries <= MAX_DRIFT_RETRIES {
                windows.minimize(key);
            }
        }
        WindowState::Offscreen { actual, retries } => {
            *actual = frame;
            if let Some(position) = hidden_position(monitors)
                && frame.x() != position.0
                && frame.y() != position.1
            {
                *retries = retries.saturating_add(1);
                if *retries <= MAX_DRIFT_RETRIES {
                    move_offscreen(key, frame, position, windows);
                }
            }
        }
        WindowState::Tiling(placement) if !fullscreen && !entry.awaiting_response => {
            if frame == placement.target {
                placement.actual = frame;
                return;
            }
            if display_changed_at
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_secs(10))
                || placement
                    .suppress_detect_until
                    .is_some_and(|until| now < until)
            {
                placement.actual = frame;
                return;
            }
            let attributable = matches!(origin, PositionReadOrigin::Settle(burst)
                if placement.placed_at <= burst.first && burst.first <= placement.placed_at + Duration::from_secs(1));
            if attributable && !placement.has_drifted(frame) {
                placement.actual = frame;
                if let Some(limits) = placement.constraint() {
                    reports.push(HubEvent::SizeConstraint { key, limits });
                }
            } else {
                placement.correct(key, frame, windows);
            }
        }
        WindowState::Float { target, .. }
            if !fullscreen
                && frame != *target
                && (!entry.awaiting_response || previous_actual != Some(frame)) =>
        {
            entry.awaiting_response = true;
            reports.push(HubEvent::FloatMoved { key, frame });
        }
        _ => {}
    }
}

fn closest_monitor(monitors: &[MonitorScene], frame: PixelRect) -> Option<usize> {
    let dimension = frame.to_dimension();
    monitors
        .iter()
        .enumerate()
        .filter_map(|(index, monitor)| {
            let bounds = monitor.bounds;
            let width = ((dimension.x + dimension.width)
                .value()
                .min((bounds.x + bounds.width).value())
                - dimension.x.value().max(bounds.x.value()))
            .max(0.0);
            let height = ((dimension.y + dimension.height)
                .value()
                .min((bounds.y + bounds.height).value())
                - dimension.y.value().max(bounds.y.value()))
            .max(0.0);
            let area = width * height;
            (area > 0.0).then_some((index, area))
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
}

fn crosses_monitor(monitors: &[MonitorScene], from: PixelRect, to: PixelRect) -> bool {
    matches!((closest_monitor(monitors, from), closest_monitor(monitors, to)),
        (Some(a), Some(b)) if monitors[a].native != monitors[b].native)
}

fn is_borderless_fullscreen(monitors: &[MonitorScene], frame: PixelRect) -> bool {
    let point = PixelRect::from_pixels(frame.x(), frame.y(), Pixels::new(1), Pixels::new(1));
    closest_monitor(monitors, point).is_some_and(|index| {
        let area = monitors[index].system_work_area;
        let tolerance = Pixels::new(2);
        frame.x() <= area.x() + tolerance
            && frame.y() <= area.y() + tolerance
            && frame.right() >= area.right() - tolerance
            && frame.bottom() >= area.bottom() - tolerance
    })
}
