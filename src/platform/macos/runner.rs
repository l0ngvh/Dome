use std::sync::atomic::{AtomicBool, Ordering};

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::{EventLoop, LoopSignal};

use crate::logging::Logger;

use super::dome::{Dome, HubEvent, HubMessage, SceneSender};
use super::login_item;

/// Install immediately after receiving the UI sender, before domain setup.
pub(super) struct DomainExitGuard {
    ui: Box<dyn SceneSender>,
}

impl DomainExitGuard {
    pub(super) fn new(ui: Box<dyn SceneSender>) -> Self {
        Self { ui }
    }
}

impl Drop for DomainExitGuard {
    fn drop(&mut self) {
        self.ui.send(HubMessage::Shutdown);
    }
}

pub(super) struct DomeRunner {
    dome: Dome,
    signal: LoopSignal,
    logger: Logger,
    bundle_path: Option<String>,
}

pub(super) fn run_dome(
    dome: Dome,
    channel: Channel<HubEvent>,
    logger: Logger,
    bundle_path: Option<String>,
) {
    install_signal_handlers();
    let mut event_loop =
        EventLoop::<'static, DomeRunner>::try_new().expect("Failed to create event loop");
    let handle = event_loop.handle();
    let mut runner = DomeRunner {
        dome,
        signal: event_loop.get_signal(),
        logger,
        bundle_path,
    };
    handle
        .insert_source(channel, |event, _, runner: &mut DomeRunner| match event {
            ChannelEvent::Msg(event) => handle_event(runner, event),
            ChannelEvent::Closed => runner.signal.stop(),
        })
        .expect("Failed to insert channel source");
    runner.dome.flush_layout();
    event_loop
        .run(None, &mut runner, |runner| {
            if SIGNAL_RECEIVED.load(Ordering::Relaxed) {
                runner.signal.stop();
            }
        })
        .expect("Event loop failed");
}

pub(super) fn apply_report(dome: &mut Dome, event: HubEvent) -> Result<(), HubEvent> {
    match event {
        HubEvent::WindowsDiscovered(windows) => dome.windows_discovered(windows),
        HubEvent::WindowGone(key) => dome.window_gone(key),
        HubEvent::WindowMinimized(key) => dome.window_minimized(key),
        HubEvent::WindowRestored(key) => dome.window_restored(key),
        HubEvent::FocusReported(key) => dome.focus_reported(key),
        HubEvent::TitleChanged { key, title } => dome.title_changed(key, title),
        HubEvent::SizeConstraint { key, limits } => dome.size_constraint(key, limits),
        HubEvent::FloatMoved { key, frame } => dome.float_moved(key, frame),
        HubEvent::FullscreenChanged {
            key,
            fullscreen,
            frame,
        } => dome.fullscreen_changed(key, fullscreen, frame),
        other => return Err(other),
    }
    dome.flush_layout();
    Ok(())
}

fn handle_event(runner: &mut DomeRunner, event: HubEvent) {
    let Err(event) = apply_report(&mut runner.dome, event) else {
        return;
    };
    match event {
        HubEvent::Shutdown => {
            tracing::info!("Shutdown requested");
            runner.signal.stop();
        }
        HubEvent::RunBinding { keymap, keystroke } => {
            runner.dome.run_binding(&keymap, &keystroke, &runner.signal);
        }
        HubEvent::ReloadConfig => {
            if let Some(config) = runner.dome.reload() {
                runner.logger.set_level(config.log_level);
                login_item::sync_login_item(config.start_at_login, runner.bundle_path.as_deref());
            }
        }
        HubEvent::ApplyLayout(path) => runner.dome.apply_layout_file(&path),
        HubEvent::Action(actions) => {
            tracing::debug!(%actions, "Executing actions");
            runner.dome.handle_actions(&actions, &runner.signal);
        }
        HubEvent::Query { query, sender } => {
            let json = match query {
                crate::action::Query::Workspaces => runner.dome.query_workspaces_json(),
                crate::action::Query::MinimizedWindows => {
                    runner.dome.query_minimized_windows_json()
                }
                crate::action::Query::Monitors => runner.dome.query_monitors_json(),
            };
            if sender.send(json).is_err() {
                tracing::debug!("Query response dropped, receiver gone");
            }
        }
        HubEvent::MonitorsChanged(monitors) => runner.dome.monitors_changed(monitors),
        HubEvent::MirrorClicked(key) => runner.dome.mirror_clicked(key),
        HubEvent::TabClicked(container, index) => runner.dome.tab_clicked(container, index),
        HubEvent::SaveLayout(path) => runner.dome.save_layout_file(std::path::Path::new(&path)),
        HubEvent::WindowsDiscovered(_)
        | HubEvent::WindowGone(_)
        | HubEvent::WindowMinimized(_)
        | HubEvent::WindowRestored(_)
        | HubEvent::FocusReported(_)
        | HubEvent::TitleChanged { .. }
        | HubEvent::SizeConstraint { .. }
        | HubEvent::FloatMoved { .. }
        | HubEvent::FullscreenChanged { .. } => unreachable!("semantic reports handled above"),
    }
}

static SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);

fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as *const () as usize);
        libc::signal(libc::SIGTERM, signal_handler as *const () as usize);
        libc::signal(libc::SIGHUP, signal_handler as *const () as usize);
    }
}

extern "C" fn signal_handler(_signal: libc::c_int) {
    SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    struct RecordingSender(mpsc::Sender<HubMessage>);

    impl SceneSender for RecordingSender {
        fn send(&self, message: HubMessage) {
            self.0.send(message).ok();
        }
    }

    #[test]
    fn drop_domain_exit_guard_on_return_and_unwind_sends_shutdown() {
        for unwind in [false, true] {
            let (sender, receiver) = mpsc::channel();
            let outcome = std::panic::catch_unwind(move || {
                let _guard = DomainExitGuard::new(Box::new(RecordingSender(sender)));
                if unwind {
                    panic!("domain setup failed after sender handshake");
                }
            });
            assert_eq!(outcome.is_err(), unwind);
            assert!(matches!(receiver.try_recv(), Ok(HubMessage::Shutdown)));
            assert!(matches!(
                receiver.try_recv(),
                Err(mpsc::TryRecvError::Disconnected)
            ));
        }
    }
}
