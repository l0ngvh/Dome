//! Test-window process for the driver. It reads one command per line on stdin
//! and answers each with one `ok ...` or `err ...` line on stdout. Coordinates
//! are CGWindowList coordinates, origin top-left of the primary display.

use std::collections::HashMap;
use std::io::BufRead;
use std::process::exit;
use std::ptr::NonNull;
use std::sync::mpsc::{Receiver, channel};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSMenu, NSScreen,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString, NSTimer};

const NORMAL_LEVEL: isize = 0;
const FLOATING_LEVEL: isize = 3;

struct Fixture {
    mtm: MainThreadMarker,
    name: String,
    primary_height: f64,
    menu_bar_height: f64,
    primary_width: f64,
    windows: HashMap<String, Retained<NSWindow>>,
    menu: Retained<NSMenu>,
    rx: Receiver<String>,
}

fn main() {
    let mut name = String::from("A");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match (flag.as_str(), args.next()) {
            ("--name", Some(value)) => name = value,
            _ => {
                eprintln!("usage: fixture --name <A|B>");
                exit(2);
            }
        }
    }

    let mtm = MainThreadMarker::new().expect("must run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let screens = NSScreen::screens(mtm);
    let screen = screens.firstObject().expect("at least one screen");
    let frame = screen.frame();
    let visible = screen.visibleFrame();
    let menu_bar_height = frame.size.height - (visible.origin.y + visible.size.height);

    let menu = NSMenu::new(mtm);
    for title in ["Alpha", "Beta", "Gamma", "Delta"] {
        unsafe {
            menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str(title),
                None,
                &NSString::from_str(""),
            );
        }
    }

    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                return;
            }
        }
        // Stdin closes when the driver exits, so the fixture quits rather than
        // leave its test windows on screen.
        let _ = tx.send(String::from("quit"));
    });

    let fixture = std::rc::Rc::new(std::cell::RefCell::new(Fixture {
        mtm,
        name,
        primary_height: frame.size.height,
        menu_bar_height,
        primary_width: frame.size.width,
        windows: HashMap::new(),
        menu,
        rx,
    }));

    let for_timer = fixture.clone();
    let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
        let Ok(mut f) = for_timer.try_borrow_mut() else {
            return;
        };
        while let Ok(line) = f.rx.try_recv() {
            f.run(&line);
        }
    });
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.01, true, &block) };
    // Common modes include the menu-tracking mode, so commands still run while
    // a menu is open.
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };

    println!("ok ready pid={}", std::process::id());
    app.run();
}

impl Fixture {
    fn run(&mut self, line: &str) {
        let words: Vec<&str> = line.split_whitespace().collect();
        if let ["spam", first, second, interval, count] = words.as_slice() {
            if let Err(message) = self.spam(first, second, interval, count) {
                println!("err {message}");
            }
            return;
        }
        match self.dispatch(&words) {
            Ok(reply) => println!("ok {reply}"),
            Err(message) => println!("err {message}"),
        }
    }

    /// Makes the two windows key in turn, one every `interval` ms, `count`
    /// times. The reply comes only after the last one, so the driver knows
    /// when the spam ended.
    fn spam(&self, first: &str, second: &str, interval: &str, count: &str) -> Result<(), String> {
        let windows = [self.window(first)?, self.window(second)?];
        let interval = num(interval)? / 1000.0;
        let count: usize = count.parse().map_err(|_| format!("bad count {count}"))?;
        let reply = format!("ok spam {first} {second} done");
        let done = std::cell::Cell::new(0usize);
        let block = RcBlock::new(move |timer: NonNull<NSTimer>| {
            let i = done.get();
            if i >= count {
                unsafe { timer.as_ref() }.invalidate();
                println!("{reply}");
                return;
            }
            windows[i % 2].makeKeyAndOrderFront(None);
            done.set(i + 1);
        });
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, true, &block) };
        Ok(())
    }

    fn dispatch(&mut self, words: &[&str]) -> Result<String, String> {
        match words {
            ["backdrop"] => {
                let rect = (
                    0.0,
                    self.menu_bar_height,
                    self.primary_width,
                    self.primary_height - self.menu_bar_height,
                );
                self.open("backdrop", rect, (0.5, 0.5, 0.5))
            }
            ["open", label, x, y, w, h, color] => {
                let rect = (num(x)?, num(y)?, num(w)?, num(h)?);
                self.open(label, rect, parse_color(color)?)
            }
            ["move", label, x, y] => {
                let window = self.window(label)?;
                let (_, _, w, h) = self.cg_frame(&window);
                window.setFrame_display(self.ns_rect((num(x)?, num(y)?, w, h)), true);
                Ok(format!("move {label}"))
            }
            ["resize", label, w, h] => {
                let window = self.window(label)?;
                let (x, y, _, _) = self.cg_frame(&window);
                window.setFrame_display(self.ns_rect((x, y, num(w)?, num(h)?)), true);
                Ok(format!("resize {label}"))
            }
            ["frame", label] => {
                let window = self.window(label)?;
                let (x, y, w, h) = self.cg_frame(&window);
                Ok(format!("frame {label} {x} {y} {w} {h}"))
            }
            ["minsize", label, w, h] => {
                self.window(label)?
                    .setContentMinSize(NSSize::new(num(w)?, num(h)?));
                Ok(format!("minsize {label}"))
            }
            ["close", label] => {
                let window = self
                    .windows
                    .remove(*label)
                    .ok_or_else(|| format!("no window {label}"))?;
                window.close();
                Ok(format!("close {label}"))
            }
            ["front", label] => {
                self.window(label)?.orderFront(None);
                Ok(format!("front {label}"))
            }
            ["float", label, state] => {
                let level = match *state {
                    "on" => FLOATING_LEVEL,
                    "off" => NORMAL_LEVEL,
                    _ => return Err(format!("bad float state {state}")),
                };
                self.window(label)?.setLevel(level);
                Ok(format!("float {label} {state}"))
            }
            ["minimize", label] => {
                self.window(label)?.miniaturize(None);
                Ok(format!("minimize {label}"))
            }
            ["restore", label] => {
                self.window(label)?.deminiaturize(None);
                Ok(format!("restore {label}"))
            }
            ["popup", label] => {
                let window = self.window(label)?;
                let content = window.contentView().ok_or("no content view")?;
                let bounds = content.bounds();
                let center = NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
                let menu = self.menu.clone();
                // The pop-up blocks until the menu closes. A one-shot timer runs
                // it after this reply, so the driver is not left waiting.
                let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
                    menu.popUpMenuPositioningItem_atLocation_inView(None, center, Some(&content));
                });
                unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(0.0, false, &block) };
                Ok(format!("popup {label}"))
            }
            ["quit"] => exit(0),
            _ => Err(format!("unknown command {}", words.join(" "))),
        }
    }

    fn open(
        &mut self,
        label: &str,
        rect: (f64, f64, f64, f64),
        (r, g, b): (f64, f64, f64),
    ) -> Result<String, String> {
        if self.windows.contains_key(label) {
            return Err(format!("window {label} exists"));
        }
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(self.mtm),
                self.ns_rect(rect),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // The map owns the window. AppKit must not free it on close as well.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&format!("{}:{label}", self.name)));
        window.setBackgroundColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
            r, g, b, 1.0,
        )));
        // The content rect above sized the content, so set the outer frame.
        window.setFrame_display(self.ns_rect(rect), false);
        if let Some(content) = window.contentView() {
            unsafe { content.setMenu(Some(&self.menu)) };
        }
        window.orderFrontRegardless();
        let number = window.windowNumber();
        self.windows.insert(label.to_string(), window);
        Ok(format!("open {label} {number}"))
    }

    fn window(&self, label: &str) -> Result<Retained<NSWindow>, String> {
        self.windows
            .get(label)
            .cloned()
            .ok_or_else(|| format!("no window {label}"))
    }

    fn ns_rect(&self, (x, y, w, h): (f64, f64, f64, f64)) -> NSRect {
        NSRect::new(NSPoint::new(x, self.primary_height - y - h), NSSize::new(w, h))
    }

    fn cg_frame(&self, window: &NSWindow) -> (f64, f64, f64, f64) {
        let f = window.frame();
        (
            f.origin.x,
            self.primary_height - f.origin.y - f.size.height,
            f.size.width,
            f.size.height,
        )
    }
}

fn num(s: &str) -> Result<f64, String> {
    s.parse().map_err(|_| format!("bad number {s}"))
}

/// `rrggbb` hex.
fn parse_color(s: &str) -> Result<(f64, f64, f64), String> {
    let v = u32::from_str_radix(s, 16).map_err(|_| format!("bad color {s}"))?;
    if s.len() != 6 {
        return Err(format!("bad color {s}"));
    }
    let channel = |shift: u32| f64::from((v >> shift) & 0xff) / 255.0;
    Ok((channel(16), channel(8), channel(0)))
}
