use super::recovery::Recovery;
use super::registry::ManagedWindow;
use crate::core::{MonitorId, Physical, PixelRect};
use crate::platform::windows::dome::events::{MonitorScene, WindowPlacement};
use crate::platform::windows::external::HwndId;
use crate::platform::windows::window_server::{FullscreenKind, WindowServer};
use std::time::Instant;

pub(in crate::platform::windows) const MAX_DRIFT_RETRIES: u8 = 5;

pub(super) struct TilingPlacement {
    placed_at: Option<Instant>,
    corrections: u8,
}

pub(super) struct FloatPlacement {
    placed_at: Option<Instant>,
}

#[derive(Default)]
pub(super) enum PlacementProgress {
    Tiling(TilingPlacement),
    Float(FloatPlacement),
    Hidden {
        placed_at: Option<Instant>,
        corrections: u8,
    },
    #[default]
    Unplaced,
}

impl PlacementProgress {
    pub(super) fn placed_at(&self) -> Option<Instant> {
        match self {
            Self::Tiling(placement) => placement.placed_at,
            Self::Float(placement) => placement.placed_at,
            Self::Hidden { placed_at, .. } => *placed_at,
            Self::Unplaced => None,
        }
    }

    pub(super) fn reset(&mut self, target: PlacementTarget) {
        let placed_at = self.placed_at();
        *self = match target {
            PlacementTarget::Tiling { .. } | PlacementTarget::Fullscreen { .. } => {
                Self::Tiling(TilingPlacement {
                    placed_at,
                    corrections: 0,
                })
            }
            PlacementTarget::Float { .. } => Self::Float(FloatPlacement { placed_at }),
            PlacementTarget::Hidden => Self::Hidden {
                placed_at,
                corrections: 0,
            },
            PlacementTarget::Minimized => return,
        };
    }

    fn submitted(&mut self, now: Instant) {
        match self {
            Self::Tiling(placement) => placement.placed_at = Some(now),
            Self::Float(placement) => placement.placed_at = Some(now),
            _ => {}
        }
    }

    pub(super) fn allow_correction(&mut self, inclusive: bool) -> bool {
        let corrections = match self {
            Self::Tiling(placement) => &mut placement.corrections,
            Self::Hidden { corrections, .. } => corrections,
            _ => return false,
        };
        *corrections = corrections.saturating_add(1);
        if inclusive {
            *corrections <= MAX_DRIFT_RETRIES
        } else {
            *corrections < MAX_DRIFT_RETRIES
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum PlacementTarget {
    Tiling {
        frame: PixelRect<Physical>,
        monitor: MonitorId,
    },
    Float {
        frame: PixelRect<Physical>,
        monitor: MonitorId,
    },
    Fullscreen {
        frame: PixelRect<Physical>,
        monitor: MonitorId,
    },
    Hidden,
    Minimized,
}

impl PlacementTarget {
    pub(super) fn from_placement(
        placement: WindowPlacement,
        monitors: &[MonitorScene],
    ) -> Option<Self> {
        Some(match placement {
            WindowPlacement::Tiling { placement, monitor } => {
                if placement.content_box.is_empty() {
                    Self::Hidden
                } else {
                    Self::Tiling {
                        frame: placement.content_box,
                        monitor,
                    }
                }
            }
            WindowPlacement::Float { placement, monitor } => {
                if placement.content_box.is_empty() {
                    Self::Hidden
                } else {
                    Self::Float {
                        frame: placement.content_box,
                        monitor,
                    }
                }
            }
            WindowPlacement::Fullscreen { monitor } => Self::Fullscreen {
                frame: monitors.iter().find(|m| m.id == monitor)?.work_area,
                monitor,
            },
            WindowPlacement::Hidden => Self::Hidden,
            WindowPlacement::Minimized => Self::Minimized,
        })
    }
    pub(super) fn frame(self) -> Option<PixelRect<Physical>> {
        match self {
            Self::Tiling { frame, .. }
            | Self::Float { frame, .. }
            | Self::Fullscreen { frame, .. } => Some(frame),
            _ => None,
        }
    }
}

pub(super) fn apply_target(
    entry: &mut ManagedWindow,
    key: HwndId,
    target: PlacementTarget,
    now: Instant,
    recovery: &mut Recovery,
    windows: &mut dyn WindowServer,
) {
    if target == PlacementTarget::Minimized {
        entry.minimized = true;
        return;
    }
    if entry.target == Some(target) {
        return;
    }
    entry.target = Some(target);
    entry.placement.reset(target);
    entry.awaiting_response = false;
    if entry.fullscreen == Some(FullscreenKind::Exclusive) {
        return;
    }
    entry.minimized = false;
    if entry.dome_minimized {
        if target == PlacementTarget::Hidden {
            return;
        }
        recovery.track(key, entry.is_maximized);
        windows.unminimize(key);
        entry.dome_minimized = false;
    }
    match target {
        PlacementTarget::Hidden => {
            recovery.track(key, entry.is_maximized);
            if entry.fullscreen == Some(FullscreenKind::Borderless) {
                windows.minimize(key);
                entry.dome_minimized = true;
            } else {
                windows.move_offscreen(key);
            }
        }
        PlacementTarget::Fullscreen { .. } if entry.fullscreen.is_some() => {}
        _ => {
            if let Some(frame) = target.frame() {
                recovery.track(key, entry.is_maximized);
                windows.set_frame(key, frame);
                entry.placement.submitted(now);
            }
        }
    }
}

pub(super) fn borderless(
    frame: PixelRect<Physical>,
    monitor: isize,
    monitors: &[MonitorScene],
) -> bool {
    monitors
        .iter()
        .find(|m| m.native == monitor)
        .is_some_and(|m| {
            let work = m.system_work_area;
            frame.x() <= work.x()
                && frame.y() <= work.y()
                && frame.right() >= work.right()
                && frame.bottom() >= work.bottom()
        })
}
