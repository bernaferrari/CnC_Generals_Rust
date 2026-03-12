use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Base pipe chain state shared by pipe implementations.
#[derive(Default)]
pub struct PipeBase {
    chain_to: Option<Rc<RefCell<dyn Pipe>>>,
    chain_from: Option<Weak<RefCell<dyn Pipe>>>,
}

impl PipeBase {
    pub fn new() -> Self {
        Self {
            chain_to: None,
            chain_from: None,
        }
    }

    pub fn chain_to(&self) -> Option<Rc<RefCell<dyn Pipe>>> {
        self.chain_to.clone()
    }

    pub fn chain_from(&self) -> Option<Weak<RefCell<dyn Pipe>>> {
        self.chain_from.clone()
    }

    pub fn set_chain_from(&mut self, from: Option<Weak<RefCell<dyn Pipe>>>) {
        self.chain_from = from;
    }

    pub fn set_chain_to(&mut self, to: Option<Rc<RefCell<dyn Pipe>>>) {
        self.chain_to = to;
    }
}

/// Pipe interface - equivalent to C++ Pipe.
pub trait Pipe {
    fn base(&self) -> &PipeBase;
    fn base_mut(&mut self) -> &mut PipeBase;

    fn put(&mut self, source: &[u8]) -> i32 {
        if let Some(next) = self.base().chain_to() {
            return next.borrow_mut().put(source);
        }
        source.len() as i32
    }

    fn flush(&mut self) -> i32 {
        if let Some(next) = self.base().chain_to() {
            return next.borrow_mut().flush();
        }
        0
    }

    fn end(&mut self) -> i32 {
        self.flush()
    }
}

/// Connect a pipe to flow data into from this pipe.
pub fn put_to(from: &Rc<RefCell<dyn Pipe>>, to: Option<Rc<RefCell<dyn Pipe>>>) {
    let mut from_guard = from.borrow_mut();
    let current_to = from_guard.base().chain_to();
    if let Some(current) = current_to {
        current.borrow_mut().base_mut().set_chain_from(None);
        current.borrow_mut().flush();
    }

    if let Some(new_to) = to.clone() {
        if let Some(existing_from) = new_to.borrow().base().chain_from() {
            if let Some(existing) = existing_from.upgrade() {
                existing.borrow_mut().base_mut().set_chain_to(None);
            }
        }
        new_to
            .borrow_mut()
            .base_mut()
            .set_chain_from(Some(Rc::downgrade(from)));
    }

    from_guard.base_mut().set_chain_to(to);
}
