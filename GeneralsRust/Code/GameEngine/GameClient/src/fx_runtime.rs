//! One bounded receiver operation per C++ FX nugget. No list-wide mutable loan.
use super::*;

#[derive(Clone, Copy)]
pub enum FxSource {
    Position {
        primary: Option<Coord3D>,
        matrix: Option<Matrix3D>,
        secondary: Option<Coord3D>,
        speed: f32,
        radius: f32,
    },
    Objects {
        primary: u32,
        secondary: Option<u32>,
    },
}
#[derive(Clone, Copy)]
pub struct FxObservation {
    pub primary: Option<Coord3D>,
    pub matrix: Option<Matrix3D>,
    pub secondary: Option<Coord3D>,
    pub speed: f32,
    pub radius: f32,
    pub object: Option<gamelogic::helpers::HostFxObjectPose>,
}
/// The driving world implements this facade. Each call ends its service borrow
/// before the next nugget (or nested list) observes the world again.
pub trait GameClientFxRuntime {
    fn find_list(&mut self, name: &str) -> Option<Arc<FXList>>;
    fn observe(&mut self, source: FxSource) -> FxObservation;
    fn visible(&mut self, observation: &FxObservation) -> bool;
    fn random_real(&mut self, low: f32, high: f32) -> f32;
    fn random_u32(&mut self) -> u32;
    fn frame(&self) -> u32;
    fn sound(&mut self, name: &str, observation: &FxObservation) -> Result<(), String>;
    fn particles(
        &mut self,
        nugget: &crate::effects::fxlist_integration::ParticleSystemFXNugget,
        observation: &FxObservation,
        matrix: Option<Mat3>,
    ) -> Result<(), String>;
    fn light(&mut self, pulse: DisplayLightPulse) -> Result<(), String>;
    fn shake(&mut self, position: Coord3D, kind: crate::display::view::CameraShakeType);
    fn scorch(&mut self, position: Coord3D, radius: f32, kind: i32);
    fn tracer(&mut self, request: &FxTracerRequest<'_>) -> Result<(), String>;
    fn ray(&mut self, request: &FxRayRequest<'_>) -> Result<(), String>;
    fn bones(
        &mut self,
        object: u32,
        name: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<Matrix3D>, String>;
}

pub fn execute_fx_with_runtime(
    runtime: &mut dyn GameClientFxRuntime,
    name: &str,
    source: FxSource,
) -> Result<bool, String> {
    let Some(list) = runtime.find_list(name) else {
        return Ok(false);
    };
    let observation = runtime.observe(source);
    if matches!(source, FxSource::Objects { .. }) && observation.object.is_none() {
        return Ok(false);
    }
    if !runtime.visible(&observation) {
        return Ok(true);
    }
    execute_list(runtime, &list, source, 0)?;
    Ok(true)
}
fn execute_list(
    runtime: &mut dyn GameClientFxRuntime,
    list: &FXList,
    source: FxSource,
    depth: usize,
) -> Result<(), String> {
    if depth >= 128 {
        return Err("recursive FX list exceeds receiving execution stack".into());
    }
    for nugget in &list.nuggets {
        // C++ doFXObj rereads objects at the next callback; do not freeze a
        // NativeSlaveSource/object-store loan across this list's callbacks.
        let input = runtime.observe(source);
        let nugget: &dyn std::any::Any = nugget.as_ref();
        if let Some(sound) = nugget.downcast_ref::<SoundFXNugget>() {
            runtime.sound(&sound.sound_name, &input)?;
        } else if let Some(particle) = nugget.downcast_ref::<ParticleSystemWrapper>() {
            if input.primary.is_none() {
                continue;
            }
            let matrix = if particle.nugget.ricochet && input.object.is_some() {
                input
                    .secondary
                    .and_then(|secondary| {
                        input.primary.map(|primary| {
                            let (s, c) = (primary.y - secondary.y)
                                .atan2(primary.x - secondary.x)
                                .sin_cos();
                            Mat3::from_cols(Vec3::new(c, s, 0.), Vec3::new(-s, c, 0.), Vec3::Z)
                        })
                    })
                    .or_else(|| input.matrix.map(|matrix| particle_fx_basis(&matrix)))
            } else {
                input.matrix.map(|matrix| particle_fx_basis(&matrix))
            };
            runtime.particles(&particle.nugget, &input, matrix)?;
        } else if let Some(light) = nugget.downcast_ref::<LightPulseFXNugget>() {
            if let Some(primary) = input.primary {
                let radius = input
                    .object
                    .filter(|_| light.bounding_circle_pct > 0.)
                    .map_or(light.radius, |object| {
                        object.bounding_circle_radius * light.bounding_circle_pct
                    });
                runtime.light(light.pulse_at(&primary, radius))?;
            }
        } else if let Some(shake) = nugget.downcast_ref::<ViewShakeFXNugget>() {
            if let Some(primary) = input.primary {
                runtime.shake(primary, to_view_shake(shake.shake_type));
            }
        } else if let Some(scorch) = nugget.downcast_ref::<TerrainScorchFXNugget>() {
            if let Some(primary) = input.primary {
                let kind = if scorch.scorch < 0 {
                    (runtime.random_u32() % 4) as i32
                } else {
                    scorch.scorch
                };
                runtime.scorch(primary, scorch.radius, kind);
            }
        } else if let Some(tracer) = nugget.downcast_ref::<TracerFXNugget>() {
            if tracer.probability <= runtime.random_real(0., 1.) {
                continue;
            }
            let (Some(primary), Some(secondary)) = (input.primary, input.secondary) else {
                continue;
            };
            runtime.tracer(&FxTracerRequest {
                template: &tracer.tracer_name,
                primary,
                secondary,
                speed: if tracer.speed == 0. {
                    input.speed
                } else {
                    tracer.speed
                },
                length: tracer.length,
                width: tracer.width,
                color: tracer.color.to_array(),
                decay_at: tracer.decay_at,
                frame: runtime.frame(),
            })?;
        } else if let Some(ray) = nugget.downcast_ref::<RayEffectFXNugget>() {
            let (Some(primary), Some(secondary)) = (input.primary, input.secondary) else {
                continue;
            };
            runtime.ray(&FxRayRequest {
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
            let Some(nested) = runtime.find_list(&bone.fx_name) else {
                return Err(format!("nested FX '{}' unavailable", bone.fx_name));
            };
            for start in [0, 1] {
                let matrices = runtime.bones(
                    object.id,
                    &bone.bone_name,
                    start,
                    FXListAtBonePosFXNugget::MAX_BONE_POINTS,
                )?;
                for matrix in matrices {
                    // Refresh the object's transform at the source callback point.
                    let current = runtime.observe(source);
                    let Some(object) = current.object else {
                        continue;
                    };
                    let world = object.transform * matrix;
                    let (_, _, position) = world.to_scale_rotation_translation();
                    let nested_source = FxSource::Position {
                        primary: Some(position),
                        matrix: Some(world),
                        secondary: None,
                        speed: 0.,
                        radius: 0.,
                    };
                    // C++ calls FXList::doFXPos at each bone, so the receiving
                    // cell gate applies again at each generated world position.
                    let observation = runtime.observe(nested_source);
                    if runtime.visible(&observation) {
                        execute_list(runtime, &nested, nested_source, depth + 1)?;
                    }
                }
            }
        } else {
            return Err("FX nugget has no receiving execution contract".into());
        }
    }
    Ok(())
}
