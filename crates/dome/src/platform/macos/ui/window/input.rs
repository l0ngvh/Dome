use super::{MouseButton, Point};
use crate::platform::macos::event_loop::RunLoop;
use crate::platform::tab_bar::TabBarWidget;
use dome_core::{Logical, WindowId};
use std::cell::RefCell;
use std::rc::Rc;

pub(in crate::platform::macos) enum WindowInput {
    Tabs(Rc<RefCell<TabBarWidget>>),
    Mirror(WindowId),
}

impl WindowInput {
    pub(in crate::platform::macos) fn mouse_down(
        &mut self,
        at: Point<Logical>,
        _button: MouseButton,
    ) {
        match self {
            Self::Tabs(widget) => {
                let mut widget = widget.borrow_mut();
                widget.push_pointer_button(pointer_position(at), true);
                widget.render();
            }
            Self::Mirror(window_id) => {
                let window_id = *window_id;
                let run_loop = RunLoop::current();
                run_loop.spawn(async move {
                    RunLoop::with_dome(|dome| {
                        dome.mirror_clicked(window_id);
                        dome.update_menu_bar();
                    });
                });
            }
        }
    }

    pub(in crate::platform::macos) fn mouse_up(
        &mut self,
        at: Point<Logical>,
        _button: MouseButton,
    ) {
        if let Self::Tabs(widget) = self {
            let mut widget = widget.borrow_mut();
            widget.push_pointer_button(pointer_position(at), false);
            if let Some((container, tab_index)) = widget.render() {
                let run_loop = RunLoop::current();
                run_loop.spawn(async move {
                    RunLoop::with_dome(|dome| {
                        dome.tab_clicked(container, tab_index);
                        dome.update_menu_bar();
                    });
                });
            }
        }
    }
}

fn pointer_position(at: Point<Logical>) -> egui::Pos2 {
    egui::pos2(at.x().value() as f32, at.y().value() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::tests::{pointer_button, tab_bar, tab_bar_frame};

    #[test]
    fn click_tab_with_logical_pointer_at_double_density_selects_first_tab() {
        let (context, container) = tab_bar();
        let position = pointer_position(Point::new(40, 10));
        assert_eq!(position, egui::pos2(40.0, 10.0));
        assert_eq!(
            tab_bar_frame(&context, container, vec![pointer_button(position, true)]),
            None
        );
        assert_eq!(
            tab_bar_frame(&context, container, vec![pointer_button(position, false)]),
            Some((container, 0))
        );
    }
}
