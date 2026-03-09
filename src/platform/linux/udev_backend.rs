use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use calloop::{EventLoop, RegistrationToken};
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::compositor::FrameFlags;
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmNode};
use smithay::backend::egl::{EGLDevice, EGLDisplay};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{Kind, Wrap};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::multigpu::GpuManager;
use smithay::backend::renderer::{ImportAll, ImportMem};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent};
use smithay::desktop::layer_map_for_output;
use smithay::desktop::space::{SpaceRenderElements, space_render_elements};
use smithay::input::pointer::{CursorIcon, CursorImageStatus, CursorImageSurfaceData};
use smithay::output::{Mode as WlMode, Output, PhysicalProperties};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::drm::control::{connector, crtc, ModeTypeFlags};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{DeviceFd, Transform};
use smithay::wayland::compositor::with_states;
use smithay_drm_extras::display_info;
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};

use super::CalloopData;
use super::state::DomeState;

const COLOR_FORMATS: &[Fourcc] = &[Fourcc::Argb8888, Fourcc::Abgr8888];

smithay::backend::renderer::element::render_elements! {
    DrmRenderElement<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
    Wrapped=Wrap<WaylandSurfaceRenderElement<R>>,
}

type GbmDrmOutputManager = DrmOutputManager<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;

type GbmDrmOutput = DrmOutput<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;

pub(super) struct UdevData {
    pub(super) session: LibSeatSession,
    primary_gpu: DrmNode,
    gpus: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    backends: HashMap<DrmNode, BackendData>,
    pointer_image: MemoryRenderBuffer,
    gl: Option<std::sync::Arc<glow::Context>>,
    egui_fbo: Option<EguiFbo>,
    cursor_theme: xcursor::CursorTheme,
    cursor_size: u32,
    cursor_cache: HashMap<CursorIcon, (MemoryRenderBuffer, (i32, i32))>,
}

struct EguiFbo {
    fbo: glow::Framebuffer,
    texture: glow::Texture,
    width: u32,
    height: u32,
}

struct BackendData {
    drm_output_manager: GbmDrmOutputManager,
    drm_scanner: DrmScanner,
    surfaces: HashMap<crtc::Handle, SurfaceData>,
    render_node: Option<DrmNode>,
    registration_token: RegistrationToken,
}

struct SurfaceData {
    drm_output: GbmDrmOutput,
    output: Output,
    global: Option<GlobalId>,
    dh: DisplayHandle,
}

impl Drop for SurfaceData {
    fn drop(&mut self) {
        if let Some(global) = self.global.take() {
            self.dh.remove_global::<DomeState>(global);
        }
    }
}

fn load_default_cursor() -> MemoryRenderBuffer {
    let size = 16;
    let mut pixels = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            if x <= y && x + y < size {
                let idx = (y * size + x) * 4;
                pixels[idx] = 0xFF;
                pixels[idx + 1] = 0xFF;
                pixels[idx + 2] = 0xFF;
                pixels[idx + 3] = 0xFF;
            }
        }
    }
    MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Argb8888,
        (size as i32, size as i32),
        1,
        Transform::Normal,
        None,
    )
}

fn load_xcursor(
    theme: &xcursor::CursorTheme,
    name: &str,
    size: u32,
) -> Option<(MemoryRenderBuffer, (i32, i32))> {
    let path = theme.load_icon(name)?;
    let data = std::fs::read(&path).ok()?;
    let images = xcursor::parser::parse_xcursor(&data)?;
    let image = images
        .iter()
        .min_by_key(|img| (img.size as i32 - size as i32).unsigned_abs())?;
    let buf = MemoryRenderBuffer::from_slice(
        &image.pixels_argb,
        Fourcc::Argb8888,
        (image.width as i32, image.height as i32),
        1,
        Transform::Normal,
        None,
    );
    Some((buf, (image.xhot as i32, image.yhot as i32)))
}

pub(super) fn init_udev_backend(
    event_loop: &mut EventLoop<'static, CalloopData>,
    data: &mut CalloopData,
) -> Result<()> {
    let (session, notifier) = LibSeatSession::new()?;
    let seat_name = session.seat();

    let primary_gpu = primary_gpu(&seat_name)?
        .and_then(|p| DrmNode::from_path(p).ok())
        .unwrap_or_else(|| {
            all_gpus(&seat_name)
                .unwrap()
                .into_iter()
                .find_map(|p| DrmNode::from_path(p).ok())
                .expect("No GPU found")
        });
    tracing::info!("Using {} as primary GPU", primary_gpu);

    let gpus = GpuManager::new(GbmGlesBackend::<GlesRenderer, DrmDeviceFd>::default())?;

    let cursor_theme_name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());
    let cursor_size: u32 = std::env::var("XCURSOR_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    data.state.udev_data = Some(UdevData {
        session: session.clone(),
        primary_gpu,
        gpus,
        backends: HashMap::new(),
        pointer_image: load_default_cursor(),
        gl: None,
        egui_fbo: None,
        cursor_theme: xcursor::CursorTheme::load(&cursor_theme_name),
        cursor_size,
        cursor_cache: HashMap::new(),
    });

    // Libinput
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput_context
        .udev_assign_seat(&seat_name)
        .map_err(|_| anyhow::anyhow!("Failed to assign libinput seat"))?;
    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());

    event_loop
        .handle()
        .insert_source(libinput_backend, |event, _, data| {
            data.state.process_input_event(event);
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert libinput source: {e}"))?;

    // Udev — enumerate existing GPUs
    let udev_backend = UdevBackend::new(&seat_name)?;
    for (device_id, path) in udev_backend.device_list() {
        if let Ok(node) = DrmNode::from_dev_id(device_id) {
            if let Err(e) = device_added(&mut data.state, node, path) {
                tracing::warn!("Skipping device {device_id}: {e}");
            }
        }
    }

    // Udev hotplug events
    event_loop
        .handle()
        .insert_source(udev_backend, move |event, _, data| match event {
            UdevEvent::Added { device_id, path } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    if let Err(e) = device_added(&mut data.state, node, &path) {
                        tracing::warn!("Skipping device {device_id}: {e}");
                    }
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    device_changed(&mut data.state, node);
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    device_removed(&mut data.state, node);
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert udev source: {e}"))?;

    // Session events (VT switch)
    event_loop
        .handle()
        .insert_source(notifier, move |event, _, data| {
            let udev = data.state.udev_data.as_mut().unwrap();
            match event {
                SessionEvent::PauseSession => {
                    tracing::info!("Session paused");
                    libinput_context.suspend();
                    for backend in udev.backends.values_mut() {
                        backend.drm_output_manager.pause();
                    }
                }
                SessionEvent::ActivateSession => {
                    tracing::info!("Session resumed");
                    if let Err(e) = libinput_context.resume() {
                        tracing::error!("Failed to resume libinput: {e:?}");
                    }
                    let nodes: Vec<_> = udev.backends.keys().copied().collect();
                    for backend in udev.backends.values_mut() {
                        backend
                            .drm_output_manager
                            .activate(false)
                            .expect("Failed to activate DRM backend");
                    }
                    for node in nodes {
                        data.state.loop_handle.insert_idle(move |data| {
                            device_changed(&mut data.state, node);
                        });
                    }
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert session source: {e}"))?;

    Ok(())
}

fn device_added(state: &mut DomeState, node: DrmNode, path: &Path) -> Result<()> {
    let udev = state.udev_data.as_mut().unwrap();

    let fd = udev
        .session
        .open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)
        .map_err(|e| anyhow::anyhow!("Failed to open device: {e}"))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));

    let (drm, notifier) =
        DrmDevice::new(fd.clone(), true).map_err(|e| anyhow::anyhow!("DRM init failed: {e}"))?;
    let gbm = GbmDevice::new(fd).map_err(|e| anyhow::anyhow!("GBM init failed: {e}"))?;

    // SAFETY: GBM device is valid, EGL display creation is standard GPU init
    let render_node = unsafe { EGLDisplay::new(gbm.clone()) }
        .ok()
        .and_then(|display| EGLDevice::device_for_display(&display).ok())
        .and_then(|egl_device| egl_device.try_get_render_node().ok().flatten())
        .unwrap_or(node);

    udev.gpus.as_mut().add_node(render_node, gbm.clone())?;

    let allocator =
        GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let framebuffer_exporter = GbmFramebufferExporter::new(gbm.clone(), Some(render_node));

    let mut renderer = udev.gpus.single_renderer(&render_node)?;
    let render_formats = renderer.as_mut().egl_context().dmabuf_render_formats().clone();

    // Create glow context for egui on first GPU
    let need_egui_init = udev.gl.is_none();
    if need_egui_init {
        unsafe { renderer.as_mut().egl_context().make_current()? };
        let gl = std::sync::Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                smithay::backend::egl::get_proc_address(s) as *const _
            })
        });
        udev.gl = Some(gl);
    }

    // Init egui painter (needs &mut state, so drop udev borrow first)
    if need_egui_init {
        let gl = state.udev_data.as_ref().unwrap().gl.clone().unwrap();
        state.init_egui_painter(gl);
    }
    let udev = state.udev_data.as_mut().unwrap();

    let drm_output_manager = DrmOutputManager::new(
        drm,
        allocator,
        framebuffer_exporter,
        Some(gbm),
        COLOR_FORMATS.iter().copied(),
        render_formats,
    );

    let token = state
        .loop_handle
        .insert_source(notifier, move |event, _metadata, data| match event {
            DrmEvent::VBlank(crtc) => frame_finish(&mut data.state, node, crtc),
            DrmEvent::Error(e) => tracing::error!("DRM error: {e:?}"),
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert DRM source: {e}"))?;

    let udev = state.udev_data.as_mut().unwrap();
    udev.backends.insert(
        node,
        BackendData {
            drm_output_manager,
            drm_scanner: DrmScanner::new(),
            surfaces: HashMap::new(),
            render_node: Some(render_node),
            registration_token: token,
        },
    );

    device_changed(state, node);
    Ok(())
}

fn device_changed(state: &mut DomeState, node: DrmNode) {
    let udev = state.udev_data.as_mut().unwrap();
    let Some(device) = udev.backends.get_mut(&node) else { return };

    let scan_result =
        match device.drm_scanner.scan_connectors(device.drm_output_manager.device()) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to scan connectors: {e}");
                return;
            }
        };

    let events: Vec<_> = scan_result.into_iter().collect();
    for event in events {
        match event {
            DrmScanEvent::Connected {
                connector,
                crtc: Some(crtc),
            } => connector_connected(state, node, connector, crtc),
            DrmScanEvent::Disconnected {
                connector: _,
                crtc: Some(crtc),
            } => connector_disconnected(state, node, crtc),
            _ => {}
        }
    }
}

fn connector_connected(
    state: &mut DomeState,
    node: DrmNode,
    connector: connector::Info,
    crtc: crtc::Handle,
) {
    let udev = state.udev_data.as_mut().unwrap();
    let Some(device) = udev.backends.get_mut(&node) else {
        return;
    };
    let render_node = device.render_node.unwrap_or(udev.primary_gpu);

    let mut renderer = match udev.gpus.single_renderer(&render_node) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to get renderer: {e}");
            return;
        }
    };

    let output_name = format!(
        "{}-{}",
        connector.interface().as_str(),
        connector.interface_id()
    );
    tracing::info!(output = %output_name, ?crtc, "Connector connected");

    let drm_device = device.drm_output_manager.device();
    let display_info = display_info::for_connector(drm_device, connector.handle());

    let mode_id = match connector
        .modes()
        .iter()
        .position(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
    {
        Some(i) => i,
        None if !connector.modes().is_empty() => 0,
        None => {
            tracing::warn!("Connector has no modes, skipping");
            return;
        }
    };
    let drm_mode = connector.modes()[mode_id];
    let wl_mode = WlMode::from(drm_mode);

    let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
    let make = display_info
        .as_ref()
        .and_then(|i| i.make())
        .unwrap_or_else(|| "Unknown".into());
    let model = display_info
        .as_ref()
        .and_then(|i| i.model())
        .unwrap_or_else(|| "Unknown".into());

    let output = Output::new(
        output_name,
        PhysicalProperties {
            size: (phys_w as i32, phys_h as i32).into(),
            subpixel: connector.subpixel().into(),
            make,
            model,
        },
    );
    let global = output.create_global::<DomeState>(&state.display_handle);
    output.set_preferred(wl_mode);
    output.change_current_state(
        Some(wl_mode),
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    state.space.map_output(&output, (0, 0));

    let monitor_id = state.hub.focused_monitor();
    state.hub.update_monitor_dimension(
        monitor_id,
        crate::core::Dimension {
            x: 0.0,
            y: 0.0,
            width: wl_mode.size.w as f32,
            height: wl_mode.size.h as f32,
        },
    );

    let drm_output =
        match device.drm_output_manager.initialize_output::<_, DrmRenderElement<_>>(
            crtc,
            drm_mode,
            &[connector.handle()],
            &output,
            None,
            &mut renderer,
            &DrmOutputRenderElements::default(),
        ) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!("Failed to initialize DRM output: {e:?}");
                return;
            }
        };

    device.surfaces.insert(
        crtc,
        SurfaceData {
            drm_output,
            output,
            global: Some(global),
            dh: state.display_handle.clone(),
        },
    );

    state.loop_handle.clone().insert_idle(move |data| {
        render_surface(&mut data.state, node, crtc);
    });
}

fn connector_disconnected(state: &mut DomeState, node: DrmNode, crtc: crtc::Handle) {
    let udev = state.udev_data.as_mut().unwrap();
    if let Some(device) = udev.backends.get_mut(&node) {
        if let Some(surface) = device.surfaces.remove(&crtc) {
            state.space.unmap_output(&surface.output);
        }
    }
}

fn device_removed(state: &mut DomeState, node: DrmNode) {
    let udev = state.udev_data.as_mut().unwrap();
    let Some(backend) = udev.backends.get_mut(&node) else {
        return;
    };

    let crtcs: Vec<_> = backend.drm_scanner.crtcs().map(|(_, crtc)| crtc).collect();
    for crtc in crtcs {
        connector_disconnected(state, node, crtc);
    }

    let udev = state.udev_data.as_mut().unwrap();
    if let Some(backend) = udev.backends.remove(&node) {
        state.loop_handle.remove(backend.registration_token);
    }

    tracing::info!("Device removed: {node}");
}

fn frame_finish(state: &mut DomeState, node: DrmNode, crtc: crtc::Handle) {
    let udev = state.udev_data.as_mut().unwrap();
    let Some(device) = udev.backends.get_mut(&node) else {
        return;
    };
    let Some(surface) = device.surfaces.get_mut(&crtc) else {
        return;
    };

    if let Err(e) = surface.drm_output.frame_submitted() {
        tracing::warn!("frame_submitted error: {e:?}");
        return;
    }

    let frame_duration = surface
        .output
        .current_mode()
        .map(|m| Duration::from_secs_f64(1_000.0 / m.refresh as f64))
        .unwrap_or(Duration::from_millis(16));

    let timer = Timer::from_duration(frame_duration * 6 / 10);
    state
        .loop_handle
        .insert_source(timer, move |_, _, data| {
            render_surface(&mut data.state, node, crtc);
            TimeoutAction::Drop
        })
        .ok();
}


fn ensure_egui_fbo(
    gl: &glow::Context,
    fbo: &mut Option<EguiFbo>,
    width: u32,
    height: u32,
) {
    use glow::HasContext;
    if let Some(existing) = fbo {
        if existing.width == width && existing.height == height {
            return;
        }
        unsafe {
            gl.delete_framebuffer(existing.fbo);
            gl.delete_texture(existing.texture);
        }
    }

    unsafe {
        let texture = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0, glow::RGBA8 as i32,
            width as i32, height as i32, 0,
            glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);

        let fbo_id = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo_id));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D, Some(texture), 0,
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.bind_texture(glow::TEXTURE_2D, None);

        *fbo = Some(EguiFbo { fbo: fbo_id, texture, width, height });
    }
}

fn render_egui_to_buffer(
    state: &mut DomeState,
    gl: &std::sync::Arc<glow::Context>,
    fbo: &mut Option<EguiFbo>,
    width: u32,
    height: u32,
) -> Option<MemoryRenderBuffer> {
    use glow::HasContext;

    if state.egui_painter.is_none() {
        return None;
    }

    ensure_egui_fbo(gl, fbo, width, height);
    let egui_fbo = fbo.as_ref().unwrap();

    let (meshes, textures_delta, pixels_per_point) = state.build_egui_shapes(width, height);

    let mut painter = state.egui_painter.take().unwrap();

    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(egui_fbo.fbo));
        gl.viewport(0, 0, width as i32, height as i32);
        gl.clear_color(0.0, 0.0, 0.0, 0.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
    }

    painter.paint_and_update_textures(
        [width, height],
        pixels_per_point,
        &meshes,
        &textures_delta,
    );

    let mut pixels = vec![0u8; (width * height * 4) as usize];
    unsafe {
        gl.read_pixels(
            0, 0, width as i32, height as i32,
            glow::RGBA, glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut pixels)),
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    }

    state.egui_painter = Some(painter);

    // glReadPixels returns bottom-up, flip vertically
    let stride = (width * 4) as usize;
    for y in 0..(height as usize / 2) {
        let top = y * stride;
        let bot = ((height as usize) - 1 - y) * stride;
        for x in 0..stride {
            pixels.swap(top + x, bot + x);
        }
    }

    Some(MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Abgr8888,
        (width as i32, height as i32),
        1,
        Transform::Normal,
        None,
    ))
}
fn render_surface(state: &mut DomeState, node: DrmNode, crtc: crtc::Handle) {
    // Render egui to a buffer before borrowing udev (needs &mut state)
    let egui_buf = {
        let udev = state.udev_data.as_mut().unwrap();
        let Some(device) = udev.backends.get(&node) else { return };
        let Some(surface) = device.surfaces.get(&crtc) else { return };
        let output_mode = surface.output.current_mode();
        let gl = udev.gl.clone();
        let mut fbo = udev.egui_fbo.take();

        let buf = match (gl, output_mode) {
            (Some(gl), Some(mode)) => {
                unsafe { let _ = udev.gpus.single_renderer(&udev.primary_gpu)
                    .map(|mut r| r.as_mut().egl_context().make_current()); }
                render_egui_to_buffer(
                    state, &gl, &mut fbo,
                    mode.size.w as u32, mode.size.h as u32,
                )
            }
            _ => None,
        };
        state.udev_data.as_mut().unwrap().egui_fbo = fbo;
        buf
    };

    let udev = state.udev_data.as_mut().unwrap();
    let Some(device) = udev.backends.get_mut(&node) else {
        return;
    };
    let Some(surface) = device.surfaces.get_mut(&crtc) else {
        return;
    };
    let render_node = device.render_node.unwrap_or(udev.primary_gpu);
    let output = surface.output.clone();

    let mut renderer = match udev.gpus.single_renderer(&render_node) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to get renderer: {e}");
            return;
        }
    };

    let mut elements: Vec<DrmRenderElement<_>> = Vec::new();

    // Cursor (rendered on top)
    let pointer_loc = state.seat.get_pointer().unwrap().current_location();
    match &state.cursor_status {
        CursorImageStatus::Surface(cursor_surface) => {
            let hotspot = with_states(cursor_surface, |states| {
                states
                    .data_map
                    .get::<CursorImageSurfaceData>()
                    .map(|d| d.lock().unwrap().hotspot)
                    .unwrap_or_default()
            });
            let pos = (pointer_loc - hotspot.to_f64()).to_i32_round();
            let cursor_elements = render_elements_from_surface_tree(
                &mut renderer,
                cursor_surface,
                pos.to_physical(1),
                1.0,
                1.0,
                Kind::Cursor,
            );
            elements.extend(cursor_elements.into_iter().map(DrmRenderElement::Surface));
        }
        CursorImageStatus::Named(icon) => {
            if !udev.cursor_cache.contains_key(icon) {
                if let Some(entry) = load_xcursor(&udev.cursor_theme, icon.name(), udev.cursor_size) {
                    udev.cursor_cache.insert(*icon, entry);
                }
            }
            let (buf, hotspot) = udev.cursor_cache.get(icon)
                .map(|(b, h)| (b, *h))
                .unwrap_or((&udev.pointer_image, (0, 0)));
            let pos = pointer_loc - smithay::utils::Point::from(hotspot).to_f64();
            let cursor_elem = MemoryRenderBufferRenderElement::from_buffer(
                &mut renderer,
                pos.to_i32_round().to_physical(1).to_f64(),
                buf,
                None, None, None,
                Kind::Cursor,
            );
            if let Ok(elem) = cursor_elem {
                elements.push(DrmRenderElement::Memory(elem));
            }
        }
        _ => {
            let cursor_elem = MemoryRenderBufferRenderElement::from_buffer(
                &mut renderer,
                pointer_loc.to_i32_round().to_physical(1).to_f64(),
                &udev.pointer_image,
                None, None, None,
                Kind::Cursor,
            );
            if let Ok(elem) = cursor_elem {
                elements.push(DrmRenderElement::Memory(elem));
            }
        }
    }

    // Egui overlays (borders, tab bars) — between cursor and space
    if let Some(ref egui_buf) = egui_buf {
        let elem = MemoryRenderBufferRenderElement::from_buffer(
            &mut renderer,
            smithay::utils::Point::<f64, smithay::utils::Physical>::from((0.0, 0.0)),
            egui_buf,
            None, None, None,
            Kind::Unspecified,
        );
        if let Ok(e) = elem {
            elements.push(DrmRenderElement::Memory(e));
        }
    }

    // Space elements (windows + layer surfaces) with damage tracking
    match space_render_elements(&mut renderer, [&state.space], &output, 1.0) {
        Ok(space_elems) => {
            for elem in space_elems {
                match elem {
                    SpaceRenderElements::Surface(ws) => {
                        elements.push(DrmRenderElement::Surface(ws));
                    }
                    SpaceRenderElements::Element(wrap) => {
                        elements.push(DrmRenderElement::Wrapped(wrap));
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to get space render elements: {e:?}");
        }
    }

    let result = surface.drm_output.render_frame(
        &mut renderer,
        &elements,
        [0.1, 0.1, 0.1, 1.0],
        FrameFlags::DEFAULT,
    );

    match result {
        Ok(render_result) => {
            if !render_result.is_empty {
                if let Err(e) = surface.drm_output.queue_frame(()) {
                    tracing::warn!("queue_frame error: {e:?}");
                }
            } else {
                let timer = Timer::from_duration(Duration::from_millis(16));
                state
                    .loop_handle
                    .insert_source(timer, move |_, _, data| {
                        render_surface(&mut data.state, node, crtc);
                        TimeoutAction::Drop
                    })
                    .ok();
            }
        }
        Err(e) => tracing::warn!("render_frame error: {e:?}"),
    }

    // Send frame callbacks
    let elapsed = state.start_time.elapsed();
    state.space.elements().for_each(|window| {
        window.send_frame(
            &output,
            elapsed,
            Some(Duration::ZERO),
            |_, _| Some(output.clone()),
        );
    });
    {
        let layer_map = layer_map_for_output(&output);
        for layer in layer_map.layers() {
            layer.send_frame(
                &output,
                elapsed,
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        }
    }

    state.space.refresh();
    state.popups.cleanup();
    layer_map_for_output(&output).cleanup();
    state.display_handle.flush_clients().ok();
}
