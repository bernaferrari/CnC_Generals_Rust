//! Shadow final-death deliveries belong to the driving GameWorld.
//! These are transient Rust coupling messages, not original C++ Xfer fields.
//! Preserve independent channel insertion order and the session's
//! Jet -> Helicopter -> Slow consumer order. Reset/clear/drop discard delivery.
use super::GameWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowDeathCompletionKind {
    Jet,
    Helicopter,
    Slow,
}

#[derive(Debug, Default)]
pub(super) struct ShadowDeathCompletions {
    jet: Vec<u32>,
    helicopter: Vec<u32>,
    slow: Vec<u32>,
}

impl ShadowDeathCompletions {
    fn channel_mut(&mut self, kind: ShadowDeathCompletionKind) -> &mut Vec<u32> {
        match kind {
            ShadowDeathCompletionKind::Jet => &mut self.jet,
            ShadowDeathCompletionKind::Helicopter => &mut self.helicopter,
            ShadowDeathCompletionKind::Slow => &mut self.slow,
        }
    }
}

impl GameWorld {
    /// Queue the mapped Main host ID after an ordinary final timer transition.
    /// No ambient world selection is involved; callers borrow the producer world.
    pub fn record_shadow_death_completion(&mut self, kind: ShadowDeathCompletionKind, host_id: u32) {
        self.shadow_death_completions.channel_mut(kind).push(host_id);
    }

    /// Transfer one channel to this world's post-host consumer, retaining order.
    pub fn take_shadow_death_completions(&mut self, kind: ShadowDeathCompletionKind) -> Vec<u32> {
        std::mem::take(self.shadow_death_completions.channel_mut(kind))
    }

    /// Discard transient deliveries on an authority or world boundary.
    pub fn clear_shadow_death_completions(&mut self) {
        self.shadow_death_completions = ShadowDeathCompletions::default();
    }
}
