//! Completed specialized Drawable visuals at the explicit client→renderer boundary.
//!
//! C++ retains tread/laser/debris state on Draw modules. Main shares the completed
//! value across mesh inputs on its event-loop thread; no mesh queries live client
//! state, and a replacement frame cannot inherit an earlier world's ObjectIDs.

use super::*;
use crate::presentation_frame::{PresentationFrame, UnitRenderInput};
use game_client::core::PresentationSpecializedDrawSnapshot;
use std::rc::Weak;

pub(super) struct FrozenSpecializedDrawFrame {
    frame: Weak<PresentationFrame>,
    host_epoch: u64,
    snapshots: HashMap<ObjectID, Rc<PresentationSpecializedDrawSnapshot>>,
}

impl FrozenSpecializedDrawFrame {
    pub(super) fn capture<'a>(
        frame: &Rc<PresentationFrame>,
        host_epoch: u64,
        snapshots: impl IntoIterator<Item = (u32, &'a PresentationSpecializedDrawSnapshot)>,
    ) -> Self {
        let mut snapshots = snapshots.into_iter().peekable();
        if snapshots.peek().is_none() {
            return Self {
                frame: Rc::downgrade(frame),
                host_epoch,
                snapshots: HashMap::new(),
            };
        }
        // Ordinary frozen rows cover host/shadow fallback inputs. Resident
        // direct rows additionally retain C++ deferred death/rubble visuals.
        // Build this set once at capture, never once per object or mesh.
        let roster: HashSet<_> = frame
            .objects
            .iter()
            .map(|object| object.id)
            .chain(
                frame
                    .direct_host_drawables
                    .iter()
                    .filter(|direct| direct.resident)
                    .map(|direct| direct.object.id),
            )
            .collect();
        Self {
            frame: Rc::downgrade(frame),
            host_epoch,
            snapshots: snapshots
                .filter(|(id, _)| roster.contains(&ObjectID(*id)))
                .map(|(id, snapshot)| (ObjectID(id), Rc::new(snapshot.clone())))
                .collect(),
        }
    }

    /// The source closure is not evaluated for a completed exact-frame bundle.
    /// UI changes can replace an immutable frame while logic is paused; only
    /// that replacement requires recapture from the explicitly driving client.
    pub(super) fn ensure_for_frame<'a, I>(
        bundle: &mut Option<Self>,
        frame: &Rc<PresentationFrame>,
        host_epoch: u64,
        snapshots: impl FnOnce() -> I,
    ) -> bool
    where
        I: IntoIterator<Item = (u32, &'a PresentationSpecializedDrawSnapshot)>,
    {
        if bundle
            .as_ref()
            .is_some_and(|bundle| bundle.host_epoch == host_epoch && bundle.matches(frame))
        {
            return false;
        }
        *bundle = Some(Self::capture(frame, host_epoch, snapshots()));
        true
    }

    fn matches(&self, frame: &Rc<PresentationFrame>) -> bool {
        self.frame.ptr_eq(&Rc::downgrade(frame))
    }

    pub(super) fn retain_for_frame(
        bundle: &mut Option<Self>,
        frame: Option<&Rc<PresentationFrame>>,
    ) {
        if !bundle
            .as_ref()
            .is_some_and(|bundle| frame.is_some_and(|frame| bundle.matches(frame)))
        {
            *bundle = None;
        }
    }

    /// Join once per object before sorting/mesh traversal. The returned count is
    /// the exact number of map lookups, independent of the object's mesh count.
    pub(super) fn apply_to_inputs(
        &self,
        frame: &Rc<PresentationFrame>,
        inputs: &mut [UnitRenderInput],
    ) -> usize {
        if !self.matches(frame) {
            for input in inputs {
                input.specialized_draw = None;
            }
            return 0;
        }
        let lookups = inputs.len();
        for input in inputs {
            input.specialized_draw = self.snapshots.get(&input.id).cloned();
        }
        lookups
    }
}

#[cfg(test)]
#[path = "specialized_draw_inputs_tests.rs"]
mod specialized_draw_bundle_tests;
