use std::borrow::Cow;

use smithay::backend::input::KeyState;
use smithay::desktop::PopupKind;
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
    GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
    GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
};
use smithay::input::touch::{
    DownEvent, OrientationEvent, ShapeEvent, TouchTarget, UpEvent,
    MotionEvent as TouchMotionEvent,
};
use smithay::input::Seat;
use smithay::reexports::wayland_server::backend::ObjectId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{IsAlive, Serial};
use smithay::wayland::seat::WaylandFocus;

use super::state::DomeState;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum KeyboardFocusTarget {
    Surface(WlSurface),
    Popup(PopupKind),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum PointerFocusTarget {
    Surface(WlSurface),
}

// --- IsAlive ---

impl IsAlive for KeyboardFocusTarget {
    fn alive(&self) -> bool {
        match self {
            Self::Surface(s) => s.alive(),
            Self::Popup(p) => p.alive(),
        }
    }
}

impl IsAlive for PointerFocusTarget {
    fn alive(&self) -> bool {
        match self {
            Self::Surface(s) => s.alive(),
        }
    }
}

// --- WaylandFocus ---

impl WaylandFocus for KeyboardFocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Self::Surface(s) => s.wl_surface(),
            Self::Popup(p) => Some(Cow::Borrowed(p.wl_surface())),
        }
    }
}

impl WaylandFocus for PointerFocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Self::Surface(s) => s.wl_surface(),
        }
    }

    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            Self::Surface(s) => s.same_client_as(object_id),
        }
    }
}

// --- From conversions ---

impl From<PopupKind> for KeyboardFocusTarget {
    fn from(p: PopupKind) -> Self {
        Self::Popup(p)
    }
}

impl From<WlSurface> for PointerFocusTarget {
    fn from(s: WlSurface) -> Self {
        Self::Surface(s)
    }
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    fn from(target: KeyboardFocusTarget) -> Self {
        match target {
            KeyboardFocusTarget::Surface(s) => Self::Surface(s),
            KeyboardFocusTarget::Popup(p) => Self::Surface(p.wl_surface().clone()),
        }
    }
}

// --- KeyboardTarget ---

impl KeyboardTarget<DomeState> for KeyboardFocusTarget {
    fn enter(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            Self::Surface(s) => KeyboardTarget::enter(s, seat, data, keys, serial),
            Self::Popup(p) => KeyboardTarget::enter(p.wl_surface(), seat, data, keys, serial),
        }
    }

    fn leave(&self, seat: &Seat<DomeState>, data: &mut DomeState, serial: Serial) {
        match self {
            Self::Surface(s) => KeyboardTarget::leave(s, seat, data, serial),
            Self::Popup(p) => KeyboardTarget::leave(p.wl_surface(), seat, data, serial),
        }
    }

    fn key(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            Self::Surface(s) => KeyboardTarget::key(s, seat, data, key, state, serial, time),
            Self::Popup(p) => {
                KeyboardTarget::key(p.wl_surface(), seat, data, key, state, serial, time)
            }
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            Self::Surface(s) => KeyboardTarget::modifiers(s, seat, data, modifiers, serial),
            Self::Popup(p) => {
                KeyboardTarget::modifiers(p.wl_surface(), seat, data, modifiers, serial)
            }
        }
    }
}

// --- PointerTarget ---

impl PointerTarget<DomeState> for PointerFocusTarget {
    fn enter(&self, seat: &Seat<DomeState>, data: &mut DomeState, event: &MotionEvent) {
        match self {
            Self::Surface(s) => PointerTarget::enter(s, seat, data, event),
        }
    }

    fn motion(&self, seat: &Seat<DomeState>, data: &mut DomeState, event: &MotionEvent) {
        match self {
            Self::Surface(s) => PointerTarget::motion(s, seat, data, event),
        }
    }

    fn relative_motion(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &RelativeMotionEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::relative_motion(s, seat, data, event),
        }
    }

    fn button(&self, seat: &Seat<DomeState>, data: &mut DomeState, event: &ButtonEvent) {
        match self {
            Self::Surface(s) => PointerTarget::button(s, seat, data, event),
        }
    }

    fn axis(&self, seat: &Seat<DomeState>, data: &mut DomeState, frame: AxisFrame) {
        match self {
            Self::Surface(s) => PointerTarget::axis(s, seat, data, frame),
        }
    }

    fn frame(&self, seat: &Seat<DomeState>, data: &mut DomeState) {
        match self {
            Self::Surface(s) => PointerTarget::frame(s, seat, data),
        }
    }

    fn leave(&self, seat: &Seat<DomeState>, data: &mut DomeState, serial: Serial, time: u32) {
        match self {
            Self::Surface(s) => PointerTarget::leave(s, seat, data, serial, time),
        }
    }

    fn gesture_swipe_begin(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GestureSwipeBeginEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_swipe_begin(s, seat, data, event),
        }
    }

    fn gesture_swipe_update(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GestureSwipeUpdateEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_swipe_update(s, seat, data, event),
        }
    }

    fn gesture_swipe_end(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GestureSwipeEndEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_swipe_end(s, seat, data, event),
        }
    }

    fn gesture_pinch_begin(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GesturePinchBeginEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_pinch_begin(s, seat, data, event),
        }
    }

    fn gesture_pinch_update(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GesturePinchUpdateEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_pinch_update(s, seat, data, event),
        }
    }

    fn gesture_pinch_end(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GesturePinchEndEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_pinch_end(s, seat, data, event),
        }
    }

    fn gesture_hold_begin(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GestureHoldBeginEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_hold_begin(s, seat, data, event),
        }
    }

    fn gesture_hold_end(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &GestureHoldEndEvent,
    ) {
        match self {
            Self::Surface(s) => PointerTarget::gesture_hold_end(s, seat, data, event),
        }
    }
}

// --- TouchTarget ---

impl TouchTarget<DomeState> for PointerFocusTarget {
    fn down(&self, seat: &Seat<DomeState>, data: &mut DomeState, event: &DownEvent, seq: Serial) {
        match self {
            Self::Surface(s) => TouchTarget::down(s, seat, data, event, seq),
        }
    }

    fn up(&self, seat: &Seat<DomeState>, data: &mut DomeState, event: &UpEvent, seq: Serial) {
        match self {
            Self::Surface(s) => TouchTarget::up(s, seat, data, event, seq),
        }
    }

    fn motion(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &TouchMotionEvent,
        seq: Serial,
    ) {
        match self {
            Self::Surface(s) => TouchTarget::motion(s, seat, data, event, seq),
        }
    }

    fn frame(&self, seat: &Seat<DomeState>, data: &mut DomeState, seq: Serial) {
        match self {
            Self::Surface(s) => TouchTarget::frame(s, seat, data, seq),
        }
    }

    fn cancel(&self, seat: &Seat<DomeState>, data: &mut DomeState, seq: Serial) {
        match self {
            Self::Surface(s) => TouchTarget::cancel(s, seat, data, seq),
        }
    }

    fn shape(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &ShapeEvent,
        seq: Serial,
    ) {
        match self {
            Self::Surface(s) => TouchTarget::shape(s, seat, data, event, seq),
        }
    }

    fn orientation(
        &self,
        seat: &Seat<DomeState>,
        data: &mut DomeState,
        event: &OrientationEvent,
        seq: Serial,
    ) {
        match self {
            Self::Surface(s) => TouchTarget::orientation(s, seat, data, event, seq),
        }
    }
}
