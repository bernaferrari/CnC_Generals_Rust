//! CPU native map state owned by one detached admission operation.
use super::*;

#[derive(Default)]
pub(crate) struct NativeMapCandidate {
    #[cfg(feature = "game_client")]
    visual: Option<game_client::terrain::terrain_visual::TerrainVisualImpl>,
}
impl NativeMapCandidate {
    pub(super) fn receiver(&mut self) -> Option<&mut dyn crate::game_logic::NativeMapAdmission> {
        #[cfg(feature = "game_client")]
        {
            Some(self)
        }
        #[cfg(not(feature = "game_client"))]
        {
            None
        }
    }
    #[cfg(all(test, feature = "game_client"))]
    pub(super) fn raw_height_for_test(&self, x: i32, y: i32) -> Option<i32> {
        self.visual
            .as_ref()
            .map(|visual| visual.get_raw_map_height(x, y))
    }
    pub(super) fn install(self, pipeline: &mut RenderPipeline) {
        #[cfg(feature = "game_client")]
        pipeline.install_native_terrain_cpu(self.visual);
        #[cfg(not(feature = "game_client"))]
        let _ = pipeline;
    }
}
#[cfg(feature = "game_client")]
impl crate::game_logic::NativeMapAdmission for NativeMapCandidate {
    fn prepare(
        &mut self,
        world: &crate::game_logic::GameLogic,
        metadata: Option<&crate::game_logic::script_loader::MapMetadata>,
    ) -> Result<(), String> {
        let terrain = world
            .world_services
            .terrain()
            .read()
            .map_err(|_| "receiving map terrain poisoned")?;
        let frozen = world.try_terrain_heightmap_snapshot_with_terrain_logic(&terrain)?;
        let samples = terrain.has_height_map().then(|| {
            let (width, height) = terrain.logic_height_map_extents();
            (width, height, terrain.logic_height_map_border_size())
        });
        let Some((min, max)) =
            crate::game_logic::script_loader::resolved_map_height_bounds(metadata, samples)
        else {
            self.visual = None;
            return Ok(());
        };
        let env = crate::presentation_frame::PresentationWorldEnv {
            terrain_definitions: Some(std::sync::Arc::clone(world.effective_terrain_definitions())),
            world_min: min.to_array(),
            world_max: max.to_array(),
            runtime_heightmap: frozen.map(|map| {
                std::sync::Arc::new(
                    crate::presentation_frame::PresentationRuntimeHeightmap::from_height_map(&map),
                )
            }),
            heightmap_hint: metadata
                .and_then(|map| map.heightmap_path.as_ref())
                .map(|path| path.to_string_lossy().into_owned()),
            authored_scorches: metadata
                .into_iter()
                .flat_map(|map| &map.authored_scorches)
                .map(|&(location, radius, kind)| {
                    crate::presentation_frame::PresentationTerrainScorch {
                        location,
                        radius,
                        kind,
                    }
                })
                .collect(),
            ..Default::default()
        };
        let water = terrain.water_grid_state().clone();
        drop(terrain);
        let mut visual =
            RenderPipeline::prepare_native_terrain_cpu(&env).map_err(|error| error.to_string())?;
        if let Some(visual) = visual.as_mut() {
            visual.admit_map_water_configuration(&water);
        }
        self.visual = visual;
        Ok(())
    }
    fn admit_restored_heights(
        &mut self,
        world: &crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        let terrain = world
            .world_services
            .terrain()
            .read()
            .map_err(|_| "receiving restored terrain poisoned")?;
        if !terrain.has_height_map() && self.visual.is_none() {
            return Ok(());
        }
        let visual = self
            .visual
            .as_mut()
            .ok_or("restored world has no prepared native terrain owner")?;
        let (width, height) = terrain.logic_height_map_extents();
        visual.admit_canonical_height_grid(
            width,
            height,
            terrain.logic_height_map_border_size(),
            terrain.logic_height_map_bytes(),
        )
    }
    fn effects(&mut self) -> crate::game_logic::ScriptPresentationEffects<'_> {
        crate::game_logic::ScriptPresentationEffects {
            ui: None,
            view: None,
            terrain: self
                .visual
                .as_mut()
                .map(|visual| visual as &mut dyn crate::game_logic::NativeTerrainEffects),
        }
    }
}

#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;
    use crate::game_logic::NativeMapAdmission;

    #[test]
    fn detached_map_candidate_lends_only_its_native_effect_receiver() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "detached_map_candidate_lends_only_its_native_effect_receiver",
            || {
                let world = crate::game_logic::GameLogic::new();
                let mut map = gamelogic::system::map_loader::MapData::new();
                map.width = 8;
                map.height = 8;
                map.heightmap = vec![40; 64];
                world
                    .world_services
                    .terrain()
                    .write()
                    .unwrap()
                    .try_load_map_geometry(map)
                    .unwrap();
                let metadata = crate::game_logic::script_loader::MapMetadata {
                    authored_scorches: vec![([10., 20., 3.], 7., 1)],
                    ..Default::default()
                };
                let mut receiving = NativeMapCandidate::default();
                let mut peer = NativeMapCandidate::default();
                receiving.prepare(&world, Some(&metadata)).unwrap();
                peer.prepare(&world, Some(&metadata)).unwrap();
                // Exercise the same ephemeral receiver loan used by map admission.
                // This component test does not execute a factory callback.
                receiving
                    .effects()
                    .terrain
                    .as_deref_mut()
                    .unwrap()
                    .add_scorch([30., 40., 5.], 11., 2);
                assert_eq!(receiving.visual.as_ref().unwrap().scorch_marks().len(), 2);
                assert_eq!(peer.visual.as_ref().unwrap().scorch_marks().len(), 1);
                receiving
                    .effects()
                    .terrain
                    .as_deref_mut()
                    .unwrap()
                    .accepted_height(gamelogic::terrain::TerrainHeightChange {
                        playable_x: 1,
                        playable_y: 1,
                        sample_x: 1,
                        sample_y: 1,
                        raw_height: 255,
                        requested_height: -1,
                    });
                assert_eq!(
                    receiving.visual.as_ref().unwrap().get_raw_map_height(1, 1),
                    255
                );
                assert_eq!(peer.visual.as_ref().unwrap().get_raw_map_height(1, 1), 40);
                assert_eq!(
                    world
                        .world_services
                        .terrain()
                        .read()
                        .unwrap()
                        .logic_height_map_bytes()[9],
                    40
                );
                let mut saved = world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes()
                    .to_vec();
                saved[9] = 7;
                world
                    .world_services
                    .terrain()
                    .write()
                    .unwrap()
                    .try_apply_logic_height_map_bytes_with_effects(&saved, &mut || {})
                    .unwrap();
                receiving.admit_restored_heights(&world).unwrap();
                assert_eq!(
                    receiving.visual.as_ref().unwrap().get_raw_map_height(1, 1),
                    7
                );
                assert_eq!(
                    receiving.visual.as_ref().unwrap().scorch_marks().len(),
                    2,
                    "restored heights keep the same owner's prior factory effects"
                );
                assert_eq!(peer.visual.as_ref().unwrap().get_raw_map_height(1, 1), 40);
            },
        );
    }
}
