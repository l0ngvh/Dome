use super::drawing::WgpuContext;
use crate::config::Appearance;
use crate::platform::tab_bar::{TabBarContent, TabBarWidget};
use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::ui::window::Window;
use crate::platform::windows::ui::window::WindowAttributes;
use crate::platform::windows::ui::window::WindowInput;
use dome_core::{ContainerId, ContainerPlacement, FractionalLength, Length, Physical, PixelRect};
use std::cell::RefCell;
use std::rc::Rc;

pub(in crate::platform::windows) struct TabBarOverlay {
    aux: Window,
    widget: Rc<RefCell<TabBarWidget>>,
}

impl TabBarOverlay {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        graphics: &WgpuContext,
        appearance: Appearance,
        container_id: ContainerId,
        rect: PixelRect,
        scale: f64,
    ) -> anyhow::Result<Self> {
        let (renderer, surface) = graphics.create_drawing(rect.size(), &appearance)?;
        let widget = Rc::new(RefCell::new(TabBarWidget::new(
            renderer,
            container_id,
            scale,
            rect.size(),
        )));
        // A tab click must leave keyboard focus with the managed window.
        let aux = Window::new(
            callback_panic,
            &WindowAttributes {
                position: rect.position(),
                size: rect.size(),
                click_through: false,
                focusable: false,
                dpi_changes: None,
            },
            Some(WindowInput::Tabs(widget.clone())),
        )?;
        surface.attach(&aux)?;
        Ok(Self { aux, widget })
    }
}

impl TabBarOverlay {
    pub(in crate::platform::windows) fn update(
        &mut self,
        placement: &ContainerPlacement,
        scale: f64,
        border_thickness: Length<Physical>,
    ) {
        let visible_band = placement.visible_tab_bar_band.to_logical(scale);
        let band = placement.tab_bar_band.to_logical(scale);
        let border = FractionalLength::from_pixels(border_thickness).to_logical(scale);
        // Updating geometry must preserve this window's stacking order.
        self.aux.set_frame(
            placement.visible_tab_bar_band.position(),
            placement.visible_tab_bar_band.size(),
        );
        self.aux.set_visible(true);
        self.widget.borrow_mut().set_content(TabBarContent {
            scale,
            surface_size: visible_band.size(),
            canvas_size: band.size(),
            border,
            titles: placement.titles.clone(),
            active_index: placement.active_tab_index,
            is_highlighted: placement.is_highlighted,
        });
        self.widget.borrow_mut().render();
    }

    pub(in crate::platform::windows) fn set_appearance(&mut self, appearance: &Appearance) {
        let mut widget = self.widget.borrow_mut();
        widget.set_style(appearance.theme, &appearance.font);
    }
}
