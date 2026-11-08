use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Default)]
pub struct StrawBase<'a> {
    chain_to: Option<Rc<RefCell<dyn Straw<'a> + 'a>>>,
    chain_from: Option<Weak<RefCell<dyn Straw<'a> + 'a>>>,
}

impl<'a> StrawBase<'a> {
    pub fn new() -> Self {
        Self {
            chain_to: None,
            chain_from: None,
        }
    }

    pub fn chain_to(&self) -> Option<Rc<RefCell<dyn Straw<'a> + 'a>>> {
        self.chain_to.clone()
    }

    pub fn chain_from(&self) -> Option<Weak<RefCell<dyn Straw<'a> + 'a>>> {
        self.chain_from.clone()
    }

    pub fn set_chain_from(&mut self, from: Option<Weak<RefCell<dyn Straw<'a> + 'a>>>) {
        self.chain_from = from;
    }

    pub fn set_chain_to(&mut self, to: Option<Rc<RefCell<dyn Straw<'a> + 'a>>>) {
        self.chain_to = to;
    }
}

/// Straw interface - equivalent to C++ Straw.
pub trait Straw<'a> {
    fn base(&self) -> &StrawBase<'a>;
    fn base_mut(&mut self) -> &mut StrawBase<'a>;

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if let Some(next) = self.base().chain_to() {
            return next.borrow_mut().get(buffer);
        }
        0
    }
}

/// Connect a straw to fetch data from another straw.
pub fn get_from<'a>(
    to: &Rc<RefCell<dyn Straw<'a> + 'a>>,
    from: Option<Rc<RefCell<dyn Straw<'a> + 'a>>>,
) {
    let mut to_guard = to.borrow_mut();
    let current_from = to_guard.base().chain_to();
    if let Some(current) = current_from {
        current.borrow_mut().base_mut().set_chain_from(None);
    }

    if let Some(new_from) = from.clone() {
        if let Some(existing_from) = new_from.borrow().base().chain_from() {
            if let Some(existing) = existing_from.upgrade() {
                existing.borrow_mut().base_mut().set_chain_to(None);
            }
        }
        new_from
            .borrow_mut()
            .base_mut()
            .set_chain_from(Some(Rc::downgrade(to)));
    }

    to_guard.base_mut().set_chain_to(from);
}
