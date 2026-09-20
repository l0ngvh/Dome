use std::sync::Mutex;
use std::time::Duration;

use smithay::backend::allocator::dmabuf::{AsDmabuf, Dmabuf};
use smithay::backend::allocator::{Buffer, Fourcc};
use smithay::backend::drm::Framebuffer;
use smithay::backend::drm::compositor::RenderFrameResult;
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::gles::GlesRenderbuffer;
use smithay::backend::renderer::{
    Bind, Blit, ExportMem, Offscreen, Renderer, TextureFilter, TextureMapping,
};
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::{
    self, ZwlrScreencopyFrameV1,
};
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::{
    self, ZwlrScreencopyManagerV1,
};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Clock, Monotonic, Physical, Rectangle, Size, Transform};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::{BufferData, with_buffer_contents, with_buffer_contents_mut};

use super::state::DomeState;

pub(super) const VERSION: u32 = 3;
const FORMAT: wl_shm::Format = wl_shm::Format::Xrgb8888;
const FOURCC: Fourcc = Fourcc::Xrgb8888;
const BYTES_PER_PIXEL: i32 = 4;

pub(super) struct PendingCopy {
    pub(super) frame: ZwlrScreencopyFrameV1,
    pub(super) buffer: WlBuffer,
    pub(super) output: Output,
    pub(super) region: Rectangle<i32, Physical>,
    pub(super) overlay_cursor: bool,
    pub(super) report_damage: bool,
}

struct Capture {
    output: Output,
    region: Rectangle<i32, Physical>,
    overlay_cursor: bool,
}

enum FrameState {
    Pending(Capture),
    Failed,
    Copied,
}

pub(super) struct FrameData(Mutex<FrameState>);

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for DomeState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrScreencopyManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for DomeState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _manager: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput {
                frame,
                overlay_cursor,
                output,
            } => init_frame(frame, overlay_cursor != 0, &output, None, data_init),
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion {
                frame,
                overlay_cursor,
                output,
                x,
                y,
                width,
                height,
            } => {
                let region = Rectangle::new((x, y).into(), (width, height).into());
                init_frame(frame, overlay_cursor != 0, &output, Some(region), data_init)
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, FrameData> for DomeState {
    fn request(
        state: &mut Self,
        _client: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &FrameData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let (buffer, report_damage) = match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => (buffer, false),
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => (buffer, true),
            _ => return,
        };

        let capture = match std::mem::replace(&mut *data.0.lock().unwrap(), FrameState::Copied) {
            FrameState::Pending(capture) => capture,
            FrameState::Failed => return,
            FrameState::Copied => {
                frame.post_error(
                    zwlr_screencopy_frame_v1::Error::AlreadyUsed,
                    "this frame was already copied",
                );
                return;
            }
        };

        if let Err(reason) = check_buffer(&buffer, capture.region.size) {
            frame.post_error(zwlr_screencopy_frame_v1::Error::InvalidBuffer, reason);
            return;
        }
        state.pending_copies.push(PendingCopy {
            frame: frame.clone(),
            buffer,
            output: capture.output,
            region: capture.region,
            overlay_cursor: capture.overlay_cursor,
            report_damage,
        });
    }
}

fn init_frame(
    frame: New<ZwlrScreencopyFrameV1>,
    overlay_cursor: bool,
    wl_output: &WlOutput,
    requested: Option<Rectangle<i32, Physical>>,
    data_init: &mut DataInit<'_, DomeState>,
) {
    let capture = Output::from_resource(wl_output).and_then(|output| {
        let mode = output.current_mode()?;
        let region = clamp_capture_region(requested, mode.size)?;
        Some(Capture {
            output,
            region,
            overlay_cursor,
        })
    });

    let Some(capture) = capture else {
        let frame = data_init.init(frame, FrameData(Mutex::new(FrameState::Failed)));
        frame.failed();
        return;
    };

    let size = capture.region.size;
    let frame = data_init.init(frame, FrameData(Mutex::new(FrameState::Pending(capture))));
    frame.buffer(
        FORMAT,
        size.w as u32,
        size.h as u32,
        (size.w * BYTES_PER_PIXEL) as u32,
    );
    if frame.version() >= 3 {
        // A client that can allocate a dmabuf gets the frame rendered straight into
        // it, with no read back through the CPU.
        frame.linux_dmabuf(FOURCC as u32, size.w as u32, size.h as u32);
        frame.buffer_done();
    }
}

/// dome renders every output at scale 1, so the logical region the protocol
/// specifies needs no conversion to reach physical pixels.
fn clamp_capture_region(
    requested: Option<Rectangle<i32, Physical>>,
    output_size: Size<i32, Physical>,
) -> Option<Rectangle<i32, Physical>> {
    let bounds = Rectangle::from_size(output_size);
    let region = match requested {
        Some(requested) => bounds.intersection(requested)?,
        None => bounds,
    };
    (!region.is_empty()).then_some(region)
}

fn check_buffer(buffer: &WlBuffer, size: Size<i32, Physical>) -> Result<(), String> {
    if let Ok(dmabuf) = get_dmabuf(buffer) {
        if dmabuf.format().code != FOURCC {
            return Err(format!(
                "dmabuf format {:?} is not {FOURCC:?}",
                dmabuf.format().code
            ));
        }
        let got = dmabuf.size();
        if got.w != size.w || got.h != size.h {
            return Err(format!(
                "dmabuf is {}x{}, the frame is {}x{}",
                got.w, got.h, size.w, size.h
            ));
        }
        return Ok(());
    }

    let stride = size.w * BYTES_PER_PIXEL;
    let check = |len: usize, spec: BufferData| {
        if spec.format != FORMAT {
            return Err(format!("format {:?} is not {FORMAT:?}", spec.format));
        }
        if spec.width != size.w || spec.height != size.h {
            return Err(format!(
                "buffer is {}x{}, the frame is {}x{}",
                spec.width, spec.height, size.w, size.h
            ));
        }
        if spec.stride != stride {
            return Err(format!(
                "stride is {}, the frame needs {stride}",
                spec.stride
            ));
        }
        let needed = spec.offset as usize + (stride * size.h) as usize;
        if len < needed {
            return Err(format!(
                "buffer holds {len} bytes, the frame needs {needed}"
            ));
        }
        Ok(())
    };

    match with_buffer_contents(buffer, |_, len, spec| check(len, spec)) {
        Ok(result) => result,
        Err(e) => Err(format!("not a shared-memory buffer: {e}")),
    }
}

pub(super) fn fail_all(copies: &[PendingCopy]) {
    for copy in copies {
        copy.frame.failed();
    }
}

/// Answers every copy waiting on one output from the frame that was just
/// rendered. Each copy re-composites the frame into an offscreen buffer,
/// because the scanout buffer may hold a client surface on its own plane
/// rather than the picture the output shows.
pub(super) fn serve<R, B, F, E>(
    renderer: &mut R,
    frame: &RenderFrameResult<'_, B, F, E>,
    output_size: Size<i32, Physical>,
    copies: &[PendingCopy],
    cursor_ids: &[Id],
) where
    R: Renderer + Bind<Dmabuf> + Blit + Offscreen<GlesRenderbuffer> + ExportMem,
    R::TextureId: 'static,
    B: Buffer + AsDmabuf,
    <B as AsDmabuf>::Error: std::fmt::Debug,
    F: Framebuffer,
    E: Element + RenderElement<R>,
{
    for copy in copies {
        if !copy.frame.is_alive() {
            continue;
        }
        match capture_output(renderer, frame, output_size, copy, cursor_ids) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!("failed to capture an output for screencopy: {e}");
                copy.frame.failed();
            }
        }
    }
}

/// Renders the frame into a client's dmabuf, which needs no read back through the
/// CPU. A whole-output capture renders straight into the client's buffer, while a
/// region capture goes through an offscreen buffer, because the re-composite takes
/// no offset and would otherwise land the output's top-left corner in the buffer.
fn capture_into_dmabuf<R, B, F, E>(
    renderer: &mut R,
    frame: &RenderFrameResult<'_, B, F, E>,
    output_size: Size<i32, Physical>,
    copy: &PendingCopy,
    hidden: Vec<Id>,
    target: &mut Dmabuf,
) -> Result<(), String>
where
    R: Renderer + Bind<Dmabuf> + Blit + Offscreen<GlesRenderbuffer> + ExportMem,
    R::TextureId: 'static,
    B: Buffer + AsDmabuf,
    <B as AsDmabuf>::Error: std::fmt::Debug,
    F: Framebuffer,
    E: Element + RenderElement<R>,
{
    let region = copy.region;
    let whole_output = region == Rectangle::from_size(output_size);

    if whole_output {
        let mut client = renderer
            .bind(target)
            .map_err(|e| format!("cannot bind the client dmabuf: {e}"))?;
        recomposite(renderer, frame, output_size, &mut client, hidden)?;
        force_completion(renderer, &client)?;
    } else {
        let buffer_size = output_size.to_logical(1).to_buffer(1, Transform::Normal);
        let mut offscreen = renderer
            .create_buffer(FOURCC, buffer_size)
            .map_err(|e| format!("no offscreen buffer: {e}"))?;
        let mut offscreen_framebuffer = renderer
            .bind(&mut offscreen)
            .map_err(|e| format!("cannot bind the offscreen buffer: {e}"))?;
        recomposite(
            renderer,
            frame,
            output_size,
            &mut offscreen_framebuffer,
            hidden,
        )?;
        let mut client = renderer
            .bind(target)
            .map_err(|e| format!("cannot bind the client dmabuf: {e}"))?;
        renderer
            .blit(
                &offscreen_framebuffer,
                &mut client,
                region,
                Rectangle::from_size(region.size),
                TextureFilter::Nearest,
            )
            .map_err(|e| format!("cannot copy the region into the client dmabuf: {e}"))?;
        force_completion(renderer, &client)?;
    }

    tracing::debug!(
        "screencopy rendered {}x{} into a client dmabuf",
        region.size.w,
        region.size.h
    );
    Ok(())
}

// A sync point from the re-composite can be empty, and a software renderer keeps
// writing into the buffer after it signals. Reading one pixel back cannot return
// before those writes land, so the client sees a whole frame rather than a torn one.
fn force_completion<R: ExportMem>(
    renderer: &mut R,
    framebuffer: &R::Framebuffer<'_>,
) -> Result<(), String> {
    let mapping = renderer
        .copy_framebuffer(framebuffer, Rectangle::from_size((1, 1).into()), FOURCC)
        .map_err(|e| format!("cannot read the client dmabuf back: {e}"))?;
    renderer
        .map_texture(&mapping)
        .map(|_| ())
        .map_err(|e| format!("cannot map the client dmabuf read: {e}"))
}

// The re-composite may run on another GPU than the one that scanned out the frame,
// so callers that read the result afterwards have to wait for it.
fn recomposite<R, B, F, E>(
    renderer: &mut R,
    frame: &RenderFrameResult<'_, B, F, E>,
    output_size: Size<i32, Physical>,
    framebuffer: &mut R::Framebuffer<'_>,
    hidden: Vec<Id>,
) -> Result<(), String>
where
    R: Renderer + Bind<Dmabuf> + Blit,
    R::TextureId: 'static,
    B: Buffer + AsDmabuf,
    <B as AsDmabuf>::Error: std::fmt::Debug,
    F: Framebuffer,
    E: Element + RenderElement<R>,
{
    frame
        .blit_frame_result(
            output_size,
            Transform::Normal,
            1.0,
            renderer,
            framebuffer,
            [Rectangle::from_size(output_size)],
            hidden,
        )
        .map_err(|e| format!("cannot re-composite the frame: {e}"))?
        .wait()
        .map_err(|e| format!("interrupted while re-compositing the frame: {e}"))
}

fn capture_output<R, B, F, E>(
    renderer: &mut R,
    frame: &RenderFrameResult<'_, B, F, E>,
    output_size: Size<i32, Physical>,
    copy: &PendingCopy,
    cursor_ids: &[Id],
) -> Result<(), String>
where
    R: Renderer + Bind<Dmabuf> + Blit + Offscreen<GlesRenderbuffer> + ExportMem,
    R::TextureId: 'static,
    B: Buffer + AsDmabuf,
    <B as AsDmabuf>::Error: std::fmt::Debug,
    F: Framebuffer,
    E: Element + RenderElement<R>,
{
    let hidden = if copy.overlay_cursor {
        Vec::new()
    } else {
        cursor_ids.to_vec()
    };

    if let Ok(client_dmabuf) = get_dmabuf(&copy.buffer) {
        let mut target = client_dmabuf.clone();
        capture_into_dmabuf(renderer, frame, output_size, copy, hidden, &mut target)?;
        answer(copy);
        return Ok(());
    }

    let buffer_size = output_size.to_logical(1).to_buffer(1, Transform::Normal);
    let mut target = renderer
        .create_buffer(Fourcc::Xrgb8888, buffer_size)
        .map_err(|e| format!("no offscreen buffer: {e}"))?;
    let mut framebuffer = renderer
        .bind(&mut target)
        .map_err(|e| format!("cannot bind the offscreen buffer: {e}"))?;

    recomposite(renderer, frame, output_size, &mut framebuffer, hidden)?;

    let mapping = renderer
        .copy_framebuffer(
            &framebuffer,
            Rectangle::from_size(buffer_size),
            Fourcc::Xrgb8888,
        )
        .map_err(|e| format!("cannot read the offscreen buffer: {e}"))?;
    let pixels = renderer
        .map_texture(&mapping)
        .map_err(|e| format!("cannot map the captured pixels: {e}"))?;

    // A mapping that counts rows from the lower left arrives bottom-up, while a
    // wl_shm buffer holds the top row first.
    let bottom_up = !TextureMapping::flipped(&mapping);
    deliver(copy, pixels, output_size, bottom_up);
    Ok(())
}

/// Writes one captured output into the client's shared-memory buffer, then answers
/// the frame. `source` holds the whole output, so a region capture reads a window of
/// each row.
fn deliver(copy: &PendingCopy, source: &[u8], source_size: Size<i32, Physical>, bottom_up: bool) {
    if !copy.frame.is_alive() {
        return;
    }

    let source_stride = (source_size.w * BYTES_PER_PIXEL) as usize;
    let region = copy.region;
    let written = with_buffer_contents_mut(&copy.buffer, |pointer, len, spec| {
        let offset = spec.offset as usize;
        // SAFETY: the shm pool owns len bytes at pointer for the call, and
        // check_buffer proved the frame fits inside them.
        let destination = unsafe { std::slice::from_raw_parts_mut(pointer, len) };
        copy_region(
            source,
            source_stride,
            region,
            source_size.h,
            bottom_up,
            &mut destination[offset..],
            spec.stride as usize,
        )
    });

    match written {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::warn!("screencopy frame does not fit its buffer: {e}");
            copy.frame.failed();
            return;
        }
        Err(e) => {
            tracing::warn!("screencopy buffer is no longer readable: {e}");
            copy.frame.failed();
            return;
        }
    }

    answer(copy);
}

fn answer(copy: &PendingCopy) {
    if copy.report_damage && copy.frame.version() >= 2 {
        copy.frame
            .damage(0, 0, copy.region.size.w as u32, copy.region.size.h as u32);
    }

    let now: Duration = Clock::<Monotonic>::new().now().into();
    let seconds = now.as_secs();
    copy.frame.ready(
        (seconds >> 32) as u32,
        (seconds & 0xFFFF_FFFF) as u32,
        now.subsec_nanos(),
    );
}

fn copy_region(
    source: &[u8],
    source_stride: usize,
    region: Rectangle<i32, Physical>,
    source_height: i32,
    bottom_up: bool,
    destination: &mut [u8],
    destination_stride: usize,
) -> Result<(), String> {
    let row_bytes = (region.size.w * BYTES_PER_PIXEL) as usize;
    let left = (region.loc.x * BYTES_PER_PIXEL) as usize;

    for row in 0..region.size.h {
        let source_row_index = if bottom_up {
            source_height - 1 - (region.loc.y + row)
        } else {
            region.loc.y + row
        };
        if source_row_index < 0 {
            return Err(format!("row {row} is above the captured output"));
        }
        let source_start = source_row_index as usize * source_stride + left;
        let destination_start = row as usize * destination_stride;
        let source_row = source
            .get(source_start..source_start + row_bytes)
            .ok_or_else(|| format!("row {row} is past the end of the captured output"))?;
        let destination_row = destination
            .get_mut(destination_start..destination_start + row_bytes)
            .ok_or_else(|| format!("row {row} is past the end of the client buffer"))?;
        destination_row.copy_from_slice(source_row);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Physical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn no_request_captures_the_whole_output() {
        assert_eq!(
            clamp_capture_region(None, (1920, 1080).into()),
            Some(region(0, 0, 1920, 1080))
        );
    }

    #[test]
    fn a_region_inside_the_output_survives() {
        assert_eq!(
            clamp_capture_region(Some(region(10, 20, 100, 50)), (1920, 1080).into()),
            Some(region(10, 20, 100, 50))
        );
    }

    #[test]
    fn a_region_hanging_off_the_edge_is_clipped() {
        assert_eq!(
            clamp_capture_region(Some(region(-10, 1000, 100, 200)), (1920, 1080).into()),
            Some(region(0, 1000, 90, 80))
        );
    }

    #[test]
    fn a_region_outside_the_output_captures_nothing() {
        assert_eq!(
            clamp_capture_region(Some(region(2000, 0, 100, 100)), (1920, 1080).into()),
            None
        );
    }

    #[test]
    fn a_zero_sized_region_captures_nothing() {
        assert_eq!(
            clamp_capture_region(Some(region(0, 0, 0, 100)), (1920, 1080).into()),
            None
        );
    }

    #[test]
    fn a_whole_output_copy_keeps_every_byte() {
        let source: Vec<u8> = (0..32u8).collect();
        let mut destination = vec![0u8; 32];
        copy_region(
            &source,
            16,
            region(0, 0, 4, 2),
            2,
            false,
            &mut destination,
            16,
        )
        .unwrap();
        assert_eq!(destination, source);
    }

    #[test]
    fn a_region_copy_takes_a_window_of_each_row() {
        let source: Vec<u8> = (0..32u8).collect();
        let mut destination = vec![0u8; 8];
        copy_region(
            &source,
            16,
            region(1, 0, 1, 2),
            2,
            false,
            &mut destination,
            4,
        )
        .unwrap();
        assert_eq!(destination, vec![4, 5, 6, 7, 20, 21, 22, 23]);
    }

    #[test]
    fn a_destination_row_may_be_wider_than_the_region() {
        let source: Vec<u8> = (0..32u8).collect();
        let mut destination = vec![0u8; 16];
        copy_region(
            &source,
            16,
            region(0, 1, 1, 1),
            2,
            false,
            &mut destination,
            8,
        )
        .unwrap();
        assert_eq!(destination[..4], [16, 17, 18, 19]);
        assert_eq!(destination[4..], [0; 12]);
    }

    #[test]
    fn a_bottom_up_source_comes_out_top_row_first() {
        let source: Vec<u8> = (0..32u8).collect();
        let mut destination = vec![0u8; 32];
        copy_region(
            &source,
            16,
            region(0, 0, 4, 2),
            2,
            true,
            &mut destination,
            16,
        )
        .unwrap();
        assert_eq!(destination[..16], source[16..]);
        assert_eq!(destination[16..], source[..16]);
    }

    #[test]
    fn a_bottom_up_region_reads_from_the_far_end() {
        let source: Vec<u8> = (0..48u8).collect();
        let mut destination = vec![0u8; 4];
        copy_region(
            &source,
            16,
            region(0, 0, 1, 1),
            3,
            true,
            &mut destination,
            4,
        )
        .unwrap();
        assert_eq!(destination, vec![32, 33, 34, 35]);
    }

    #[test]
    fn a_short_client_buffer_reports_the_row_it_ran_out_on() {
        let source: Vec<u8> = (0..32u8).collect();
        let mut destination = vec![0u8; 4];
        let e = copy_region(
            &source,
            16,
            region(0, 0, 1, 2),
            2,
            false,
            &mut destination,
            4,
        )
        .unwrap_err();
        assert_eq!(e, "row 1 is past the end of the client buffer");
    }

    #[test]
    fn a_short_source_reports_the_row_it_ran_out_on() {
        let source: Vec<u8> = (0..16u8).collect();
        let mut destination = vec![0u8; 8];
        let e = copy_region(
            &source,
            16,
            region(0, 0, 1, 2),
            2,
            false,
            &mut destination,
            4,
        )
        .unwrap_err();
        assert_eq!(e, "row 1 is past the end of the captured output");
    }
}
