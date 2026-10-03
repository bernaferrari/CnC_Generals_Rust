use super::fields::*;
use crate::definition::*;
use std::collections::HashMap;

/// Unit conversions come from the engine INI contract, not runtime globals.
pub trait FxValueParser {
    fn velocity(&self, value: &str) -> FXListResult<f32>;
    fn percent(&self, value: &str) -> FXListResult<f32>;
    fn duration(&self, value: &str) -> FXListResult<u32>;
}

/// Parse one nested C++ FX nugget block while preserving repeated nuggets.
pub fn parse_fx_nugget_definition<Name: for<'a> From<&'a str>, Values: FxValueParser>(
    values: &Values,
    kind: &str,
    properties: &HashMap<String, String>,
) -> FXListResult<FXNugget<Name>> {
    let name = |field: &str| Name::from(property(properties, field).unwrap_or(""));
    match kind.to_ascii_lowercase().as_str() {
        "sound" => Ok(FXNugget::Sound { name: name("Name") }),
        "tracer" => Ok(FXNugget::Tracer {
            name: Name::from(property(properties, "TracerName").unwrap_or("GenericTracer")),
            bone_name: name("BoneName"),
            speed: property(properties, "Speed")
                .map(|value| values.velocity(value))
                .transpose()?
                .unwrap_or(0.0),
            decay_at: real(properties, "DecayAt", 1.0)?,
            length: real(properties, "Length", 10.0)?,
            width: real(properties, "Width", 1.0)?,
            color: property(properties, "Color")
                .map(color)
                .transpose()?
                .unwrap_or((1.0, 1.0, 1.0)),
            probability: real(properties, "Probability", 1.0)?,
        }),
        "rayeffect" => Ok(FXNugget::RayEffect {
            name: name("Name"),
            primary_offset: property(properties, "PrimaryOffset")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
            secondary_offset: property(properties, "SecondaryOffset")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
        }),
        "lightpulse" => Ok(FXNugget::LightPulse {
            color: property(properties, "Color")
                .map(color)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
            radius: real(properties, "Radius", 0.0)?,
            radius_as_percent_of_object_size: property(properties, "RadiusAsPercentOfObjectSize")
                .map(|v| values.percent(v))
                .transpose()?
                .unwrap_or(0.0),
            increase_frames: property(properties, "IncreaseTime")
                .map(|v| values.duration(v))
                .transpose()
                .map_err(|_| FXListError::ParseError("invalid IncreaseTime".into()))?
                .unwrap_or(0),
            decrease_frames: property(properties, "DecreaseTime")
                .map(|v| values.duration(v))
                .transpose()
                .map_err(|_| FXListError::ParseError("invalid DecreaseTime".into()))?
                .unwrap_or(0),
        }),
        "viewshake" => {
            let shake_type = match property(properties, "Type")
                .unwrap_or("NORMAL")
                .to_ascii_uppercase()
                .as_str()
            {
                "SUBTLE" => CameraShakeType::Subtle,
                "NORMAL" => CameraShakeType::Normal,
                "STRONG" => CameraShakeType::Strong,
                "SEVERE" => CameraShakeType::Severe,
                "CINE_EXTREME" => CameraShakeType::CineExtreme,
                "CINE_INSANE" => CameraShakeType::CineInsane,
                other => {
                    return Err(FXListError::ParseError(format!(
                        "unknown view shake type '{}'",
                        other
                    )));
                }
            };
            Ok(FXNugget::ViewShake { shake_type })
        }
        "terrainscorch" => {
            let scorch_type = match property(properties, "Type")
                .unwrap_or("RANDOM")
                .to_ascii_uppercase()
                .as_str()
            {
                "SCORCH_1" => ScorchType::Scorch1,
                "SCORCH_2" => ScorchType::Scorch2,
                "SCORCH_3" => ScorchType::Scorch3,
                "SCORCH_4" => ScorchType::Scorch4,
                "SHADOW_SCORCH" => ScorchType::ShadowScorch,
                "RANDOM" => ScorchType::Random,
                other => {
                    return Err(FXListError::ParseError(format!(
                        "unknown terrain scorch type '{}'",
                        other
                    )));
                }
            };
            Ok(FXNugget::TerrainScorch {
                scorch_type,
                radius: real(properties, "Radius", 0.0)?,
            })
        }
        "particlesystem" => Ok(FXNugget::ParticleSystem {
            name: name("Name"),
            count: integer(properties, "Count", 1)?,
            offset: property(properties, "Offset")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
            radius: random_variable(properties, "Radius", 0.0)?,
            height: random_variable(properties, "Height", 0.0)?,
            initial_delay: random_variable(properties, "InitialDelay", -1.0)?,
            rotate_x: real(properties, "RotateX", 0.0)?.to_radians(),
            rotate_y: real(properties, "RotateY", 0.0)?.to_radians(),
            rotate_z: real(properties, "RotateZ", 0.0)?.to_radians(),
            orient_to_object: boolean(properties, "OrientToObject", false)?,
            ricochet: boolean(properties, "Ricochet", false)?,
            attach_to_object: boolean(properties, "AttachToObject", false)?,
            create_at_ground_height: boolean(properties, "CreateAtGroundHeight", false)?,
            use_callers_radius: boolean(properties, "UseCallersRadius", false)?,
        }),
        "fxlistatbonepos" => Ok(FXNugget::FXListAtBonePos {
            fx_name: name("FX"),
            bone_name: name("BoneName"),
            orient_to_bone: boolean(properties, "OrientToBone", true)?,
        }),
        _ => Err(FXListError::ParseError(format!(
            "unknown FX nugget type '{}'",
            kind
        ))),
    }
}
