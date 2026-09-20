// Captures an output through wlr-screencopy into a dmabuf the client allocates, and
// dumps the pixels as tightly packed BGRX rows. The udev integration suite compares
// that dump against a grim shm capture of the same output, so two independent copy
// paths have to agree.
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use smithay::backend::allocator::dmabuf::AsDmabuf;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::{Allocator, Fourcc, Modifier};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1::{
    self, ZwpLinuxBufferParamsV1,
};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    self, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

const DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Client {
    output: Option<WlOutput>,
    dmabuf: Option<ZwpLinuxDmabufV1>,
    manager: Option<ZwlrScreencopyManagerV1>,
    offered_dmabuf: Option<(u32, u32, u32)>,
    buffer_done: bool,
    ready: bool,
    failed: bool,
}

impl Dispatch<WlRegistry, ()> for Client {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_output" if state.output.is_none() => {
                state.output = Some(registry.bind(name, version.min(4), queue, ()));
            }
            "zwp_linux_dmabuf_v1" if state.dmabuf.is_none() => {
                state.dmabuf = Some(registry.bind(name, version.min(3), queue, ()));
            }
            "zwlr_screencopy_manager_v1" if state.manager.is_none() => {
                state.manager = Some(registry.bind(name, version.min(3), queue, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for Client {
    fn event(
        state: &mut Self,
        _frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::LinuxDmabuf {
                format,
                width,
                height,
            } => state.offered_dmabuf = Some((format, width, height)),
            zwlr_screencopy_frame_v1::Event::BufferDone => state.buffer_done = true,
            zwlr_screencopy_frame_v1::Event::Ready { .. } => state.ready = true,
            zwlr_screencopy_frame_v1::Event::Failed => state.failed = true,
            _ => {}
        }
    }
}

impl Dispatch<ZwpLinuxBufferParamsV1, ()> for Client {
    fn event(
        state: &mut Self,
        _params: &ZwpLinuxBufferParamsV1,
        event: zwp_linux_buffer_params_v1::Event,
        _data: &(),
        _conn: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        if matches!(event, zwp_linux_buffer_params_v1::Event::Failed) {
            state.failed = true;
        }
    }
}

delegate_noop!(Client: ignore WlOutput);
delegate_noop!(Client: ignore WlBuffer);
delegate_noop!(Client: ignore ZwpLinuxDmabufV1);
delegate_noop!(Client: ignore ZwlrScreencopyManagerV1);

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let output_path = arguments.next().context("usage: <dump.raw> [x y w h]")?;
    let region: Vec<i32> = arguments.filter_map(|a| a.parse().ok()).collect();

    let connection = Connection::connect_to_env().context("no wayland display")?;
    let mut queue = connection.new_event_queue();
    let handle = queue.handle();
    let _registry = connection.display().get_registry(&handle, ());

    let mut client = Client::default();
    queue.roundtrip(&mut client).context("registry roundtrip")?;
    let output = client.output.clone().context("no wl_output")?;
    let dmabuf = client.dmabuf.clone().context("no zwp_linux_dmabuf_v1")?;
    let manager = client
        .manager
        .clone()
        .context("no zwlr_screencopy_manager_v1")?;

    let frame = match region.as_slice() {
        [x, y, width, height] => {
            manager.capture_output_region(0, &output, *x, *y, *width, *height, &handle, ())
        }
        _ => manager.capture_output(0, &output, &handle, ()),
    };

    pump(&mut queue, &mut client, |c| c.buffer_done || c.failed)?;
    if client.failed {
        bail!("dome refused the capture before a buffer was attached");
    }
    let (format, width, height) = client
        .offered_dmabuf
        .context("dome sent no linux_dmabuf event, so it serves shared memory only")?;
    let fourcc =
        Fourcc::try_from(format).map_err(|_| anyhow::anyhow!("unknown format {format}"))?;
    println!("dome offers {fourcc:?} at {width}x{height} over dmabuf");

    let card = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/dri/card0")
        .context("open /dev/dri/card0")?;
    let gbm = GbmDevice::new(card).context("GbmDevice::new")?;
    let mut allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING);
    let buffer = allocator
        .create_buffer(width, height, fourcc, &[Modifier::Linear])
        .map_err(|e| anyhow::anyhow!("GBM allocation failed: {e}"))?;
    let exported = buffer
        .export()
        .map_err(|e| anyhow::anyhow!("export the GBM buffer: {e}"))?;
    let stride = exported.strides().next().context("no stride")?;
    let plane: OwnedFd = exported
        .handles()
        .next()
        .context("no plane")?
        .try_clone_to_owned()?;

    let parameters = dmabuf.create_params(&handle, ());
    parameters.add(plane.as_fd(), 0, 0, stride, 0, 0);
    let wl_buffer = parameters.create_immed(
        width as i32,
        height as i32,
        format,
        zwp_linux_buffer_params_v1::Flags::empty(),
        &handle,
        (),
    );
    frame.copy(&wl_buffer);

    pump(&mut queue, &mut client, |c| c.ready || c.failed)?;
    if client.failed || !client.ready {
        bail!("dome failed the dmabuf copy");
    }

    let length = stride as usize * height as usize;
    // SAFETY: mmap returns length readable bytes for the buffer the compositor
    // rendered into, and the fd outlives the mapping.
    let pixels = unsafe {
        let pointer = libc::mmap(
            std::ptr::null_mut(),
            length,
            libc::PROT_READ,
            libc::MAP_SHARED,
            plane.as_raw_fd(),
            0,
        );
        if pointer == libc::MAP_FAILED {
            bail!("mmap the dmabuf: {}", std::io::Error::last_os_error());
        }
        std::slice::from_raw_parts(pointer as *const u8, length)
    };

    // ImageMagick reads a raw dump with no padding, so drop the stride slack.
    let row_bytes = width as usize * 4;
    let mut file = std::fs::File::create(&output_path)?;
    for row in 0..height as usize {
        let start = row * stride as usize;
        file.write_all(&pixels[start..start + row_bytes])?;
    }
    // SAFETY: the pointer and length come from the mmap call above.
    unsafe { libc::munmap(pixels.as_ptr() as *mut libc::c_void, length) };

    println!("wrote {width}x{height} BGRX rows to {output_path}");
    Ok(())
}

fn pump(
    queue: &mut wayland_client::EventQueue<Client>,
    client: &mut Client,
    done: impl Fn(&Client) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + DEADLINE;
    while !done(client) {
        if Instant::now() > deadline {
            bail!("dome did not answer within {DEADLINE:?}");
        }
        queue.blocking_dispatch(client).context("dispatch")?;
    }
    Ok(())
}
