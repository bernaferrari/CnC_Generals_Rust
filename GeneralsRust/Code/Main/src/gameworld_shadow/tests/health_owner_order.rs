//! C++ ActiveBody commits healing before later readers; health admission keeps
//! the order of those operations when the presentation shadow catches up.
use super::*;
use crate::game_logic::{ObjectId, Player};

fn injured_world() -> (GameLogic, GameWorldShadow, ObjectId) {
    let mut logic = GameLogic::new();
    logic.set_damage_authority(true);
    logic.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("OwnerHealingVehicle");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    // C++ ignores experience on untrainable objects. Neighboring XP admission
    // tests need a body that actually accepts those points.
    template.is_trainable = true;
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("OwnerHealingVehicle", Team::USA, Vec3::ZERO)
        .expect("owner object");
    let object = logic.host_object_mut(id).unwrap();
    object.health.current = 40.0;
    object.previous_health = 40.0;
    object.refresh_model_condition_bits();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
    (logic, shadow, id)
}

#[test]
fn actual_coupled_fixed_frame_auto_heal_commits_owner_before_boundary() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, id) = injured_world();
    // This is the same runtime admitted by StartsActive DefaultAutoHeal. Its
    // ordinary production fixed-frame update, not a fabricated script action,
    // calls Object::heal before the AI phase and the post-step shadow batch.
    logic.host_object_mut(id).unwrap().default_auto_heal =
        Some(crate::game_logic::host_heal::HostDefaultAutoHealData::new());
    let eid = shadow.entity_for_host(id).unwrap();
    let _couple = ShadowCoupleGuard::enter();
    with_coupled_shadow(&mut shadow, || {
        logic.tick_logic_frame_with_boundary(1.0 / 30.0, None, None, |_| {});
        let object = logic.host_objects().get(&id).unwrap();
        assert_eq!(
            object.health.current, 42.0,
            "fixed-frame healing must commit on its owner"
        );
        assert_eq!(object.previous_health, 40.0);
        let events = crate::game_logic::host_heal_log::snapshot();
        assert_eq!(events.len(), 1);
        assert!(events[0].owner_health_already_applied());
        assert_eq!(
            crate::gameworld_shadow::with_active_shadow(|s| s.world().entity(eid).unwrap().health),
            Some(40.0)
        );
    });
    eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 42.0);
    assert_eq!(shadow.world().entity(eid).unwrap().health, 42.0);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn coupled_health_batch_preserves_completed_heal_then_damage() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    // Real Object body operations. Both have committed before shadow admission.
    logic.host_object_mut(id).unwrap().heal(30.0);
    logic.host_object_mut(id).unwrap().take_damage(20.0);
    assert_eq!(logic.host_object(id).unwrap().health.current, 50.0);
    let _couple = ShadowCoupleGuard::enter();
    eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
    assert_eq!(
        shadow.world().entity(eid).unwrap().health,
        50.0,
        "heal 40→70 then damage 70→50 must retain that order"
    );
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 50.0);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

fn record_pending_pair(id: ObjectId, heal_first: bool) {
    if heal_first {
        crate::game_logic::host_heal_log::record(id, 80.0);
        crate::game_logic::host_damage_log::record(id, 10.0, None, false);
    } else {
        crate::game_logic::host_damage_log::record(id, 10.0, None, false);
        crate::game_logic::host_heal_log::record(id, 80.0);
    }
}

#[test]
fn pending_health_records_keep_order_in_eager_and_session_paths() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    for eager in [true, false] {
        for heal_first in [true, false] {
            let (mut logic, mut shadow, id) = injured_world();
            let eid = shadow.entity_for_host(id).unwrap();
            record_pending_pair(id, heal_first);
            let _couple = ShadowCoupleGuard::enter();
            if eager {
                eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
            }
            let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
            let expected = if heal_first { 70.0 } else { 80.0 };
            assert_eq!(
                shadow.world().entity(eid).unwrap().health,
                expected,
                "eager={eager}, heal_first={heal_first}"
            );
            assert_eq!(logic.host_object(id).unwrap().health.current, expected);
            let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
            assert_eq!(
                logic.host_object(id).unwrap().health.current,
                expected,
                "empty admission must not replay a batch"
            );
        }
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn pending_health_records_keep_order_without_shadow_session() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    for heal_first in [true, false] {
        let (mut logic, _shadow, id) = injured_world();
        record_pending_pair(id, heal_first);
        run_post_logic_shadow_boundary(None, &mut logic);
        let expected = if heal_first { 70.0 } else { 80.0 };
        assert_eq!(
            logic.host_object(id).unwrap().health.current,
            expected,
            "heal_first={heal_first}"
        );
        run_post_logic_shadow_boundary(None, &mut logic);
        assert_eq!(logic.host_object(id).unwrap().health.current, expected);
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn coupled_owner_healing_is_visible_to_repeated_borrows_and_damage_state() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, id) = injured_world();
    logic.host_object_mut(id).unwrap().health.current = 30.0;
    logic
        .host_object_mut(id)
        .unwrap()
        .refresh_model_condition_bits();
    shadow.sync_from_host(&logic);
    let _couple = ShadowCoupleGuard::enter();
    with_coupled_shadow(&mut shadow, || {
        logic
            .with_host_logic_after_sync(|owner| {
                owner.host_object_mut(id).unwrap().heal(30.0);
                assert_eq!(owner.host_authoritative_health(id), Some(60.0));
                owner.host_object_mut(id).unwrap().heal(30.0);
                let object = owner.host_object(id).unwrap();
                assert_eq!(object.health.current, 90.0);
                assert_eq!(object.previous_health, 60.0);
                assert_eq!(
                    object.body_damage_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
                );
                owner.host_object_mut(id).unwrap().heal(100.0);
                assert_eq!(owner.host_authoritative_health(id), Some(100.0));
                assert_eq!(owner.host_object(id).unwrap().previous_health, 90.0);
            })
            .expect("owner phase");
        // The heal log also protects consecutive ordinary loans outside the
        // owner phase from re-importing the pre-admission shadow HP.
        assert_eq!(logic.host_object_mut(id).unwrap().health.current, 100.0);
    });
    eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 100.0);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn health_admission_preserves_max_health_and_experience_neighbors() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    for eager in [true, false] {
        let (mut logic, mut shadow, id) = injured_world();
        crate::game_logic::host_max_health_log::clear();
        crate::game_logic::host_experience_log::clear();
        let eid = shadow.entity_for_host(id).unwrap();
        logic.host_object_mut(id).unwrap().heal(50.0);
        // The existing max-health channel applies after the health batch in
        // production: lowering this cap must retain its final HP clamp.
        let object = logic.host_object_mut(id).unwrap();
        object.health.maximum = 60.0;
        object.max_health = 60.0;
        object.health.current = 60.0;
        object.record_host_max_health();
        crate::game_logic::host_experience_log::record(id, 12.0);
        let _couple = ShadowCoupleGuard::enter();
        if eager {
            eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
        }
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        let entity = shadow.world().entity(eid).unwrap();
        assert_eq!(entity.health, 60.0, "eager={eager}");
        assert_eq!(entity.max_health, 60.0);
        assert_eq!(logic.host_object(id).unwrap().health.current, 60.0);
        assert_eq!(logic.host_object(id).unwrap().health.maximum, 60.0);
        assert_eq!(
            logic.host_object(id).unwrap().experience.current,
            12.0,
            "eager={eager}"
        );
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
    crate::game_logic::host_max_health_log::clear();
    crate::game_logic::host_experience_log::clear();
}

#[test]
fn new_object_health_is_admitted_after_mapping_without_losing_heal() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, _initial_shadow, id) = injured_world();
    let mut shadow = GameWorldShadow::new(64);
    logic.host_object_mut(id).unwrap().heal(40.0);
    logic.host_object_mut(id).unwrap().take_damage(10.0);
    assert!(shadow.entity_for_host(id).is_none());
    let _couple = ShadowCoupleGuard::enter();
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    let eid = shadow.entity_for_host(id).expect("newly mapped object");
    assert_eq!(shadow.world().entity(eid).unwrap().health, 70.0);
    assert_eq!(logic.host_object(id).unwrap().health.current, 70.0);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn damage_disabled_session_retains_heal_only_admission_after_sync() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, id) = injured_world();
    logic.set_damage_authority(false);
    let eid = shadow.entity_for_host(id).unwrap();
    crate::game_logic::host_heal_log::record(id, 80.0);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(shadow.world().entity(eid).unwrap().health, 80.0);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn completed_damage_after_coupled_healing_is_not_replayed_without_session() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    for heal_first in [true, false] {
        let (mut logic, mut shadow, id) = injured_world();
        let _couple = ShadowCoupleGuard::enter();
        with_coupled_shadow(&mut shadow, || {
            logic
                .with_host_logic_after_sync(|owner| {
                    let object = owner.host_object_mut(id).unwrap();
                    if heal_first {
                        object.heal(30.0);
                        object.take_damage(20.0);
                    } else {
                        object.take_damage(20.0);
                        object.heal(30.0);
                    }
                    assert_eq!(object.health.current, 50.0);
                })
                .unwrap();
        });
        // The no-session fallback consumes observations without erasing the
        // later body operation; repeated admission is inert.
        for _ in 0..2 {
            run_post_logic_shadow_boundary(None, &mut logic);
            assert_eq!(logic.host_object(id).unwrap().health.current, 50.0);
        }
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn damage_reactions_observe_damage_state_before_later_healing() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    let entity = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
    entity.fwwd_active = true;
    entity.fwwd_damage_amount = 1.0;
    entity.fwwd_reaction_pristine = "HealthyReaction".into();
    entity.fwwd_reaction_damaged = "DamagedReaction".into();
    entity.fwwd_reaction_really_damaged = "CriticalReaction".into();
    crate::game_logic::host_fwwd_reaction_log::clear();
    // Retail really-damaged threshold is 35%; the first operation leaves 39%,
    // then the next leaves 20%, before healing restores pristine health.
    crate::game_logic::host_damage_log::record(id, 1.0, None, false);
    crate::game_logic::host_damage_log::record(id, 19.0, None, false);
    crate::game_logic::host_heal_log::record(id, 90.0);
    crate::game_logic::host_damage_log::record(id, 5.0, None, false);
    let _couple = ShadowCoupleGuard::enter();
    eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
    assert_eq!(shadow.world().entity(eid).unwrap().health, 85.0);
    assert_eq!(
        crate::game_logic::host_fwwd_reaction_log::drain(),
        vec![
            (id, "DamagedReaction".into()),
            (id, "CriticalReaction".into()),
            (id, "HealthyReaction".into())
        ]
    );
    // Remove the fixture reaction names before session delivery (they are
    // observation assertions, not authored weapon definitions).
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
    crate::game_logic::host_fwwd_reaction_log::clear();
}

#[test]
fn pending_lethal_damage_keeps_destruction_admission_before_later_absolute_health() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    let (mut logic, _shadow, id) = injured_world();
    crate::game_logic::host_damage_log::record(id, 100.0, None, true);
    crate::game_logic::host_heal_log::record(id, 80.0);
    run_post_logic_shadow_boundary(None, &mut logic);
    let object = logic.host_object(id).unwrap();
    assert!(object.status.on_die_started);
    assert!(object.status.destroyed);
    assert!(
        !object.is_alive(),
        "a later pending absolute write must not cancel death admission"
    );
    assert!(crate::game_logic::host_damage_log::snapshot().is_empty());
    assert!(crate::game_logic::host_heal_log::snapshot().is_empty());
    run_post_logic_shadow_boundary(None, &mut logic);
    assert!(logic.host_object(id).unwrap().status.on_die_started);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn fatal_owner_damage_reacts_to_zero_health_and_cannot_write_back_stale_hp() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    let entity = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
    entity.fwwd_active = true;
    entity.fwwd_damage_amount = 1.0;
    entity.fwwd_reaction_damaged = "DamagedReaction".into();
    entity.fwwd_reaction_rubble = "RubbleReaction".into();
    crate::game_logic::host_fwwd_reaction_log::clear();
    let _couple = ShadowCoupleGuard::enter();
    with_coupled_shadow(&mut shadow, || {
        logic
            .with_host_logic_after_sync(|owner| {
                assert!(owner.host_object_mut(id).unwrap().take_damage(100.0));
                assert_eq!(owner.host_authoritative_health(id), Some(0.0));
            })
            .unwrap();
    });
    let damage = crate::game_logic::host_damage_log::snapshot();
    assert_eq!(damage.len(), 1);
    assert!(damage[0].destroyed && damage[0].owner_health_already_applied());
    eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
    assert_eq!(
        crate::game_logic::host_fwwd_reaction_log::drain(),
        vec![(id, "RubbleReaction".into())],
        "ActiveBody updates HP before onDamage, then admits death"
    );
    assert_eq!(shadow.world().entity(eid).unwrap().health, 0.0);
    assert!(shadow.world().entity(eid).unwrap().destroyed);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    if let Some(object) = logic.host_object(id) {
        assert_eq!(object.health.current, 0.0);
        assert!(!object.is_alive());
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
    crate::game_logic::host_fwwd_reaction_log::clear();
}

#[test]
fn eager_receipt_does_not_claim_unmapped_health_or_experience_was_applied() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut logic, mut shadow, mapped) = injured_world();
    // Ordinary production creation inside an active coupled shadow maps
    // synchronously. This exercises the helper's incomplete-mapping boundary:
    // creation occurred before the coupled scope, so the batch must retain its
    // actual admission status when eager spawn mapping runs at its tail.
    let new_id = logic
        .create_object("OwnerHealingVehicle", Team::USA, Vec3::ZERO)
        .unwrap();
    assert!(shadow.entity_for_host(new_id).is_none());
    crate::game_logic::host_experience_log::clear();
    crate::game_logic::host_heal_log::record(mapped, 80.0);
    crate::game_logic::host_damage_log::record(new_id, 10.0, None, false);
    crate::game_logic::host_damage_log::record(mapped, 10.0, None, false);
    crate::game_logic::host_experience_log::record(new_id, 12.0);
    let _couple = ShadowCoupleGuard::enter();
    eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    let new_eid = shadow.entity_for_host(new_id).unwrap();
    assert_eq!(
        (
            shadow.world().entity(new_eid).unwrap().health,
            logic.host_object(new_id).unwrap().experience.current,
            logic.host_object(mapped).unwrap().health.current,
        ),
        (90.0, 12.0, 70.0),
        "admit unmapped effects without replaying the mapped damage prefix"
    );
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(new_id).unwrap().health.current, 90.0);
    assert_eq!(logic.host_object(mapped).unwrap().health.current, 70.0);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
    crate::game_logic::host_experience_log::clear();
}

#[test]
fn newly_mapped_body_does_not_replay_completed_damage() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    for eager in [true, false] {
        let (mut logic, _initial_shadow, id) = injured_world();
        logic.host_object_mut(id).unwrap().take_damage(10.0);
        assert_eq!(logic.host_object(id).unwrap().health.current, 30.0);
        let mut shadow = GameWorldShadow::new(64);
        let _couple = ShadowCoupleGuard::enter();
        if eager {
            eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
        }
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        assert_eq!(
            logic.host_object(id).unwrap().health.current,
            30.0,
            "eager={eager}: mapping from post-damage owner state must not subtract again"
        );
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn unmapped_owner_fallback_does_not_replay_completed_damage() {
    let (mut logic, _shadow, id) = injured_world();
    logic.host_object_mut(id).unwrap().take_damage(10.0);
    let events = crate::game_logic::host_damage_log::snapshot();
    assert_eq!(events.len(), 1);
    assert!(events[0].owner_health_already_applied());
    let applied = logic.apply_host_unmapped_damage_fallback(&events, |_| false);
    assert_eq!(
        (applied, logic.host_object(id).unwrap().health.current),
        (0, 30.0),
        "an observation cannot become another owner mutation when mapping is absent"
    );
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}

#[test]
fn completed_damage_reaction_retains_the_cap_at_the_damage_point() {
    let (mut logic, mut shadow, id) = injured_world();
    logic.host_object_mut(id).unwrap().take_damage(1.0);
    let events = crate::game_logic::host_damage_log::drain();
    let eid = shadow.entity_for_host(id).unwrap();
    let entity = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
    // Model a later cap import before damage observation admission. 39/100
    // was Damaged at the operation; 39/50 would incorrectly select Pristine.
    entity.max_health = 50.0;
    entity.fwwd_active = true;
    entity.fwwd_damage_amount = 1.0;
    entity.fwwd_reaction_pristine = "HealthyReaction".into();
    entity.fwwd_reaction_damaged = "DamagedReaction".into();
    crate::game_logic::host_fwwd_reaction_log::clear();
    let _ = shadow.apply_host_damage_events(&events);
    assert_eq!(
        crate::game_logic::host_fwwd_reaction_log::drain(),
        vec![(id, "DamagedReaction".into())]
    );
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
    crate::game_logic::host_fwwd_reaction_log::clear();
}

#[test]
fn pending_destroy_transport_does_not_invent_a_body_health_change() {
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    // Pending destroyed records currently include C++ destroyObject producers
    // (DeletionUpdate and GrantStealth). Until those intents are separated,
    // preserve their existing Destroy-only adapter contract. This tests the
    // transport boundary, not complete marker lifecycle or death parity.
    crate::game_logic::host_damage_log::record(id, 40.0, None, true);
    let events = crate::game_logic::host_damage_log::drain();
    assert!(!events[0].owner_health_already_applied());
    let _ = shadow.apply_host_damage_events(&events);
    let entity = shadow.world().entity(eid).unwrap();
    assert!(entity.destroyed);
    assert_eq!(entity.health, 40.0, "direct destruction is not body damage");
    assert_eq!(logic.host_object(id).unwrap().health.current, 40.0);
    // Keep the owning fixture alive until all observations are checked.
    logic.set_damage_authority(false);
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_heal_log::clear();
}
