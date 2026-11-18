//! Per-render memo of values that are pure functions of one frozen frame.
//!
//! `unit_render_inputs()` is rebuilt (draw-model selection per object) by every
//! render consumer: blob shadows, shadow volumes, prewarm and the mesh
//! collect. C++ computes a Drawable's model state once per draw. The memo is
//! only live between `arm` and `disarm` of one engine render, where the frame
//! cannot change; outside that window every call computes fresh, so UI
//! mutations (`Rc::make_mut` on the frame) can never observe a stale value.
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
