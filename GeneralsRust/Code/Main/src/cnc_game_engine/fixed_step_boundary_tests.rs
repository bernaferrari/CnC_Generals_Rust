//! Execute the same borrowed fixed-step adapter used by the live engine.
use super::*;
use crate::gameworld_shadow::{self, GameWorldShadow, ShadowCoupleGuard};

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
    logic
        .templates
        .insert("BoundaryKeepAlive".into(), keep_alive);
    for (team, position) in [
        (Team::USA, Vec3::new(60.0, 0.0, 60.0)),
        (Team::China, Vec3::new(160.0, 0.0, 160.0)),
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
