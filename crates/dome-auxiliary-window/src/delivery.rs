#[cfg(all(test, target_os = "macos"))]
#[path = "wake_tests.rs"]
mod tests;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use crate::{AppHandler, MenuEntry, Shell};

pub(crate) enum Callback<W> {
    Started,
    Wake(W),
    Menu(Box<dyn FnOnce(Vec<MenuEntry>)>),
    Selected(u32),
    DisplayChanged,
    #[cfg(target_os = "windows")]
    WorkAreaChanged,
}

pub(crate) trait Callbacks {
    fn started(&self, shell: Shell);
    fn wake(&self);
    fn menu(&self, apply: Box<dyn FnOnce(Vec<MenuEntry>)>);
    fn selected(&self, id: u32);
    fn display_changed(&self);
    #[cfg(target_os = "windows")]
    fn work_area_changed(&self);
    fn stopping(&self);
    #[cfg(target_os = "macos")]
    fn active(&self) -> bool;
}

pub(crate) struct WakeDelivery<W> {
    receiver: RefCell<Option<Receiver<W>>>,
    handler: RefCell<Box<dyn AppHandler<Wake = W>>>,
    shell: RefCell<Option<Rc<Shell>>>,
    pending: RefCell<VecDeque<Callback<W>>>,
    active: Cell<bool>,
    ready: Cell<bool>,
    closed: Cell<bool>,
    stopping: Cell<bool>,
    stopped: Cell<bool>,
}

impl<W: Clone + Eq + Hash + Send + 'static> WakeDelivery<W> {
    pub(crate) fn new(receiver: Receiver<W>, handler: Box<dyn AppHandler<Wake = W>>) -> Self {
        Self {
            receiver: RefCell::new(Some(receiver)),
            handler: RefCell::new(handler),
            shell: RefCell::new(None),
            pending: RefCell::new(VecDeque::new()),
            active: Cell::new(false),
            ready: Cell::new(false),
            closed: Cell::new(false),
            stopping: Cell::new(false),
            stopped: Cell::new(false),
        }
    }

    pub(crate) fn close(&self) {
        self.closed.set(true);
        let receiver = self.receiver.borrow_mut().take();
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        // User-owned values may run destructors that call back into the loop.
        drop(receiver);
        drop(pending);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.get()
    }

    pub(crate) fn enqueue(&self, callback: Callback<W>) {
        if !self.closed.get() {
            self.pending.borrow_mut().push_back(callback);
            self.drain();
        }
    }

    fn drain(&self) {
        if self.active.replace(true) {
            return;
        }
        struct Access<'a>(&'a Cell<bool>);
        impl Drop for Access<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _access = Access(&self.active);
        loop {
            if self.stopping.get() && !self.stopped.replace(true) {
                self.handler.borrow_mut().on_stopping();
            }
            if self.closed.get() {
                return;
            }
            let callback = self.pending.borrow_mut().pop_front();
            let Some(callback) = callback else { return };
            let shell = self.shell.borrow().clone();
            match callback {
                Callback::Started => {
                    self.handler
                        .borrow_mut()
                        .on_started(shell.as_ref().unwrap());
                    self.ready.set(true);
                    self.receive();
                }
                Callback::Wake(wake) => {
                    self.handler
                        .borrow_mut()
                        .on_wake(shell.as_ref().unwrap(), wake);
                }
                Callback::Menu(apply) => {
                    let entries = self.handler.borrow_mut().menu();
                    if !self.closed.get() {
                        self.active.set(false);
                        apply(entries);
                        self.active.set(true);
                    }
                }
                Callback::Selected(id) => self.handler.borrow_mut().on_menu_selected(id),
                Callback::DisplayChanged => self.handler.borrow_mut().on_display_changed(),
                #[cfg(target_os = "windows")]
                Callback::WorkAreaChanged => self.handler.borrow_mut().on_work_area_changed(),
            }
        }
    }

    fn receive(&self) {
        if !self.ready.get() || self.closed.get() {
            return;
        }
        loop {
            let wake = self
                .receiver
                .borrow()
                .as_ref()
                .and_then(|receiver| receiver.try_recv().ok());
            let Some(wake) = wake else { break };
            self.pending.borrow_mut().push_back(Callback::Wake(wake));
        }
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> Callbacks for WakeDelivery<W> {
    fn started(&self, shell: Shell) {
        *self.shell.borrow_mut() = Some(Rc::new(shell));
        self.enqueue(Callback::Started);
    }

    fn wake(&self) {
        self.receive();
        self.drain();
    }

    fn menu(&self, apply: Box<dyn FnOnce(Vec<MenuEntry>)>) {
        self.enqueue(Callback::Menu(apply));
    }

    fn selected(&self, id: u32) {
        self.enqueue(Callback::Selected(id));
    }

    fn display_changed(&self) {
        self.enqueue(Callback::DisplayChanged);
    }

    #[cfg(target_os = "windows")]
    fn work_area_changed(&self) {
        self.enqueue(Callback::WorkAreaChanged);
    }

    fn stopping(&self) {
        self.close();
        self.stopping.set(true);
        self.drain();
    }

    #[cfg(target_os = "macos")]
    fn active(&self) -> bool {
        self.active.get()
    }
}
