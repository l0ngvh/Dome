use super::drawing::{WgpuContext, create_drawing};
use super::{ContainerShow, frame_attrs, physical_size};
use crate::config::Appearance;
use crate::platform::macos::ui::window::Window;
use crate::platform::macos::ui::window::WindowAttributes;
use crate::platform::macos::ui::window::WindowInput;
use crate::platform::macos::ui::window::WindowLevel;
use crate::platform::tab_bar::{TabBarContent, TabBarWidget};
use dome_core::{FractionalLength, Logical};
use std::cell::RefCell;
use std::rc::Rc;

pub(in crate::platform::macos) struct TabBarOverlay {
    window: Window,
    widget: Rc<RefCell<TabBarWidget>>,
}

impl TabBarOverlay {
    pub(in crate::platform::macos) fn new(
        gpu: &WgpuContext,
        appearance: Appearance,
        show: &ContainerShow,
        scale: f64,
    ) -> Self {
        let size = physical_size(0.0, 0.0, scale);
        let (renderer, surface) = create_drawing(gpu, scale, size, &appearance);
        let widget = Rc::new(RefCell::new(TabBarWidget::new(
            renderer,
            show.placement.id,
            scale,
            size,
        )));
        let (position, size) = frame_attrs(show.tab_bar_cocoa_frame);
        let window = Window::new(
            &WindowAttributes {
                position,
                size,
                click_through: false,
                focusable: false,
            },
            Some(WindowInput::Tabs(widget.clone())),
        )
        .expect("auxiliary window on main thread");
        window.set_level(WindowLevel::Bottom);
        surface.attach(&window);
        window.set_visible(true);
        Self { window, widget }
    }
}

impl TabBarOverlay {
    pub(in crate::platform::macos) fn render(
        &self,
        cs: &ContainerShow,
        scale: f64,
        border_thickness: FractionalLength<Logical>,
    ) {
        let (position, size) = frame_attrs(cs.tab_bar_cocoa_frame);
        self.window.set_frame(position, size);
        let visible_bar = cs.placement.visible_tab_bar_band.to_dimension();
        let bar = cs.tab_bar_dim;
        self.widget.borrow_mut().set_content(TabBarContent {
            scale,
            surface_size: visible_bar.size(),
            canvas_size: bar.size(),
            border: border_thickness,
            titles: cs.placement.titles.clone(),
            active_index: cs.placement.active_tab_index,
            is_highlighted: cs.placement.is_highlighted,
        });
        self.widget.borrow_mut().render();
        self.window.set_visible(true);
    }

    pub(in crate::platform::macos) fn set_appearance(&self, appearance: &Appearance) {
        let mut widget = self.widget.borrow_mut();
        widget.set_style(appearance.theme, &appearance.font);
    }
}
