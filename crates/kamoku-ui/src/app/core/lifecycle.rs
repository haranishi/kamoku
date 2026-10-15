// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, Event, EventTarget, KeyboardEvent};

type Cleanup = (u64, Box<dyn FnOnce()>);
type KeyHandler = Rc<dyn Fn(KeyboardEvent) -> bool>;
struct Listener {
    target: EventTarget,
    event: String,
    capture: bool,
    callback: Closure<dyn FnMut(Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            &self.event,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        );
    }
}
struct Inner {
    alive: Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
    cleanup: RefCell<Vec<Cleanup>>,
    next: Cell<u64>,
    pub key: RefCell<Option<KeyHandler>>,
    pub dirty: RefCell<Option<Rc<dyn Fn() -> bool>>>,
    stay: RefCell<Option<Rc<dyn Fn()>>>,
    pub allow: Cell<bool>,
}
#[derive(Clone)]
pub(crate) struct Scope(Rc<Inner>);
impl Scope {
    pub(crate) fn new() -> Self {
        Self(Rc::new(Inner {
            alive: Cell::new(true),
            listeners: RefCell::new(Vec::new()),
            cleanup: RefCell::new(Vec::new()),
            next: Cell::new(0),
            key: RefCell::new(None),
            dirty: RefCell::new(None),
            stay: RefCell::new(None),
            allow: Cell::new(false),
        }))
    }
    pub(crate) fn alive(&self) -> bool {
        self.0.alive.get()
    }
    pub(crate) fn on(&self, target: &Element, event: &str, f: impl Fn(Event) + 'static) {
        self.listen(target.as_ref(), event, false, f);
    }
    pub(crate) fn listen(
        &self,
        target: &EventTarget,
        event: &str,
        capture: bool,
        f: impl Fn(Event) + 'static,
    ) {
        let weak = Rc::downgrade(&self.0);
        let callback = Closure::wrap(Box::new(move |event: Event| {
            if weak.upgrade().is_some_and(|s| s.alive.get()) {
                f(event);
            }
        }) as Box<dyn FnMut(Event)>);
        target
            .add_event_listener_with_callback_and_bool(
                event,
                callback.as_ref().unchecked_ref(),
                capture,
            )
            .expect("browser event registration");
        self.0.listeners.borrow_mut().push(Listener {
            target: target.clone(),
            event: event.into(),
            capture,
            callback,
        });
    }
    pub(crate) fn cleanup(&self, f: impl FnOnce() + 'static) {
        self.register_cleanup(f);
    }
    pub(crate) fn register_cleanup(&self, f: impl FnOnce() + 'static) -> u64 {
        let id = self.0.next.get();
        self.0.next.set(id + 1);
        self.0.cleanup.borrow_mut().push((id, Box::new(f)));
        id
    }
    pub(crate) fn remove_cleanup(&self, id: u64) {
        self.0.cleanup.borrow_mut().retain(|(key, _)| *key != id);
    }
    pub(crate) fn dispose(&self) {
        if !self.0.alive.replace(false) {
            return;
        }
        self.0.listeners.borrow_mut().clear();
        self.0.key.borrow_mut().take();
        self.0.dirty.borrow_mut().take();
        self.0.stay.borrow_mut().take();
        let callbacks = std::mem::take(&mut *self.0.cleanup.borrow_mut());
        for (_, f) in callbacks {
            f();
        }
    }
    pub(crate) fn key(&self, f: impl Fn(KeyboardEvent) -> bool + 'static) {
        *self.0.key.borrow_mut() = Some(Rc::new(f));
    }
    pub(crate) fn dispatch(&self, event: KeyboardEvent) -> bool {
        let f = self.0.key.borrow().clone();
        f.is_some_and(|f| f(event))
    }
    pub(crate) fn dirty(&self, f: impl Fn() -> bool + 'static) {
        *self.0.dirty.borrow_mut() = Some(Rc::new(f));
    }
    pub(crate) fn is_dirty(&self) -> bool {
        !self.0.allow.get() && self.0.dirty.borrow().as_ref().is_some_and(|f| f())
    }
    pub(crate) fn allow_leave(&self) {
        self.0.allow.set(true);
    }
    pub(crate) fn on_stay(&self, callback: impl Fn() + 'static) {
        *self.0.stay.borrow_mut() = Some(Rc::new(callback));
    }
    pub(crate) fn stay(&self) {
        let callback = self.0.stay.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
}
