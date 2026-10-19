use super::dome::Dome;
use super::event_loop::{NativeEvent, RunLoop};
use super::timer::Timer;
use anyhow::{Context, Result};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID};
use objc2_foundation::{NSNumber, NSString};
use std::time::Duration;

pub(super) struct MonitorObservation {
    pub(super) display_id: CGDirectDisplayID,
    pub(super) name: String,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) top_inset: f64,
    pub(super) bottom_inset: f64,
    pub(super) is_primary: bool,
    pub(super) backing_scale: f64,
}

pub(super) fn get_all_monitors(main_thread: MainThreadMarker) -> Result<Vec<MonitorObservation>> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(main_thread)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen)?;
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            Ok(MonitorObservation {
                display_id,
                name,
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
                top_inset,
                bottom_inset,
                is_primary: display_id == primary_id,
                backing_scale: screen.backingScaleFactor(),
            })
        })
        .collect()
}

pub(super) fn register(events: UnboundedReceiver<NativeEvent>) {
    let (sender, settled) = RunLoop::channel();
    let events = futures::stream::select(events.map(|_| DisplayChange::Changed), settled);
    let mut changes = DisplayChanges::default();
    RunLoop::insert(events, move |event, dome: &mut Dome| match event {
        DisplayChange::Changed => {
            changes.read_completed(
                get_all_monitors(MainThreadMarker::new().expect("owner thread")),
                &sender,
                dome,
            );
        }
        DisplayChange::Settled(generation) => {
            if changes.finish(generation) {
                dome.finish_monitor_settle();
            }
        }
    });
}

fn get_display_id(screen: &NSScreen) -> Result<CGDirectDisplayID> {
    let description = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    let object = description
        .objectForKey(&key)
        .context("NSScreen deviceDescription is missing NSScreenNumber")?;
    let number: &NSNumber = object
        .downcast_ref()
        .context("NSScreenNumber is not an NSNumber")?;
    Ok(number.unsignedIntValue())
}

enum DisplayChange {
    Changed,
    Settled(u64),
}

#[derive(Default)]
struct DisplayChanges {
    generation: u64,
    pending: bool,
    timer: Option<Timer>,
}

impl DisplayChanges {
    fn read_completed(
        &mut self,
        result: Result<Vec<MonitorObservation>>,
        sender: &super::event_loop::EventSender<DisplayChange>,
        dome: &mut Dome,
    ) {
        if let Some((generation, monitors)) = self.accept_read(result) {
            let sender = sender.clone();
            self.timer = Some(Timer::once(Duration::from_secs(10), move || {
                let _ = sender.send(DisplayChange::Settled(generation));
            }));
            dome.monitors_changed(monitors.into_iter().map(Into::into).collect());
            dome.update_menu_bar();
        }
    }

    fn accept_read(
        &mut self,
        result: Result<Vec<MonitorObservation>>,
    ) -> Option<(u64, Vec<MonitorObservation>)> {
        match result {
            Ok(monitors) => Some((self.next_generation(), monitors)),
            Err(error) => {
                tracing::error!(%error, "Failed to enumerate monitors");
                None
            }
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("monitor settle generation exhausted");
        self.pending = true;
        self.generation
    }

    fn finish(&mut self, generation: u64) -> bool {
        if !self.pending || generation != self.generation {
            return false;
        }
        self.pending = false;
        self.timer = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::macos::dome::MonitorInfo;
    use dome_core::PixelRect;

    fn work_area() -> MonitorObservation {
        MonitorObservation {
            display_id: 1,
            name: "Primary".into(),
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 40.0,
            top_inset: 5.0,
            bottom_inset: 3.0,
            is_primary: true,
            backing_scale: 2.0,
        }
    }

    #[test]
    fn read_display_on_success_preserves_insets_and_rejects_obsolete_settle() {
        let mut changes = DisplayChanges::default();
        let (earlier, monitors) = changes.accept_read(Ok(vec![work_area()])).unwrap();
        let monitor = MonitorInfo::from(monitors.into_iter().next().unwrap());
        assert_eq!(monitor.work_area, PixelRect::new(0, 5, 120, 32));

        let (current, _) = changes.accept_read(Ok(vec![work_area()])).unwrap();
        assert!(!changes.finish(earlier));
        assert!(changes.finish(current));
        assert!(!changes.finish(current));
    }

    #[test]
    fn read_display_on_failure_rejects_replacement_and_preserves_pending_settle() {
        let mut changes = DisplayChanges::default();
        let (pending, _) = changes.accept_read(Ok(vec![work_area()])).unwrap();
        assert!(
            changes
                .accept_read(Err(anyhow::anyhow!("screen unavailable")))
                .is_none()
        );
        assert!(changes.finish(pending));
    }
}
