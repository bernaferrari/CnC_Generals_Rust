//! Runtime W3DModelDraw transition playback identities.
//!
//! Authored INI records stay in `ini_parser`; mutable current/next state is
//! owned by `RenderPipeline` and these immutable identities cross the frozen
//! presentation boundary.

use std::hash::Hash;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LiveDrawPlayback {
    pub(crate) current_index: u32,
    pub(crate) next_index: Option<u32>,
    pub(crate) animation_complete: bool,
}

/// Immutable identity captured with a presentation value. The token makes
/// equal ObjectIds in different GameLogic instances distinct without putting
/// renderer-mutated state in the Send-bound simulation object.
#[derive(Debug, Clone)]
pub(crate) struct LiveDrawPlaybackIdentity {
    world: std::sync::Arc<()>,
    object: std::sync::Arc<()>,
    world_epoch: u64,
    object_id: u32,
    object_generation: u64,
}

impl LiveDrawPlaybackIdentity {
    pub(crate) fn new(
        world: std::sync::Arc<()>,
        object: std::sync::Arc<()>,
        world_epoch: u64,
        object_id: u32,
        object_generation: u64,
    ) -> Self {
        Self {
            world,
            object,
            world_epoch,
            object_id,
            object_generation,
        }
    }

    pub(crate) fn object_id(&self) -> u32 {
        self.object_id
    }
    pub(crate) fn world_epoch(&self) -> u64 {
        self.world_epoch
    }
    pub(crate) fn object_generation(&self) -> u64 {
        self.object_generation
    }

    pub(crate) fn completion_target(&self, module_index: u32) -> LiveDrawAnimationCompletionTarget {
        LiveDrawAnimationCompletionTarget {
            identity: self.clone(),
            module_index,
        }
    }

    pub(crate) fn playback_key(&self, module_index: u32) -> LiveDrawPlaybackKey {
        LiveDrawPlaybackKey {
            world: std::sync::Arc::downgrade(&self.world),
            object: std::sync::Arc::downgrade(&self.object),
            world_epoch: self.world_epoch,
            object_id: self.object_id,
            object_generation: self.object_generation,
            module_index,
        }
    }
}

impl PartialEq for LiveDrawPlaybackIdentity {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.world, &other.world)
            && std::sync::Arc::ptr_eq(&self.object, &other.object)
            && self.object_id == other.object_id
            && self.world_epoch == other.world_epoch
            && self.object_generation == other.object_generation
    }
}

impl Eq for LiveDrawPlaybackIdentity {}

impl std::hash::Hash for LiveDrawPlaybackIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::sync::Arc::as_ptr(&self.world).hash(state);
        std::sync::Arc::as_ptr(&self.object).hash(state);
        self.world_epoch.hash(state);
        self.object_id.hash(state);
        self.object_generation.hash(state);
    }
}

/// RenderPipeline key; Weak preserves allocation identity without keeping a
/// retired world's token alive after all frozen frames have been dropped.
#[derive(Debug, Clone)]
pub(crate) struct LiveDrawPlaybackKey {
    world: std::sync::Weak<()>,
    object: std::sync::Weak<()>,
    world_epoch: u64,
    object_id: u32,
    object_generation: u64,
    module_index: u32,
}

impl PartialEq for LiveDrawPlaybackKey {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.world, &other.world)
            && std::sync::Weak::ptr_eq(&self.object, &other.object)
            && self.world_epoch == other.world_epoch
            && self.object_id == other.object_id
            && self.object_generation == other.object_generation
            && self.module_index == other.module_index
    }
}
impl Eq for LiveDrawPlaybackKey {}
impl LiveDrawPlaybackKey {
    pub(crate) fn is_alive(&self) -> bool {
        self.world.strong_count() != 0 && self.object.strong_count() != 0
    }
}
impl std::hash::Hash for LiveDrawPlaybackKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.world.as_ptr().hash(state);
        self.object.as_ptr().hash(state);
        self.world_epoch.hash(state);
        self.object_id.hash(state);
        self.object_generation.hash(state);
        self.module_index.hash(state);
    }
}

/// Explicit, generation-safe callback target frozen with one render input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveDrawAnimationCompletionTarget {
    identity: LiveDrawPlaybackIdentity,
    module_index: u32,
}

impl LiveDrawAnimationCompletionTarget {
    pub(crate) fn object_id(&self) -> u32 {
        self.identity.object_id
    }

    pub(crate) fn world_epoch(&self) -> u64 {
        self.identity.world_epoch
    }

    pub(crate) fn object_generation(&self) -> u64 {
        self.identity.object_generation
    }

    pub(crate) fn module_index(&self) -> u32 {
        self.module_index
    }

    pub(crate) fn playback_key(&self) -> LiveDrawPlaybackKey {
        self.identity.playback_key(self.module_index)
    }
}
