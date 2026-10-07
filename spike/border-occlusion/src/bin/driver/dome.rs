//! The driver acting as Dome. It writes each window frame through AX, so the
//! overlay learns the new frames only from its own AX reads.

use objc2_application_services::AXUIElement;
use objc2_core_foundation::CFRetained;
use spike::ax;
use spike::geometry::Rect;

use crate::input;
use crate::stage::Stage;

const AX_TIMEOUT_SECS: f32 = 0.5;

pub type Layout = Vec<(&'static str, Rect)>;

struct Window {
    label: String,
    pid: i32,
    element: CFRetained<AXUIElement>,
}

pub struct Dome {
    windows: Vec<Window>,
}

impl Dome {
    pub fn new(stage: &Stage, labels: &[&str]) -> Result<Dome, String> {
        let mut windows = Vec::new();
        for label in labels {
            let w = stage.window(label);
            let title = stage.title(w.owner, label);
            let pid = stage.pid(w.owner);
            let element = ax::window_with_title(pid, &title, AX_TIMEOUT_SECS)
                .ok_or_else(|| format!("no AX window titled {title}"))?;
            windows.push(Window {
                label: label.to_string(),
                pid,
                element,
            });
        }
        Ok(Dome { windows })
    }

    fn window(&self, label: &str) -> Option<&Window> {
        self.windows.iter().find(|w| w.label == label)
    }

    /// Writes every frame in `layout` back to back. Returns the labels whose
    /// write failed.
    pub fn apply(&self, layout: &[(&str, Rect)]) -> Vec<String> {
        let mut failed = Vec::new();
        for (label, rect) in layout {
            let ok = self
                .window(label)
                .is_some_and(|w| ax::set_frame(&w.element, rect));
            if !ok {
                failed.push(label.to_string());
            }
        }
        failed
    }

    /// False when the AX write fails.
    pub fn move_window(&self, label: &str, (x, y): (f64, f64)) -> bool {
        self.window(label)
            .is_some_and(|w| ax::set_position(&w.element, x, y))
    }

    /// False when an AX call fails.
    pub fn focus(&self, label: &str) -> bool {
        let Some(w) = self.window(label) else {
            return false;
        };
        let active = input::activate(w.pid);
        let raised = ax::raise_as_main(&w.element);
        active && raised
    }
}

/// Two tilings that fit inside the stage.
pub fn tilings() -> [Layout; 2] {
    [
        vec![
            ("a1", Rect::new(40.0, 80.0, 700.0, 420.0)),
            ("a2", Rect::new(760.0, 80.0, 700.0, 420.0)),
            ("b1", Rect::new(40.0, 520.0, 700.0, 420.0)),
            ("b2", Rect::new(760.0, 520.0, 700.0, 420.0)),
        ],
        vec![
            ("a1", Rect::new(40.0, 80.0, 700.0, 860.0)),
            ("a2", Rect::new(760.0, 80.0, 700.0, 270.0)),
            ("b1", Rect::new(760.0, 370.0, 700.0, 270.0)),
            ("b2", Rect::new(760.0, 660.0, 700.0, 280.0)),
        ],
    ]
}
