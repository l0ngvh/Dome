#![cfg(target_os = "linux")]

use std::{
    io::{Error, ErrorKind},
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    sync::atomic::{AtomicU32, Ordering},
    thread::JoinHandle,
};

use calloop::channel::{Sender, channel};

use dome::WlcsEvent;

use wayland_sys::{
    client::{wayland_client_handle, wl_display, wl_proxy},
    common::{wl_fixed_t, wl_fixed_to_double},
    ffi_dispatch,
};
use wlcs::{
    Wlcs, extension_list,
    ffi_display_server_api::{
        WlcsExtensionDescriptor, WlcsIntegrationDescriptor, WlcsServerIntegration,
    },
    ffi_wrappers::wlcs_server,
    wlcs_server_integration,
};

wlcs_server_integration!(DomeDisplayServerHandle);

static SUPPORTED_EXTENSIONS: &[WlcsExtensionDescriptor] = extension_list!(
    ("wl_compositor", 4),
    ("wl_subcompositor", 1),
    ("wl_data_device_manager", 3),
    ("wl_seat", 7),
    ("wl_output", 4),
    ("xdg_wm_base", 3),
);

static DESCRIPTOR: WlcsIntegrationDescriptor = WlcsIntegrationDescriptor {
    version: 1,
    num_extensions: SUPPORTED_EXTENSIONS.len(),
    supported_extensions: SUPPORTED_EXTENSIONS.as_ptr(),
};

static DEVICE_ID: AtomicU32 = AtomicU32::new(0);

struct DomeDisplayServerHandle {
    server: Option<(Sender<WlcsEvent>, JoinHandle<()>)>,
}

impl Wlcs for DomeDisplayServerHandle {
    type Pointer = PointerHandle;
    type Touch = TouchHandle;

    fn new() -> Self {
        DomeDisplayServerHandle { server: None }
    }

    fn start(&mut self) {
        let (tx, rx) = channel();
        let join = std::thread::spawn(move || dome::wlcs_run(rx));
        self.server = Some((tx, join));
    }

    fn stop(&mut self) {
        if let Some((sender, join)) = self.server.take() {
            sender.send(WlcsEvent::Exit).ok();
            join.join().ok();
        }
    }

    fn create_client_socket(&self) -> std::io::Result<OwnedFd> {
        if let Some((ref sender, _)) = self.server {
            if let Ok((client_side, server_side)) = UnixStream::pair() {
                if let Err(e) = sender.send(WlcsEvent::NewClient {
                    stream: server_side,
                    client_id: client_side.as_raw_fd(),
                }) {
                    return Err(Error::new(ErrorKind::ConnectionReset, e));
                }
                return Ok(client_side.into());
            }
        }
        Err(Error::from(ErrorKind::NotFound))
    }

    fn position_window_absolute(
        &self,
        display: *mut wl_display,
        surface: *mut wl_proxy,
        x: i32,
        y: i32,
    ) {
        let client_id =
            unsafe { ffi_dispatch!(wayland_client_handle(), wl_display_get_fd, display) };
        let surface_id =
            unsafe { ffi_dispatch!(wayland_client_handle(), wl_proxy_get_id, surface) };
        if let Some((ref sender, _)) = self.server {
            sender
                .send(WlcsEvent::PositionWindow {
                    client_id,
                    surface_id,
                    location: (x, y).into(),
                })
                .ok();
        }
    }

    fn create_pointer(&mut self) -> Option<Self::Pointer> {
        let Some(ref server) = self.server else {
            return None;
        };
        Some(PointerHandle {
            device_id: DEVICE_ID.fetch_add(1, Ordering::Relaxed),
            sender: server.0.clone(),
        })
    }

    fn create_touch(&mut self) -> Option<Self::Touch> {
        let Some(ref server) = self.server else {
            return None;
        };
        Some(TouchHandle {
            device_id: DEVICE_ID.fetch_add(1, Ordering::Relaxed),
            sender: server.0.clone(),
        })
    }

    fn get_descriptor(&self) -> &WlcsIntegrationDescriptor {
        &crate::DESCRIPTOR
    }
}

struct PointerHandle {
    device_id: u32,
    sender: Sender<WlcsEvent>,
}

impl wlcs::Pointer for PointerHandle {
    fn move_absolute(&mut self, x: wl_fixed_t, y: wl_fixed_t) {
        self.sender
            .send(WlcsEvent::PointerMoveAbsolute {
                device_id: self.device_id,
                location: (wl_fixed_to_double(x), wl_fixed_to_double(y)).into(),
            })
            .ok();
    }

    fn move_relative(&mut self, dx: wl_fixed_t, dy: wl_fixed_t) {
        self.sender
            .send(WlcsEvent::PointerMoveRelative {
                device_id: self.device_id,
                delta: (wl_fixed_to_double(dx), wl_fixed_to_double(dy)).into(),
            })
            .ok();
    }

    fn button_up(&mut self, button: i32) {
        self.sender
            .send(WlcsEvent::PointerButtonUp {
                device_id: self.device_id,
                button_id: button,
            })
            .ok();
    }

    fn button_down(&mut self, button: i32) {
        self.sender
            .send(WlcsEvent::PointerButtonDown {
                device_id: self.device_id,
                button_id: button,
            })
            .ok();
    }

    fn destroy(&mut self) {}
}

struct TouchHandle {
    device_id: u32,
    sender: Sender<WlcsEvent>,
}

impl wlcs::Touch for TouchHandle {
    fn touch_down(&mut self, x: wl_fixed_t, y: wl_fixed_t) {
        self.sender
            .send(WlcsEvent::TouchDown {
                device_id: self.device_id,
                location: (wl_fixed_to_double(x), wl_fixed_to_double(y)).into(),
            })
            .ok();
    }

    fn touch_move(&mut self, x: wl_fixed_t, y: wl_fixed_t) {
        self.sender
            .send(WlcsEvent::TouchMove {
                device_id: self.device_id,
                location: (wl_fixed_to_double(x), wl_fixed_to_double(y)).into(),
            })
            .ok();
    }

    fn touch_up(&mut self) {
        self.sender
            .send(WlcsEvent::TouchUp {
                device_id: self.device_id,
            })
            .ok();
    }

    fn destroy(&mut self) {}
}
