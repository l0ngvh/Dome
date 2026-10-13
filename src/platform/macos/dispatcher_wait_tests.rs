use super::super::window_server::schedule::QUIET_INTERVAL;

#[derive(Debug, PartialEq)]
struct WaitRequest {
    deadline: Option<Instant>,
    disconnected: bool,
}

enum WaitReply {
    TimeoutAt(Instant),
    ReceiveAt(Instant),
    Native,
}

pub(super) struct WaitControl {
    pub(super) now: Instant,
    requests: mpsc::Sender<WaitRequest>,
    replies: mpsc::Receiver<WaitReply>,
}
impl WaitControl {
    pub(super) fn receive(
        &mut self,
        commands: &mpsc::Receiver<ApplicationCommand>,
        deadline: Option<Instant>,
    ) -> Option<Result<ApplicationCommand, mpsc::RecvTimeoutError>> {
        self.requests
            .send(WaitRequest {
                deadline,
                disconnected: false,
            })
            .unwrap();
        match self.replies.recv().unwrap() {
            WaitReply::TimeoutAt(now) => {
                self.now = now;
                Some(Err(mpsc::RecvTimeoutError::Timeout))
            }
            WaitReply::ReceiveAt(now) => {
                self.now = now;
                Some(commands.recv_timeout(Duration::from_secs(3)))
            }
            WaitReply::Native => None,
        }
    }

    pub(super) fn sleep_until(&mut self, deadline: Instant) {
        self.requests
            .send(WaitRequest {
                deadline: Some(deadline),
                disconnected: true,
            })
            .unwrap();
        let WaitReply::TimeoutAt(now) = self.replies.recv().unwrap() else {
            panic!("disconnected workers must sleep, not receive")
        };
        self.now = now;
    }
}

struct WaitDriver {
    requests: mpsc::Receiver<WaitRequest>,
    replies: mpsc::Sender<WaitReply>,
}
impl WaitDriver {
    fn install(fixture: &mut Fixture, now: Instant) -> Self {
        let (requests, receiver) = mpsc::channel();
        let (replies, response) = mpsc::channel();
        fixture.owner.wait = Some(WorkerWait {
            control: Some(WaitControl {
                now,
                requests,
                replies: response,
            }),
        });
        Self {
            requests: receiver,
            replies,
        }
    }

    fn expect(&self, deadline: Option<Instant>, disconnected: bool) {
        assert_eq!(
            self.requests
                .recv_timeout(Duration::from_secs(3))
                .expect("worker reached wait primitive"),
            WaitRequest {
                deadline,
                disconnected
            }
        );
    }

    fn reply(&self, reply: WaitReply) {
        self.replies.send(reply).unwrap();
    }

    fn finish(&self, fixture: &mut Fixture, now: Instant) {
        fixture.owner.terminate();
        self.reply(WaitReply::ReceiveAt(now));
        assert_eq!(fixture.next().kind, "terminate");
        assert!(fixture.calls.try_recv().is_err());
    }
}

fn movement(fixture: &mut Fixture, observed_at: Instant) {
    fixture.owner.send(ApplicationCommand::Event(
        ApplicationEvent::MovedOrResized { observed_at },
    ));
}

#[test]
fn wait_at_quiet_boundary_selects_geometry_once() {
    for after in [Duration::ZERO, Duration::from_nanos(1)] {
        let mut fixture = Fixture::new(None);
        let start = Instant::now();
        let last = start + Duration::from_millis(50);
        let deadline = last + QUIET_INTERVAL;
        let driver = WaitDriver::install(&mut fixture, start);
        fixture.enter_probe();
        movement(&mut fixture, start);
        movement(&mut fixture, last);
        fixture.read(InspectionKind::AppWindows);
        fixture.read(InspectionKind::Space);
        fixture.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
        fixture.release_probe();
        fixture.skip_initial();
        driver.expect(Some(deadline), false);
        assert!(fixture.calls.try_recv().is_err());
        driver.reply(WaitReply::TimeoutAt(deadline - Duration::from_nanos(1)));
        driver.expect(Some(deadline), false);
        assert!(
            fixture.calls.try_recv().is_err(),
            "geometry selected before quiet"
        );
        driver.reply(WaitReply::TimeoutAt(deadline + after));
        assert_eq!(fixture.next().kind, "enumerate");
        assert_eq!(fixture.next().kind, "space");
        let settled = fixture.next();
        assert_eq!(settled.kind, "positions");
        assert_eq!(
            settled.origin,
            Some(PositionReadOrigin::Settle(DebounceBurst {
                first: start,
                last
            }))
        );
        driver.expect(None, false);
        assert!(fixture.calls.try_recv().is_err());
        fixture.read(InspectionKind::Title(7));
        driver.reply(WaitReply::ReceiveAt(deadline + Duration::from_secs(1)));
        let title = fixture.next();
        assert_eq!((title.kind, title.key), ("title", 7));
        driver.expect(None, false);
        assert!(
            fixture.calls.try_recv().is_err(),
            "quiet expiry was consumed twice"
        );
        driver.finish(&mut fixture, deadline + Duration::from_secs(1));
    }
}

#[test]
fn wait_while_hidden_or_suspended_consumes_expiry_without_zero_time_loop() {
    for hidden in [false, true] {
        let mut fixture = Fixture::new(None);
        let start = Instant::now();
        let deadline = start + QUIET_INTERVAL;
        let driver = WaitDriver::install(&mut fixture, start);
        fixture.enter_probe();
        fixture.owner.send(ApplicationCommand::Event(if hidden {
            ApplicationEvent::Hidden
        } else {
            ApplicationEvent::Suspended(true)
        }));
        movement(&mut fixture, start);
        fixture.read(InspectionKind::AppWindows);
        fixture.release_probe();
        driver.expect(Some(deadline), false);
        assert!(fixture.calls.try_recv().is_err());
        driver.reply(WaitReply::TimeoutAt(deadline));
        driver.expect(None, false);
        assert!(fixture.calls.try_recv().is_err());
        fixture.operation(setter(0, 1, 17));
        driver.reply(WaitReply::ReceiveAt(deadline));
        let write = fixture.next();
        assert_eq!((write.kind, write.value), ("frame", 17));
        driver.expect(None, false);
        assert!(fixture.calls.try_recv().is_err());
        fixture.owner.send(ApplicationCommand::Event(if hidden {
            ApplicationEvent::Shown
        } else {
            ApplicationEvent::Suspended(false)
        }));
        driver.reply(WaitReply::ReceiveAt(deadline));
        driver.expect(None, false);
        let calls: Vec<_> = fixture.calls.try_iter().collect();
        assert!(calls.iter().any(|call| call.kind == "enumerate"));
        assert!(calls.iter().any(|call| call.origin
            == Some(PositionReadOrigin::Settle(DebounceBurst {
                first: start,
                last: start,
            }))));
        driver.finish(&mut fixture, deadline);
    }
}

#[test]
fn disconnect_worker_with_quiet_deadline_pending_sleeps_then_drains_buffered_work() {
    let mut fixture = Fixture::new(None);
    let start = Instant::now();
    let deadline = start + QUIET_INTERVAL;
    let driver = WaitDriver::install(&mut fixture, start);
    fixture.enter_probe();
    movement(&mut fixture, start);
    fixture.read(InspectionKind::AppWindows);
    fixture.operation(setter(0, 1, 23));
    fixture.owner.worker = None;
    fixture.release_probe();
    fixture.skip_initial();
    let write = fixture.next();
    assert_eq!((write.kind, write.value), ("frame", 23));
    driver.expect(Some(deadline), true);
    assert!(fixture.calls.try_recv().is_err());
    driver.reply(WaitReply::TimeoutAt(deadline - Duration::from_nanos(1)));
    driver.expect(Some(deadline), true);
    assert!(fixture.calls.try_recv().is_err());
    driver.reply(WaitReply::TimeoutAt(deadline));
    assert_eq!(fixture.next().kind, "enumerate");
    assert_eq!(
        fixture.next().origin,
        Some(PositionReadOrigin::Settle(DebounceBurst {
            first: start,
            last: start,
        }))
    );
    assert_eq!(fixture.next().kind, "terminate");
    assert!(fixture.calls.try_recv().is_err());
}

#[test]
fn replace_periodic_tail_after_delayed_movement_preserves_burst_gap_and_settle_origin() {
    let mut fixture = Fixture::new(Some("title"));
    let start = Instant::now();
    let delivered = start + Duration::from_secs(10);
    let first = start + Duration::from_millis(400);
    let last = start + Duration::from_millis(450);
    let driver = WaitDriver::install(&mut fixture, delivered);
    fixture.enter_probe();
    for observed_at in [start, start + Duration::from_millis(50), first, last] {
        movement(&mut fixture, observed_at);
    }
    fixture.release_probe();
    fixture.skip_initial();
    fixture.operation(setter(0, 1, 10));
    fixture.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    fixture.operation(setter(0, 2, 20));
    fixture.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    fixture.operation(setter(0, 1, 30));
    fixture.release_operation();
    for (key, value) in [(1, 30), (2, 20)] {
        let call = fixture.next();
        assert_eq!((call.kind, call.key, call.value), ("frame", key, value));
    }
    let settled = fixture.next();
    assert_eq!(settled.kind, "positions");
    assert_eq!(
        settled.origin,
        Some(PositionReadOrigin::Settle(DebounceBurst { first, last }))
    );
    driver.expect(None, false);
    assert!(fixture.calls.try_recv().is_err());
    driver.finish(&mut fixture, delivered);
}

#[test]
fn request_app_windows_during_native_read_produces_one_serial_followup() {
    let mut fixture = Fixture::new(Some("enumerate"));
    let now = Instant::now();
    let driver = WaitDriver::install(&mut fixture, now);
    fixture.enter_probe();
    fixture.read(InspectionKind::AppWindows);
    fixture.release_probe();
    fixture.skip_initial();
    let executing = fixture.next();
    assert_eq!(executing.kind, "enumerate");
    fixture.read(InspectionKind::AppWindows);
    fixture.read(InspectionKind::AppWindows);
    fixture.operation(setter(0, 1, 41));
    fixture.read(InspectionKind::AppWindows);
    assert!(fixture.calls.try_recv().is_err());
    fixture.release_operation();
    let write = fixture.next();
    assert_eq!((write.kind, write.value), ("frame", 41));
    let followup = fixture.next();
    assert_eq!(followup.kind, "enumerate");
    assert_eq!(followup.worker, executing.worker);
    driver.expect(None, false);
    assert!(fixture.calls.try_recv().is_err());
    driver.finish(&mut fixture, now);
}

#[test]
fn write_during_native_channel_wait_with_buffered_send_control_wakes_worker() {
    for buffered in [false, true] {
        let mut fixture = Fixture::new(None);
        let start = Instant::now();
        let observed_at = start + Duration::from_secs(30);
        let deadline = observed_at + QUIET_INTERVAL;
        let driver = WaitDriver::install(&mut fixture, start);
        fixture.enter_probe();
        movement(&mut fixture, observed_at);
        fixture.read(InspectionKind::AppWindows);
        fixture.release_probe();
        fixture.skip_initial();
        driver.expect(Some(deadline), false);
        if !buffered {
            driver.reply(WaitReply::Native);
        }
        let sent = Instant::now();
        fixture.operation(setter(0, 1, 31));
        if buffered {
            driver.reply(WaitReply::Native);
        }
        let write = fixture.next();
        assert_eq!((write.kind, write.value), ("frame", 31));
        assert!(sent.elapsed() < Duration::from_secs(3));
        driver.expect(Some(deadline), false);
        assert!(fixture.calls.try_recv().is_err());
        driver.reply(WaitReply::TimeoutAt(deadline));
        assert_eq!(fixture.next().kind, "enumerate");
        assert_eq!(
            fixture.next().origin,
            Some(PositionReadOrigin::Settle(DebounceBurst {
                first: observed_at,
                last: observed_at,
            }))
        );
        driver.expect(None, false);
        driver.finish(&mut fixture, deadline);
    }
}
