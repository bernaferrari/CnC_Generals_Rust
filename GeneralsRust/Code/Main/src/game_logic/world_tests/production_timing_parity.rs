//! C++-derived timing regressions for Main construction calculations.
use super::super::*;
use super::helpers::*;

static LOW_POWER_CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Copy)]
struct LowEnergyRates {
    penalty: f32,
    min_speed: f32,
    max_speed: f32,
}

struct LowEnergyConfigGuard {
    _serial: std::sync::MutexGuard<'static, ()>,
    global:
        std::sync::Arc<parking_lot::RwLock<game_engine::common::ini::ini_game_data::GlobalData>>,
    previous: LowEnergyRates,
}

impl LowEnergyConfigGuard {
    fn install(rates: LowEnergyRates) -> Self {
        let serial = LOW_POWER_CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let global = game_engine::common::ini::ini_game_data::ensure_global_data();
        let previous = {
            let data = global.read();
            LowEnergyRates {
                penalty: data.low_energy_penalty_modifier,
                min_speed: data.min_low_energy_production_speed,
                max_speed: data.max_low_energy_production_speed,
            }
        };
        {
            let mut data = global.write();
            data.low_energy_penalty_modifier = rates.penalty;
            data.min_low_energy_production_speed = rates.min_speed;
            data.max_low_energy_production_speed = rates.max_speed;
        }

        Self {
            _serial: serial,
            global,
            previous,
        }
    }
}

impl Drop for LowEnergyConfigGuard {
    fn drop(&mut self) {
        let mut data = self.global.write();
        data.low_energy_penalty_modifier = self.previous.penalty;
        data.min_low_energy_production_speed = self.previous.min_speed;
        data.max_low_energy_production_speed = self.previous.max_speed;
    }
}

#[test]
fn unmapped_dozer_hp_gain_uses_energy_adjusted_build_time_frames() {
    let _config = LowEnergyConfigGuard::install(LowEnergyRates {
        penalty: 1.0,
        min_speed: 0.5,
        max_speed: 0.8,
    });

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
    let configured_power_factor = power.get(&0).copied();

    // Main host-authoritative production path; one elapsed second is 30 logic frames.
    logic.update_construction(&[sid], 1.0);
    let after = logic.host_object(sid).expect("site after");
    let progress = after.construction_percent - before.construction_percent;
    let health_gain = after.health.current - before.health.current;
    assert_eq!(
        configured_power_factor,
        Some(0.5),
        "empty grid is half speed"
    );

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
            GameLogic::cpp_build_time_frames_from_modifiers(seconds, handicap, player_template,),
            cpp_frames,
            "C++ staged conversion for {seconds}s × {handicap} × {player_template}"
        );
    }
}

#[test]
fn main_power_factors_follow_configured_bounds_and_cpp_order() {
    let cases = [
        // Low-power Min drives the factor below 0.01; CPP float division
        // yields 14999.999... and Int truncates to 14999.
        (
            LowEnergyRates {
                penalty: 1.0,
                min_speed: 0.002,
                max_speed: 0.005,
            },
            0,
            10,
            0.002,
            14_999,
        ),
        // Zero fields stay authoritative, then CPP applies penaltyRate=0.01.
        (
            LowEnergyRates {
                penalty: 0.0,
                min_speed: 0.0,
                max_speed: 0.0,
            },
            0,
            10,
            0.01,
            3_000,
        ),
        // Below-full-power applies Max after Min.
        (
            LowEnergyRates {
                penalty: 1.0,
                min_speed: 0.8,
                max_speed: 0.2,
            },
            0,
            10,
            0.2,
            150,
        ),
        // Full power still applies Min, but does not apply Max.
        (
            LowEnergyRates {
                penalty: 1.0,
                min_speed: 1.5,
                max_speed: 0.8,
            },
            10,
            10,
            1.5,
            20,
        ),
    ];

    for (rates, power_produced, power_consumed, expected_factor, expected_frames) in cases {
        let _config = LowEnergyConfigGuard::install(rates);
        let mut logic = GameLogic::new();
        ensure_test_player_for_team(&mut logic, Team::USA);
        if let Some(player) = logic.players.get_mut(&0) {
            player.power_produced = power_produced;
            player.power_consumed = power_consumed;
        }
        let configured_factor = logic.compute_player_power_factors().get(&0).copied();
        let final_frames = configured_factor
            .map(|factor| GameLogic::cpp_build_time_frames_after_power(30, factor));

        assert_eq!(configured_factor, Some(expected_factor));
        assert_eq!(final_frames, Some(expected_frames));
    }
}
