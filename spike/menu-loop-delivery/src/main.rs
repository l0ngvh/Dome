//! Counts which deliveries reach the UI thread while `TrackPopupMenu` runs its modal loop.
//! README.md describes the run and the result the self-drawn-borders design expects.

#[cfg(not(windows))]
fn main() {
    eprintln!("menu-loop-delivery-spike runs only on Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    std::process::exit(probe::run());
}

#[cfg(windows)]
mod probe {
    use std::ffi::c_void;
    use std::sync::atomic::Ordering::Relaxed;
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32};
    use std::thread;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, EndPaint, PAINTSTRUCT, RDW_INTERNALPAINT, RDW_INVALIDATE, RedrawWindow,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
        DispatchMessageW, EnumThreadWindows, GetClassNameW, GetCursorPos, GetMessageW,
        HWND_MESSAGE, IsWindowVisible, KillTimer, MF_STRING, MSG, PostMessageW, PostThreadMessageW,
        RegisterClassW, SMTO_ABORTIFHUNG, SW_SHOWNOACTIVATE, SendMessageTimeoutW,
        SetForegroundWindow, SetTimer, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CANCELMODE, WM_KEYDOWN,
        WM_NULL, WM_PAINT, WM_QUIT, WM_TIMER, WNDCLASSW, WNDPROC, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };
    use windows::core::{BOOL, PCWSTR, w};

    const WM_THREAD_PING: u32 = WM_APP + 1;
    const WM_WINDOW_PING: u32 = WM_APP + 2;
    const WM_OPEN_MENU: u32 = WM_APP + 3;
    const WINDOW_TIMER_ID: usize = 1;
    const VK_ESCAPE: usize = 0x1B;

    const TICK: Duration = Duration::from_millis(50);
    const BEFORE_MENU: Duration = Duration::from_secs(1);
    const MENU_OPEN: Duration = Duration::from_secs(2);
    const AFTER_MENU: Duration = Duration::from_millis(500);
    const ESCAPE_AFTER: Duration = Duration::from_secs(4);
    const GIVE_UP_AFTER: Duration = Duration::from_secs(7);

    const BEFORE: u8 = 0;
    const IN_MENU: u8 = 1;
    const AFTER: u8 = 2;

    static PHASE: AtomicU8 = AtomicU8::new(BEFORE);
    static MENU_OPEN_MS: AtomicU32 = AtomicU32::new(0);
    static MENU_WINDOW_SEEN: AtomicBool = AtomicBool::new(false);
    static RECEIVED: [[AtomicU32; 3]; 6] = [const { [const { AtomicU32::new(0) }; 3] }; 6];
    static THREAD_PINGS_POSTED_IN_MENU: AtomicU32 = AtomicU32::new(0);
    static THREAD_PINGS_DELIVERED_LATE: AtomicU32 = AtomicU32::new(0);

    #[derive(Clone, Copy)]
    enum Delivery {
        ThreadTimer,
        WindowMessage,
        Paint,
        WindowTimer,
        ThreadMessage,
        InvalidatedPaint,
    }

    struct Row {
        delivery: Delivery,
        label: &'static str,
        in_design: bool,
    }

    const ROWS: [Row; 5] = [
        Row {
            delivery: Delivery::ThreadTimer,
            label: "Thread timer with a TIMERPROC (design deadline)",
            in_design: true,
        },
        Row {
            delivery: Delivery::WindowMessage,
            label: "Message to a message-only window (design wake)",
            in_design: true,
        },
        Row {
            delivery: Delivery::Paint,
            label: "WM_PAINT from RDW_INTERNALPAINT (frame callback)",
            in_design: true,
        },
        Row {
            delivery: Delivery::WindowTimer,
            label: "Timer on the message-only window (comparison)",
            in_design: false,
        },
        Row {
            delivery: Delivery::InvalidatedPaint,
            label: "WM_PAINT from RDW_INVALIDATE (candidate fix)",
            in_design: false,
        },
    ];

    struct Targets {
        ui_thread: u32,
        owner: isize,
        wake: isize,
        paint: isize,
        invalidated_paint: isize,
    }

    pub fn run() -> i32 {
        if let Err(error) = pump() {
            eprintln!("{error}");
            return 2;
        }
        report()
    }

    fn pump() -> Result<(), String> {
        let instance: HINSTANCE = unsafe { GetModuleHandleW(None) }
            .map_err(|error| format!("GetModuleHandleW failed: {error}"))?
            .into();
        let owner = create_window(
            instance,
            w!("MenuProbeOwner"),
            Some(owner_procedure),
            WINDOW_EX_STYLE(0),
            None,
            0,
            0,
        )?;
        let wake = create_window(
            instance,
            w!("MenuProbeWake"),
            Some(wake_procedure),
            WINDOW_EX_STYLE(0),
            Some(HWND_MESSAGE),
            0,
            0,
        )?;
        let paint = create_window(
            instance,
            w!("MenuProbePaint"),
            Some(paint_procedure),
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            None,
            0,
            16,
        )?;
        let invalidated_paint = create_window(
            instance,
            w!("MenuProbeInvalidatedPaint"),
            Some(invalidated_paint_procedure),
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            None,
            24,
            16,
        )?;
        unsafe {
            let _ = ShowWindow(paint, SW_SHOWNOACTIVATE);
            let _ = ShowWindow(invalidated_paint, SW_SHOWNOACTIVATE);
        }

        let tick_ms = TICK.as_millis() as u32;
        let thread_timer = unsafe { SetTimer(None, 0, tick_ms, Some(thread_timer_procedure)) };
        let window_timer = unsafe { SetTimer(Some(wake), WINDOW_TIMER_ID, tick_ms, None) };
        if thread_timer == 0 || window_timer == 0 {
            return Err(format!(
                "SetTimer failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let targets = Targets {
            ui_thread: unsafe { GetCurrentThreadId() },
            owner: owner.0 as isize,
            wake: wake.0 as isize,
            paint: paint.0 as isize,
            invalidated_paint: invalidated_paint.0 as isize,
        };
        let driver = thread::spawn(move || drive(&targets));

        let mut message = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            if result == 0 || result == -1 {
                break;
            }
            // A thread message names no window, so this loop is the only place it can arrive.
            if message.hwnd.0.is_null() && message.message == WM_THREAD_PING {
                count_thread_ping(message.wParam);
                continue;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        unsafe {
            let _ = KillTimer(None, thread_timer);
            let _ = KillTimer(Some(wake), WINDOW_TIMER_ID);
        }
        let _ = driver.join();
        Ok(())
    }

    fn create_window(
        instance: HINSTANCE,
        class: PCWSTR,
        procedure: WNDPROC,
        ex_style: WINDOW_EX_STYLE,
        parent: Option<HWND>,
        x: i32,
        size: i32,
    ) -> Result<HWND, String> {
        let class_info = WNDCLASSW {
            lpfnWndProc: procedure,
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class_info) } == 0 {
            return Err(format!(
                "RegisterClassW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        unsafe {
            CreateWindowExW(
                ex_style,
                class,
                w!(""),
                WS_POPUP,
                x,
                0,
                size,
                size,
                parent,
                None,
                Some(instance),
                None,
            )
        }
        .map_err(|error| format!("CreateWindowExW failed: {error}"))
    }

    fn count(delivery: Delivery) {
        RECEIVED[delivery as usize][PHASE.load(Relaxed) as usize].fetch_add(1, Relaxed);
    }

    fn received(delivery: Delivery, phase: u8) -> u32 {
        RECEIVED[delivery as usize][phase as usize].load(Relaxed)
    }

    fn count_thread_ping(sent_phase: WPARAM) {
        count(Delivery::ThreadMessage);
        if sent_phase.0 == IN_MENU as usize && PHASE.load(Relaxed) == AFTER {
            THREAD_PINGS_DELIVERED_LATE.fetch_add(1, Relaxed);
        }
    }

    unsafe extern "system" fn owner_procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_OPEN_MENU {
            open_menu(window);
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    unsafe extern "system" fn wake_procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_WINDOW_PING => count(Delivery::WindowMessage),
            WM_TIMER if wparam.0 == WINDOW_TIMER_ID => count(Delivery::WindowTimer),
            _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
        LRESULT(0)
    }

    unsafe extern "system" fn paint_procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message != WM_PAINT {
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        paint_and_count(window, Delivery::Paint)
    }

    unsafe extern "system" fn invalidated_paint_procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message != WM_PAINT {
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        paint_and_count(window, Delivery::InvalidatedPaint)
    }

    fn paint_and_count(window: HWND, delivery: Delivery) -> LRESULT {
        let mut paint = PAINTSTRUCT::default();
        unsafe {
            BeginPaint(window, &mut paint);
            let _ = EndPaint(window, &paint);
        }
        count(delivery);
        LRESULT(0)
    }

    unsafe extern "system" fn thread_timer_procedure(_: HWND, _: u32, _: usize, _: u32) {
        count(Delivery::ThreadTimer);
    }

    /// Holds `PHASE` at `IN_MENU` for exactly the span of the `TrackPopupMenu` call, so a count
    /// taken in that phase ran inside the menu's modal loop.
    fn open_menu(owner: HWND) {
        let menu = match unsafe { CreatePopupMenu() } {
            Ok(menu) => menu,
            Err(error) => {
                eprintln!("CreatePopupMenu failed: {error}");
                PHASE.store(AFTER, Relaxed);
                return;
            }
        };
        unsafe {
            let _ = AppendMenuW(menu, MF_STRING, 1, w!("Probe item"));
            let _ = AppendMenuW(menu, MF_STRING, 2, w!("Another probe item"));
        }
        if !unsafe { SetForegroundWindow(owner) }.as_bool() {
            eprintln!("SetForegroundWindow failed, so the menu may not close by itself.");
        }
        let mut point = POINT::default();
        let _ = unsafe { GetCursorPos(&mut point) };

        PHASE.store(IN_MENU, Relaxed);
        let opened = Instant::now();
        let _ = unsafe {
            TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                None,
                owner,
                None,
            )
        };
        MENU_OPEN_MS.store(opened.elapsed().as_millis() as u32, Relaxed);
        PHASE.store(AFTER, Relaxed);

        unsafe {
            let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
        }
    }

    fn window(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    fn drive(targets: &Targets) {
        let started = Instant::now();
        let mut menu_requested = false;
        let mut menu_opened_at: Option<Instant> = None;
        let mut after_started_at: Option<Instant> = None;
        let mut cancel_sent = false;
        let mut escape_sent = false;
        loop {
            let phase = PHASE.load(Relaxed);
            post_pings(targets, phase);
            match phase {
                BEFORE => {
                    if !menu_requested && started.elapsed() >= BEFORE_MENU {
                        let _ = unsafe {
                            PostMessageW(
                                Some(window(targets.owner)),
                                WM_OPEN_MENU,
                                WPARAM(0),
                                LPARAM(0),
                            )
                        };
                        menu_requested = true;
                    }
                }
                IN_MENU => {
                    let open_for = menu_opened_at.get_or_insert_with(Instant::now).elapsed();
                    let menu = menu_window(targets.ui_thread);
                    if menu.is_some() {
                        MENU_WINDOW_SEEN.store(true, Relaxed);
                    }
                    if open_for >= MENU_OPEN && !cancel_sent {
                        // DefWindowProc ends menu mode on WM_CANCELMODE, and a send runs it on the
                        // menu's own thread.
                        let _ = unsafe {
                            SendMessageTimeoutW(
                                window(targets.owner),
                                WM_CANCELMODE,
                                WPARAM(0),
                                LPARAM(0),
                                SMTO_ABORTIFHUNG,
                                1000,
                                None,
                            )
                        };
                        cancel_sent = true;
                    }
                    if open_for >= ESCAPE_AFTER && !escape_sent {
                        if let Some(menu) = menu {
                            let _ = unsafe {
                                PostMessageW(Some(menu), WM_KEYDOWN, WPARAM(VK_ESCAPE), LPARAM(0))
                            };
                        }
                        escape_sent = true;
                    }
                    if open_for >= GIVE_UP_AFTER {
                        eprintln!("The menu did not close, so the probe gave up.");
                        std::process::exit(3);
                    }
                }
                _ => {
                    if after_started_at.get_or_insert_with(Instant::now).elapsed() >= AFTER_MENU {
                        let _ = unsafe {
                            PostThreadMessageW(targets.ui_thread, WM_QUIT, WPARAM(0), LPARAM(0))
                        };
                        return;
                    }
                }
            }
            thread::sleep(TICK);
        }
    }

    fn post_pings(targets: &Targets, phase: u8) {
        let sent_phase = WPARAM(phase as usize);
        unsafe {
            let _ = PostThreadMessageW(targets.ui_thread, WM_THREAD_PING, sent_phase, LPARAM(0));
            let _ = PostMessageW(
                Some(window(targets.wake)),
                WM_WINDOW_PING,
                sent_phase,
                LPARAM(0),
            );
            let _ = RedrawWindow(Some(window(targets.paint)), None, None, RDW_INTERNALPAINT);
            let _ = RedrawWindow(
                Some(window(targets.invalidated_paint)),
                None,
                None,
                RDW_INVALIDATE,
            );
        }
        if phase == IN_MENU {
            THREAD_PINGS_POSTED_IN_MENU.fetch_add(1, Relaxed);
        }
    }

    fn menu_window(thread: u32) -> Option<HWND> {
        let mut found: isize = 0;
        let _ = unsafe {
            EnumThreadWindows(
                thread,
                Some(match_menu_window),
                LPARAM((&raw mut found) as isize),
            )
        };
        (found != 0).then(|| window(found))
    }

    unsafe extern "system" fn match_menu_window(candidate: HWND, found: LPARAM) -> BOOL {
        let mut class = [0u16; 16];
        let length = unsafe { GetClassNameW(candidate, &mut class) }.max(0) as usize;
        let visible = unsafe { IsWindowVisible(candidate) }.as_bool();
        if visible && String::from_utf16_lossy(&class[..length]) == "#32768" {
            unsafe { *(found.0 as *mut isize) = candidate.0 as isize };
            return BOOL(0);
        }
        BOOL(1)
    }

    fn report() -> i32 {
        let open_ms = MENU_OPEN_MS.load(Relaxed);
        let seen = if MENU_WINDOW_SEEN.load(Relaxed) {
            "was"
        } else {
            "was not"
        };
        println!("TrackPopupMenu stayed open for {open_ms} ms, and its menu window {seen} seen.");
        println!();
        println!(
            "{:<50} {:>8} {:>8} {:>8}",
            "Delivery", "before", "in menu", "after"
        );
        for row in &ROWS {
            println!(
                "{:<50} {:>8} {:>8} {:>8}",
                row.label,
                received(row.delivery, BEFORE),
                received(row.delivery, IN_MENU),
                received(row.delivery, AFTER),
            );
        }
        let posted = THREAD_PINGS_POSTED_IN_MENU.load(Relaxed);
        let late = THREAD_PINGS_DELIVERED_LATE.load(Relaxed);
        println!();
        println!(
            "Thread messages (today's wake): {posted} posted while the menu was open, {late} delivered after it closed, {} lost.",
            posted.saturating_sub(late),
        );
        println!();

        if open_ms < MENU_OPEN.as_millis() as u32 / 2 {
            println!("Inconclusive: the menu did not stay open, so nothing ran inside its loop.");
            return 2;
        }
        let mut exit_code = 0;
        for row in &ROWS {
            if received(row.delivery, BEFORE) == 0 {
                println!(
                    "Inconclusive: {} never arrived, even before the menu opened.",
                    row.label
                );
                exit_code = 2;
                continue;
            }
            let arrived = received(row.delivery, IN_MENU) > 0;
            let outcome = if arrived {
                "arrives inside the menu loop"
            } else {
                "does not arrive inside the menu loop"
            };
            let verdict = match (row.in_design, arrived) {
                (false, _) => "For comparison",
                (true, true) => "As the design expects",
                (true, false) => {
                    exit_code = exit_code.max(1);
                    "NOT as the design expects"
                }
            };
            println!("{verdict}: {} {outcome}.", row.label);
        }

        if received(Delivery::ThreadMessage, BEFORE) == 0 || posted == 0 {
            println!(
                "Inconclusive: no thread message was counted, so the thread-message check proves nothing."
            );
            return 2;
        }
        if late == 0 {
            println!(
                "As the design expects: a thread message posted while the menu is open is lost."
            );
        } else {
            exit_code = exit_code.max(1);
            println!(
                "NOT as the design expects: {late} of {posted} thread messages waited until the menu closed instead of being lost. A wake sent this way still stalls while the menu is open."
            );
        }
        exit_code
    }
}
