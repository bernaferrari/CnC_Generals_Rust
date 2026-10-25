//! Presentation-only callbacks borrow their actual current Object; no ambient
//! world selection or list-wide ObjectStore loan is needed by these nuggets.
use super::*;
use game_client::fx_list::{
    FxObservation, FxRayRequest, FxSource, FxTracerRequest, GameClientFxRuntime,
};
use std::collections::{HashMap, HashSet};

/// Immutable external observations for one bounded Object operation. Mutable
/// primary fields are deliberately absent: every nugget rereads the borrower.
pub(crate) struct ObjectFxEnvironment {
    pub world_identity: std::sync::Arc<()>,
    pub world_revision: u64,
    pub frame: u32,
    pub player: Option<u32>,
    pub weather: (bool, bool),
    pub local_team: crate::game_logic::Team,
    pub detected: HashSet<crate::game_logic::ObjectId>,
    pub tunnel_counts: HashMap<crate::game_logic::ObjectId, u16>,
    pub localities: HashMap<i32, game_engine::common::audio::AudioSubmissionLocality>,
    pub drawings: HashMap<String, std::sync::Arc<crate::game_logic::thing::ReceivingDrawModels>>,
    pub things: HashMap<String, std::sync::Arc<game_engine::common::thing::ThingTemplate>>,
}
impl crate::game_logic::GameLogic {
    pub(crate) fn observe_object_fx_environment(&self) -> ObjectFxEnvironment {
        let player = self.local_player_id();
        let mut local_team = player
            .and_then(|id| self.get_player(id))
            .map(|player| player.team)
            .unwrap_or(crate::game_logic::Team::Neutral);
        if local_team == crate::game_logic::Team::Neutral {
            local_team = self
                .get_players()
                .values()
                .find(|player| player.is_local && player.team != crate::game_logic::Team::Neutral)
                .map(|player| player.team)
                .unwrap_or(crate::game_logic::Team::USA);
        }
        let mut localities = self
            .get_players()
            .keys()
            .map(|id| {
                (
                    *id as i32,
                    self.receiving_audio_locality_for_player(*id as i32),
                )
            })
            .collect::<HashMap<_, _>>();
        localities.insert(-1, self.receiving_audio_locality_for_player(-1));
        let mut things = HashMap::new();
        if let Some(content) = self.receiving_module_content.as_ref() {
            let factory = content.compatibility_template_view();
            for (name, _) in content.admitted_templates() {
                if let Some(template) = factory.find_template(name, false) {
                    things.insert(name.to_owned(), template);
                }
            }
        }
        ObjectFxEnvironment {
            world_identity: std::sync::Arc::clone(&self.live_draw_playback_world),
            world_revision: self.host_visual_world_epoch(),
            frame: self.frame,
            player,
            weather: (
                self.weather_state()
                    .current_weather
                    .to_ascii_lowercase()
                    .contains("snow")
                    && self.lighting_rules().force_models_to_follow_weather,
                matches!(
                    self.lighting_rules().time_of_day,
                    game_engine::common::ini::ini_game_data::TimeOfDay::Night
                ),
            ),
            local_team,
            detected: self
                .host_objects()
                .registered_values()
                .filter(|object| object.status.detected)
                .map(|object| object.id)
                .collect(),
            tunnel_counts: self
                .host_objects()
                .registered_values()
                .filter(|object| {
                    object.is_tunnel_network_style_container()
                        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                            &object.template_name,
                        )
                })
                .map(|object| {
                    (
                        object.id,
                        self.host_authoritative_occupant_count(object.id)
                            .unwrap_or(0),
                    )
                })
                .collect(),
            localities,
            drawings: self
                .templates
                .iter()
                .filter_map(|(name, template)| {
                    template
                        .receiving_draw_models()
                        .map(|models| (name.clone(), models))
                })
                .collect(),
            things,
        }
    }
}
impl ObjectFxEnvironment {
    fn source(
        &self,
        object: &crate::game_logic::Object,
    ) -> Result<crate::game_logic::NativeSlaveSource, String> {
        let name = crate::presentation_frame::current_object_visual_template(object);
        let drawings = self
            .drawings
            .get(&name)
            .cloned()
            .or_else(|| {
                (name == object.get_template().name)
                    .then(|| object.get_template().receiving_draw_models())
                    .flatten()
            })
            .ok_or("missing receiving FX Draw definitions")?;
        let first_detected = object
            .contained_units()
            .first()
            .is_some_and(|id| self.detected.contains(id));
        let hide = object
            .building_data
            .as_ref()
            .is_some_and(|building| building.hide_garrisoned_state)
            && !first_detected
            && object.team != self.local_team;
        let occupancy = self
            .tunnel_counts
            .get(&object.id)
            .copied()
            .unwrap_or_else(|| {
                object
                    .building_data
                    .as_ref()
                    .map_or(object.occupants.len(), |building| {
                        building.garrisoned_units.len()
                    })
                    .min(u16::MAX as usize) as u16
            });
        let facts = crate::presentation_frame::object_native_draw_observation(
            object,
            self.frame,
            self.weather,
            hide,
            occupancy,
        );
        if !object.drawable_instance_scale.is_finite() {
            return Err("nonfinite receiving FX Drawable scale".into());
        }
        let geometry = object.geometry_info();
        Ok(crate::game_logic::NativeSlaveSource {
            identity: crate::assets::LiveDrawPlaybackIdentity::new(
                std::sync::Arc::clone(&self.world_identity),
                std::sync::Arc::clone(&object.visual_draw_identity_token),
                self.world_revision,
                object.id.0,
                object.visual_object_generation,
            )
            .with_native_pose(object.native_pose_revision(), object.get_transform_matrix()),
            template_name: name.to_owned(),
            receiving_draw_models: drawings,
            condition_bits: facts.condition_bits,
            mesh_scale: object.drawable_instance_scale,
            float_yaw: object.float_update.as_ref().map_or(0., |state| state.yaw),
            float_pitch: object.float_update.as_ref().map_or(0., |state| state.pitch),
            under_construction: facts.under_construction,
            construction_percent: facts.construction_percent,
            max_height_above_position: if geometry.authored {
                geometry.max_height_above_position()
            } else {
                0.
            },
        })
    }
}
pub(crate) struct ObjectFxServices<'a, 's> {
    pub session: &'a mut crate::presentation_frame::SessionView,
    pub player: Option<u32>,
    pub shroud:
        &'a std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
    pub effects: &'a mut crate::game_logic::ScriptPresentationEffects<'s>,
    pub environment: &'a ObjectFxEnvironment,
    pub terrain: &'a std::sync::Arc<std::sync::RwLock<gamelogic::terrain::TerrainLogic>>,
    pub logic_random: &'a game_engine::common::random_value::SessionRandom,
}
impl ObjectFxServices<'_, '_> {
    /// One short loan of this operation's actual native particle owner.
    /// No active manager lookup, epoch-domain shortcut or stored receipt.
    pub(crate) fn retire_particles(
        &mut self,
        ids: crate::game_logic::host_bone_fx_damage::BoneParticleRetirement,
    ) {
        crate::game_logic::host_bone_fx_damage::retire_particle_ids(ids, self);
    }
    pub(crate) fn particle_systems_mut(
        &mut self,
    ) -> Option<&mut game_client::effects::ParticleSystemManager> {
        self.effects.view.as_deref_mut()?.particle_systems_mut()
    }

    pub fn position(
        &mut self,
        name: &str,
        position: Vec3,
        secondary: Option<Vec3>,
        speed: f32,
        radius: f32,
        matrix: Option<glam::Mat4>,
    ) -> bool {
        let source = FxSource::Position {
            primary: Some(host_to_leftover_coord(position)),
            secondary: secondary.map(host_to_leftover_coord),
            speed,
            radius,
            matrix: matrix.map(host_to_leftover_mat4),
        };
        self.execute(name, None, None, source)
    }
    pub fn object(
        &mut self,
        name: &str,
        object: &crate::game_logic::Object,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) -> bool {
        self.execute(
            name,
            Some(BorrowedFxPrimary::Object(object)),
            secondary.copied(),
            FxSource::Objects {
                primary: object.id.0,
                secondary: secondary.map(|pose| pose.id),
            },
        )
    }
    pub fn projectile(
        &mut self,
        name: &str,
        projectile: &crate::game_logic::combat::Projectile,
    ) -> bool {
        self.execute(
            name,
            Some(BorrowedFxPrimary::Projectile(projectile)),
            None,
            FxSource::Objects {
                primary: projectile.id.0,
                secondary: None,
            },
        )
    }
    fn execute(
        &mut self,
        name: &str,
        primary: Option<BorrowedFxPrimary<'_>>,
        secondary: Option<gamelogic::helpers::HostFxObjectPose>,
        source: FxSource,
    ) -> bool {
        match game_client::fx_list::execute_fx_with_runtime(
            &mut ObjectFxRuntime {
                services: self,
                primary,
                secondary,
            },
            name,
            source,
        ) {
            Ok(found) => found,
            Err(error) => {
                log::error!("receiving Object FX '{name}': {error}");
                true
            }
        }
    }
    pub fn scorch(&mut self, position: [f32; 3], radius: f32, kind: i32) {
        if let Some(terrain) = self.effects.terrain.as_deref_mut() {
            terrain.add_scorch(position, radius, kind);
        }
    }
}
impl crate::game_logic::GameLogic {
    pub(crate) fn observe_host_fx_projectile(
        projectile: &crate::game_logic::combat::Projectile,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
    ) -> gamelogic::helpers::HostFxObjectPose {
        use gamelogic::common::types::ObjectShroudStatus;
        let is_shrouded = match (player, shroud.lock()) {
            (Some(player), Ok(shroud)) => shroud
                .get_host_object_shroud_status(player, projectile.id.0)
                .is_some_and(|status| (status as u8) >= ObjectShroudStatus::Fogged as u8),
            _ => true,
        };
        let p = projectile.position;
        gamelogic::helpers::HostFxObjectPose {
            id: projectile.id.0,
            position: gamelogic::common::Coord3D::new(p.x, p.z, p.y),
            transform: glam::Mat4::from_translation(Vec3::new(p.x, p.z, p.y))
                * glam::Mat4::from_rotation_z(projectile.velocity.z.atan2(projectile.velocity.x)),
            player_index: projectile.source_owner_player_id.map_or(-1, |id| id as i32),
            bounding_circle_radius: 0.,
            is_shrouded,
        }
    }
}
#[derive(Clone, Copy)]
enum BorrowedFxPrimary<'a> {
    Object(&'a crate::game_logic::Object),
    Projectile(&'a crate::game_logic::combat::Projectile),
}
impl BorrowedFxPrimary<'_> {
    fn id(self) -> u32 {
        match self {
            Self::Object(o) => o.id.0,
            Self::Projectile(p) => p.id.0,
        }
    }
    fn observe(
        self,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
    ) -> gamelogic::helpers::HostFxObjectPose {
        match self {
            Self::Object(object) => {
                crate::game_logic::GameLogic::observe_host_fx_object(object, player, shroud)
            }
            Self::Projectile(projectile) => {
                crate::game_logic::GameLogic::observe_host_fx_projectile(projectile, player, shroud)
            }
        }
    }
}
struct ObjectFxRuntime<'a, 'b, 's, 'o> {
    services: &'a mut ObjectFxServices<'b, 's>,
    primary: Option<BorrowedFxPrimary<'o>>,
    secondary: Option<gamelogic::helpers::HostFxObjectPose>,
}
impl GameClientFxRuntime for ObjectFxRuntime<'_, '_, '_, '_> {
    fn find_list(&mut self, name: &str) -> Option<std::sync::Arc<game_client::fx_list::FXList>> {
        self.services
            .session
            .fx
            .catalog
            .as_ref()?
            .find_fx_list(name)
    }
    fn observe(&mut self, source: FxSource) -> FxObservation {
        match source {
            FxSource::Position {
                primary,
                matrix,
                secondary,
                speed,
                radius,
            } => FxObservation {
                primary,
                matrix,
                secondary,
                speed,
                radius,
                object: None,
            },
            FxSource::Objects { primary, .. } => {
                let object = self
                    .primary
                    .filter(|object| object.id() == primary)
                    .map(|object| object.observe(self.services.player, self.services.shroud));
                let secondary = self.secondary.map(|secondary| {
                    if secondary.id == primary {
                        object.map_or(secondary.position, |object| object.position)
                    } else {
                        secondary.position
                    }
                });
                FxObservation {
                    primary: object.map(|object| object.position),
                    matrix: object.map(|object| object.transform),
                    secondary,
                    speed: 0.,
                    radius: 0.,
                    object,
                }
            }
        }
    }
    fn visible(&mut self, input: &FxObservation) -> bool {
        if let Some(object) = input.object {
            return !object.is_shrouded;
        }
        let (Some(player), Some(position)) = (self.services.player, input.primary) else {
            return false;
        };
        self.services
            .shroud
            .lock()
            .map(|shroud| {
                matches!(
                    shroud.get_shroud_state(player, &position),
                    gamelogic::system::shroud_manager::ShroudState::Visible
                )
            })
            .unwrap_or(false)
    }
    fn random_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.services.session.client_random.next_real(lo, hi)
    }
    fn random_u32(&mut self) -> u32 {
        self.services.session.client_random.next_u32()
    }
    fn frame(&self) -> u32 {
        self.services.environment.frame
    }
    fn sound(&mut self, name: &str, input: &FxObservation) -> Result<(), String> {
        let player = input
            .object
            .map(|object| object.player_index)
            .filter(|player| *player >= 0);
        let locality = self
            .services
            .environment
            .localities
            .get(&player.unwrap_or(-1))
            .or_else(|| self.services.environment.localities.get(&-1))
            .ok_or("receiving audio locality absent")?;
        let mut selection = super::receiving::FxAudioSelection {
            logic: self.services.logic_random,
            audio: &mut self.services.session.audio_random,
        };
        if let Some(audio) = self.services.effects.audio.as_deref_mut() {
            audio.submit_fx_sound(
                name,
                input.primary.as_ref(),
                player,
                locality,
                &mut selection,
            )
        } else {
            Ok(())
        }
    }
    fn particles(
        &mut self,
        nugget: &game_client::effects::fxlist_integration::ParticleSystemFXNugget,
        input: &FxObservation,
        matrix: Option<glam::Mat3>,
    ) -> Result<(), String> {
        let Some(position) = input.primary else {
            return Ok(());
        };
        let services = &mut *self.services;
        let random = &mut services.session.client_random;
        let terrain = services.terrain;
        let manager = services
            .effects
            .view
            .as_deref_mut()
            .and_then(|view| view.particle_systems_mut())
            .ok_or("Object FX has no receiving particle owner")?;
        manager
            .admit_effective_definitions()
            .map_err(|error| error.to_string())?;
        nugget.do_fx_with_services(
            Vec3::new(position.x, position.y, position.z),
            matrix.as_ref(),
            input.object.map(|object| object.id),
            if input.object.is_some() {
                0.
            } else {
                input.radius
            },
            manager,
            &mut |lo, hi| random.next_real(lo, hi),
            &mut |position| {
                let terrain = terrain.read().unwrap_or_else(|error| error.into_inner());
                let point = gamelogic::common::Coord3D::new(position.x, position.y, position.z);
                let layer = terrain.get_layer_for_destination(&point);
                Some(terrain.get_layer_height(point.x, point.y, layer, None, true))
            },
        );
        Ok(())
    }
    fn light(&mut self, pulse: game_client::fx_list::DisplayLightPulse) -> Result<(), String> {
        self.services
            .effects
            .view
            .as_deref_mut()
            .ok_or("Object FX has no receiving view")?
            .create_light_pulse(pulse)
            .map(|_| ())
    }
    fn shake(
        &mut self,
        position: gamelogic::common::Coord3D,
        kind: game_client::display::view::CameraShakeType,
    ) {
        let session = &mut self.services.session;
        let rules = session.camera.shake_policy.current();
        session.impulse.enqueue_with_rules(
            Vec3::new(position.x, position.z, position.y),
            session.target,
            kind.into(),
            &rules,
            &mut |lo, hi| session.client_random.next_real(lo, hi),
        );
    }
    fn scorch(&mut self, position: gamelogic::common::Coord3D, radius: f32, kind: i32) {
        self.services
            .scorch([position.x, position.y, position.z], radius, kind);
    }
    fn tracer(&mut self, request: &FxTracerRequest<'_>) -> Result<(), String> {
        let template = self
            .services
            .environment
            .things
            .get(request.template)
            .ok_or("receiving tracer definition absent")?;
        self.services
            .effects
            .view
            .as_deref_mut()
            .ok_or("receiving tracer view absent")?
            .spawn_tracer(template, request)
    }
    fn ray(&mut self, request: &FxRayRequest<'_>) -> Result<(), String> {
        let template = self
            .services
            .environment
            .things
            .get(request.template)
            .ok_or("receiving ray definition absent")?;
        self.services
            .effects
            .view
            .as_deref_mut()
            .ok_or("receiving ray view absent")?
            .create_ray(template, request)
    }
    fn bones(
        &mut self,
        id: u32,
        name: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let object = match self.primary.filter(|object| object.id() == id) {
            Some(BorrowedFxPrimary::Object(object)) => object,
            // Host projectile carriers have no installed native Drawable. Do not
            // borrow a different object's pose or synthesize a bone transform.
            Some(BorrowedFxPrimary::Projectile(_)) => return Ok(Vec::new()),
            None => return Err("receiving current FX Object absent".into()),
        };
        let source = self.services.environment.source(object)?;
        self.services
            .effects
            .view
            .as_deref_mut()
            .ok_or("receiving FX bone view absent")?
            .current_client_bone_transforms(&source, name, start, limit)
    }
}

impl crate::game_logic::combat::ProjectilePresentationRuntime for ObjectFxServices<'_, '_> {
    fn fx(&mut self, name: &str, source: crate::game_logic::combat::ProjectileFxObject<'_>) {
        match source {
            crate::game_logic::combat::ProjectileFxObject::Object(object) => {
                self.object(name, object, None);
            }
            crate::game_logic::combat::ProjectileFxObject::Projectile(projectile) => {
                self.projectile(name, projectile);
            }
        }
    }
    fn pose(&mut self, _: &crate::game_logic::combat::Projectile) {}
    fn attach_exhaust(
        &mut self,
        projectile: &crate::game_logic::combat::Projectile,
    ) -> Option<u32> {
        let manager = self.particle_systems_mut()?;
        if let Err(error) = manager.admit_effective_definitions() {
            log::error!("receiving missile exhaust definitions: {error}");
            return None;
        }
        let template = manager.find_template(&projectile.exhaust_name)?;
        let id = manager.create_particle_system(&template, true).ok()?;
        manager
            .find_particle_system_mut(id)?
            .attach_to_object(projectile.id.0);
        Some(id)
    }
}
