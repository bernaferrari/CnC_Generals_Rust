use super::*;
use std::collections::HashMap;

struct Values;
impl FxValueParser for Values {
    fn velocity(&self, v: &str) -> FXListResult<f32> {
        Ok(v.parse::<f32>().unwrap() / 30.0)
    }
    fn percent(&self, v: &str) -> FXListResult<f32> {
        Ok(v.trim_end_matches('%').parse::<f32>().unwrap() / 100.0)
    }
    fn duration(&self, v: &str) -> FXListResult<u32> {
        Ok((v.parse::<f32>().unwrap() * 30.0 / 1000.0).ceil() as u32)
    }
}
fn parse(kind: &str, fields: &[(&str, &str)]) -> FXListResult<FXNugget> {
    parse_fx_nugget_definition(
        &Values,
        kind,
        &fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<HashMap<_, _>>(),
    )
}
#[test]
fn tracer_defaults_and_conversions_match_cpp() {
    let FXNugget::Tracer {
        name,
        speed,
        color,
        length,
        width,
        decay_at,
        probability,
        ..
    } = parse("Tracer", &[("Speed", "900")]).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (name.as_str(), speed, color),
        ("GenericTracer", 30.0, (1.0, 1.0, 1.0))
    );
    assert_eq!(
        (length, width, decay_at, probability),
        (10.0, 1.0, 1.0, 1.0)
    );
}
#[test]
fn particle_ranges_are_not_sampled_or_truncated() {
    let FXNugget::ParticleSystem {
        radius,
        height,
        initial_delay,
        rotate_y,
        ..
    } = parse(
        "ParticleSystem",
        &[
            ("Radius", "3 17 UNIFORM"),
            ("Height", "2 8 GAUSSIAN"),
            ("RotateY", "90"),
        ],
    )
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (radius.minimum, radius.maximum, radius.distribution),
        (3.0, 17.0, Distribution::Uniform)
    );
    assert_eq!(
        (height.minimum, height.maximum, height.distribution),
        (2.0, 8.0, Distribution::Gaussian)
    );
    assert_eq!(initial_delay, (-1.0).into());
    assert_eq!(rotate_y, std::f32::consts::FRAC_PI_2);
}
#[test]
fn light_color_percent_and_duration_use_cpp_units() {
    let FXNugget::LightPulse {
        color,
        radius_as_percent_of_object_size,
        increase_frames,
        decrease_frames,
        ..
    } = parse(
        "LightPulse",
        &[
            ("Color", "R:255 G:128 B:0"),
            ("RadiusAsPercentOfObjectSize", "25"),
            ("IncreaseTime", "1"),
            ("DecreaseTime", "500"),
        ],
    )
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(color, (1.0, 128.0 / 255.0, 0.0));
    assert_eq!(
        (
            radius_as_percent_of_object_size,
            increase_frames,
            decrease_frames
        ),
        (0.25, 1, 15)
    );
}
#[test]
fn all_eight_nugget_types_keep_their_cpp_defaults() {
    for kind in [
        "Sound",
        "Tracer",
        "RayEffect",
        "LightPulse",
        "ViewShake",
        "TerrainScorch",
        "ParticleSystem",
        "FXListAtBonePos",
    ] {
        parse(kind, &[]).unwrap();
    }
    assert!(matches!(
        parse("FXListAtBonePos", &[]).unwrap(),
        FXNugget::FXListAtBonePos {
            orient_to_bone: true,
            ..
        }
    ));
    assert!(matches!(
        parse("TerrainScorch", &[]).unwrap(),
        FXNugget::TerrainScorch {
            scorch_type: ScorchType::Random,
            ..
        }
    ));
}
#[test]
fn malformed_ranges_colors_and_types_fail_closed() {
    for (kind, fields) in [
        ("ParticleSystem", vec![("Radius", "1 9 NORMAL")]),
        ("ParticleSystem", vec![("Radius", "1")]),
        ("Tracer", vec![("Color", "R:256 G:0 B:0")]),
        ("RayEffect", vec![("PrimaryOffset", "X:1 Y:2")]),
        ("ViewShake", vec![("Type", "UNKNOWN")]),
    ] {
        assert!(parse(kind, &fields).is_err());
    }
    assert!(parse("Unknown", &[]).is_err());
}
#[test]
fn retail_offset_numeric_prefix_is_accepted() {
    let FXNugget::RayEffect { primary_offset, .. } =
        parse("RayEffect", &[("PrimaryOffset", "X:1 Y:15: Z:-2")]).unwrap()
    else {
        panic!()
    };
    assert_eq!(primary_offset, (1.0, 15.0, -2.0));
}
#[test]
fn owned_catalogs_preserve_exact_names_null_and_replacement() {
    let mut first: FxCatalog<i32> = FxCatalog::default();
    let mut second: FxCatalog<i32> = FxCatalog::default();
    first.insert("FX_Same".into(), 1);
    second.insert("FX_Same".into(), 2);
    assert_eq!(first.find("FX_Same"), Some(&1));
    assert_eq!(second.find("FX_Same"), Some(&2));
    first.insert("FX_Same".into(), 3);
    assert_eq!(first.find("FX_Same"), Some(&3));
    assert_eq!(first.find("fx_same"), None);
    first.insert("None".into(), 4);
    assert_eq!(first.find("nOnE"), None);
    first = FxCatalog::default();
    assert_eq!(first.find("FX_Same"), None);
    assert_eq!(second.find("FX_Same"), Some(&2));
}
