use crate::platform::macos::event_loop::RunLoop;
use crate::platform::macos::{capture::WindowCapture, window_server::NativeWindowServer};
use async_trait::async_trait;
use dispatch2::{DispatchQueue, DispatchRetained};
use dome_core::{Dimension, WindowId};
use futures::StreamExt;
use futures::channel::mpsc;
use futures::future::{AbortHandle, Abortable};
use futures::stream::LocalBoxStream;
use objc2::rc::Retained;
use objc2_io_surface::IOSurface;
use std::future::Future;
use std::rc::Rc;

#[cfg(test)]
mod tests {
    use super::*;
    use dome_core::{FractionalCoordinate, FractionalLength};
    use futures::Stream;
    use futures::channel::{mpsc, oneshot};
    use futures::executor::LocalPool;
    use futures::task::LocalSpawnExt;
    use std::cell::{Cell, RefCell};
    use std::pin::Pin;
    use std::rc::Weak;
    use std::task::{Context, Poll};

    #[derive(Default)]
    struct Gate {
        attempts: Cell<usize>,
        held: RefCell<Option<oneshot::Receiver<anyhow::Result<bool>>>>,
    }

    impl Gate {
        fn hold(&self) -> PendingOperation {
            let (sender, receiver) = oneshot::channel();
            assert!(self.held.borrow_mut().replace(receiver).is_none());
            PendingOperation(sender)
        }

        async fn enter(&self) -> anyhow::Result<bool> {
            self.attempts.set(self.attempts.get() + 1);
            let held = self.held.borrow_mut().take();
            match held {
                Some(receiver) => receiver.await.expect("complete held capture operation"),
                None => Ok(true),
            }
        }
    }

    #[derive(Default)]
    struct SessionState {
        live: Cell<bool>,
        frames_live: Cell<bool>,
        released: Cell<bool>,
        running: Cell<bool>,
        configured: Cell<Option<(Dimension, f64)>>,
        configure: Gate,
        start: Gate,
        stop: Gate,
    }

    struct SimulatedCapture(Rc<SessionState>);

    #[async_trait(?Send)]
    impl Capture for SimulatedCapture {
        fn is_running(&self) -> bool {
            self.0.running.get()
        }

        async fn update_configuration(
            &mut self,
            source: Dimension,
            scale: f64,
        ) -> anyhow::Result<()> {
            self.0.configure.enter().await?;
            self.0.configured.set(Some((source, scale)));
            Ok(())
        }

        async fn start(&mut self) -> anyhow::Result<()> {
            self.0.start.enter().await?;
            self.0.running.set(true);
            Ok(())
        }

        async fn stop(&mut self) -> anyhow::Result<()> {
            self.0.stop.enter().await?;
            self.0.running.set(false);
            Ok(())
        }
    }

    impl Drop for SimulatedCapture {
        fn drop(&mut self) {
            self.0.live.set(false);
            self.0.released.set(true);
        }
    }

    struct Frames {
        source: mpsc::UnboundedReceiver<Retained<IOSurface>>,
        state: Rc<SessionState>,
    }

    impl Stream for Frames {
        type Item = Retained<IOSurface>;

        fn poll_next(
            mut self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            Pin::new(&mut self.source).poll_next(context)
        }
    }

    impl Drop for Frames {
        fn drop(&mut self) {
            self.state.frames_live.set(false);
        }
    }

    #[derive(Default)]
    struct Captures {
        create: Gate,
        state: Rc<SessionState>,
        source: RefCell<Option<mpsc::UnboundedSender<Retained<IOSurface>>>>,
    }

    #[async_trait(?Send)]
    impl CreateCapture for Captures {
        async fn create(&self, _: WindowId) -> anyhow::Result<Option<CaptureSession>> {
            if !self.create.enter().await? {
                return Ok(None);
            }
            let (sender, source) = mpsc::unbounded();
            self.source.replace(Some(sender));
            self.state.live.set(true);
            self.state.frames_live.set(true);
            Ok(Some(CaptureSession {
                capture: Box::new(SimulatedCapture(self.state.clone())),
                frames: Frames {
                    source,
                    state: self.state.clone(),
                }
                .boxed_local(),
            }))
        }
    }

    struct Surface;

    impl CaptureSurface for Surface {
        fn set_contents(&self, _: &IOSurface) {
            panic!("these lifecycle tests never produce native frames");
        }

        fn set_scale(&self, _: f64) {
            panic!("capture drivers do not configure surface scale");
        }
    }

    fn settings(scale: f64) -> CaptureSettings {
        CaptureSettings {
            source: Dimension::new(
                FractionalCoordinate::new(7.0),
                FractionalCoordinate::new(3.0),
                FractionalLength::new(40.0),
                FractionalLength::new(20.0),
            ),
            scale,
        }
    }

    #[derive(Clone, Copy)]
    enum Phase {
        Create,
        Configure,
        Start,
        Stop,
    }

    struct PendingOperation(oneshot::Sender<anyhow::Result<bool>>);

    impl PendingOperation {
        fn succeed(self, fixture: &mut CaptureFixture) {
            self.complete(fixture, Ok(true));
        }

        fn fail(self, fixture: &mut CaptureFixture) {
            self.complete(fixture, Err(anyhow::anyhow!("capture operation failed")));
        }

        fn missing(self, fixture: &mut CaptureFixture) {
            self.complete(fixture, Ok(false));
        }

        fn complete(self, fixture: &mut CaptureFixture, result: anyhow::Result<bool>) {
            self.0.send(result).unwrap();
            fixture.pool.run_until_stalled();
        }
    }

    struct CaptureFixture {
        pool: LocalPool,
        captures: Rc<Captures>,
        stream: Option<CaptureStream>,
        completed: Rc<Cell<bool>>,
        surface: Weak<Surface>,
    }

    impl CaptureFixture {
        fn new() -> Self {
            let mut fixture = Self::prepare();
            fixture.update(settings(1.0));
            fixture
        }

        fn pending(phase: Phase) -> (Self, PendingOperation) {
            let mut fixture = Self::prepare();
            let pending = fixture.hold(phase);
            fixture.update(settings(1.0));
            (fixture, pending)
        }

        fn prepare() -> Self {
            let pool = LocalPool::new();
            let captures = Rc::new(Captures::default());
            let surface = Rc::new(Surface);
            let weak_surface = Rc::downgrade(&surface);
            let (stream, driver) =
                CaptureStream::prepare(WindowId::new(1), surface, settings(1.0), captures.clone());
            let completed = Rc::new(Cell::new(false));
            let finished = completed.clone();
            pool.spawner()
                .spawn_local(async move {
                    driver.await;
                    finished.set(true);
                })
                .unwrap();
            Self {
                pool,
                captures,
                stream: Some(stream),
                completed,
                surface: weak_surface,
            }
        }

        fn hold(&self, phase: Phase) -> PendingOperation {
            match phase {
                Phase::Create => self.captures.create.hold(),
                Phase::Configure => self.state().configure.hold(),
                Phase::Start => self.state().start.hold(),
                Phase::Stop => self.state().stop.hold(),
            }
        }

        fn state(&self) -> &SessionState {
            &self.captures.state
        }

        fn update(&mut self, settings: CaptureSettings) {
            self.stream.as_ref().unwrap().update(settings);
            self.pool.run_until_stalled();
        }

        fn retire(&mut self) {
            drop(self.stream.take());
            self.pool.run_until_stalled();
        }

        fn close_source(&mut self) {
            drop(self.captures.source.borrow_mut().take());
            self.pool.run_until_stalled();
        }
    }

    #[test]
    fn create_stream_without_further_publication_configures_and_starts_capture() {
        let mut fixture = CaptureFixture::prepare();
        fixture.pool.run_until_stalled();
        assert_eq!(
            fixture.state().configured.get(),
            Some((settings(1.0).source, 1.0))
        );
        assert!(fixture.state().running.get());
        fixture.retire();
        assert!(fixture.completed.get());
        assert!(!fixture.state().live.get());
    }

    #[test]
    fn update_pending_configuration_applies_latest_target_before_start() {
        let (mut fixture, configuring) = CaptureFixture::pending(Phase::Configure);
        fixture.update(settings(2.0));
        let mut latest = settings(3.0);
        latest.source.width = FractionalLength::new(60.0);
        fixture.update(latest);
        assert_eq!(fixture.state().configured.get(), None);
        assert!(!fixture.state().running.get());
        configuring.succeed(&mut fixture);
        assert_eq!(fixture.state().configured.get(), Some((latest.source, 3.0)));
        assert!(fixture.state().running.get());
        assert_eq!(fixture.state().configure.attempts.get(), 2);
        assert_eq!(fixture.captures.create.attempts.get(), 1);
        fixture.update(latest);
        assert_eq!(fixture.state().configure.attempts.get(), 2);
        assert_eq!(fixture.state().start.attempts.get(), 1);
        fixture.retire();
        assert!(fixture.completed.get());
        assert!(!fixture.state().live.get());
    }

    #[test]
    fn update_pending_creation_or_start_applies_latest_target() {
        for phase in [Phase::Create, Phase::Start] {
            let (mut fixture, pending) = CaptureFixture::pending(phase);
            fixture.update(settings(2.0));
            fixture.update(settings(3.0));
            pending.succeed(&mut fixture);
            assert_eq!(
                fixture.state().configured.get(),
                Some((settings(3.0).source, 3.0))
            );
            assert!(fixture.state().running.get());
            assert_eq!(fixture.captures.create.attempts.get(), 1);
            assert_eq!(fixture.state().start.attempts.get(), 1);
            fixture.retire();
            assert!(fixture.completed.get());
        }
    }

    #[test]
    fn update_during_failed_operation_retries_once_without_reusing_consumed_change() {
        for (phase, missing) in [
            (Phase::Create, false),
            (Phase::Create, true),
            (Phase::Configure, false),
            (Phase::Start, false),
        ] {
            let (mut fixture, pending) = CaptureFixture::pending(phase);
            fixture.update(settings(2.0));
            fixture.update(settings(3.0));
            let retry = fixture.hold(phase);
            if missing {
                pending.missing(&mut fixture);
            } else {
                pending.fail(&mut fixture);
            }
            let attempts = |fixture: &CaptureFixture| match phase {
                Phase::Create => fixture.captures.create.attempts.get(),
                Phase::Configure => fixture.state().configure.attempts.get(),
                Phase::Start => fixture.state().start.attempts.get(),
                Phase::Stop => unreachable!(),
            };
            assert_eq!(attempts(&fixture), 2);
            retry.fail(&mut fixture);
            assert_eq!(attempts(&fixture), 2);
            assert!(!fixture.state().running.get());
            fixture.update(settings(3.0));
            assert_eq!(attempts(&fixture), 3);
            assert_eq!(
                fixture.state().configured.get(),
                Some((settings(3.0).source, 3.0))
            );
            assert!(fixture.state().running.get());
            fixture.retire();
            assert!(fixture.completed.get());
        }
    }

    #[test]
    fn retire_pending_configuration_or_start_aborts_frames_before_stop_releases_capture() {
        for (phase, failed_operation) in [
            (Phase::Configure, false),
            (Phase::Configure, true),
            (Phase::Start, false),
            (Phase::Start, true),
        ] {
            let (mut fixture, pending) = CaptureFixture::pending(phase);
            let stopping = fixture.hold(Phase::Stop);
            assert!(fixture.state().frames_live.get());
            fixture.retire();
            assert!(!fixture.state().frames_live.get());
            assert!(fixture.surface.upgrade().is_none());
            assert!(fixture.state().live.get());
            assert!(!fixture.completed.get());
            if failed_operation {
                pending.fail(&mut fixture);
            } else {
                pending.succeed(&mut fixture);
            }
            assert_eq!(fixture.state().stop.attempts.get(), 1);
            assert!(fixture.state().live.get());
            if matches!(phase, Phase::Configure) {
                assert_eq!(fixture.state().start.attempts.get(), 0);
            }
            stopping.succeed(&mut fixture);
            assert!(!fixture.state().running.get());
            assert!(!fixture.state().live.get());
            assert!(fixture.state().released.get());
            assert!(fixture.completed.get());
        }
    }

    #[test]
    fn retry_failed_creation_waits_for_new_target_change() {
        for missing in [true, false] {
            let (mut fixture, pending) = CaptureFixture::pending(Phase::Create);
            if missing {
                pending.missing(&mut fixture);
            } else {
                pending.fail(&mut fixture);
            }
            assert!(!fixture.state().live.get());
            assert_eq!(fixture.captures.create.attempts.get(), 1);
            fixture.update(settings(2.0));
            assert!(fixture.state().running.get());
            assert_eq!(
                fixture.state().configured.get(),
                Some((settings(2.0).source, 2.0))
            );
            assert_eq!(fixture.captures.create.attempts.get(), 2);
            fixture.retire();
            assert!(fixture.completed.get());
        }
    }

    #[test]
    fn retry_failed_configuration_or_start_keeps_live_session_and_successful_configuration() {
        for starting in [false, true] {
            let phase = if starting {
                Phase::Start
            } else {
                Phase::Configure
            };
            let (mut fixture, pending) = CaptureFixture::pending(phase);
            pending.fail(&mut fixture);
            assert!(fixture.state().live.get());
            assert!(!fixture.state().running.get());
            assert_eq!(fixture.state().configure.attempts.get(), 1);
            assert_eq!(fixture.state().start.attempts.get(), usize::from(starting));
            assert_eq!(
                fixture.state().configured.get(),
                starting.then_some((settings(1.0).source, 1.0))
            );
            fixture.update(settings(1.0));
            assert!(fixture.state().running.get());
            assert_eq!(fixture.captures.create.attempts.get(), 1);
            assert_eq!(
                fixture.state().configure.attempts.get(),
                if starting { 1 } else { 2 }
            );
            assert_eq!(
                fixture.state().start.attempts.get(),
                if starting { 2 } else { 1 }
            );
            fixture.retire();
            assert!(fixture.completed.get());
        }
    }

    #[test]
    fn retire_pending_creation_releases_late_session_without_stopping_replacement() {
        let (mut retired, creating) = CaptureFixture::pending(Phase::Create);
        retired.retire();
        let mut replacement = CaptureFixture::new();
        assert!(replacement.state().running.get());
        creating.succeed(&mut retired);
        assert!(retired.completed.get());
        assert!(retired.state().released.get());
        assert!(!retired.state().live.get());
        assert!(!retired.state().frames_live.get());
        assert!(retired.surface.upgrade().is_none());
        assert_eq!(retired.state().configured.get(), None);
        assert_eq!(retired.state().start.attempts.get(), 0);
        assert_eq!(retired.state().stop.attempts.get(), 0);
        assert!(replacement.state().running.get());
        assert_eq!(replacement.state().stop.attempts.get(), 0);
        replacement.retire();
        assert!(replacement.completed.get());
        assert!(!replacement.state().live.get());
    }

    #[test]
    fn retire_pending_failed_creation_finishes_without_retry() {
        for missing in [false, true] {
            let (mut fixture, pending) = CaptureFixture::pending(Phase::Create);
            fixture.retire();
            if missing {
                pending.missing(&mut fixture);
            } else {
                pending.fail(&mut fixture);
            }
            assert!(fixture.completed.get());
            assert!(fixture.surface.upgrade().is_none());
            assert_eq!(fixture.captures.create.attempts.get(), 1);
            assert!(!fixture.state().live.get());
        }
    }

    #[test]
    fn retire_before_driver_runs_never_creates_capture() {
        let mut fixture = CaptureFixture::prepare();
        fixture.retire();
        assert!(fixture.completed.get());
        assert!(fixture.surface.upgrade().is_none());
        assert_eq!(fixture.captures.create.attempts.get(), 0);
    }

    #[test]
    fn close_frame_source_releases_consumer_without_retiring_capture() {
        let mut fixture = CaptureFixture::new();
        fixture.close_source();
        assert!(!fixture.state().frames_live.get());
        assert!(fixture.surface.upgrade().is_none());
        assert!(fixture.state().live.get());
        assert!(fixture.state().running.get());
        assert!(!fixture.completed.get());
        fixture.update(settings(2.0));
        assert_eq!(
            fixture.state().configured.get(),
            Some((settings(2.0).source, 2.0))
        );
        fixture.retire();
        assert!(fixture.completed.get());
        assert!(!fixture.state().live.get());
    }

    #[test]
    fn retire_capture_when_stop_fails_releases_session() {
        let mut fixture = CaptureFixture::new();
        let stopping = fixture.hold(Phase::Stop);
        fixture.retire();
        assert!(fixture.state().live.get());
        assert!(!fixture.state().frames_live.get());
        stopping.fail(&mut fixture);
        assert!(fixture.completed.get());
        assert!(fixture.state().released.get());
        assert!(!fixture.state().live.get());
        assert_eq!(fixture.state().stop.attempts.get(), 1);
    }
}

pub(in crate::platform::macos) trait CaptureSurface {
    fn set_contents(&self, surface: &IOSurface);
    fn set_scale(&self, scale: f64);
}

pub(in crate::platform::macos) struct CaptureSession {
    pub capture: Box<dyn Capture>,
    pub frames: LocalBoxStream<'static, Retained<IOSurface>>,
}

#[async_trait(?Send)]
pub(in crate::platform::macos) trait CreateCapture {
    async fn create(&self, window_id: WindowId) -> anyhow::Result<Option<CaptureSession>>;
}

pub(in crate::platform::macos) struct NativeCaptures {
    pub server: Rc<NativeWindowServer>,
    pub queue: DispatchRetained<DispatchQueue>,
}

#[async_trait(?Send)]
impl CreateCapture for NativeCaptures {
    async fn create(&self, window_id: WindowId) -> anyhow::Result<Option<CaptureSession>> {
        Ok(self
            .server
            .create_capture(window_id, &self.queue)
            .await?
            .map(|session| CaptureSession {
                capture: Box::new(session.capture),
                frames: session.frames.boxed_local(),
            }))
    }
}

#[derive(Clone, Copy)]
pub(super) struct CaptureSettings {
    pub source: Dimension,
    pub scale: f64,
}

pub(super) struct CaptureStream {
    updates: mpsc::UnboundedSender<Option<CaptureSettings>>,
    frames: AbortHandle,
}

impl CaptureStream {
    pub(super) fn new(
        window_id: WindowId,
        surface: Rc<dyn CaptureSurface>,
        settings: CaptureSettings,
        captures: Rc<dyn CreateCapture>,
    ) -> Self {
        let (stream, driver) = Self::prepare(window_id, surface, settings, captures);
        RunLoop::current().spawn(driver);
        stream
    }

    pub(super) fn update(&self, settings: CaptureSettings) {
        if self.updates.unbounded_send(Some(settings)).is_err() {
            tracing::debug!("Capture driver closed, settings dropped");
        }
    }

    fn prepare(
        window_id: WindowId,
        surface: Rc<dyn CaptureSurface>,
        settings: CaptureSettings,
        captures: Rc<dyn CreateCapture>,
    ) -> (Self, impl Future<Output = ()> + 'static) {
        let (updates, mut receiver) = mpsc::unbounded();
        updates
            .unbounded_send(Some(settings))
            .expect("capture receiver just created");
        let (frames, cancellation) = AbortHandle::new_pair();
        let driver = async move {
            let Some((session, settings)) =
                create_capture(window_id, &mut receiver, &*captures).await
            else {
                return;
            };
            let frames = async move {
                let mut source = session.frames;
                Abortable::new(
                    async move {
                        while let Some(frame) = source.next().await {
                            surface.set_contents(&frame);
                        }
                    },
                    cancellation,
                )
                .await
                .unwrap_or(());
            };
            futures::join!(
                frames,
                modify_capture(window_id, session.capture, settings, receiver)
            );
        };
        (Self { updates, frames }, driver)
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        self.frames.abort();
        if self.updates.unbounded_send(None).is_err() {
            tracing::debug!("Capture driver already closed");
        }
    }
}

#[async_trait(?Send)]
pub(in crate::platform::macos) trait Capture {
    fn is_running(&self) -> bool;
    async fn update_configuration(&mut self, source: Dimension, scale: f64) -> anyhow::Result<()>;
    async fn start(&mut self) -> anyhow::Result<()>;
    async fn stop(&mut self) -> anyhow::Result<()>;
}

async fn create_capture(
    window_id: WindowId,
    updates: &mut mpsc::UnboundedReceiver<Option<CaptureSettings>>,
    captures: &dyn CreateCapture,
) -> Option<(CaptureSession, CaptureSettings)> {
    let mut desired = updates.next().await.flatten();
    loop {
        while let Ok(latest) = updates.try_recv() {
            desired = latest;
        }
        desired?;
        match captures.create(window_id).await {
            Ok(Some(capture)) => {
                while let Ok(latest) = updates.try_recv() {
                    desired = latest;
                }
                return Some((capture, desired?));
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%window_id, %error, "capture creation failed"),
        }
        desired = updates.next().await.flatten();
    }
}

#[async_trait(?Send)]
impl Capture for WindowCapture {
    fn is_running(&self) -> bool {
        self.is_running()
    }

    async fn update_configuration(&mut self, source: Dimension, scale: f64) -> anyhow::Result<()> {
        self.update_configuration(source, scale).await
    }

    async fn start(&mut self) -> anyhow::Result<()> {
        self.start().await
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        self.stop().await
    }
}

async fn modify_capture(
    window_id: WindowId,
    mut capture: Box<dyn Capture>,
    settings: CaptureSettings,
    mut updates: mpsc::UnboundedReceiver<Option<CaptureSettings>>,
) {
    let mut desired = Some(settings);
    let mut configured = None;
    loop {
        while let Ok(latest) = updates.try_recv() {
            desired = latest;
        }
        let Some(settings) = desired else {
            break;
        };
        let configuration = (settings.source, settings.scale);
        if configured != Some(configuration) {
            match capture
                .update_configuration(settings.source, settings.scale)
                .await
            {
                Ok(()) => {
                    configured = Some(configuration);
                    continue;
                }
                Err(error) => tracing::warn!(%window_id, %error, "capture configuration failed"),
            }
        } else if !capture.is_running() {
            match capture.start().await {
                Ok(()) => continue,
                Err(error) => tracing::warn!(%window_id, %error, "capture start failed"),
            }
        }
        desired = updates.next().await.flatten();
    }
    if let Err(error) = capture.stop().await {
        tracing::warn!(%window_id, %error, "capture stop failed");
    }
}
