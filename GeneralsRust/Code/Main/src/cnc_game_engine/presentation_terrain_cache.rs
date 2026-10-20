//! Map-lifetime terrain payload cache for immutable presentation frames.
//!
//! C++ `W3DTerrainVisual` owns one `WorldHeightMap` while a map is loaded.
//! Keep the Rust presentation copy at the same engine-owned lifetime: a frame
//! may be cloned into the render pipeline without duplicating the full
//! height/blend payload or reading live GameLogic during rendering.

use super::*;
use std::sync::{Arc, RwLock, Weak};

#[derive(Debug, Default)]
pub(super) struct PresentationTerrainCache {
    revision: u64,
    cached_revision: Option<(u64, u64)>,
    terrain_owner: Option<Weak<RwLock<gamelogic::terrain::TerrainLogic>>>,
    runtime_heightmap: Option<Arc<crate::presentation_frame::PresentationRuntimeHeightmap>>,
}

impl PresentationTerrainCache {
    /// Mark the map terrain source as replaced, reset, or restored.
    pub(super) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.cached_revision = None;
        self.terrain_owner = None;
        self.runtime_heightmap = None;
    }

    /// Freeze the actual receiving bytes once per owner and height revision.
    /// Published frames retain their immutable prior payload after mutation.
    pub(super) fn runtime_heightmap_for_logic(
        &mut self,
        logic: &crate::game_logic::GameLogic,
    ) -> Option<Arc<crate::presentation_frame::PresentationRuntimeHeightmap>> {
        #[cfg(not(feature = "game_client"))]
        {
            // No presentation payload exists on this backend; do not acquire
            // a terrain loan merely to cache another None.
            let _ = logic;
            None
        }
        #[cfg(feature = "game_client")]
        {
            let owner = logic.world_services.terrain();
            // A Weak identifies the existing owner without keeping a destroyed
            // world alive or allowing a recycled allocation to match its ID.
            let identity = Arc::downgrade(owner);
            let same_owner = self
                .terrain_owner
                .as_ref()
                .is_some_and(|previous| Weak::ptr_eq(previous, &identity));
            let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
            let revision = (self.revision, terrain.height_revision());
            if !same_owner || self.cached_revision != Some(revision) {
                self.runtime_heightmap = logic
                    .terrain_heightmap_snapshot_with_terrain_logic(&terrain)
                    .map(|heightmap| {
                        let mut snapshot = crate::presentation_frame::PresentationRuntimeHeightmap::from_height_map(&heightmap);
                        snapshot.source_byte_heights = terrain.has_height_map();
                        Arc::new(snapshot)
                    });
                self.cached_revision = Some(revision);
                self.terrain_owner = Some(identity);
            }
            self.runtime_heightmap.clone()
        }
    }

    #[cfg(test)]
    fn revision(&self) -> u64 {
        self.revision
    }
}

impl CnCGameEngine {
    /// Return the revision-frozen terrain payload for the next presentation frame.
    pub(super) fn presentation_runtime_heightmap_for_frame(
        &mut self,
    ) -> Option<Arc<crate::presentation_frame::PresentationRuntimeHeightmap>> {
        let (cache, logic) = (&mut self.presentation_terrain_cache, &self.game_logic);
        cache.runtime_heightmap_for_logic(logic)
    }

    /// Invalidate only at an engine boundary which can replace terrain data.
    pub(super) fn invalidate_presentation_terrain_cache(&mut self) {
        self.presentation_terrain_cache.invalidate();
    }
}

#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;

    fn logic_with_terrain(heights: &[f32]) -> GameLogic {
        let mut logic = GameLogic::new();
        logic.override_world_size(10.0, 10.0);
        assert!(logic.restore_terrain_heights_from_grid(2, 2, heights));
        logic
    }

    #[test]
    fn cache_reuses_payload_for_same_terrain_revision() {
        let mut logic = logic_with_terrain(&[0.0, 0.0, 0.0, 10.0]);
        let mut cache = PresentationTerrainCache::default();

        let first = cache
            .runtime_heightmap_for_logic(&logic)
            .expect("terrain payload");
        let second = cache
            .runtime_heightmap_for_logic(&logic)
            .expect("same terrain payload");

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(cache.revision(), 0);
    }

    #[test]
    fn presentation_frame_clone_shares_cached_terrain_payload() {
        let mut logic = logic_with_terrain(&[0.0, 0.0, 0.0, 10.0]);
        let mut cache = PresentationTerrainCache::default();
        let shared = cache
            .runtime_heightmap_for_logic(&logic)
            .expect("terrain payload");

        let frame =
            crate::presentation_frame::PresentationFrame::build_for_engine_with_runtime_heightmap(
                &mut logic,
                0,
                None,
                Some(shared),
            );
        // RenderPipeline receives a cloned immutable frame, so this is the
        // exact ownership operation used by the presentation handoff.
        let pipeline_frame = frame.clone();
        let frame_payload = frame
            .world_env
            .runtime_heightmap
            .as_ref()
            .expect("frame terrain payload");
        let pipeline_payload = pipeline_frame
            .world_env
            .runtime_heightmap
            .as_ref()
            .expect("pipeline terrain payload");

        assert!(Arc::ptr_eq(frame_payload, pipeline_payload));
    }

    #[test]
    fn terrain_replacement_gets_new_payload_and_keeps_old_frame_sampling_frozen() {
        let mut logic = logic_with_terrain(&[0.0, 0.0, 0.0, 10.0]);
        let mut cache = PresentationTerrainCache::default();
        let first = cache
            .runtime_heightmap_for_logic(&logic)
            .expect("initial terrain payload");
        let first_env =
            crate::presentation_frame::PresentationWorldEnv::from_logic_with_runtime_heightmap(
                &logic,
                Some(first.clone()),
            );
        let first_sample = first_env
            .sample_gameplay_terrain_height(5.0, 5.0)
            .expect("initial frozen sample");

        assert!(logic.restore_terrain_heights_from_grid(2, 2, &[20.0, 20.0, 20.0, 20.0]));
        let previous_revision = cache.revision();
        cache.invalidate();
        assert_ne!(cache.revision(), previous_revision);

        let replacement = cache
            .runtime_heightmap_for_logic(&logic)
            .expect("replacement terrain payload");
        let replacement_env =
            crate::presentation_frame::PresentationWorldEnv::from_logic_with_runtime_heightmap(
                &logic,
                Some(replacement.clone()),
            );
        let replacement_sample = replacement_env
            .sample_gameplay_terrain_height(5.0, 5.0)
            .expect("replacement frozen sample");

        assert!(!Arc::ptr_eq(&first, &replacement));
        assert!(
            (first_sample - replacement_sample).abs() > 0.1,
            "old and replacement snapshots must expose their own terrain samples"
        );
        assert_eq!(
            first_env
                .sample_gameplay_terrain_height(5.0, 5.0)
                .expect("old frame stays usable"),
            first_sample
        );
    }

    #[test]
    fn engine_invalidates_only_at_map_replace_reset_and_full_restore_boundaries() {
        let authority = include_str!("host_authority.rs");
        let reset = &authority[authority
            .find("fn host_reset_game_logic(")
            .expect("reset boundary")..];
        let replace = &authority[authority
            .find("fn host_replace_game_logic(")
            .expect("replace boundary")..];
        assert!(
            reset[..reset.find("fn host_destroy_object(").expect("next method")]
                .contains("self.invalidate_presentation_terrain_cache()"),
            "reset discards its terrain payload"
        );
        assert!(
            replace[..replace
                .find("pub(super) fn host_save_game_authority(")
                .expect("next method")]
                .contains("self.invalidate_presentation_terrain_cache()"),
            "full GameLogic replacement covers staged save terrain restore and startup install"
        );

        let map_load = include_str!("ui_commands.rs");
        let map_load = &map_load[map_load
            .find("fn host_load_map_or_default(")
            .expect("map-load boundary")..];
        let loaded = map_load
            .find("let Some(loaded) = loaded else")
            .expect("successful map guard");
        let invalidation = map_load
            .find("self.invalidate_presentation_terrain_cache()")
            .expect("map terrain invalidation");
        assert!(
            loaded < invalidation,
            "failed map attempts must not manufacture a new presentation revision"
        );
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "presentation_terrain_revision_tests.rs"]
mod revision_tests;
