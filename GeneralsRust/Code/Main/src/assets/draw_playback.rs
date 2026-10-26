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

/// Native render namespace. An auxiliary Drawable is never a gameplay Object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeDrawOwnerId {
    Actor(u32),
    #[cfg(feature = "game_client")]
    Aux(game_client::core::DrawableId),
}
impl NativeDrawOwnerId {
    pub(crate) fn actor_id(self) -> Option<u32> {
        match self {
            Self::Actor(id) => Some(id),
            #[cfg(feature = "game_client")]
            Self::Aux(_) => None,
        }
    }
}

/// Immutable identity captured with a presentation value. The token makes
/// equal ObjectIds in different GameLogic instances distinct without putting
/// renderer-mutated state in the Send-bound simulation object.
#[derive(Debug, Clone)]
pub(crate) struct LiveDrawPlaybackIdentity {
    world: std::sync::Arc<()>,
    object: std::sync::Arc<()>,
    world_epoch: u64,
    owner_id: NativeDrawOwnerId,
    object_generation: u64,
    native_pose: Option<(u64, glam::Mat4)>,
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
            owner_id: NativeDrawOwnerId::Actor(object_id),
            object_generation,
            native_pose: None,
        }
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn new_auxiliary(
        world: std::sync::Arc<()>,
        world_epoch: u64,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
    ) -> Result<Self, String> {
        Ok(Self {
            world,
            object: receipt
                .owner_token()
                .ok_or("retired auxiliary Drawable owner")?,
            world_epoch,
            owner_id: NativeDrawOwnerId::Aux(receipt.drawable_id()),
            object_generation: receipt.generation(),
            native_pose: None,
        })
    }

    /// A DrawableID/generation is meaningful only within its receiving client.
    /// Do not let a renderer caller substitute an equal-number peer receipt.
    #[cfg(feature = "game_client")]
    pub(crate) fn matches_auxiliary_receipt(
        &self,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
    ) -> bool {
        self.owner_id == NativeDrawOwnerId::Aux(receipt.drawable_id())
            && self.object_generation == receipt.generation()
            && receipt
                .owner_token()
                .is_some_and(|owner| std::sync::Arc::ptr_eq(&self.object, &owner))
    }

    /// Freeze actual setter-owned transform facts, independent of visual interpolation.
    pub(crate) fn with_native_pose(mut self, revision: u64, transform: glam::Mat4) -> Self {
        self.native_pose = Some((revision, transform));
        self
    }

    pub(crate) fn native_pose(&self) -> Option<(u64, glam::Mat4)> {
        self.native_pose
    }

    pub(crate) fn object_id(&self) -> u32 {
        self.owner_id
            .actor_id()
            .expect("actor-only playback access")
    }
    pub(crate) fn owner_id(&self) -> NativeDrawOwnerId {
        self.owner_id
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
            owner_id: self.owner_id,
            object_generation: self.object_generation,
            module_index,
        }
    }
}

impl PartialEq for LiveDrawPlaybackIdentity {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.world, &other.world)
            && std::sync::Arc::ptr_eq(&self.object, &other.object)
            && self.owner_id == other.owner_id
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
        self.owner_id.hash(state);
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
    owner_id: NativeDrawOwnerId,
    object_generation: u64,
    module_index: u32,
}

impl PartialEq for LiveDrawPlaybackKey {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.world, &other.world)
            && std::sync::Weak::ptr_eq(&self.object, &other.object)
            && self.world_epoch == other.world_epoch
            && self.owner_id == other.owner_id
            && self.object_generation == other.object_generation
            && self.module_index == other.module_index
    }
}
impl Eq for LiveDrawPlaybackKey {}
impl LiveDrawPlaybackKey {
    pub(crate) fn object_id(&self) -> u32 {
        self.owner_id
            .actor_id()
            .expect("actor-only playback access")
    }
    pub(crate) fn owner_id(&self) -> NativeDrawOwnerId {
        self.owner_id
    }
    pub(crate) fn is_alive(&self) -> bool {
        self.world.strong_count() != 0 && self.object.strong_count() != 0
    }
}
impl std::hash::Hash for LiveDrawPlaybackKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.world.as_ptr().hash(state);
        self.object.as_ptr().hash(state);
        self.world_epoch.hash(state);
        self.owner_id.hash(state);
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
        self.identity.object_id()
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

#[cfg(test)]
mod native_pose_identity_tests {
    use super::*;

    #[test]
    fn frozen_setter_facts_preserve_identity_and_do_not_alias_other_admissions() {
        let world = std::sync::Arc::new(());
        let object = std::sync::Arc::new(());
        let original = LiveDrawPlaybackIdentity::new(world.clone(), object.clone(), 1, 7, 2)
            .with_native_pose(10, glam::Mat4::IDENTITY);
        let moved = LiveDrawPlaybackIdentity::new(world.clone(), object.clone(), 1, 7, 2)
            .with_native_pose(11, glam::Mat4::from_translation(glam::Vec3::X));
        assert_eq!(original, moved);
        assert_ne!(original.native_pose(), moved.native_pose());
        assert_eq!(original.clone().native_pose(), original.native_pose());
        let peer = LiveDrawPlaybackIdentity::new(std::sync::Arc::new(()), object.clone(), 1, 7, 2)
            .with_native_pose(10, glam::Mat4::IDENTITY);
        let readmitted =
            LiveDrawPlaybackIdentity::new(world.clone(), std::sync::Arc::new(()), 1, 7, 2)
                .with_native_pose(0, glam::Mat4::IDENTITY);
        let reset = LiveDrawPlaybackIdentity::new(world, object, 2, 7, 2)
            .with_native_pose(0, glam::Mat4::IDENTITY);
        assert_ne!(original, peer);
        assert_ne!(original, readmitted);
        assert_ne!(original, reset);
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "draw_playback_aux_owner_tests.rs"]
mod auxiliary_owner_tests;
