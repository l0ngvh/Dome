use super::{MouseButton, Point};
use crate::platform::tab_bar::TabBarWidget;
use crate::platform::windows::event_loop::RunLoop;
use dome_core::{Physical, WindowId};
use std::cell::RefCell;
use std::rc::Rc;

pub(in crate::platform::windows) enum WindowInput {
    Tabs(Rc<RefCell<TabBarWidget>>),
    Thumbnail(WindowId),
}

impl WindowInput {
    pub(in crate::platform::windows) fn mouse_moved(&mut self, at: Point<Physical>) {
        if let Self::Tabs(widget) = self {
            let mut widget = widget.borrow_mut();
            let position = pointer_position(at, widget.scale());
            widget.push_pointer_moved(position);
        }
    }

    pub(in crate::platform::windows) fn mouse_down(
        &mut self,
        at: Point<Physical>,
        button: MouseButton,
    ) {
        match self {
            Self::Tabs(widget) => {
                let mut widget = widget.borrow_mut();
                let Some(position) = primary_pointer_position(at, button, widget.scale()) else {
                    return;
                };
                widget.push_pointer_button(position, true);
            }
            Self::Thumbnail(window_id) => {
                let window_id = *window_id;
                let run_loop = RunLoop::current();
                run_loop.spawn(async move {
                    RunLoop::with_dome(|dome| {
                        dome.thumbnail_clicked(window_id);
                        dome.update_tray();
                    });
                });
            }
        }
    }

    pub(in crate::platform::windows) fn mouse_up(
        &mut self,
        at: Point<Physical>,
        button: MouseButton,
    ) {
        if let Self::Tabs(widget) = self {
            let mut widget = widget.borrow_mut();
            let Some(position) = primary_pointer_position(at, button, widget.scale()) else {
                return;
            };
            widget.push_pointer_button(position, false);
            if let Some((container, tab_index)) = widget.render() {
                let run_loop = RunLoop::current();
                run_loop.spawn(async move {
                    RunLoop::with_dome(|dome| {
                        dome.tab_clicked(container, tab_index);
                        dome.update_tray();
                    });
                });
            }
        }
    }
}

fn pointer_position(at: Point<Physical>, scale: f64) -> egui::Pos2 {
    let at = at.to_logical(scale);
    egui::pos2(at.x().value() as f32, at.y().value() as f32)
}

fn primary_pointer_position(
    at: Point<Physical>,
    button: MouseButton,
    scale: f64,
) -> Option<egui::Pos2> {
    (button == MouseButton::Primary).then(|| pointer_position(at, scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::tests::{pointer_button, tab_bar, tab_bar_frame};

    #[test]
    fn click_tab_with_physical_pointer_at_double_density_selects_first_tab() {
        let (context, container) = tab_bar();
        let at = Point::new(80, 20);
        let moved = pointer_position(at, 2.0);
        let position = primary_pointer_position(at, MouseButton::Primary, 2.0).unwrap();
        assert_eq!(moved, egui::pos2(40.0, 10.0));
        assert_eq!(position, moved);
        assert_eq!(
            tab_bar_frame(
                &context,
                container,
                vec![
                    egui::Event::PointerMoved(moved),
                    pointer_button(position, true),
                    pointer_button(position, false),
                ]
            ),
            Some((container, 0))
        );
    }

    #[test]
    fn click_tab_with_secondary_button_does_not_select_tab() {
        let (context, container) = tab_bar();
        let mut events = Vec::new();
        for pressed in [true, false] {
            if let Some(position) =
                primary_pointer_position(Point::new(80, 20), MouseButton::Secondary, 2.0)
            {
                events.push(pointer_button(position, pressed));
            }
        }
        assert_eq!(tab_bar_frame(&context, container, events), None);
    }
}
