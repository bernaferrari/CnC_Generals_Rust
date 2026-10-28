//! Ephemeral loans for one synchronous script/callback walk, never world state.
use crate::assets::LiveDrawPlaybackIdentity;
#[derive(Clone, Debug)]
pub struct NativeSlaveSource {
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

#[cfg(feature = "game_client")]
#[derive(Clone, Debug)]
pub(crate) struct NativeAuxiliarySource {
    pub(crate) world: std::sync::Arc<()>,
    pub(crate) epoch: u64,
    pub(crate) template:
        std::sync::Arc<game_engine::common::thing::thing_template::AdmittedReceivingTemplate>,
    pub(crate) draws: std::sync::Arc<crate::game_logic::thing::ReceivingDrawModels>,
    pub(crate) scale: f32,
    pub(crate) frame: u32,
    pub(crate) birth: game_client::core::game_client::NativeDrawableBirthContext,
    pub(crate) ambient_audio_locality: game_engine::common::audio::AudioSubmissionLocality,
}

#[cfg(feature = "game_client")]
impl PartialEq for NativeAuxiliarySource {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.world, &other.world)
            && self.epoch == other.epoch
            && std::sync::Arc::ptr_eq(&self.template, &other.template)
            && std::sync::Arc::ptr_eq(&self.draws, &other.draws)
            && self.scale == other.scale
            && self.frame == other.frame
            && self.birth == other.birth
            && self.ambient_audio_locality == other.ambient_audio_locality
    }
}

use crate::presentation_frame::camera::build::SlaveCameraSample;

/// Playback observation for one receiving session's ambient loop retry.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AmbientAudioRetryObservation {
    pub(crate) is_playing: bool,
    /// `None` while playing keeps the source's lazy permanence query.
    pub(crate) is_permanent: Option<bool>,
}

/// One immediate receiving audio admission, borrowed for a synchronous operation.
/// The service owns its actual pending AR_Play requests; gameplay keeps no receipt log.
pub(crate) trait NativeAudioEffects {
    fn music_track_completed(
        &mut self,
        _track: &str,
        _number_of_times: i32,
        _additional_handles: &[u32],
    ) -> Result<bool, String> {
        Err("receiving audio service does not own music completion receipts".into())
    }

    fn accepts_audio_device(
        &self,
        _device: &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
    ) -> bool {
        false
    }

    fn script_audio(
        &mut self,
        _request: gamelogic::scripting::engine::ScriptAudioRequest<'_>,
        _policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
        _additional_handles: &[u32],
    ) -> Result<(), String> {
        Err("receiving audio service does not own script policy receipts".into())
    }

    /// Receiving-session observation used by Drawable's permanent ambient
    /// loop retry. Implementations filter playing records by this session's
    /// accepted audio receipts; event definitions/settings remain shared data.
    fn ambient_retry_frames(&mut self) -> Option<u32> {
        None
    }
    fn ambient_retry_observation(
        &mut self,
        _object_id: u32,
        _name: &str,
    ) -> Option<AmbientAudioRetryObservation> {
        None
    }

    fn submit_selected_audio(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
        _selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        let mut event = event.clone();
        event.admit_script_policy(policy);
        self.add_audio_event(&event, locality);
        Ok(())
    }
    fn add_prepared_audio_event(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    ) {
        self.add_audio_event(event, locality);
    }

    #[cfg(feature = "game_client")]
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&gamelogic::common::Coord3D>,
        player: Option<i32>,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
        _selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        let mut event = game_engine::common::audio::AudioEventRts::with_event_name(name);
        if let Some(position) = position {
            event.set_position(&game_engine::common::audio::audio_event_rts::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            });
        }
        if let Some(player) = player {
            event.set_player_index(player);
        }
        event.admit_script_policy(policy);
        self.add_audio_event(&event, locality);
        Ok(())
    }

    /// Actual captured mixer capability; detached add-only candidates cannot
    /// fabricate Drawable playing-handle ownership.
    #[cfg(feature = "game_client")]
    fn native_ambient(
        &mut self,
    ) -> Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio> {
        None
    }
    fn add_audio_event(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    );
}
impl NativeAudioEffects for game_engine::common::audio::AudioManager {
    fn add_prepared_audio_event(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    ) {
        self.add_prepared_audio_event_with_locality(event, locality);
    }

    #[cfg(feature = "game_client")]
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&gamelogic::common::Coord3D>,
        player: Option<i32>,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        game_client::fx_list::submit_sound_fx_with_policy(
            name, position, player, self, locality, selection, policy,
        )
        .map(|_| ())
    }

    #[cfg(feature = "game_client")]
    fn native_ambient(
        &mut self,
    ) -> Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio> {
        Some(self)
    }
    fn add_audio_event(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    ) {
        self.add_audio_event_with_locality(event, locality);
    }
}

#[cfg(feature = "game_client")]
pub(crate) struct NativeBodyVisualChange {
    pub(crate) audio_policy: game_engine::common::audio::script_policy::ScriptAudioPolicy,
    pub(crate) identity: LiveDrawPlaybackIdentity,
    pub(crate) state: gamelogic::common::types::BodyDamageType,
    pub(crate) loading_map: bool,
    pub(crate) time_of_day: Option<game_client::system::TimeOfDay>,
    pub(crate) locality: game_engine::common::audio::AudioSubmissionLocality,
}

#[cfg(feature = "game_client")]
impl NativeBodyVisualChange {
    /// Shared synchronous delivery; no service discovery or deferred receipt.
    pub(in crate::game_logic) fn deliver(
        &mut self,
        state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
        view: &mut dyn NativeViewEffects,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType as Host;
        use gamelogic::common::types::BodyDamageType as Native;
        self.state = match state {
            Host::Pristine => Native::Pristine,
            Host::Damaged => Native::Damaged,
            Host::ReallyDamaged => Native::ReallyDamaged,
            Host::Rubble => Native::Rubble,
        };
        view.body_visual_changed(self, audio)
            .expect("receiving native Body visual callback failed after canonical HP mutation");
    }
}

/// Operation-local client allocation. A missing/unsupported visual resource
/// is an ordinary allocation result, never a gameplay failure.
#[cfg(feature = "game_client")]
pub(crate) trait NativeProjectedRadiusAllocator {
    /// Position is original C++ XYZ (Z up), matching client effect rendering.
    fn allocate_radius(
        &mut self,
        texture: &str,
        radius: f32,
        position: [f32; 3],
        color: u32,
        opacity: f32,
    ) -> Option<game_client::radius_decal::ShadowHandle>;
}

pub(crate) trait NativeViewEffects {
    fn visit_owned_ambient_audio_handles(&self, _visit: &mut dyn FnMut(u32)) {}

    #[cfg(feature = "game_client")]
    fn create_jet_lockon(
        &mut self,
        source: &NativeAuxiliarySource,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<game_client::core::game_client::NativeAuxiliaryReceipt, String> {
        let _ = (source, content);
        Err("receiving native lock-on constructor unavailable".into())
    }
    #[cfg(feature = "game_client")]
    fn position_jet_lockon(
        &mut self,
        source: &NativeAuxiliarySource,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
        position: [f32; 3],
        orientation: Option<f32>,
    ) -> Result<(), String> {
        let _ = (source, receipt, position, orientation);
        Err("receiving native lock-on pose unavailable".into())
    }
    #[cfg(feature = "game_client")]
    fn set_jet_lockon_scene_flags(
        &mut self,
        source: &NativeAuxiliarySource,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
        hidden: bool,
        shroud_object: Option<u32>,
    ) -> Result<(), String> {
        let _ = (source, receipt, hidden, shroud_object);
        Err("receiving native lock-on visibility unavailable".into())
    }
    #[cfg(feature = "game_client")]
    fn retire_jet_lockon(
        &mut self,
        source: &NativeAuxiliarySource,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
    ) -> Result<(), String> {
        let _ = (source, receipt);
        Err("receiving native lock-on retirement unavailable".into())
    }

    #[cfg(feature = "game_client")]
    fn spawn_tracer(
        &mut self,
        template: &game_engine::common::thing::ThingTemplate,
        request: &game_client::fx_list::FxTracerRequest<'_>,
    ) -> Result<(), String> {
        let _ = (template, request);
        Err("receiving native tracer factory unavailable".into())
    }
    #[cfg(feature = "game_client")]
    fn create_ray(
        &mut self,
        template: &game_engine::common::thing::ThingTemplate,
        request: &game_client::fx_list::FxRayRequest<'_>,
    ) -> Result<(), String> {
        let _ = (template, request);
        Err("receiving native ray factory unavailable".into())
    }

    #[cfg(feature = "game_client")]
    fn create_light_pulse(
        &mut self,
        pulse: game_client::fx_list::DisplayLightPulse,
    ) -> Result<bool, String> {
        let _ = pulse;
        Err("receiving native light scene unavailable".into())
    }

    /// Mutable particles belong to this exact native presentation lifetime.
    /// Omitted only by explicitly headless/recording adapters.
    fn particle_systems_mut(
        &mut self,
    ) -> Option<&mut game_client::effects::particle_manager::ParticleSystemManager> {
        None
    }

    /// Query actual installed native bones at this immediate nugget point.
    /// The caller supplies fresh source observations from the receiving world.
    #[cfg(feature = "game_client")]
    fn current_client_bone_transforms(
        &mut self,
        source: &NativeSlaveSource,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let _ = (source, bone_name, start, max_bones);
        Err("native FX bone query has no receiving installed-render capability".into())
    }

    /// Actual receiving Drawable instance state, separate from Object pose.
    /// None is an explicit no-Drawable recipient; native adapters must validate birth.
    /// None is a genuine no-Drawable receiver, never a stale/missing binding.
    fn drawable_model_condition_bits(
        &mut self,
        _source: &NativeSlaveSource,
    ) -> Result<Option<u128>, String> {
        Err("receiving native Drawable condition query unavailable".into())
    }
    fn drawable_instance_transform(
        &mut self,
        source: &NativeSlaveSource,
    ) -> Result<Option<glam::Mat4>, String> {
        let _ = source;
        Err("receiving Drawable instance query unavailable".into())
    }
    fn set_drawable_instance_transform(
        &mut self,
        source: &NativeSlaveSource,
        matrix: Option<glam::Mat4>,
    ) -> Result<bool, String> {
        let _ = (source, matrix);
        Err("receiving Drawable instance mutation unavailable".into())
    }

    /// Logical-condition pristine bones, independent of the current HAnim.
    #[cfg(feature = "game_client")]
    fn pristine_client_bone_transforms(
        &mut self,
        source: &NativeSlaveSource,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let _ = (source, bone_name, start, max_bones);
        Err("native pristine bone query has no receiving installed-render capability".into())
    }

    /// Explicit capability: native forwarding adapters must forward this call.
    /// A genuinely detached recipient declares no Drawable by returning false.
    #[cfg(feature = "game_client")]
    fn body_visual_changed(
        &mut self,
        change: &NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String>;
    /// A short loan of the receiving resource manager. Absence is checked by
    /// the driving native operation before any simulation mutation.
    #[cfg(feature = "game_client")]
    fn projected_radius_allocator(&mut self) -> Option<&mut dyn NativeProjectedRadiusAllocator> {
        None
    }
    fn local_player_id(&self) -> u32;
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample;
    #[cfg(feature = "game_client")]
    fn retire_physical_object_drawable(
        &mut self,
        source: &NativeObjectRetirement<'_>,
    ) -> Result<(), String>;
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

/// Object::~Object retains its original Object/team after GameLogic removed
/// the Object from both lookup and visitation. These are bounded read loans
/// into that physical phase, never an alternate object registry.
#[cfg(feature = "game_client")]
pub(crate) struct NativeObjectRetirement<'a> {
    pub(crate) identity: LiveDrawPlaybackIdentity,
    pub(crate) object: &'a super::Object,
    pub(crate) live_objects: &'a super::HostObjectStore,
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
    fn on_level_start(&mut self, world: &super::GameLogic) -> Result<(), String>;
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

#[cfg(feature = "game_client")]
impl NativeTerrainUnitMove {
    fn tree_collision_unit(self) -> game_client::terrain::TreeCollisionUnit {
        game_client::terrain::TreeCollisionUnit {
            object_id: self.object_id,
            position: self.position,
            direction_2d: self.direction_2d,
            major_radius: self.major_radius,
            minor_radius: self.minor_radius,
            geometry_type: if self.is_box {
                game_client::terrain::TreeGeometryType::Box
            } else {
                game_client::terrain::TreeGeometryType::Cylinder
            },
            crusher_level: self.crusher_level,
            immobile: false,
        }
    }
}

/// One synchronous receiving native terrain loan; never stored by gameplay.
pub(crate) trait NativeTerrainEffects {
    /// Test/standalone effect recorders may intentionally omit tree simulation.
    /// The real receiving TerrainVisual implementation performs it synchronously.
    fn unit_moved(&mut self, _unit: NativeTerrainUnitMove, _frame: u32) {}
    #[cfg(feature = "game_client")]
    fn begin_tree_move(
        &mut self,
        unit: NativeTerrainUnitMove,
        frame: u32,
    ) -> Option<game_client::terrain::TreeMoveCursor> {
        self.unit_moved(unit, frame);
        None // headless recorders have no tree state
    }
    #[cfg(feature = "game_client")]
    fn advance_tree_move(
        &mut self,
        _cursor: &mut game_client::terrain::TreeMoveCursor,
        _unit: NativeTerrainUnitMove,
    ) -> Option<game_client::terrain::TreeFxEvent> {
        unreachable!("tree cursor requires its terrain owner")
    }
    #[cfg(feature = "game_client")]
    fn finish_tree_move_fx(&mut self, _cursor: &mut game_client::terrain::TreeMoveCursor) {
        unreachable!("tree cursor requires its terrain owner")
    }

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
    fn begin_tree_move(
        &mut self,
        unit: NativeTerrainUnitMove,
        frame: u32,
    ) -> Option<game_client::terrain::TreeMoveCursor> {
        self.tree_buffer_mut()
            .begin_unit_move(unit.tree_collision_unit(), frame)
    }
    fn advance_tree_move(
        &mut self,
        cursor: &mut game_client::terrain::TreeMoveCursor,
        unit: NativeTerrainUnitMove,
    ) -> Option<game_client::terrain::TreeFxEvent> {
        self.tree_buffer_mut()
            .advance_unit_move(cursor, unit.tree_collision_unit())
    }
    fn finish_tree_move_fx(&mut self, cursor: &mut game_client::terrain::TreeMoveCursor) {
        self.tree_buffer_mut().finish_unit_move_fx(cursor);
    }

    fn unit_moved(&mut self, unit: NativeTerrainUnitMove, frame: u32) {
        self.unit_moved(unit.tree_collision_unit(), frame);
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
    /// Exact synchronous interpreter event loan; no ambient recipient discovery.
    pub(crate) script_events: Option<super::ScriptEventRecipient<'a>>,
    pub(crate) ui: Option<&'a mut dyn super::ScriptUiEffects>,
    pub(crate) view: Option<&'a mut dyn NativeViewEffects>,
    /// Immutable decoded content capability, usable independently of a GPU.
    pub(crate) pristine_bones: Option<&'a dyn crate::assets::pristine_bones::PristineBoneAssets>,
    pub(crate) terrain: Option<&'a mut dyn NativeTerrainEffects>,
    pub(crate) audio: Option<&'a mut dyn NativeAudioEffects>,
    /// Historical type name: this is an operation's borrowed service context.
    /// Present only around an Object operation after split borrowing the
    /// driving GameLogic fields; never retained across an &mut GameLogic call.
    pub(crate) partition: Option<&'a mut crate::game_logic::partition_manager::PartitionManager>,
}
impl ScriptPresentationEffects<'_> {
    /// Reject an invalid supplied native mode before the driving world starts
    /// script/command/module callbacks. Headless sessions need no device owner.
    pub(crate) fn validate_projected_radius_recipient(
        &mut self,
        required_native: bool,
    ) -> gamelogic::GameLogicResult<()> {
        if !required_native {
            return Ok(());
        }
        #[cfg(feature = "game_client")]
        if self
            .view
            .as_deref_mut()
            .and_then(|view| view.projected_radius_allocator())
            .is_some()
        {
            return Ok(());
        }
        Err(gamelogic::GameLogicError::Configuration(
            "native operation requires its receiving projected resource owner".into(),
        ))
    }

    pub(crate) fn reborrow_for_object<'a>(
        &'a mut self,
        partition: &'a mut crate::game_logic::partition_manager::PartitionManager,
    ) -> ScriptPresentationEffects<'a> {
        let mut local = self.reborrow();
        local.partition = Some(partition);
        local
    }

    pub(crate) fn reborrow(&mut self) -> ScriptPresentationEffects<'_> {
        ScriptPresentationEffects {
            script_events: self.script_events,
            pristine_bones: self.pristine_bones,
            partition: self.partition.as_deref_mut(),
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

#[cfg(all(test, feature = "game_client"))]
#[path = "script_presentation/projected_radius_tests.rs"]
mod projected_radius_tests;
