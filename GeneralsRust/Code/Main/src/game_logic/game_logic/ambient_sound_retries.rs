//! Presentation retry pacing owned by one Main GameLogic audio queue.
//! This transient state is neither cloned with Objects nor transferred by
//! Snapshot/Xfer. Reset and roster replacement discard only this owner's pace.
use super::ObjectId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct AmbientSoundRetries {
    objects: HashMap<ObjectId, AmbientRestartPacing>,
}

#[derive(Default)]
struct AmbientRestartPacing {
    name: String,
    last_attempt_frame: Option<u32>,
    was_playing: bool,
}

impl AmbientSoundRetries {
    pub(super) fn clear(&mut self) {
        self.objects.clear();
    }

    pub(super) fn retain_objects(&mut self, mut is_present: impl FnMut(ObjectId) -> bool) {
        self.objects.retain(|id, _| is_present(*id));
    }

    /// Called only when the owning queue has no duplicate start request.
    /// Audio queries stay lazy: a playing event never queries permanence.
    pub(super) fn should_restart(
        &mut self,
        id: ObjectId,
        name: &str,
        now: u32,
        retry_frames: u32,
        is_playing: impl FnOnce() -> bool,
        is_permanent: impl FnOnce() -> bool,
    ) -> bool {
        let state = self.objects.entry(id).or_default();
        if state.name != name {
            *state = AmbientRestartPacing {
                name: name.to_owned(),
                ..Default::default()
            };
        }
        if is_playing() {
            state.was_playing = true;
            return false;
        }
        if !is_permanent() {
            return false;
        }
        // Preserve Main's existing failed-attempt interval and immediate
        // restart when a previously playing permanent loop was culled.
        let backoff_elapsed = match state.last_attempt_frame {
            Some(last) => now.saturating_sub(last) >= retry_frames,
            None => true,
        };
        if !(state.was_playing || backoff_elapsed) {
            return false;
        }
        state.was_playing = false;
        state.last_attempt_frame = Some(now);
        true
    }
}
