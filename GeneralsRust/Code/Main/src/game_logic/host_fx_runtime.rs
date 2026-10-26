//! Native FX executes one receiving operation at each original nugget boundary.
use super::*;
use game_client::fx_list::{
    FxObservation, FxRayRequest, FxSource, FxTracerRequest, GameClientFxRuntime,
};
use game_engine::common::random_value::RandomState;

#[derive(Default)]
pub(crate) struct HostFxState {
    pub(super) catalog: Option<game_client::fx_list::FXListStore>,
}

impl std::fmt::Debug for HostFxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostFxState")
            .field("catalog_admitted", &self.catalog.is_some())
            .finish_non_exhaustive()
    }
}
pub(crate) struct HostFxRuntime<'a, 's> {
    pub(crate) world: &'a mut crate::game_logic::GameLogic,
    pub(crate) effects: &'a mut crate::game_logic::ScriptPresentationEffects<'s>,
}
impl crate::game_logic::GameLogic {
    pub(crate) fn seed_fx_random(&mut self, seed: u32) {
        self.session_view.client_random.seed_random(seed);
        self.session_view.audio_random.seed_random(seed);
    }
    pub(crate) fn append_fx_random_capsule(&self, bytes: &mut Vec<u8>) {
        for word in self
            .session_view
            .client_random
            .seed_words()
            .into_iter()
            .chain(self.session_view.audio_random.seed_words())
        {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(b"FXR1");
    }
    pub(crate) fn restore_fx_random_capsule(&mut self, words: Option<[[u32; 6]; 2]>) {
        if let Some(words) = words {
            self.session_view.client_random.set_seed_words(words[0]);
            self.session_view.audio_random.set_seed_words(words[1]);
        }
    }
    pub(crate) fn split_fx_random_capsule(
        bytes: &[u8],
    ) -> Result<(&[u8], Option<[[u32; 6]; 2]>), String> {
        if bytes.len() < 4 || &bytes[bytes.len() - 4..] != b"FXR1" {
            return Ok((bytes, None));
        }
        if bytes.len() < 52 {
            return Err("truncated native FX random capsule".into());
        }
        let start = bytes.len() - 52;
        let words = std::array::from_fn(|stream| {
            std::array::from_fn(|word| {
                u32::from_le_bytes(
                    bytes[start + stream * 24 + word * 4..start + stream * 24 + word * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            })
        });
        Ok((&bytes[..start], Some(words)))
    }

    /// App/content admission hands the selected immutable catalog to one
    /// session. Fixtures use this same receiver without process registrations.
    pub(crate) fn admit_fx_catalog(&mut self, catalog: game_client::fx_list::FXListStore) {
        self.session_view.fx.catalog = Some(catalog);
    }
    pub(crate) fn admit_fx_definitions(&mut self) {
        // Explicit map/content setup only, never called by a runtime lookup.
        let catalog = game_client::fx_list::get_fx_list_store().clone();
        self.admit_fx_catalog(catalog);
    }
}
impl HostFxRuntime<'_, '_> {
    fn view(&mut self) -> Result<&mut (dyn crate::game_logic::NativeViewEffects + '_), String> {
        match self.effects.view.as_deref_mut() {
            Some(view) => Ok(view),
            None => Err("FX requires receiving native view".into()),
        }
    }
    fn template(
        &self,
        name: &str,
    ) -> Result<std::sync::Arc<game_engine::common::thing::ThingTemplate>, String> {
        self.world
            .receiving_module_content
            .as_ref()
            .and_then(|content| {
                content
                    .compatibility_template_view()
                    .find_template(name, false)
            })
            .ok_or_else(|| format!("receiving FX Drawable template '{name}' unavailable"))
    }
}
impl GameClientFxRuntime for HostFxRuntime<'_, '_> {
    fn find_list(&mut self, name: &str) -> Option<std::sync::Arc<game_client::fx_list::FXList>> {
        self.world
            .session_view
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
            FxSource::Objects { primary, secondary } => {
                let player = self.world.local_player_id();
                let object = self
                    .world
                    .host_object(crate::game_logic::ObjectId(primary))
                    .map(|object| {
                        crate::game_logic::GameLogic::observe_host_fx_object(
                            object,
                            player,
                            self.world.world_services.shroud(),
                        )
                    });
                let secondary = secondary
                    .and_then(|id| self.world.host_object(crate::game_logic::ObjectId(id)))
                    .map(|object| host_to_leftover_coord(object.get_position()));
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
        let (Some(player), Some(position)) = (self.world.local_player_id(), input.primary) else {
            return false;
        };
        self.world
            .world_services
            .shroud()
            .lock()
            .map(|shroud| {
                matches!(
                    shroud.get_shroud_state(player, &position),
                    gamelogic::system::shroud_manager::ShroudState::Visible
                )
            })
            .unwrap_or(false)
    }
    fn random_real(&mut self, low: f32, high: f32) -> f32 {
        self.world.session_view.client_random.next_real(low, high)
    }
    fn random_u32(&mut self) -> u32 {
        self.world.session_view.client_random.next_u32()
    }
    fn frame(&self) -> u32 {
        self.world.frame
    }
    fn sound(&mut self, name: &str, input: &FxObservation) -> Result<(), String> {
        if name.is_empty() || name == "NoSound" {
            return Ok(());
        }
        let player = input
            .object
            .map(|object| object.player_index)
            .filter(|player| *player >= 0);
        let locality = self
            .world
            .receiving_audio_locality_for_player(player.unwrap_or(-1));
        let mut selection = FxAudioSelection {
            logic: &self.world.logic_random,
            audio: &mut self.world.session_view.audio_random,
        };
        let Some(audio) = self.effects.audio.as_deref_mut() else {
            return Ok(());
        };
        audio.submit_fx_sound(
            name,
            input.primary.as_ref(),
            player,
            &locality,
            &mut selection,
            self.world.queued_audio_events.policy(),
        )
    }
    fn particles(
        &mut self,
        nugget: &game_client::effects::fxlist_integration::ParticleSystemFXNugget,
        input: &FxObservation,
        matrix: Option<glam::Mat3>,
    ) -> Result<(), String> {
        let Some(primary) = input.primary else {
            return Ok(());
        };
        let world = &mut *self.world;
        let random = &mut world.session_view.client_random;
        let terrain = world.world_services.terrain();
        let view = self
            .effects
            .view
            .as_deref_mut()
            .ok_or("particle FX requires receiving native view")?;
        let manager = view
            .particle_systems_mut()
            .ok_or("particle FX requires receiving namespace")?;
        manager
            .admit_effective_definitions()
            .map_err(|error| error.to_string())?;
        nugget.do_fx_with_services(
            glam::Vec3::new(primary.x, primary.y, primary.z),
            matrix.as_ref(),
            input.object.map(|object| object.id),
            if input.object.is_some() {
                0.
            } else {
                input.radius
            },
            manager,
            &mut |low, high| random.next_real(low, high),
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
        self.view()?.create_light_pulse(pulse).map(|_| ())
    }
    fn shake(
        &mut self,
        position: gamelogic::common::Coord3D,
        kind: game_client::display::view::CameraShakeType,
    ) {
        let view = &mut self.world.session_view;
        let target = view.target;
        let rules = view.camera.shake_policy.current();
        view.impulse.enqueue_with_rules(
            Vec3::new(position.x, position.z, position.y),
            target,
            kind.into(),
            &rules,
            &mut |low, high| view.client_random.next_real(low, high),
        );
    }
    fn scorch(&mut self, position: gamelogic::common::Coord3D, radius: f32, kind: i32) {
        if let Some(terrain) = self.effects.terrain.as_deref_mut() {
            terrain.add_scorch([position.x, position.y, position.z], radius, kind);
        }
    }
    fn tracer(&mut self, request: &FxTracerRequest<'_>) -> Result<(), String> {
        let template = self.template(request.template)?;
        self.view()?.spawn_tracer(&template, request)
    }
    fn ray(&mut self, request: &FxRayRequest<'_>) -> Result<(), String> {
        let template = self.template(request.template)?;
        self.view()?.create_ray(&template, request)
    }
    fn bones(
        &mut self,
        object: u32,
        name: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let player = self
            .world
            .local_player_id()
            .ok_or("FX bone query has no receiving local player")?;
        let source = self
            .world
            .current_native_slave_source(crate::game_logic::ObjectId(object), player)
            .map_err(str::to_string)?;
        self.view()?
            .current_client_bone_transforms(&source, name, start, limit)
    }
}
pub(super) use crate::game_logic::SessionAudioSelection as FxAudioSelection;
