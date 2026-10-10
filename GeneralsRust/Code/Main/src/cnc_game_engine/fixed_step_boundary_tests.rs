//! Execute the same borrowed fixed-step adapter used by the live engine.
use super::*;
use crate::gameworld_shadow::{self, GameWorldShadow, ShadowCoupleGuard};

#[path = "owned_frame_boundary_tests.rs"]
mod owned_frames;

fn fixture(name: &str, health: f32) -> (GameLogic, ObjectId, GameWorldShadow) {
    let mut logic = GameLogic::new();
    let config = crate::skirmish_config::golden_skirmish_config(name);
    crate::skirmish_config::apply_skirmish_config(&mut logic, &config).expect("content admission");
    // Retail short-game rules defeat structure-less players and remove their
    // armies. Admit both players' real victory-counting structures so these
    // fixed-step witnesses survive ordinary victory processing.
    let mut keep_alive = ThingTemplate::new("BoundaryKeepAlive");
    keep_alive
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::MpCountForVictory);
    keep_alive.energy_production = Some(0);
    logic
        .templates
        .insert("BoundaryKeepAlive".into(), keep_alive);
    for (team, position) in [
        (Team::USA, Vec3::new(60.0, 0.0, 60.0)),
        (Team::GLA, Vec3::new(160.0, 0.0, 160.0)),
    ] {
        logic
            .create_object("BoundaryKeepAlive", team, position)
            .expect("retail victory fixture");
    }
    let mut template = ThingTemplate::new(name);
    template.set_health(health);
    template.add_kind_of(KindOf::Infantry);
    logic.templates.insert(name.into(), template);
    let id = logic
        .create_object(name, Team::USA, Vec3::new(3.0, 0.0, 4.0))
        .expect("object admission");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.entity_for_host(id).is_some());
    (logic, id, shadow)
}

#[test]
fn shadow_boundary_observes_each_completed_frame_before_the_next_step() {
    let _lock = gameworld_shadow::authority_env_lock();
    // Constructors are inert; preserve the prior authority selected by other tests.
    let previous =
        crate::game_logic::game_logic::gameworld_authority::current_gameworld_authority();
    gameworld_shadow::with_gameworld_authority(previous, || {
        assert!(
            gameworld_shadow::gameworld_shadow_enabled(),
            "production shadow default"
        );
        let (mut logic, id, mut shadow) = fixture("BoundaryOwner", 100.0);
        let _couple = ShadowCoupleGuard::enter();
        let mut frames = Vec::new();
        let mut shadow_frames = Vec::new();
        gameworld_shadow::with_coupled_shadow(&mut shadow, || {
            let snap = CnCGameEngine::host_update_logic_frame(
                &mut logic,
                false,
                2.0 / 30.0,
                None,
                Some(2),
                |logic| {
                    frames.push(logic.get_frame());
                    gameworld_shadow::with_active_shadow_mut(|shadow| {
                        gameworld_shadow::eager_apply_all_host_residuals_after_logic(shadow, logic);
                    });
                    CnCGameEngine::host_run_gameworld_shadow_after_logic(logic);
                    shadow_frames.push(
                        gameworld_shadow::with_active_shadow(|shadow| shadow.world().frame())
                            .unwrap(),
                    );
                    assert!(logic.host_object(id).is_some());
                },
            );
            assert_eq!(snap.steps_run, 2);
            assert_eq!(snap.frame, 2);
        });
        assert_eq!(
            frames,
            vec![1, 2],
            "each boundary must run before another host step advances"
        );
        assert_eq!(
            shadow_frames,
            vec![1, 2],
            "timer and door ticks must receive distinct completed frames"
        );
    });
}

#[test]
fn same_id_world_boundaries_keep_their_own_shadow_health() {
    let _lock = gameworld_shadow::authority_env_lock();
    let previous =
        crate::game_logic::game_logic::gameworld_authority::current_gameworld_authority();
    gameworld_shadow::with_gameworld_authority(previous, || {
        let (mut a, id_a, mut shadow_a) = fixture("BoundaryA", 100.0);
        let (mut b, id_b, mut shadow_b) = fixture("BoundaryB", 240.0);
        assert_eq!(id_a, id_b);
        let _couple = ShadowCoupleGuard::enter();
        let mut a_frames = Vec::new();
        let mut b_frames = Vec::new();
        gameworld_shadow::with_coupled_shadow(&mut shadow_a, || {
            CnCGameEngine::host_update_logic_frame(
                &mut a,
                false,
                2.0 / 30.0,
                None,
                Some(2),
                |logic| {
                    a_frames.push(logic.get_frame());
                    CnCGameEngine::host_run_gameworld_shadow_after_logic(logic);
                    assert_eq!(logic.host_authoritative_health(id_a), Some(100.0));
                    gameworld_shadow::with_coupled_shadow(&mut shadow_b, || {
                        CnCGameEngine::host_update_logic_frame(
                            &mut b,
                            false,
                            1.0 / 30.0,
                            None,
                            Some(1),
                            |other| {
                                b_frames.push(other.get_frame());
                                CnCGameEngine::host_run_gameworld_shadow_after_logic(other);
                                assert_eq!(other.host_authoritative_health(id_b), Some(240.0));
                            },
                        );
                    });
                    assert_eq!(
                        logic.host_authoritative_health(id_a),
                        Some(100.0),
                        "nested boundary restores A's shadow"
                    );
                },
            );
        });
        assert_eq!(a_frames, vec![1, 2]);
        assert_eq!(b_frames, vec![1, 2]);
        assert_eq!(a.get_frame(), 2);
        assert_eq!(b.get_frame(), 2);
    });
}

#[test]
fn zero_frozen_paused_and_budgeted_offers_preserve_their_timing() {
    let _lock = gameworld_shadow::authority_env_lock();
    let dt = 1.0 / 30.0;
    let mut zero = GameLogic::new();
    let mut callbacks = 0;
    let snap =
        CnCGameEngine::host_update_logic_frame(&mut zero, false, dt * 0.5, None, Some(4), |_| {
            callbacks += 1
        });
    assert_eq!(callbacks, 0);
    assert_eq!(snap.frame, 0);
    assert_eq!(snap.steps_run, 0);
    assert_eq!(snap.accumulated_time_seconds, dt * 0.5);

    let mut frozen = GameLogic::new();
    frozen.set_script_time_frozen_for_test(true);
    let snap =
        CnCGameEngine::host_update_logic_frame(&mut frozen, false, 4.0 * dt, None, Some(2), |_| {
            callbacks += 1
        });
    assert_eq!(callbacks, 0);
    assert_eq!(snap.frame, 0);
    assert_eq!(snap.steps_run, 0);
    assert!(snap.budget_hit);
    assert_eq!(frozen.fixed_step_diagnostics().frozen_steps, 2);
    assert!(snap.accumulated_time_seconds >= dt);

    let mut paused = GameLogic::new();
    paused.set_paused(true);
    let snap = CnCGameEngine::host_update_logic_frame(&mut paused, true, dt, None, Some(1), |_| {
        callbacks += 1
    });
    assert_eq!(snap, SimTimingSnapshot::default());
    assert!(paused.is_paused());
    assert_eq!(callbacks, 0);
    let snap =
        CnCGameEngine::host_update_logic_frame(&mut paused, false, dt, None, Some(1), |_| {
            callbacks += 1
        });
    assert_eq!(snap.steps_run, 1);
    assert!(!paused.is_paused());
    assert_eq!(callbacks, 1);

    for (budget, expected, dropped) in [(Some(4), 4, false), (None, 1, true)] {
        let mut logic = GameLogic::new();
        let mut frames = Vec::new();
        let snap = CnCGameEngine::host_update_logic_frame(
            &mut logic,
            false,
            20.0 * dt,
            None,
            budget,
            |logic| frames.push(logic.get_frame()),
        );
        assert_eq!(frames, (1..=expected).collect::<Vec<_>>());
        assert_eq!(snap.steps_run, expected as usize);
        assert!(snap.budget_hit);
        assert_eq!(logic.fixed_step_diagnostics().dropped_excess_time, dropped);
        if dropped {
            assert_eq!(snap.accumulated_time_seconds, 0.0);
        } else {
            assert!(snap.accumulated_time_seconds >= dt);
        }
    }
}

/// C++ RadarUpgrade grants its controlling Player, even when two slots use
/// the same PlayerTemplate/faction. The live post-logic boundary cannot turn
/// that owner-specific count into a faction-wide grant.
#[test]
fn production_shadow_boundary_preserves_same_faction_player_radar_ownership() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "production_shadow_boundary_preserves_same_faction_player_radar_ownership",
        production_shadow_boundary_preserves_same_faction_player_radar_ownership_case,
    );
}

fn production_shadow_boundary_preserves_same_faction_player_radar_ownership_case() {
    use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;

    let make_world = || {
        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::Skirmish);
        logic.clear_all_players();
        logic.add_player(Player::new(0, Team::USA, "USA provider owner", true));
        logic.add_player(Player::new(1, Team::USA, "USA without radar", true));
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.set_health(1000.0);
        cc.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let mut barracks = ThingTemplate::new("AmericaBarracks");
        barracks.set_health(1000.0);
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("AmericaBarracks".into(), barracks);
        let provider = logic
            .create_object_for_player("AmericaCommandCenter", 0, Vec3::new(60.0, 0.0, 60.0))
            .expect("completed radar provider belongs to slot 0");
        logic
            .host_object_mut(provider)
            .expect("provider")
            .extend_radar(1);
        logic
            .create_object_for_player("AmericaBarracks", 1, Vec3::new(160.0, 0.0, 160.0))
            .expect("slot 1 has a real victory-counting building, but no provider");
        // Real radar providers lose availability during a brownout. Keep this
        // owner-identity witness powered, using an actual authored plant per slot.
        let mut power = ThingTemplate::new("OwnedRadarPowerPlant");
        power.add_kind_of(KindOf::Structure).set_health(1000.0);
        power.energy_production = Some(10);
        logic.templates.insert("OwnedRadarPowerPlant".into(), power);
        for (player, position) in [
            (0, Vec3::new(80.0, 0.0, 60.0)),
            (1, Vec3::new(180.0, 0.0, 160.0)),
        ] {
            logic
                .create_object_for_player("OwnedRadarPowerPlant", player, position)
                .expect("real owned power source");
        }
        assert_eq!(
            *logic.gameworld_authority(),
            GameWorldAuthority::DEFAULT_OFF
        );
        logic
    };
    let assert_radar = |logic: &GameLogic| {
        let owner = logic.get_player(0).expect("provider owner");
        let other = logic
            .get_player(1)
            .expect("same-faction independent player");
        assert_eq!(owner.radar_count, 1, "only one owned radar provider");
        assert!(owner.has_radar());
        assert_eq!(
            other.radar_count, 0,
            "faction does not confer another player's radar"
        );
        assert!(!other.has_radar());
        assert_eq!(
            logic.host_radar.online_transitions, 1,
            "one actual owner came online"
        );
        assert_eq!(logic.host_radar.offline_transitions, 0);
        let dish = logic
            .host_objects()
            .values()
            .find(|obj| obj.template_name == "AmericaCommandCenter")
            .expect("actual provider object");
        // The adapter reports the next frame, after completing the previous
        // frame's RadarUpdate phase. C++ compares that processed frame > 1.
        let completed = logic.get_frame().saturating_sub(1) > 1;
        assert_eq!(
            dish.radar_extend_complete, completed,
            "C++ strict frame > deadline"
        );
        assert_eq!(dish.radar_extend_done_frame, if completed { 0 } else { 1 });
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, radar_extending_model_bit, radar_upgraded_model_bit,
        };
        assert_eq!(
            host_model_condition_has(dish.model_condition_bits, radar_extending_model_bit()),
            !completed
        );
        assert_eq!(
            host_model_condition_has(dish.model_condition_bits, radar_upgraded_model_bit()),
            completed
        );
    };

    let mut without_shadow = make_world();
    // Three actual 30-FPS frame offers avoid rounding a single f32 3/30
    // interval below three stored fixed steps. Every deadline assertion still
    // observes each completed frame, including the strict frame > 1 crossing.
    let mut completed_steps = 0;
    for expected_frame in 1..=3 {
        let snap = CnCGameEngine::host_update_logic_frame(
            &mut without_shadow,
            false,
            1.0 / 30.0,
            None,
            Some(1),
            |logic| assert_radar(logic),
        );
        assert_eq!(snap.steps_run, 1);
        assert_eq!(snap.frame, expected_frame);
        completed_steps += snap.steps_run;
    }
    assert_eq!(completed_steps, 3);
    assert_radar(&without_shadow);

    let source = make_world();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("pending radar snapshot");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore pending radar timer");
    for mut logic in [source, restored] {
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let _couple = ShadowCoupleGuard::enter();
        gameworld_shadow::with_coupled_shadow(&mut shadow, || {
            let mut completed_steps = 0;
            for expected_frame in 1..=3 {
                let snap = CnCGameEngine::host_update_logic_frame(
                    &mut logic,
                    false,
                    1.0 / 30.0,
                    None,
                    Some(1),
                    |logic| {
                        assert_radar(logic);
                        gameworld_shadow::with_active_shadow_mut(|shadow| {
                            gameworld_shadow::eager_apply_all_host_residuals_after_logic(
                                shadow, logic,
                            );
                        });
                        CnCGameEngine::host_run_gameworld_shadow_after_logic(logic);
                        assert_radar(logic);
                    },
                );
                assert_eq!(snap.steps_run, 1);
                assert_eq!(snap.frame, expected_frame);
                completed_steps += snap.steps_run;
            }
            assert_eq!(completed_steps, 3);
            assert_eq!(logic.get_frame(), 3);
        });
        assert_radar(&logic);
    }

    // Ordinary production keeps its shadow enabled as an explicitly borrowed
    // observer. Exercise both a fresh owner and its real pending-timer restore.
    let ordinary_source = make_world();
    let mut ordinary_restored = GameLogic::new();
    ordinary_restored.templates = ordinary_source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut ordinary_restored)
        .expect("restore ordinary pending radar timer");
    for mut logic in [ordinary_source, ordinary_restored] {
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let mut completed_steps = 0;
        for expected_frame in 1..=3 {
            let snap = CnCGameEngine::host_update_logic_frame(
                &mut logic,
                false,
                1.0 / 30.0,
                None,
                Some(1),
                |logic| {
                    assert!(!gameworld_shadow::shadow_coupled_tick_active());
                    assert_radar(logic);
                    gameworld_shadow::run_owned_host_boundary(Some(&mut shadow), logic);
                    assert_radar(logic);
                },
            );
            assert_eq!(snap.steps_run, 1);
            assert_eq!(snap.frame, expected_frame);
            completed_steps += snap.steps_run;
        }
        assert_eq!(completed_steps, 3);
        assert_eq!(logic.get_frame(), 3);
        assert_radar(&logic);
    }
}
