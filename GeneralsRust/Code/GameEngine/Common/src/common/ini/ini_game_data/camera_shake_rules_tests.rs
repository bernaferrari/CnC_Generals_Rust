use super::*;

#[test]
fn source_eight_defaults_and_private_map_overlay_preserve_unspecified_fields() {
    // GlobalData.cpp851-858, in the original six-category order.
    let base = CameraShakeRules::default();
    assert_eq!(
        [
            base.shake_subtle_intensity,
            base.shake_normal_intensity,
            base.shake_strong_intensity,
            base.shake_severe_intensity,
            base.shake_cine_extreme_intensity,
            base.shake_cine_insane_intensity,
            base.max_shake_intensity,
            base.max_shake_range
        ],
        [0.5, 1., 2.5, 5., 8., 12., 10., 150.]
    );
    let peer = base;
    let mapped = base.overlay_ini_text("GameData\n ShakeSubtleIntensity = 0.25\n ShakeNormalIntensity = 3\n ShakeStrongIntensity = 4\n ShakeSevereIntensity = 6\n ShakeCineExtremeIntensity = 9\n ShakeCineInsaneIntensity = 13\n MaxShakeIntensity = 15\n MaxShakeRange = 250\n End\n").unwrap();
    assert_eq!(
        [
            mapped.shake_subtle_intensity,
            mapped.shake_normal_intensity,
            mapped.shake_strong_intensity,
            mapped.shake_severe_intensity,
            mapped.shake_cine_extreme_intensity,
            mapped.shake_cine_insane_intensity,
            mapped.max_shake_intensity,
            mapped.max_shake_range
        ],
        [0.25, 3., 4., 6., 9., 13., 15., 250.]
    );
    let partial = mapped
        .overlay_ini_text("GameData\n MaxShakeRange = 300\n End\n")
        .unwrap();
    assert_eq!(partial.max_shake_range, 300.);
    assert_eq!(partial.shake_cine_insane_intensity, 13.);
    assert_eq!(base, peer);
}

#[test]
fn finite_signed_rules_preserve_source_arithmetic_domain_and_nonfinite_rejects() {
    for range in [0., -1., 150.] {
        assert!(
            CameraShakeRules {
                max_shake_range: range,
                shake_normal_intensity: -2.,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
    assert!(
        CameraShakeRules {
            max_shake_range: f32::NAN,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        CameraShakeRules {
            shake_cine_insane_intensity: f32::INFINITY,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
