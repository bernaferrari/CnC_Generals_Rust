//! CPU native map state owned by one detached admission operation.
use super::*;

#[derive(Default)]
pub(crate) struct NativeMapCandidate {
    audio: super::native_audio_effects::PendingNativeAudio,
    #[cfg(feature = "game_client")]
    drawables: game_client::core::game_client::NativeDrawableWorld,
    draw: crate::graphics::render_pipeline::native_draw_world::NativeDrawWorld,
    // Allocated by the driving application before factory birth, never inferred
    // from a template/ObjectId and never rebased after callback observations.
    epoch: Option<u64>,
    #[cfg(feature = "game_client")]
    visual: Option<game_client::terrain::terrain_visual::TerrainVisualImpl>,
}
impl NativeMapCandidate {
    pub(super) fn with_epoch(epoch: u64) -> Self {
        assert_ne!(epoch, 0, "native candidate epoch must be nonzero");
        Self {
            epoch: Some(epoch),
            ..Self::default()
        }
    }
    pub(super) fn epoch(&self) -> Option<u64> {
        self.epoch
    }

    /// Terrain-only diagnostics borrow the actual prepared CPU visual without
    /// claiming the factory/Client/renderer capability of NativeMapAttempt.
    #[cfg(feature = "game_client")]
    pub(super) fn terrain_effects(
        &mut self,
    ) -> Option<&mut dyn crate::game_logic::NativeTerrainEffects> {
        self.visual
            .as_mut()
            .map(|visual| visual as &mut dyn crate::game_logic::NativeTerrainEffects)
    }

    /// A lexical producer can borrow these disjoint existing owners together.
    /// The caller supplies its renderer/device service loan and the driving
    /// GameLogic supplies ModuleFactory directly to each birth operation.
    #[cfg(all(test, feature = "game_client"))]
    pub(super) fn drawable_owners(
        &mut self,
    ) -> (
        game_client::core::game_client::NativeDrawableOwner<'_>,
        &mut crate::graphics::render_pipeline::native_draw_world::NativeDrawWorld,
    ) {
        (self.drawables.borrow_native(), &mut self.draw)
    }

    /// Accepted-map commit. Call while old Client still resolves its old world,
    /// before replacing GameLogic. No candidate callback executes in this swap.
    /// The application commits `epoch()` unchanged in its display identity.
    #[cfg(feature = "game_client")]
    pub(super) fn install_owned_worlds(
        mut self,
        client: &mut game_client::core::game_client::GameClient,
        pipeline: &mut RenderPipeline,
    ) -> Result<super::native_audio_effects::PendingNativeAudio, (String, Self)> {
        if let Err(error) = client.retire_native_drawable_world_for_replace() {
            return Err((
                format!("retiring previous native Drawable world: {error}"),
                self,
            ));
        }
        client.install_prepared_native_drawable_world(self.drawables);
        pipeline.install_native_draw_world(self.draw);
        pipeline.install_native_terrain_cpu(self.visual.take());
        Ok(self.audio)
    }

    // Component-only terrain/audio tests cannot admit a real object roster.
    // Production must borrow the complete NativeMapAttempt instead.
    #[cfg(test)]
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
    #[cfg(test)]
    pub(super) fn admit_audio_to(&mut self, audio: &mut dyn crate::game_logic::NativeAudioEffects) {
        self.audio.deliver_to(audio);
    }
    #[cfg(not(feature = "game_client"))]
    pub(super) fn install_visuals(
        mut self,
        pipeline: &mut RenderPipeline,
    ) -> super::native_audio_effects::PendingNativeAudio {
        let _ = pipeline;
        self.audio
    }
}
#[cfg(all(test, feature = "game_client"))]
impl crate::game_logic::NativeMapAdmission for NativeMapCandidate {
    fn discard_attempt(&mut self) -> Result<(), String> {
        self.drawables
            .borrow_native()
            .try_clear_for_map_attempt(None)
            .map_err(|error| error.to_string())?;
        self.draw = Default::default();
        self.visual = None;
        self.audio.clear();
        Ok(())
    }
    fn admit_restored_object_drawables(
        &mut self,
        world: &mut crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        // The component receiver has no native constructor service. It may
        // accept an empty roster, never fake admission for a populated save.
        if !world.host_objects().is_empty() {
            return Err("saved object Drawables require the lexical native map attempt".into());
        }
        self.drawables
            .borrow_native()
            .try_clear_for_map_attempt(None)
            .map_err(|error| error.to_string())?;
        self.draw = Default::default();
        Ok(())
    }
    fn on_level_start(&mut self) -> Result<(), String> {
        self.drawables
            .borrow_native()
            .on_level_start()
            .map_err(|error| error.to_string())
    }
    fn prepare(
        &mut self,
        world: &crate::game_logic::GameLogic,
        metadata: Option<&crate::game_logic::script_loader::MapMetadata>,
    ) -> Result<(), String> {
        prepare_native_map_visual(&mut self.visual, world, metadata)
    }
    fn admit_restored_heights(
        &mut self,
        world: &crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        admit_native_map_restored_heights(&mut self.visual, world)
    }
    fn effects(&mut self) -> crate::game_logic::ScriptPresentationEffects<'_> {
        crate::game_logic::ScriptPresentationEffects {
            audio: Some(&mut self.audio),
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

#[cfg(all(test, feature = "game_client"))]
mod audio_owner_tests {
    use super::*;
    use crate::game_logic::NativeMapAdmission;
    use crate::game_logic::containment_audio_owner_tests as fixture;
    use crate::game_logic::containment_audio_test_support as audio;
    #[test]
    fn rejected_native_map_discards_audio_and_accepted_install_delivers_once() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "rejected_native_map_discards_audio_and_accepted_install_delivers_once",
            || {
                let (mut world, container, _) = fixture::world("Map");
                world.frame = 7;
                let mut output = audio::manager(&["MapEnter", "MapLoad"]);
                let mut candidate = NativeMapCandidate::default();
                world.play_container_enter_sound(container, &mut candidate.effects());
                // Actual failed map kernel, including early rejection before prepare.
                assert!(!world.load_map_with_progress_and_native(
                    "/__generals_missing_containment_audio_map__/absent.map",
                    |_, _| {},
                    candidate.receiver()
                ));
                candidate.admit_audio_to(&mut output);
                audio::assert_events(&output, &[]);
                // A reused receiver (fallback) begins with none of the failed attempt's cues.
                let (mut accepted, container, _) = fixture::world("Map");
                accepted.frame = 8;
                accepted.play_container_enter_sound(container, &mut candidate.effects());
                audio::assert_events(&output, &[]);
                candidate.admit_audio_to(&mut output);
                audio::assert_events(&output, &[("MapEnter", container), ("MapLoad", container)]);
                candidate.admit_audio_to(&mut output);
                audio::assert_events(&output, &[("MapEnter", container), ("MapLoad", container)]);
                // Dropping an uncommitted candidate discards its owned payload.
                // The staged-restore regression separately exercises a real late error.
                let mut late_rejected = NativeMapCandidate::default();
                accepted.frame = 9;
                accepted.play_container_enter_sound(container, &mut late_rejected.effects());
                drop(late_rejected);
                audio::assert_events(&output, &[("MapEnter", container), ("MapLoad", container)]);
            },
        );
    }
}

/// One actual map-loader walk. All service references expire before candidate
/// transfer to StartupResult/commit. No factory, graphics device or registry is
/// stored in the detached world.
#[cfg(feature = "game_client")]
pub(super) struct NativeMapAttempt<'a> {
    audio: &'a mut super::native_audio_effects::PendingNativeAudio,
    visual: &'a mut Option<game_client::terrain::terrain_visual::TerrainVisualImpl>,
    view: super::native_view_effects::NativeRendererViewEffects<'a>,
}
#[cfg(feature = "game_client")]
impl NativeMapCandidate {
    pub(super) fn attempt<'a>(
        &'a mut self,
        services: crate::graphics::render_pipeline::native_draw_world::NativeDrawServices<'a>,
        graphics: &'a mut GraphicsSystem,
        local_player_id: u32,
        policy: game_client::core::game_client::ReceivingDrawablePolicy,
        retirement: &'a mut dyn game_client::core::game_client::NativeDrawableRetirement,
    ) -> Result<NativeMapAttempt<'a>, String> {
        let epoch = self
            .epoch
            .ok_or("map birth requires a reserved candidate epoch")?;
        let poses = services.bind_world(&mut self.draw);
        let view = super::native_view_effects::NativeRendererViewEffects::new(
            poses,
            graphics,
            self.drawables.borrow_native(),
            epoch,
            local_player_id,
            policy,
            retirement,
        );
        Ok(NativeMapAttempt {
            audio: &mut self.audio,
            visual: &mut self.visual,
            view,
        })
    }
}
#[cfg(feature = "game_client")]
impl crate::game_logic::NativeMapAdmission for NativeMapAttempt<'_> {
    fn discard_attempt(&mut self) -> Result<(), String> {
        self.view.discard_map_attempt()?;
        *self.visual = None;
        self.audio.clear();
        Ok(())
    }
    fn admit_restored_object_drawables(
        &mut self,
        world: &mut crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        self.view.admit_restored_object_drawables(world)
    }
    fn on_level_start(&mut self) -> Result<(), String> {
        self.view.on_level_start()
    }
    fn prepare(
        &mut self,
        world: &crate::game_logic::GameLogic,
        metadata: Option<&crate::game_logic::script_loader::MapMetadata>,
    ) -> Result<(), String> {
        prepare_native_map_visual(self.visual, world, metadata)
    }
    fn admit_restored_heights(
        &mut self,
        world: &crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        admit_native_map_restored_heights(self.visual, world)
    }
    fn effects(&mut self) -> crate::game_logic::ScriptPresentationEffects<'_> {
        crate::game_logic::ScriptPresentationEffects {
            audio: Some(self.audio),
            ui: None,
            view: Some(&mut self.view),
            terrain: self
                .visual
                .as_mut()
                .map(|visual| visual as &mut dyn crate::game_logic::NativeTerrainEffects),
        }
    }
}
#[cfg(feature = "game_client")]
fn prepare_native_map_visual(
    visual: &mut Option<game_client::terrain::terrain_visual::TerrainVisualImpl>,
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
        *visual = None;
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
            .map(
                |&(location, radius, kind)| crate::presentation_frame::PresentationTerrainScorch {
                    location,
                    radius,
                    kind,
                },
            )
            .collect(),
        ..Default::default()
    };
    let water = terrain.water_grid_state().clone();
    drop(terrain);
    let mut prepared_visual =
        RenderPipeline::prepare_native_terrain_cpu(&env).map_err(|error| error.to_string())?;
    if let Some(visual) = prepared_visual.as_mut() {
        visual.admit_map_water_configuration(&water);
    }
    *visual = prepared_visual;
    Ok(())
}

#[cfg(feature = "game_client")]
fn admit_native_map_restored_heights(
    visual: &mut Option<game_client::terrain::terrain_visual::TerrainVisualImpl>,
    world: &crate::game_logic::GameLogic,
) -> Result<(), String> {
    let terrain = world
        .world_services
        .terrain()
        .read()
        .map_err(|_| "receiving restored terrain poisoned")?;
    if !terrain.has_height_map() && visual.is_none() {
        return Ok(());
    }
    let visual = visual
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
