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
        let list = self
            .catalog
            .find_fx_list(name)
            .ok_or_else(|| format!("FX definition '{name}' unavailable in receiving catalog"))?;
        self.validate_list(&list, input, &mut Vec::new())?;
        self.execute_list(&list, input)
    }

    fn validate_list(
        &self,
        list: &FXList,
        input: &FxInvocation,
        visiting: &mut Vec<String>,
    ) -> Result<(), String> {
        for nugget in &list.nuggets {
            let nugget: &dyn std::any::Any = nugget.as_ref();
            if let Some(sound) = nugget.downcast_ref::<SoundFXNugget>() {
                if !sound.sound_name.is_empty()
                    && sound.sound_name != "NoSound"
                    && self
                        .audio
                        .find_audio_event_info(&sound.sound_name)
                        .is_none()
                {
                    return Err(format!(
                        "FX sound '{}' unavailable in receiving audio owner",
                        sound.sound_name
                    ));
                }
            } else if let Some(particle) = nugget.downcast_ref::<ParticleSystemWrapper>() {
                if input.primary.is_none() {
                    continue;
                }
                if self
                    .particles
                    .find_template(&particle.nugget.template_name)
                    .is_none()
                {
                    return Err(format!(
                        "FX particle '{}' unavailable in receiving particle owner",
                        particle.nugget.template_name
                    ));
                }
            } else if let Some(tracer) = nugget.downcast_ref::<TracerFXNugget>() {
                if input.primary.is_none() || input.secondary.is_none() {
                    continue;
                }
                self.drawables
                    .as_deref()
                    .ok_or("FX tracer requires receiving drawable owner")?
                    .validate_tracer(&tracer.tracer_name)?;
            } else if let Some(ray) = nugget.downcast_ref::<RayEffectFXNugget>() {
                if input.primary.is_none() || input.secondary.is_none() {
                    continue;
                }
                self.drawables
                    .as_deref()
                    .ok_or("FX ray requires receiving drawable owner")?
                    .validate_ray(&ray.template_name)?;
            } else if let Some(bone) = nugget.downcast_ref::<FXListAtBonePosFXNugget>() {
                if input.object.is_none() || bone.bone_name.is_empty() {
                    continue;
                }
                if visiting.contains(&bone.fx_name) {
                    return Err(format!("Cyclic FX bone definition '{}'", bone.fx_name));
                }
                self.drawables
                    .as_deref()
                    .ok_or("FX bones require receiving drawable owner")?;
                let nested = self
                    .catalog
                    .find_fx_list(&bone.fx_name)
                    .ok_or_else(|| format!("Nested FX '{}' unavailable", bone.fx_name))?;
                visiting.push(bone.fx_name.clone());
                let nested_input = FxInvocation {
                    object: None,
                    secondary: None,
                    primary_speed: 0.0,
                    override_radius: 0.0,
                    ..*input
                };
                self.validate_list(&nested, &nested_input, visiting)?;
                visiting.pop();
            } else if !nugget.is::<LightPulseFXNugget>()
                && !nugget.is::<ViewShakeFXNugget>()
                && !nugget.is::<TerrainScorchFXNugget>()
            {
                return Err("FX nugget has no receiving-owner execution contract".into());
            }
        }
        Ok(())
    }

    fn execute_list(&mut self, list: &FXList, input: &FxInvocation) -> Result<(), String> {
        for nugget in &list.nuggets {
            let nugget: &dyn std::any::Any = nugget.as_ref();
            if let Some(sound) = nugget.downcast_ref::<SoundFXNugget>() {
                let player = input
                    .object
                    .map(|object| object.player_index)
                    .filter(|&player| player >= 0);
                let locality = AudioSubmissionLocality::from_resolver(
                    player.unwrap_or(-1),
                    self.audio_locality,
                );
                let mut selection = AudioSelectionStreams {
                    logic: &mut *self.logic_random,
                    audio: &mut *self.audio_random,
                };
                submit_sound_fx_owned(
                    &sound.sound_name,
                    input.primary.as_ref(),
                    player,
                    self.audio,
                    &locality,
                    &mut selection,
                )?;
            } else if let Some(particle) = nugget.downcast_ref::<ParticleSystemWrapper>() {
                let Some(primary) = input.primary else {
                    continue;
                };
                let matrix = if particle.nugget.ricochet && input.object.is_some() {
                    input
                        .secondary
                        .map(|secondary| {
                            let (s, c) = (primary.y - secondary.y)
                                .atan2(primary.x - secondary.x)
                                .sin_cos();
                            Mat3::from_cols(Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0), Vec3::Z)
                        })
                        .or_else(|| input.matrix.map(|matrix| particle_fx_basis(&matrix)))
                } else {
                    input.matrix.map(|matrix| particle_fx_basis(&matrix))
                };
                let object = input.object.map(|object| object.id);
                let radius = if object.is_some() {
                    0.0
                } else {
                    input.override_radius
                };
                particle.nugget.do_fx_with_services(
                    Vec3::new(primary.x, primary.y, primary.z),
                    matrix.as_ref(),
                    object,
                    radius,
                    self.particles,
                    &mut |low, high| self.client_random.next_real(low, high),
                    self.ground_height,
                );
            } else if let Some(light) = nugget.downcast_ref::<LightPulseFXNugget>() {
                let Some(primary) = input.primary else {
                    continue;
                };
                let radius = input
                    .object
                    .filter(|_| light.bounding_circle_pct > 0.0)
                    .map_or(light.radius, |object| {
                        object.bounding_circle_radius * light.bounding_circle_pct
                    });
                self.lights.create_pulse(light.pulse_at(&primary, radius));
            } else if let Some(shake) = nugget.downcast_ref::<ViewShakeFXNugget>() {
                if let Some(primary) = input.primary {
                    self.view.shake_with_services(
                        &ViewPoint3::new(primary.x, primary.y, primary.z),
                        to_view_shake(shake.shake_type),
                        self.rules,
                        &mut |low, high| self.client_random.next_real(low, high),
                    );
                }
            } else if let Some(scorch) = nugget.downcast_ref::<TerrainScorchFXNugget>() {
                if let Some(primary) = input.primary {
                    let kind = if scorch.scorch < 0 {
                        (self.client_random.next_u32() % 4) as i32
                    } else {
                        scorch.scorch
                    };
                    self.scorches.add_scorch(
                        [primary.x, primary.y, primary.z],
                        scorch.radius,
                        kind,
                    );
                }
            } else if let Some(tracer) = nugget.downcast_ref::<TracerFXNugget>() {
                if tracer.probability <= self.client_random.next_real(0.0, 1.0) {
                    continue;
                }
                let (Some(primary), Some(secondary)) = (input.primary, input.secondary) else {
                    continue;
                };
                self.drawables
                    .as_deref_mut()
                    .ok_or("FX tracer requires receiving drawable owner")?
                    .spawn_tracer(&FxTracerRequest {
                        template: &tracer.tracer_name,
                        primary,
                        secondary,
                        speed: if tracer.speed == 0.0 {
                            input.primary_speed
                        } else {
                            tracer.speed
                        },
                        length: tracer.length,
                        width: tracer.width,
                        color: tracer.color.to_array(),
                        decay_at: tracer.decay_at,
                        frame: self.frame,
                    })?;
            } else if let Some(ray) = nugget.downcast_ref::<RayEffectFXNugget>() {
                let (Some(primary), Some(secondary)) = (input.primary, input.secondary) else {
                    continue;
                };
                self.drawables
                    .as_deref_mut()
                    .ok_or("FX ray requires receiving drawable owner")?
                    .create_ray(&FxRayRequest {
                        template: &ray.template_name,
                        primary: primary + ray.primary_offset,
                        secondary: secondary + ray.secondary_offset,
                    })?;
            } else if let Some(bone) = nugget.downcast_ref::<FXListAtBonePosFXNugget>() {
                let Some(object) = input.object else {
                    continue;
                };
                if bone.bone_name.is_empty() {
                    continue;
                }
                let nested = self
                    .catalog
                    .find_fx_list(&bone.fx_name)
                    .ok_or("Nested FX definition disappeared")?;
                for start in [0, 1] {
                    let matrices = self
                        .drawables
                        .as_deref_mut()
                        .ok_or("FX bones require receiving drawable owner")?
                        .bone_transforms(
                            object.id,
                            &bone.bone_name,
                            start,
                            FXListAtBonePosFXNugget::MAX_BONE_POINTS,
                        )?;
                    for matrix in matrices {
                        let world = object.transform * matrix;
                        let (_, _, position) = world.to_scale_rotation_translation();
                        let nested_input = FxInvocation {
                            primary: Some(position),
                            matrix: Some(world),
                            secondary: None,
                            primary_speed: 0.0,
                            override_radius: 0.0,
                            object: None,
                            visible: input.visible,
                        };
                        self.execute_list(&nested, &nested_input)?;
                    }
                }
            }
        }
        Ok(())
    }
}
