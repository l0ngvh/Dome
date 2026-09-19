use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use calloop::EventLoop;
use calloop::channel::{Channel, Event as ChannelEvent};

use smithay::backend::input::ButtonState;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::test::{DummyFramebuffer, DummyRenderer};
use smithay::input::pointer::{ButtonEvent, MotionEvent, RelativeMotionEvent};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::seat::WaylandFocus;

use crate::config::Config;

use super::CalloopData;
use super::focus::KeyboardFocusTarget;
use super::render::DomeRenderElement;
use super::state::{ClientState, DomeState};

const OUTPUT_NAME: &str = "dome-wlcs";

/// Event sent by WLCS to control the compositor. Defined inside dome because the
/// headless loop that consumes it touches `DomeState`'s crate-private internals.
#[derive(Debug)]
pub enum WlcsEvent {
    Exit,
    NewClient {
        stream: UnixStream,
        client_id: i32,
    },
    PositionWindow {
        client_id: i32,
        surface_id: u32,
        location: Point<i32, Logical>,
    },
    NewPointer {
        device_id: u32,
    },
    PointerMoveAbsolute {
        device_id: u32,
        location: Point<f64, Logical>,
    },
    PointerMoveRelative {
        device_id: u32,
        delta: Point<f64, Logical>,
    },
    PointerButtonDown {
        device_id: u32,
        button_id: i32,
    },
    PointerButtonUp {
        device_id: u32,
        button_id: i32,
    },
    PointerRemoved {
        device_id: u32,
    },
    NewTouch {
        device_id: u32,
    },
    TouchDown {
        device_id: u32,
        location: Point<f64, Logical>,
    },
    TouchMove {
        device_id: u32,
        location: Point<f64, Logical>,
    },
    TouchUp {
        device_id: u32,
    },
    TouchRemoved {
        device_id: u32,
    },
}

pub fn run(channel: Channel<WlcsEvent>) {
    let mut event_loop: EventLoop<'static, CalloopData> =
        EventLoop::try_new().expect("Failed to init the WLCS event loop");

    let state = DomeState::new(&mut event_loop, Config::default())
        .expect("Failed to init headless DomeState");
    let mut data = CalloopData { state };

    // Kept local so DomeState gains no backend-specific field.
    let mut clients: HashMap<i32, Client> = HashMap::new();
    event_loop
        .handle()
        .insert_source(channel, move |event, &mut (), data| match event {
            ChannelEvent::Msg(evt) => handle_event(evt, &mut data.state, &mut clients),
            ChannelEvent::Closed => handle_event(WlcsEvent::Exit, &mut data.state, &mut clients),
        })
        .expect("Failed to insert WLCS event source");

    let mode = Mode {
        size: (800, 600).into(),
        refresh: 60_000,
    };
    let output = Output::new(
        OUTPUT_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "WLCS".into(),
        },
    );
    let _global = output.create_global::<DomeState>(&data.state.display_handle);
    output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
    output.set_preferred(mode);
    data.state.space.map_output(&output, (0, 0));

    let mut renderer = DummyRenderer;
    let mut framebuffer = DummyFramebuffer;
    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    event_loop
        .run(Some(Duration::from_millis(16)), &mut data, move |data| {
            // Pixels do not matter for WLCS protocol tests. An empty element list
            // still drives the damage tracker and the frame-callback path.
            let elements: Vec<DomeRenderElement<DummyRenderer>> = Vec::new();
            damage_tracker
                .render_output(
                    &mut renderer,
                    &mut framebuffer,
                    0,
                    &elements,
                    [0.1, 0.1, 0.1, 1.0],
                )
                .expect("WLCS headless render failed");

            let now = data.state.start_time.elapsed();
            data.state.space.elements().for_each(|window| {
                window.send_frame(&output, now, Some(Duration::ZERO), |_, _| {
                    Some(output.clone())
                });
            });

            data.state.space.refresh();
            data.state.popups.cleanup();
            data.state.display_handle.flush_clients().ok();
        })
        .expect("WLCS event loop failed");
}

fn handle_event(event: WlcsEvent, state: &mut DomeState, clients: &mut HashMap<i32, Client>) {
    match event {
        WlcsEvent::Exit => state.loop_signal.stop(),
        WlcsEvent::NewClient { stream, client_id } => {
            let client = state
                .display_handle
                .insert_client(stream, Arc::new(ClientState::default()))
                .expect("Failed to insert WLCS client");
            clients.insert(client_id, client);
        }
        WlcsEvent::PositionWindow {
            client_id,
            surface_id,
            location,
        } => {
            let client = clients.get(&client_id);
            let toplevel = state.space.elements().find(|w| {
                if let Some(surface) = w.wl_surface() {
                    state.display_handle.get_client(surface.id()).ok().as_ref() == client
                        && surface.id().protocol_id() == surface_id
                } else {
                    false
                }
            });
            if let Some(toplevel) = toplevel.cloned() {
                state.space.map_element(toplevel, location, false);
            }
        }
        WlcsEvent::NewPointer { .. } => {}
        WlcsEvent::PointerMoveAbsolute { location, .. } => {
            let serial = SERIAL_COUNTER.next_serial();
            let under = state.surface_under(location);
            let time = state.start_time.elapsed().as_millis() as u32;
            let ptr = state.seat.get_pointer().unwrap();
            ptr.motion(
                state,
                under,
                &MotionEvent {
                    location,
                    serial,
                    time,
                },
            );
            ptr.frame(state);
        }
        WlcsEvent::PointerMoveRelative { delta, .. } => {
            let ptr = state.seat.get_pointer().unwrap();
            let location = ptr.current_location() + delta;
            let serial = SERIAL_COUNTER.next_serial();
            let under = state.surface_under(location);
            let time = state.start_time.elapsed().as_millis() as u32;
            let utime = state.start_time.elapsed().as_micros() as u64;
            ptr.motion(
                state,
                under.clone(),
                &MotionEvent {
                    location,
                    serial,
                    time,
                },
            );
            ptr.relative_motion(
                state,
                under,
                &RelativeMotionEvent {
                    delta,
                    delta_unaccel: delta,
                    utime,
                },
            );
            ptr.frame(state);
        }
        WlcsEvent::PointerButtonDown { button_id, .. } => {
            let serial = SERIAL_COUNTER.next_serial();
            let ptr = state.seat.get_pointer().unwrap();
            if !ptr.is_grabbed() {
                let under = state
                    .space
                    .element_under(ptr.current_location())
                    .map(|(w, _)| w.clone());
                if let Some(window) = under.as_ref() {
                    state.space.raise_element(window, true);
                }
                let focus = under.and_then(|w| {
                    w.toplevel()
                        .map(|t| KeyboardFocusTarget::Surface(t.wl_surface().clone()))
                });
                let keyboard = state.seat.get_keyboard().unwrap();
                keyboard.set_focus(state, focus, serial);
            }
            let time = state.start_time.elapsed().as_millis() as u32;
            ptr.button(
                state,
                &ButtonEvent {
                    button: button_id as u32,
                    state: ButtonState::Pressed,
                    serial,
                    time,
                },
            );
            ptr.frame(state);
        }
        WlcsEvent::PointerButtonUp { button_id, .. } => {
            let serial = SERIAL_COUNTER.next_serial();
            let time = state.start_time.elapsed().as_millis() as u32;
            let ptr = state.seat.get_pointer().unwrap();
            ptr.button(
                state,
                &ButtonEvent {
                    button: button_id as u32,
                    state: ButtonState::Released,
                    serial,
                    time,
                },
            );
            ptr.frame(state);
        }
        WlcsEvent::PointerRemoved { .. } => {}
        WlcsEvent::NewTouch { .. } => {}
        WlcsEvent::TouchDown { .. } => {}
        WlcsEvent::TouchMove { .. } => {}
        WlcsEvent::TouchUp { .. } => {}
        WlcsEvent::TouchRemoved { .. } => {}
    }
}
