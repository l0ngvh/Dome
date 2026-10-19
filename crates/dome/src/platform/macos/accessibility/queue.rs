use std::collections::VecDeque;

use anyhow::Result;
use dome_core::{FractionalCoordinate, Logical, PixelRect};
use futures::channel::oneshot;
use objc2_core_graphics::CGWindowID;

use crate::platform::macos::window_server::{NativeAppObservation, NativeFocusedWindow};

pub(super) enum ApplicationRequest {
    SetFrame(CGWindowID, PixelRect<Logical>),
    HideAt(
        CGWindowID,
        FractionalCoordinate<Logical>,
        FractionalCoordinate<Logical>,
    ),
    Minimize(CGWindowID),
    Unminimize(CGWindowID),
    Close(CGWindowID),
    Focus(CGWindowID),
    RefreshObserver,
    ForgetWindow(CGWindowID),
    ReadWindows(
        Vec<CGWindowID>,
        oneshot::Sender<Result<Option<NativeAppObservation>>>,
    ),
    ReadFocus(oneshot::Sender<Result<Option<NativeFocusedWindow>>>),
    ReadTitle(CGWindowID, oneshot::Sender<Result<Option<String>>>),
}

pub(super) fn enqueue_request(
    pending: &mut VecDeque<ApplicationRequest>,
    incoming: ApplicationRequest,
) {
    if matches!((&incoming, pending.back()),
        (ApplicationRequest::Close(a), Some(ApplicationRequest::Close(b)))
        | (ApplicationRequest::Unminimize(a), Some(ApplicationRequest::Unminimize(b))) if a == b)
    {
        return;
    }
    for index in (0..pending.len()).rev() {
        let previous = &pending[index];
        if previous.is_boundary() {
            break;
        }
        if incoming.replaces(previous) {
            if incoming.is_read() {
                pending.remove(index);
                pending.push_back(incoming);
            } else {
                pending[index] = incoming;
            }
            return;
        }
    }
    pending.push_back(incoming);
}

impl ApplicationRequest {
    fn is_boundary(&self) -> bool {
        matches!(
            self,
            Self::Close(_) | Self::Unminimize(_) | Self::RefreshObserver
        )
    }

    fn is_read(&self) -> bool {
        matches!(
            self,
            Self::ReadWindows(..) | Self::ReadFocus(..) | Self::ReadTitle(..)
        )
    }

    fn setter_window(&self) -> Option<CGWindowID> {
        match self {
            Self::SetFrame(id, _) | Self::HideAt(id, ..) | Self::Minimize(id) => Some(*id),
            _ => None,
        }
    }

    fn replaces(&self, previous: &Self) -> bool {
        if let Some(id) = self.setter_window() {
            return previous.setter_window() == Some(id);
        }
        match (self, previous) {
            (Self::Focus(_), Self::Focus(_))
            | (Self::ReadWindows(..), Self::ReadWindows(..))
            | (Self::ReadFocus(_), Self::ReadFocus(_)) => true,
            (Self::ReadTitle(a, _), Self::ReadTitle(b, _)) => a == b,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use dome_core::{Logical, PixelRect};
    use futures::channel::oneshot;

    use super::{ApplicationRequest, enqueue_request};

    const W: u32 = 11;
    const X: u32 = 22;

    #[test]
    fn replace_setter_keeps_position_before_read() {
        let mut pending = VecDeque::new();
        let a = PixelRect::<Logical>::new(10, 20, 300, 200);
        let b = PixelRect::<Logical>::new(40, 50, 320, 240);
        let (reply, _receiver) = oneshot::channel();
        enqueue_request(&mut pending, ApplicationRequest::SetFrame(W, a));
        enqueue_request(
            &mut pending,
            ApplicationRequest::ReadWindows(vec![W], reply),
        );
        enqueue_request(&mut pending, ApplicationRequest::SetFrame(W, b));
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::SetFrame(W, rect)) if rect == b)
        );
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::ReadWindows(ids, _)) if ids == vec![W])
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn replace_read_moves_to_tail_with_only_new_arguments() {
        let mut pending = VecDeque::new();
        let a = PixelRect::<Logical>::new(10, 20, 300, 200);
        let (first, mut first_receiver) = oneshot::channel();
        let (second, _second_receiver) = oneshot::channel();
        enqueue_request(
            &mut pending,
            ApplicationRequest::ReadWindows(vec![W], first),
        );
        enqueue_request(&mut pending, ApplicationRequest::SetFrame(W, a));
        enqueue_request(
            &mut pending,
            ApplicationRequest::ReadWindows(vec![X], second),
        );
        assert!(first_receiver.try_recv().is_err());
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::SetFrame(W, rect)) if rect == a)
        );
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::ReadWindows(ids, _)) if ids == vec![X])
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn replace_focus_in_place_but_read_focus_at_tail() {
        let mut pending = VecDeque::new();
        let (first, mut cancelled) = oneshot::channel();
        let (latest, _receiver) = oneshot::channel();
        enqueue_request(&mut pending, ApplicationRequest::Focus(W));
        enqueue_request(&mut pending, ApplicationRequest::ReadFocus(first));
        enqueue_request(&mut pending, ApplicationRequest::Minimize(W));
        enqueue_request(&mut pending, ApplicationRequest::Focus(X));
        enqueue_request(&mut pending, ApplicationRequest::ReadFocus(latest));
        assert!(cancelled.try_recv().is_err());
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Focus(X))
        ));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Minimize(W))
        ));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::ReadFocus(_))
        ));
        assert!(pending.is_empty());
    }

    #[test]
    fn replace_title_read_at_tail_preserves_other_window_read() {
        let mut pending = VecDeque::new();
        let (first, mut cancelled) = oneshot::channel();
        let (other, mut other_receiver) = oneshot::channel();
        let (latest, mut latest_receiver) = oneshot::channel();
        enqueue_request(&mut pending, ApplicationRequest::ReadTitle(W, first));
        enqueue_request(&mut pending, ApplicationRequest::ReadTitle(X, other));
        enqueue_request(&mut pending, ApplicationRequest::Minimize(W));
        enqueue_request(&mut pending, ApplicationRequest::ReadTitle(W, latest));
        assert!(cancelled.try_recv().is_err());
        let Some(ApplicationRequest::ReadTitle(X, reply)) = pending.pop_front() else {
            panic!("distinct window title read must survive");
        };
        reply.send(Ok(Some("Other".into()))).unwrap();
        assert_eq!(
            other_receiver
                .try_recv()
                .unwrap()
                .unwrap()
                .unwrap()
                .as_deref(),
            Some("Other")
        );
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Minimize(W))
        ));
        let Some(ApplicationRequest::ReadTitle(W, reply)) = pending.pop_front() else {
            panic!("replacement title read must follow the setter");
        };
        reply.send(Ok(Some("Latest".into()))).unwrap();
        assert_eq!(
            latest_receiver
                .try_recv()
                .unwrap()
                .unwrap()
                .unwrap()
                .as_deref(),
            Some("Latest")
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn set_frame_replaces_pending_minimize() {
        let mut pending = VecDeque::new();
        let b = PixelRect::<Logical>::new(40, 50, 320, 240);
        enqueue_request(&mut pending, ApplicationRequest::Minimize(W));
        enqueue_request(&mut pending, ApplicationRequest::SetFrame(W, b));
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::SetFrame(W, rect)) if rect == b)
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn replace_setter_after_other_window_unminimize_appends() {
        let mut pending = VecDeque::new();
        let a = PixelRect::<Logical>::new(10, 20, 300, 200);
        let b = PixelRect::<Logical>::new(40, 50, 320, 240);
        enqueue_request(&mut pending, ApplicationRequest::SetFrame(W, a));
        enqueue_request(&mut pending, ApplicationRequest::Unminimize(X));
        enqueue_request(&mut pending, ApplicationRequest::SetFrame(W, b));
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::SetFrame(W, rect)) if rect == a)
        );
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Unminimize(X))
        ));
        assert!(
            matches!(pending.pop_front(), Some(ApplicationRequest::SetFrame(W, rect)) if rect == b)
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn enqueue_consecutive_close_and_unminimize_collapses_duplicates() {
        let mut pending = VecDeque::new();
        enqueue_request(&mut pending, ApplicationRequest::Close(W));
        enqueue_request(&mut pending, ApplicationRequest::Close(W));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Close(W))
        ));
        assert!(pending.is_empty());
        enqueue_request(&mut pending, ApplicationRequest::Unminimize(W));
        enqueue_request(&mut pending, ApplicationRequest::Unminimize(W));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Unminimize(W))
        ));
        assert!(pending.is_empty());
    }

    #[test]
    fn enqueue_unminimize_after_minimize_keeps_both_unminimizes() {
        let mut pending = VecDeque::new();
        enqueue_request(&mut pending, ApplicationRequest::Unminimize(W));
        enqueue_request(&mut pending, ApplicationRequest::Minimize(W));
        enqueue_request(&mut pending, ApplicationRequest::Unminimize(W));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Unminimize(W))
        ));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Minimize(W))
        ));
        assert!(matches!(
            pending.pop_front(),
            Some(ApplicationRequest::Unminimize(W))
        ));
        assert!(pending.is_empty());
    }
}
