use super::{TilingOverlayPlacement, drawing::WgpuContext};
use crate::config::Appearance;
use crate::overlay;
use crate::platform::overlay_renderer::OverlayRenderer;
use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::event_loop::{EventSender, NativeEvent};
use crate::platform::windows::external::NativeWindowOrder;
use crate::platform::windows::handle::OverlayHwnd;
use crate::platform::windows::ui::window::Window;
use crate::platform::windows::ui::window::WindowAttributes;
use crate::platform::windows::ui::window::WindowLevel;
use dome_core::{
    ContainerPlacement, FloatWindowPlacement, FractionalLength, Length, Logical, Physical,
    PixelRect, Size, TilingWindowPlacement,
};
use std::rc::Rc;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TilingWindowShow {
    pub(crate) placement: TilingWindowPlacement,
    pub(crate) corner_radius: FractionalLength<Logical>,
}

pub(in crate::platform::windows) struct TilingOverlay {
    renderer: OverlayRenderer,
    monitor: PixelRect,
    windows: Vec<TilingWindowShow>,
    containers: Vec<ContainerPlacement>,
    border_thickness: Length<Physical>,
    aux: Window,
    handle: OverlayHwnd,
    scale: f64,
}

impl TilingOverlay {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        dpi_changes: EventSender<NativeEvent>,
        graphics: &WgpuContext,
        appearance: Appearance,
        placement: &TilingOverlayPlacement,
    ) -> anyhow::Result<Self> {
        let monitor = placement.monitor;
        let aux = Window::new(
            callback_panic,
            &WindowAttributes {
                position: monitor.position(),
                size: monitor.size(),
                click_through: true,
                focusable: true,
                dpi_changes: Some(dpi_changes),
            },
            None,
        )?;
        let (renderer, surface) = graphics.create_drawing(monitor.size(), &appearance)?;
        surface.attach(&aux)?;
        let mut overlay = Self {
            renderer,
            monitor,
            windows: placement.windows.clone(),
            containers: placement.containers.clone(),
            border_thickness: placement.border_thickness,
            handle: OverlayHwnd::new(aux.hwnd()),
            aux,
            scale: placement.scale,
        };
        overlay.rerender();
        overlay.aux.set_level(WindowLevel::Bottom);
        overlay.aux.set_visible(true);
        Ok(overlay)
    }

    pub(in crate::platform::windows) fn set_z_order(&self, order: NativeWindowOrder) {
        self.handle.set_z_order(order);
    }

    pub(in crate::platform::windows) fn focus(&self) {
        self.handle.focus();
    }

    fn rerender(&mut self) {
        let scale = self.scale;
        let monitor_logical = self.monitor.to_logical(scale);
        let windows_logical: Vec<overlay::LogicalTiledWindow> = self
            .windows
            .iter()
            .map(|show| overlay::LogicalTiledWindow {
                id: show.placement.id,
                frame: show.placement.border_box.to_logical(scale),
                visible_frame: show.placement.visible_border_box.to_logical(scale),
                is_highlighted: show.placement.is_highlighted,
                spawn_direction: show.placement.spawn_direction,
                corner_radius: show.corner_radius,
            })
            .collect();
        let containers_logical: Vec<overlay::LogicalTiledContainer> = self
            .containers
            .iter()
            .map(|cp| overlay::LogicalTiledContainer {
                id: cp.id,
                frame: cp.border_box.to_logical(scale),
                visible_frame: cp.visible_border_box.to_logical(scale),
                tab_bar_height: FractionalLength::from_pixels(cp.visible_tab_bar_band.height())
                    .to_logical(scale),
                is_highlighted: cp.is_highlighted,
                spawn_direction: cp.spawn_direction,
                is_tabbed: cp.is_tabbed,
                titles: &cp.titles,
            })
            .collect();
        let theme = self.renderer.theme();
        let border = overlay::BorderMetrics::from_thickness(
            FractionalLength::from_pixels(self.border_thickness).to_logical(scale),
        );
        let _ = self.renderer.render(scale as f32, vec![], |ui| {
            overlay::paint_tiling_overlay(
                ui.ctx(),
                monitor_logical,
                &windows_logical,
                &containers_logical,
                &theme,
                border,
            )
        });
    }
}

impl TilingOverlay {
    pub(in crate::platform::windows) fn update(
        &mut self,
        monitor: PixelRect,
        windows: &[TilingWindowShow],
        containers: &[ContainerPlacement],
        scale: f64,
        border_thickness: Length<Physical>,
    ) {
        if self.monitor != monitor {
            self.renderer.resize(scale as f32, monitor.size());
            self.aux.set_frame(monitor.position(), monitor.size());
            self.aux.set_level(WindowLevel::Bottom);
            self.aux.set_visible(true);
        }

        self.monitor = monitor;
        self.windows = windows.to_vec();
        self.containers = containers.to_vec();
        self.scale = scale;
        self.border_thickness = border_thickness;
        self.rerender();
    }

    pub(in crate::platform::windows) fn set_appearance(&mut self, appearance: &Appearance) {
        self.renderer.set_theme(appearance.theme);
        self.renderer.set_font(&appearance.font);
    }
}

pub(in crate::platform::windows) struct FloatBorderOverlay {
    renderer: OverlayRenderer,
    size: Size<Physical>,
    aux: Window,
    handle: OverlayHwnd,
}

impl FloatBorderOverlay {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        graphics: &WgpuContext,
        appearance: Appearance,
        visible_border_box: PixelRect,
    ) -> anyhow::Result<Self> {
        let size = visible_border_box.size();
        let aux = Window::new(
            callback_panic,
            &WindowAttributes {
                position: visible_border_box.position(),
                size: visible_border_box.size(),
                click_through: true,
                focusable: false,
                dpi_changes: None,
            },
            None,
        )?;
        let (renderer, surface) = graphics.create_drawing(size, &appearance)?;
        surface.attach(&aux)?;
        let handle = OverlayHwnd::new(aux.hwnd());
        Ok(Self {
            renderer,
            size,
            handle,
            aux,
        })
    }

    pub(in crate::platform::windows) fn set_z_order(&self, order: NativeWindowOrder) {
        self.handle.set_z_order(order);
    }
}

impl FloatBorderOverlay {
    pub(in crate::platform::windows) fn update(
        &mut self,
        wp: &FloatWindowPlacement,
        scale: f64,
        border_thickness: Length<Physical>,
        corner_radius: FractionalLength<Logical>,
        visible: bool,
    ) {
        if !visible {
            self.aux.set_visible(false);
            return;
        }
        let vf = wp.visible_border_box;
        let size = vf.size();

        if self.size != size {
            self.renderer.resize(scale as f32, size);
            self.size = size;
        }

        self.aux.set_frame_without_redraw(vf.position(), vf.size());

        let vf_logical = vf.to_logical(scale);
        let frame_logical = wp.border_box.to_logical(scale);
        let theme = self.renderer.theme();
        let border = overlay::BorderMetrics::from_thickness(
            FractionalLength::from_pixels(border_thickness).to_logical(scale),
        );
        let is_highlighted = wp.is_highlighted;

        self.renderer.render(scale as f32, vec![], |ui| {
            overlay::paint_float_border(
                ui.ctx(),
                frame_logical,
                vf_logical,
                is_highlighted,
                corner_radius,
                &theme,
                border,
            );
        });
        self.aux.set_visible(true);
    }

    pub(in crate::platform::windows) fn set_appearance(&mut self, appearance: &Appearance) {
        self.renderer.set_theme(appearance.theme);
        self.renderer.set_font(&appearance.font);
    }
}
