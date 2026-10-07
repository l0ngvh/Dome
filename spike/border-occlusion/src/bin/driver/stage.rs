//! The fixture processes and the test windows they own.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use spike::geometry::Rect;
use spike::now_ms;

use crate::input;
use crate::procs::{Control, Fixture};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Owner {
    A,
    B,
}

#[derive(Clone, Copy, Debug)]
pub struct TestWindow {
    pub owner: Owner,
    pub id: u32,
    pub rect: Rect,
    /// Whether the overlay was told to border it.
    pub managed: bool,
}

pub struct Stage {
    /// Owns only the backdrop. Activating an app can raise all of its windows,
    /// so a backdrop in A or B would cover the other fixture's windows.
    pub backdrop_fixture: Fixture,
    pub a: Fixture,
    pub b: Fixture,
    pub backdrop: Rect,
    pub backdrop_id: u32,
    pub windows: HashMap<String, TestWindow>,
    control: Option<Control>,
}

impl Stage {
    pub fn start(bin_dir: &Path) -> Result<Stage, String> {
        let fixture = bin_dir.join("fixture");
        let mut backdrop_fixture = Fixture::spawn(&fixture, "S")?;
        let a = Fixture::spawn(&fixture, "A")?;
        let b = Fixture::spawn(&fixture, "B")?;
        let backdrop_id = backdrop_fixture.backdrop()?;
        let reply = backdrop_fixture.cmd("frame backdrop")?;
        let backdrop = parse_frame(&reply)?;
        Ok(Stage {
            backdrop_fixture,
            a,
            b,
            backdrop,
            backdrop_id,
            windows: HashMap::new(),
            control: None,
        })
    }

    pub fn attach(&mut self, control: Control) {
        for (label, w) in &self.windows {
            if w.managed {
                control.send(&self.manage_line(label, w));
            }
        }
        self.control = Some(control);
    }

    pub fn detach(&mut self) {
        self.control = None;
    }

    pub fn title(&self, owner: Owner, label: &str) -> String {
        let name = match owner {
            Owner::A => &self.a.name,
            Owner::B => &self.b.name,
        };
        format!("{name}:{label}")
    }

    fn manage_line(&self, label: &str, w: &TestWindow) -> String {
        format!(
            "manage {} {} {}",
            self.pid(w.owner),
            w.id,
            self.title(w.owner, label)
        )
    }

    pub fn pids(&self) -> Vec<i32> {
        vec![self.backdrop_fixture.pid, self.a.pid, self.b.pid]
    }

    pub fn pid(&self, owner: Owner) -> i32 {
        match owner {
            Owner::A => self.a.pid,
            Owner::B => self.b.pid,
        }
    }

    pub fn fixture(&mut self, owner: Owner) -> &mut Fixture {
        match owner {
            Owner::A => &mut self.a,
            Owner::B => &mut self.b,
        }
    }

    pub fn window(&self, label: &str) -> TestWindow {
        self.windows[label]
    }

    pub fn cmd(&mut self, label: &str, verb: &str, rest: &str) -> Result<String, String> {
        let owner = self.windows[label].owner;
        self.fixture(owner).cmd(&format!("{verb} {label} {rest}"))
    }

    pub fn open(&mut self, owner: Owner, label: &str, rect: Rect, color: &str) -> Result<u32, String> {
        self.open_as(owner, label, rect, color, true)
    }

    /// Opens a window the overlay must not border.
    pub fn open_unmanaged(
        &mut self,
        owner: Owner,
        label: &str,
        rect: Rect,
        color: &str,
    ) -> Result<u32, String> {
        self.open_as(owner, label, rect, color, false)
    }

    /// Returns the time the last key change happened, in Unix-epoch
    /// milliseconds. Both windows must belong to one fixture.
    pub fn spam(&mut self, first: &str, second: &str, interval_ms: u64, count: u64) -> Result<u64, String> {
        let owner = self.windows[first].owner;
        let limit = Duration::from_millis(interval_ms * count + 5000);
        self.fixture(owner).cmd_within(
            &format!("spam {first} {second} {interval_ms} {count}"),
            limit,
        )?;
        Ok(now_ms().saturating_sub(interval_ms))
    }

    fn open_as(
        &mut self,
        owner: Owner,
        label: &str,
        rect: Rect,
        color: &str,
        managed: bool,
    ) -> Result<u32, String> {
        let id = self.fixture(owner).open(label, rect, color)?;
        let w = TestWindow {
            owner,
            id,
            rect,
            managed,
        };
        if managed && let Some(control) = &self.control {
            control.send(&self.manage_line(label, &w));
        }
        self.windows.insert(label.to_string(), w);
        Ok(id)
    }

    pub fn close(&mut self, label: &str) -> Result<(), String> {
        let Some(w) = self.windows.remove(label) else {
            return Ok(());
        };
        if w.managed && let Some(control) = &self.control {
            control.send(&format!("unmanage {}", w.id));
        }
        self.fixture(w.owner).cmd(&format!("close {label}")).map(|_| ())
    }

    /// Leaves only `layout` open, above every other app's windows, with
    /// fixture A active.
    pub fn reset(&mut self, layout: &[(Owner, &str, Rect, &str)]) -> Result<(), String> {
        let labels: Vec<String> = self.windows.keys().cloned().collect();
        for label in labels {
            self.close(&label)?;
        }
        input::activate(self.backdrop_fixture.pid);
        std::thread::sleep(Duration::from_millis(300));
        for (owner, label, rect, color) in layout {
            self.open(*owner, label, *rect, color)?;
        }
        input::activate(self.a.pid);
        std::thread::sleep(Duration::from_millis(500));
        Ok(())
    }
}

fn parse_frame(reply: &str) -> Result<Rect, String> {
    let n: Vec<f64> = reply
        .split_whitespace()
        .skip(2)
        .filter_map(|w| w.parse().ok())
        .collect();
    match n.as_slice() {
        [x, y, w, h] => Ok(Rect::new(*x, *y, *w, *h)),
        _ => Err(format!("bad frame reply {reply}")),
    }
}

pub const BLUE: &str = "3050c8";
pub const GREEN: &str = "30a050";
pub const ORANGE: &str = "d08030";
pub const CYAN: &str = "30b0c0";
pub const YELLOW: &str = "d0c040";

pub const CPU_LABELS: [&str; 4] = ["a1", "a2", "b1", "b2"];

pub fn cpu_layout() -> Vec<(Owner, &'static str, Rect, &'static str)> {
    vec![
        (Owner::A, "a1", Rect::new(200.0, 150.0, 500.0, 350.0), BLUE),
        (Owner::A, "a2", Rect::new(450.0, 300.0, 500.0, 350.0), GREEN),
        (Owner::B, "b1", Rect::new(800.0, 200.0, 450.0, 400.0), ORANGE),
        (Owner::B, "b2", Rect::new(1000.0, 450.0, 400.0, 300.0), CYAN),
    ]
}
