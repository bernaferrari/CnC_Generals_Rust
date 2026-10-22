//! Birth descriptions owned by the receiving UI session. Native playback owns
//! its current frame and position; a fresh snapshot never rewrites those values.
use crate::presentation_frame::PresentationWorldAnim;
use game_engine::common::ini::ini_animation::{Anim2DMode, Anim2DTemplate};
use std::collections::VecDeque;

#[derive(Debug)]
struct WorldAnimation {
    birth: PresentationWorldAnim,
    expire_frame: u32,
}

#[derive(Debug)]
pub(crate) struct WorldAnimations {
    entries: VecDeque<WorldAnimation>,
    next_identity: u64,
    visual_random: fastrand::Rng,
}
impl Default for WorldAnimations {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            next_identity: 1,
            visual_random: fastrand::Rng::with_seed(0),
        }
    }
}
impl WorldAnimations {
    pub(crate) fn admit(&mut self, mut birth: PresentationWorldAnim, template: &Anim2DTemplate) {
        let Some(expire_frame) = game_engine::common::world_animation::expiry_frame(
            birth.spawn_frame,
            birth.display_time_seconds,
        ) else {
            return;
        };
        let Some(next) = self.next_identity.checked_add(1) else {
            return;
        };
        birth.identity = self.next_identity;
        self.next_identity = next;
        let frames = template.get_num_frames();
        birth.initial_frame = if template.is_randomized_start_frame() && frames > 0 {
            self.visual_random.u16(0..frames)
        } else {
            match template.get_anim_mode() {
                Anim2DMode::OnceBackwards
                | Anim2DMode::LoopBackwards
                | Anim2DMode::PingPongBackwards => frames.saturating_sub(1),
                _ => 0,
            }
        };
        // InGameUI::addWorldAnimation prepends, including equal-frame births.
        self.entries.push_front(WorldAnimation {
            birth,
            expire_frame,
        });
    }
    /// Explicit preDraw admission, separate from read-only snapshot collection.
    pub(crate) fn advance(&mut self, frame: u32, paused: bool) {
        if !paused {
            self.entries.retain(|entry| frame < entry.expire_frame);
        }
    }
    pub(crate) fn births(&self) -> impl Iterator<Item = &PresentationWorldAnim> {
        self.entries.iter().map(|entry| &entry.birth)
    }
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.visual_random = fastrand::Rng::with_seed(0);
        // Preserve identity across resets of this owner; the native epoch also
        // separates replacement sessions that start their own identity stream.
    }
}
