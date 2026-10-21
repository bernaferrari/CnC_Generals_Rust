//! Ephemeral loans for one synchronous script/callback walk, never world state.
use crate::assets::LiveDrawPlaybackIdentity;
#[derive(Clone)]
pub(crate) struct NativeSlaveSource {
    pub(crate) identity: LiveDrawPlaybackIdentity,
    pub(crate) template_name: String,
    pub(crate) receiving_draw_models: std::sync::Arc<crate::game_logic::thing::ReceivingDrawModels>,
    pub(crate) condition_bits: u128,
    pub(crate) mesh_scale: f32,
    pub(crate) float_yaw: f32,
    pub(crate) float_pitch: f32,
    pub(crate) under_construction: bool,
    pub(crate) construction_percent: f32,
    pub(crate) max_height_above_position: f32,
}

use crate::presentation_frame::camera::build::SlaveCameraSample;

/// One immediate receiving audio admission, borrowed for a synchronous operation.
/// The service owns its actual pending AR_Play requests; gameplay keeps no receipt log.
pub(crate) trait NativeAudioEffects {
    fn add_audio_event(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    );
}
impl NativeAudioEffects for game_engine::common::audio::AudioManager {
    fn add_audio_event(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    ) {
        self.add_audio_event_with_locality(event, locality);
    }
}

pub(crate) trait NativeViewEffects {
    fn local_player_id(&self) -> u32;
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample;
    #[cfg(feature = "game_client")]
    fn host_epoch(&self) -> u64;
    #[cfg(feature = "game_client")]
    fn admit_factory_drawable(
        &mut self,
        source: NativeFactoryDrawableSource,
        birth: game_client::core::game_client::NativeDrawableBirthContext,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String>;
}

/// One bounded read of the canonical object at initObject/sendObjectCreated.
/// It never owns an object, advances a clock, or drains frame effects.
#[cfg(feature = "game_client")]
pub(crate) struct NativeFactoryDrawableSource {
    pub(crate) drawable: game_client::core::game_client::PresentationDrawableSync,
    pub(crate) render: NativeSlaveSource,
}
/// Detached native map admission, borrowed by the one production map loader.
/// The owner is returned to its application only after that load succeeds.
pub(crate) trait NativeMapAdmission {
    /// Rejection or a new attempt discards only this candidate's native outputs.
    fn discard_attempt(&mut self) -> Result<(), String>;
    /// Actual receiving Drawable prepend roster, before the driving map gate
    /// closes. Saved-map foundation never invokes this new-level callback.
    fn on_level_start(&mut self) -> Result<(), String>;
    fn prepare(
        &mut self,
        world: &super::GameLogic,
        metadata: Option<&super::script_loader::MapMetadata>,
    ) -> Result<(), String>;
    fn admit_restored_heights(&mut self, world: &super::GameLogic) -> Result<(), String>;
    /// Snapshot replaced host instances after map foundation. Reconstruct only
    /// their native constructor state, without replaying gameplay creation.
    fn admit_restored_object_drawables(
        &mut self,
        world: &mut super::GameLogic,
    ) -> Result<(), String>;
    fn effects(&mut self) -> ScriptPresentationEffects<'_>;
}

/// Owned input for the immediate C++ Object -> terrain tree callback.
/// Coordinates are source XYZ (host Y-up is converted at the pose boundary).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NativeTerrainUnitMove {
    pub(crate) object_id: u32,
    pub(crate) position: glam::Vec3,
    pub(crate) direction_2d: glam::Vec2,
    pub(crate) major_radius: f32,
    pub(crate) minor_radius: f32,
    pub(crate) is_box: bool,
    pub(crate) crusher_level: i32,
}

/// One synchronous receiving native terrain loan; never stored by gameplay.
pub(crate) trait NativeTerrainEffects {
    /// Test/standalone effect recorders may intentionally omit tree simulation.
    /// The real receiving TerrainVisual implementation performs it synchronously.
    fn unit_moved(&mut self, _unit: NativeTerrainUnitMove, _frame: u32) {}
    fn oversize_terrain(&mut self, amount: i32);
    fn remove_for_construction(
        &mut self,
        position: [f32; 3],
        major: f32,
        minor: f32,
        is_box: bool,
        angle: f32,
    );
    fn placement_bib(
        &mut self,
        object: u32,
        transform: glam::Mat4,
        major: f32,
        minor: f32,
        is_box: bool,
    );
    fn retain_placement_bibs(&mut self, owners: &[u32]);
    fn clear_placement_bibs(&mut self);
    fn add_water_velocity(&mut self, x: f32, y: f32, velocity: f32, preferred_height: f32);
    fn add_scorch(&mut self, location: [f32; 3], radius: f32, kind: i32);
    fn accepted_height(&mut self, change: gamelogic::terrain::TerrainHeightChange);
    fn water_state(&mut self, state: &gamelogic::terrain_water::WaterGridState);
}
#[cfg(feature = "game_client")]
impl NativeTerrainEffects for game_client::terrain::terrain_visual::TerrainVisualImpl {
    fn unit_moved(&mut self, unit: NativeTerrainUnitMove, frame: u32) {
        self.unit_moved(
            game_client::terrain::TreeCollisionUnit {
                object_id: unit.object_id,
                position: unit.position,
                direction_2d: unit.direction_2d,
                major_radius: unit.major_radius,
                minor_radius: unit.minor_radius,
                geometry_type: if unit.is_box {
                    game_client::terrain::TreeGeometryType::Box
                } else {
                    game_client::terrain::TreeGeometryType::Cylinder
                },
                crusher_level: unit.crusher_level,
                immobile: false,
            },
            frame,
        );
    }

    fn oversize_terrain(&mut self, amount: i32) {
        game_client::terrain::terrain_visual::TerrainVisualImpl::oversize_terrain(self, amount);
    }

    fn remove_for_construction(
        &mut self,
        position: [f32; 3],
        major: f32,
        minor: f32,
        is_box: bool,
        angle: f32,
    ) {
        self.remove_trees_and_props_for_construction(position, major, minor, is_box, angle);
    }

    fn placement_bib(
        &mut self,
        object: u32,
        transform: glam::Mat4,
        major: f32,
        minor: f32,
        is_box: bool,
    ) {
        let _ = self.add_faction_bib(
            object,
            game_client::terrain::terrain_visual::TerrainBibOwnerKind::Object,
            transform,
            major,
            minor,
            is_box,
            0.0,
            0.0,
            true,
            0.0,
        );
    }
    fn retain_placement_bibs(&mut self, owners: &[u32]) {
        self.retain_placement_highlight_bibs(owners);
    }
    fn clear_placement_bibs(&mut self) {
        self.clear_placement_highlight_bibs();
    }

    fn add_water_velocity(&mut self, x: f32, y: f32, velocity: f32, preferred_height: f32) {
        game_client::terrain::terrain_visual::TerrainVisualImpl::add_water_velocity(
            self,
            x,
            y,
            velocity,
            preferred_height,
        );
    }

    fn add_scorch(&mut self, location: [f32; 3], radius: f32, kind: i32) {
        game_client::terrain::terrain_visual::TerrainVisualImpl::add_scorch(
            self, location, radius, kind,
        );
    }

    fn accepted_height(&mut self, change: gamelogic::terrain::TerrainHeightChange) {
        self.apply_accepted_height(change.sample_x, change.sample_y, change.raw_height);
    }
    fn water_state(&mut self, state: &gamelogic::terrain_water::WaterGridState) {
        self.admit_water_state(state);
    }
}
#[derive(Default)]
pub(crate) struct ScriptPresentationEffects<'a> {
    pub(crate) ui: Option<&'a mut dyn super::ScriptUiEffects>,
    pub(crate) view: Option<&'a mut dyn NativeViewEffects>,
    pub(crate) terrain: Option<&'a mut dyn NativeTerrainEffects>,
    pub(crate) audio: Option<&'a mut dyn NativeAudioEffects>,
}
impl ScriptPresentationEffects<'_> {
    pub(crate) fn reborrow(&mut self) -> ScriptPresentationEffects<'_> {
        ScriptPresentationEffects {
            audio: self
                .audio
                .as_mut()
                .map(|audio| &mut **audio as &mut dyn NativeAudioEffects),
            terrain: self
                .terrain
                .as_mut()
                .map(|terrain| &mut **terrain as &mut dyn NativeTerrainEffects),
            ui: self
                .ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn super::ScriptUiEffects),
            view: self
                .view
                .as_mut()
                .map(|view| &mut **view as &mut dyn NativeViewEffects),
        }
    }
}
