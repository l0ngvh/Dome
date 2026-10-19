#[cfg(target_os = "macos")]
mod lifecycle {
    const CASES: &[&str] = &[
        "wake-delivery",
        "timer-once-cancel",
        "timer-self-cancel",
        "throttle-burst",
        "signal-int",
        "signal-term",
        "signal-hup",
    ];

    pub(super) fn run() {
        let arguments = std::env::args().skip(1).collect::<Vec<_>>();
        if arguments
            .first()
            .is_some_and(|argument| argument == "--child")
        {
            let case = arguments.get(1).expect("child case name");
            dome::run_macos_lifecycle_test(case);
            return;
        }
        if arguments
            .first()
            .is_some_and(|argument| argument == "--list")
        {
            for case in CASES {
                println!("{case}");
            }
            return;
        }
        for case in CASES {
            run_case(case);
        }
    }

    fn run_case(case: &str) {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--child", case])
            .stdin(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "native case {case} failed with {status}");
                println!("native case {case} passed");
                return;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("native case {case} timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    lifecycle::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macos lifecycle tests require macos");
}
