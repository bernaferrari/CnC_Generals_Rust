//! FXList.cpp nugget order through receiving presentation services.
//! No native operation chooses the ambient FX/audio/particle/view registries.
use super::*;
use crate::display::view::View;
use crate::effects::particle_manager::ParticleSystemManager;
use crate::terrain::scorch_mesh::TerrainScorchBuffer;
use game_engine::common::audio::audio_event_rts::AudioSelectionStreams;
use game_engine::common::audio::game_audio::{
    AudioLocalityResolver, AudioManager, AudioSubmissionLocality,
};
use game_engine::common::global_data::GlobalData;
use game_engine::common::random_value::RandomState;

#[cfg(test)]
#[path = "fx_owned_execution_tests.rs"]
mod tests;

/// Event-time object observations are supplied by the driving owner, not an ID map.
#[derive(Clone, Copy)]
pub struct FxInvocation {
    primary: Option<Coord3D>,
    matrix: Option<Matrix3D>,
    secondary: Option<Coord3D>,
    primary_speed: f32,
    override_radius: f32,
    object: Option<gamelogic::helpers::HostFxObjectPose>,
    visible: bool,
}

impl FxInvocation {
    pub fn at_position(
        primary: Option<Coord3D>,
        matrix: Option<Matrix3D>,
        primary_speed: f32,
        secondary: Option<Coord3D>,
        override_radius: f32,
        cell_visible: bool,
    ) -> Self {
        Self {
            primary,
            matrix,
            secondary,
            primary_speed,
            override_radius,
            object: None,
            visible: cell_visible,
        }
    }
    /// All redundant pose fields derive from the single driving observation.
    pub fn on_object(
        primary: Option<gamelogic::helpers::HostFxObjectPose>,
        secondary: Option<gamelogic::helpers::HostFxObjectPose>,
    ) -> Self {
        Self {
            primary: primary.map(|object| object.position),
            matrix: primary.map(|object| object.transform),
            secondary: secondary.map(|object| object.position),
            primary_speed: 0.0,
            override_radius: 0.0,
            object: primary,
            visible: primary.is_none_or(|object| !object.is_shrouded),
        }
    }
}

pub struct FxTracerRequest<'a> {
    pub template: &'a str,
    pub primary: Coord3D,
    pub secondary: Coord3D,
    pub speed: f32,
    pub length: f32,
    pub width: f32,
    pub color: [f32; 3],
    pub decay_at: f32,
    pub frame: u32,
}
pub struct FxRayRequest<'a> {
    pub template: &'a str,
    pub primary: Coord3D,
    pub secondary: Coord3D,
}

/// Actual drawable factory/renderer owner. Missing services are errors; this
/// boundary has no default recording-only or ambient implementation.
pub trait FxDrawableExecution {
    fn validate_tracer(&self, template: &str) -> Result<(), String>;
    fn spawn_tracer(&mut self, request: &FxTracerRequest<'_>) -> Result<(), String>;
    fn validate_ray(&self, template: &str) -> Result<(), String>;
    fn create_ray(&mut self, request: &FxRayRequest<'_>) -> Result<(), String>;
    fn bone_transforms(
        &mut self,
        object: u32,
        name: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<Matrix3D>, String>;
}

pub struct FxExecution<'a> {
    pub catalog: &'a FXListStore,
    pub particles: &'a mut ParticleSystemManager,
    pub audio: &'a mut AudioManager,
    pub audio_locality: &'a dyn AudioLocalityResolver,
    pub logic_random: &'a mut RandomState,
    pub audio_random: &'a mut RandomState,
    pub client_random: &'a mut RandomState,
    pub ground_height: &'a mut dyn FnMut(Vec3) -> Option<f32>,
    pub view: &'a mut View,
    pub rules: &'a GlobalData,
    pub scorches: &'a mut TerrainScorchBuffer,
    pub lights: &'a mut DisplayLightScene,
    pub drawables: Option<&'a mut dyn FxDrawableExecution>,
    pub frame: u32,
}

impl FxExecution<'_> {
    pub fn execute_named(&mut self, name: &str, input: &FxInvocation) -> Result<(), String> {
        if !input.visible {
            return Ok(());
        }
        let source = match input.object {
            Some(object) => super::FxSource::Objects {
                primary: object.id,
                secondary: None,
            },
            None => super::FxSource::Position {
                primary: input.primary,
                matrix: input.matrix,
                secondary: input.secondary,
                speed: input.primary_speed,
                radius: input.override_radius,
            },
        };
        super::execute_fx_with_runtime(&mut BorrowedFxRuntime { owner: self, input }, name, source)
            .and_then(|found| {
                if found {
                    Ok(())
                } else {
                    Err(format!(
                        "FX definition '{name}' unavailable in receiving catalog"
                    ))
                }
            })
    }
}
/// Explicit fixture/component loans use the same nugget algorithm as Main.
/// There is no second validation/execution implementation or registry fallback.
struct BorrowedFxRuntime<'a, 'b, 'c> {
    owner: &'a mut FxExecution<'b>,
    input: &'c FxInvocation,
}
impl super::GameClientFxRuntime for BorrowedFxRuntime<'_, '_, '_> {
    fn find_list(&mut self, name: &str) -> Option<Arc<FXList>> {
        self.owner.catalog.find_fx_list(name)
    }
    fn observe(&mut self, source: super::FxSource) -> super::FxObservation {
        match source {
            super::FxSource::Position {
                primary,
                matrix,
                secondary,
                speed,
                radius,
            } => super::FxObservation {
                primary,
                matrix,
                secondary,
                speed,
                radius,
                object: None,
            },
            super::FxSource::Objects { primary, .. } => {
                let object = self.input.object.filter(|object| object.id == primary);
                super::FxObservation {
                    primary: object.map(|object| object.position),
                    matrix: object.map(|object| object.transform),
                    secondary: self.input.secondary,
                    speed: 0.,
                    radius: 0.,
                    object,
                }
            }
        }
    }
    fn visible(&mut self, _: &super::FxObservation) -> bool {
        self.input.visible
    }
    fn random_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.owner.client_random.next_real(lo, hi)
    }
    fn random_u32(&mut self) -> u32 {
        self.owner.client_random.next_u32()
    }
    fn frame(&self) -> u32 {
        self.owner.frame
    }
    fn sound(&mut self, name: &str, input: &super::FxObservation) -> Result<(), String> {
        let player = input
            .object
            .map(|object| object.player_index)
            .filter(|&player| player >= 0);
        let locality =
            AudioSubmissionLocality::from_resolver(player.unwrap_or(-1), self.owner.audio_locality);
        let mut selection = AudioSelectionStreams {
            logic: &mut *self.owner.logic_random,
            audio: &mut *self.owner.audio_random,
        };
        submit_sound_fx_owned(
            name,
            input.primary.as_ref(),
            player,
            self.owner.audio,
            &locality,
            &mut selection,
        )
        .map(|_| ())
    }
    fn particles(
        &mut self,
        nugget: &crate::effects::fxlist_integration::ParticleSystemFXNugget,
        input: &super::FxObservation,
        matrix: Option<Mat3>,
    ) -> Result<(), String> {
        let Some(primary) = input.primary else {
            return Ok(());
        };
        nugget.do_fx_with_services(
            Vec3::new(primary.x, primary.y, primary.z),
            matrix.as_ref(),
            input.object.map(|object| object.id),
            if input.object.is_some() {
                0.
            } else {
                input.radius
            },
            self.owner.particles,
            &mut |lo, hi| self.owner.client_random.next_real(lo, hi),
            self.owner.ground_height,
        );
        Ok(())
    }
    fn light(&mut self, pulse: DisplayLightPulse) -> Result<(), String> {
        self.owner.lights.create_pulse(pulse);
        Ok(())
    }
    fn shake(&mut self, position: Coord3D, kind: crate::display::view::CameraShakeType) {
        self.owner.view.shake_with_services(
            &ViewPoint3::new(position.x, position.y, position.z),
            kind,
            self.owner.rules,
            &mut |lo, hi| self.owner.client_random.next_real(lo, hi),
        );
    }
    fn scorch(&mut self, position: Coord3D, radius: f32, kind: i32) {
        self.owner
            .scorches
            .add_scorch([position.x, position.y, position.z], radius, kind);
    }
    fn tracer(&mut self, request: &FxTracerRequest<'_>) -> Result<(), String> {
        let owner = self
            .owner
            .drawables
            .as_deref_mut()
            .ok_or("FX tracer requires receiving drawable owner")?;
        owner.validate_tracer(request.template)?;
        owner.spawn_tracer(request)
    }
    fn ray(&mut self, request: &FxRayRequest<'_>) -> Result<(), String> {
        let owner = self
            .owner
            .drawables
            .as_deref_mut()
            .ok_or("FX ray requires receiving drawable owner")?;
        owner.validate_ray(request.template)?;
        owner.create_ray(request)
    }
    fn bones(
        &mut self,
        object: u32,
        name: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<Matrix3D>, String> {
        self.owner
            .drawables
            .as_deref_mut()
            .ok_or("FX bones require receiving drawable owner")?
            .bone_transforms(object, name, start, limit)
    }
}
