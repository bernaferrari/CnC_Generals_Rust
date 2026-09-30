//! C++ timing regressions staged before the production calculation changes.
use super::super::*;
use super::helpers::*;

#[test]
fn unmapped_dozer_hp_gain_uses_energy_adjusted_build_time_frames() {
    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);

    let sid = logic
        .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("scaffold");
    if let Some(site) = logic.host_object_mut(sid) {
        site.thing.template.build_time = 10.0; // 300 authored frames
        site.construction_percent = 0.0;
    }
    let did = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));

    let before = logic.host_object(sid).expect("site before").clone();
    let power = logic.compute_player_power_factors();
    assert_eq!(
        power.get(&0).copied(),
        Some(0.5),
        "empty grid is half speed"
    );

    // Main host-authoritative production path; one elapsed second is 30 logic frames.
    logic.update_construction(&[sid], 1.0);
    let after = logic.host_object(sid).expect("site after");
    let progress = after.construction_percent - before.construction_percent;
    let health_gain = after.health.current - before.health.current;

    // C++ calcTimeToBuild divides 300 authored frames by the 0.5 low-power
    // rate, then DozerAIUpdate uses that same 600-frame count for both values.
    // Thus each construction percentage point adds the same fraction of max HP.
    let expected_hp_gain = before.health.maximum * progress;
    assert!(
        (health_gain - expected_hp_gain).abs() < 0.05,
        "C++ uses the energy-adjusted 600-frame duration for percent and HP; progress={progress}, HP gain={health_gain}, expected={expected_hp_gain}"
    );
}

#[test]
fn dozer_tick_truncates_handicap_before_player_template_modifier() {
    use crate::game_logic::game_logic::PlayerTemplateIdentity;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    use game_engine::common::rts::player_template::{
        PlayerTemplate, get_player_template_store_mut,
    };

    // Give the real Main PlayerTemplate lookup an exact test template; Main's
    // production construction call reads this and the Player's handicap.
    game_engine::common::ini::ensure_player_templates_loaded();
    let mut player_template = PlayerTemplate::new("TimingRegressionGeneral".to_string());
    player_template.side = "America".into();
    player_template.base_side = "America".into();
    player_template
        .production_time_changes
        .insert(NameKeyGenerator::name_to_key("TestBuilding"), -0.05);
    get_player_template_store_mut().add_template(player_template);

    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic
        .players
        .get_mut(&0)
        .expect("player")
        .map_side
        .handicap_build_time_buildings = 0.95;
    logic.players.get_mut(&0).expect("player").power_produced = 10;
    assert!(
        logic.bind_player_template_identity(
            0,
            PlayerTemplateIdentity::from_exact_name("TimingRegressionGeneral")
                .expect("exact timing test PlayerTemplate"),
        )
    );

    let sid = logic
        .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("scaffold");
    if let Some(site) = logic.host_object_mut(sid) {
        site.thing.template.build_time = 0.3; // C++ Int(0.3 * 30) = 9 frames
        site.construction_percent = 0.0;
    }
    let did = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));

    // C++ assigns to Int after each modifier: 9 * 0.95 = 8, then
    // 8 * 0.95 = 7. One actual Main construction frame is therefore 1/7.
    logic.update_construction(&[sid], 1.0 / 30.0);
    let progress = logic.host_object(sid).expect("site").construction_percent;
    assert!(
        (progress - (1.0 / 7.0)).abs() < 1e-5,
        "Main dozer build tick must use C++ staged frame count 9 -> 8 -> 7; expected 1/7, got {progress}"
    );
}

#[test]
fn low_power_duration_truncates_cpp_non_divisible_frame_count() {
    // C++ Int(buildTime / penaltyRate): 7 authored frames / 0.8 = 8.75,
    // assigned back to Int as 8 before DozerAIUpdate derives either rate.
    assert_eq!(GameLogic::cpp_build_time_frames_after_power(7, 0.8), 8);
    // C++ only clamps a non-positive rate to 0.01; valid positive rates below
    // that threshold remain effective.
    assert_eq!(GameLogic::cpp_build_time_frames_after_power(7, 0.005), 1400);
    assert_eq!(GameLogic::cpp_build_time_frames_after_power(7, 0.0), 700);
}

#[test]
fn authored_build_frame_modifiers_keep_each_cpp_integer_boundary() {
    let cases = [
        // 9 * 0.95 = 8.55 -> 8; 8 * 0.95 = 7.6 -> 7.
        (0.3, 0.95, 0.95, 7),
        // 62 * 1.25 = 77.5 -> 77.
        (2.099, 1.0, 1.25, 77),
        // 150 * 1.5 = 225; 225 * 1.25 = 281.25 -> 281.
        (5.0, 1.5, 1.25, 281),
    ];

    for (seconds, handicap, player_template, cpp_frames) in cases {
        assert_eq!(
            GameLogic::cpp_build_time_frames_from_modifiers(
                seconds,
                handicap,
                player_template,
            ),
            cpp_frames,
            "C++ staged conversion for {seconds}s × {handicap} × {player_template}"
        );
    }
}
