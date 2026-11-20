//! Per-render memo of values that are pure functions of one frozen frame.
//!
//! Blob shadows and prewarm share frozen draw-model selection. The mesh
//! collector takes the list once those readers finish. The memo only lives
//! between `arm` and `disarm` of one engine render, while the frame is frozen.
use super::UnitRenderInput;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug, Default)]
pub(crate) struct RenderMemo {
    armed: Cell<bool>,
    unit_inputs: RefCell<Option<Rc<Vec<UnitRenderInput>>>>,
}

impl RenderMemo {
    pub(crate) fn arm(&self) {
        self.armed.set(true);
        self.unit_inputs.borrow_mut().take();
    }

    pub(crate) fn disarm(&self) {
        self.armed.set(false);
        self.unit_inputs.borrow_mut().take();
    }

    pub(crate) fn is_armed(&self) -> bool {
        self.armed.get()
    }

    pub(crate) fn unit_inputs(
        &self,
        compute: impl FnOnce() -> Vec<UnitRenderInput>,
    ) -> Rc<Vec<UnitRenderInput>> {
        if let Some(cached) = self.unit_inputs.borrow().as_ref() {
            return Rc::clone(cached);
        }
        let computed = Rc::new(compute());
        *self.unit_inputs.borrow_mut() = Some(Rc::clone(&computed));
        computed
    }

    pub(crate) fn take_unit_inputs(&self) -> Option<Vec<UnitRenderInput>> {
        self.unit_inputs.borrow_mut().take().map(Rc::unwrap_or_clone)
    }
}

/// A copy of a frame starts unarmed and empty: the memo belongs to one frame.
impl Clone for RenderMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// The memo is derived data, never part of a frame's identity.
impl PartialEq for RenderMemo {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_takes_the_allocation_after_early_readers_finish() {
        let memo = RenderMemo::default();
        memo.arm();
        let early_read = memo.unit_inputs(|| Vec::with_capacity(4));
        let allocation = early_read.as_ptr();
        drop(early_read);

        let collected = memo.take_unit_inputs().unwrap();
        assert_eq!(collected.as_ptr(), allocation);
        assert!(memo.take_unit_inputs().is_none());
    }
}
