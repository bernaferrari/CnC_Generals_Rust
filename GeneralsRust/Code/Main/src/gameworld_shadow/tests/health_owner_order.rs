//! C++ ActiveBody commits healing before later readers; health admission keeps
//! the order of those operations when the presentation shadow catches up.
use super::*;
use crate::game_logic::{ObjectId, Player};

fn owner_damage(logic: &mut GameLogic, id: ObjectId, amount: f32) -> bool {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(amount, health_events)
}

fn owner_heal(logic: &mut GameLogic, id: ObjectId, health: f32) {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.heal(health, health_events);
}

fn injured_world() -> (GameLogic, GameWorldShadow, ObjectId) {
    let mut logic = GameLogic::new();

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
    logic.health_events.clear_damage();
    logic.health_events.clear_heal();
    (logic, shadow, id)
}

fn install_actual_reactions(logic: &mut GameLogic, id: ObjectId) -> ObjectId {
    use crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData;
    logic.host_object_mut(id).unwrap().fire_weapon_when_damaged =
        Some(HostFireWeaponWhenDamagedData {
            reaction_damaged: Some("SmallPoisonFieldWeapon".into()),
            reaction_really_damaged: Some("MediumPoisonFieldWeapon".into()),
            reaction_pristine: Some("LargePoisonFieldWeapon".into()),
            reaction_rubble: Some("LargePoisonFieldWeapon".into()),
            ..Default::default()
        });
    logic.add_player(Player::new(1, Team::China, "ReactionEnemy", false));
    for (id, alliance) in [(0, 0), (1, 1)] {
        let player = logic.get_player_mut(id).unwrap();
        player.is_alive = true;
        player.alliance_team = alliance;
    }
    let mut target = ThingTemplate::new("ActualReactionTarget");
    target
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert(target.name.clone(), target);
    logic
        .create_object(
            "ActualReactionTarget",
            Team::China,
            Vec3::new(1.0, 0.0, 0.0),
        )
        .unwrap()
}

#[test]
fn actual_coupled_fixed_frame_auto_heal_commits_owner_before_boundary() {
    let (mut logic, mut shadow, id) = injured_world();
    // This is the same runtime admitted by StartsActive DefaultAutoHeal. Its
    // ordinary production fixed-frame update, not a fabricated script action,
    // calls Object::heal before the AI phase and the post-step shadow batch.
    logic.host_object_mut(id).unwrap().default_auto_heal =
        Some(crate::game_logic::host_heal::HostDefaultAutoHealData::new());
    let eid = shadow.entity_for_host(id).unwrap();

    {
        logic.tick_logic_frame_with_boundary(1.0 / 30.0, None, None, |_| {});
        let object = logic.host_objects().get(&id).unwrap();
        assert_eq!(
            object.health.current, 42.0,
            "fixed-frame healing must commit on its owner"
        );
        assert_eq!(object.previous_health, 40.0);
        let events = logic.health_events.snapshot_heal();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].health, 42.0);
        assert_eq!(Some(shadow.world().entity(eid).unwrap().health), Some(40.0));
    };
    observe_completed_owner(&mut shadow, &mut logic);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 42.0);
    assert_eq!(shadow.world().entity(eid).unwrap().health, 42.0);
}

#[test]
fn coupled_health_batch_preserves_completed_heal_then_damage() {
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    // Real Object body operations. Both have committed before shadow admission.
    owner_heal(&mut logic, id, 30.0);
    owner_damage(&mut logic, id, 20.0);
    assert_eq!(logic.host_object(id).unwrap().health.current, 50.0);

    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(
        shadow.world().entity(eid).unwrap().health,
        50.0,
        "heal 40→70 then damage 70→50 must retain that order"
    );
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 50.0);
}

fn record_pending_pair(logic: &mut GameLogic, id: ObjectId, heal_first: bool) {
    if heal_first {
        perform_owner_health(logic, id, 80.0);
        perform_owner_damage(logic, id, 10.0, None);
    } else {
        perform_owner_damage(logic, id, 10.0, None);
        perform_owner_health(logic, id, 80.0);
    }
}

#[test]
fn pending_health_records_keep_order_in_eager_and_session_paths() {
    for eager in [true, false] {
        for heal_first in [true, false] {
            let (mut logic, mut shadow, id) = injured_world();
            let eid = shadow.entity_for_host(id).unwrap();
            record_pending_pair(&mut logic, id, heal_first);

            if eager {
                observe_completed_owner(&mut shadow, &mut logic);
            }
            let _ = observe_completed_owner(&mut shadow, &mut logic);
            let expected = if heal_first { 70.0 } else { 80.0 };
            assert_eq!(
                shadow.world().entity(eid).unwrap().health,
                expected,
                "eager={eager}, heal_first={heal_first}"
            );
            assert_eq!(logic.host_object(id).unwrap().health.current, expected);
            let _ = observe_completed_owner(&mut shadow, &mut logic);
            assert_eq!(
                logic.host_object(id).unwrap().health.current,
                expected,
                "empty admission must not replay a batch"
            );
        }
    }
}

#[test]
fn pending_health_records_keep_order_without_shadow_session() {
    for heal_first in [true, false] {
        let (mut logic, _shadow, id) = injured_world();
        record_pending_pair(&mut logic, id, heal_first);
        crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
        let expected = if heal_first { 70.0 } else { 80.0 };
        assert_eq!(
            logic.host_object(id).unwrap().health.current,
            expected,
            "heal_first={heal_first}"
        );
        crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
        assert_eq!(logic.host_object(id).unwrap().health.current, expected);
    }
}

#[test]
fn coupled_owner_healing_is_visible_to_repeated_borrows_and_damage_state() {
    let (mut logic, mut shadow, id) = injured_world();
    logic.host_object_mut(id).unwrap().health.current = 30.0;
    logic
        .host_object_mut(id)
        .unwrap()
        .refresh_model_condition_bits();
    shadow.sync_from_host(&logic);

    {
        logic
            .with_host_logic_step(|owner| {
                owner_heal(owner, id, 30.0);
                assert_eq!(owner.host_authoritative_health(id), Some(60.0));
                owner_heal(owner, id, 30.0);
                let object = owner.host_object(id).unwrap();
                assert_eq!(object.health.current, 90.0);
                assert_eq!(object.previous_health, 60.0);
                assert_eq!(
                    object.body_damage_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
                );
                owner_heal(owner, id, 100.0);
                assert_eq!(owner.host_authoritative_health(id), Some(100.0));
                assert_eq!(owner.host_object(id).unwrap().previous_health, 90.0);
            })
            .expect("owner phase");
        // The heal log also protects consecutive ordinary loans outside the
        // owner phase from re-importing the pre-admission shadow HP.
        assert_eq!(logic.host_object_mut(id).unwrap().health.current, 100.0);
    };
    observe_completed_owner(&mut shadow, &mut logic);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 100.0);
}

#[test]
fn health_admission_preserves_max_health_and_experience_neighbors() {
    for eager in [true, false] {
        let (mut logic, mut shadow, id) = injured_world();
        crate::game_logic::host_max_health_log::clear();
        let eid = shadow.entity_for_host(id).unwrap();
        owner_heal(&mut logic, id, 50.0);
        // The existing max-health channel applies after the health batch in
        // production: lowering this cap must retain its final HP clamp.
        let object = logic.host_object_mut(id).unwrap();
        object.health.maximum = 60.0;
        object.max_health = 60.0;
        object.health.current = 60.0;
        object.record_host_max_health();
        logic.host_object_mut(id).unwrap().gain_experience(12.0);

        if eager {
            observe_completed_owner(&mut shadow, &mut logic);
        }
        let _ = observe_completed_owner(&mut shadow, &mut logic);
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
    crate::game_logic::host_max_health_log::clear();
}

#[test]
fn new_object_health_is_admitted_after_mapping_without_losing_heal() {
    let (mut logic, _initial_shadow, id) = injured_world();
    let mut shadow = GameWorldShadow::new(64);
    owner_heal(&mut logic, id, 40.0);
    let _ = owner_damage(&mut logic, id, 10.0);
    assert!(shadow.entity_for_host(id).is_none());

    let _ = observe_completed_owner(&mut shadow, &mut logic);
    let eid = shadow.entity_for_host(id).expect("newly mapped object");
    assert_eq!(shadow.world().entity(eid).unwrap().health, 70.0);
    assert_eq!(logic.host_object(id).unwrap().health.current, 70.0);
}

#[test]
fn damage_disabled_session_retains_heal_only_admission_after_sync() {
    let (mut logic, mut shadow, id) = injured_world();

    let eid = shadow.entity_for_host(id).unwrap();
    perform_owner_health(&mut logic, id, 80.0);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(shadow.world().entity(eid).unwrap().health, 80.0);
}

#[test]
fn completed_damage_after_coupled_healing_is_not_replayed_without_session() {
    for heal_first in [true, false] {
        let (mut logic, mut shadow, id) = injured_world();

        {
            logic
                .with_host_logic_step(|owner| {
                    let (object, health_events) =
                        owner.host_object_and_health_events_mut(id).unwrap();
                    if heal_first {
                        object.heal(30.0, health_events);
                        object.take_damage(20.0, health_events);
                    } else {
                        object.take_damage(20.0, health_events);
                        object.heal(30.0, health_events);
                    }
                    assert_eq!(object.health.current, 50.0);
                })
                .unwrap();
        };
        // The no-session fallback consumes observations without erasing the
        // later body operation; repeated admission is inert.
        for _ in 0..2 {
            crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
            assert_eq!(logic.host_object(id).unwrap().health.current, 50.0);
        }
    }
}

#[test]
fn damage_reactions_observe_damage_state_before_later_healing() {
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    let reaction_target = install_actual_reactions(&mut logic, id);
    // Retail really-damaged threshold is 35%; the first operation leaves 39%,
    // then the next leaves 20%, before healing restores pristine health.
    perform_owner_damage(&mut logic, id, 1.0, None);
    perform_owner_damage(&mut logic, id, 19.0, None);
    perform_owner_health(&mut logic, id, 90.0);
    perform_owner_damage(&mut logic, id, 5.0, None);

    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(shadow.world().entity(eid).unwrap().health, 85.0);
    let target = logic.host_object(reaction_target).unwrap();
    assert_eq!(
        target.health.current, 98.375,
        "damaged, critical and pristine reactions execute on the installed neighbor"
    );
    assert_eq!(
        target.previous_health, 99.0,
        "TankArmor POISON25%: pristine0.625 follows damaged0.5 and critical0.5"
    );
    // A second observation boundary cannot reexecute the reaction weapons.
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    logic.health_events.clear_damage();
    logic.health_events.clear_heal();
}

#[test]
fn pending_lethal_damage_keeps_destruction_admission_before_later_absolute_health() {
    let (mut logic, _shadow, id) = injured_world();
    perform_owner_damage(&mut logic, id, 100.0, None);
    perform_owner_health(&mut logic, id, 80.0);
    crate::gameworld_shadow::materialize_host_authority_logs(&mut logic);
    let object = logic.host_object(id).unwrap();
    assert!(object.status.on_die_started);
    assert!(object.status.destroyed);
    assert!(
        !object.is_alive(),
        "a later pending absolute write must not cancel death admission"
    );
    assert!(logic.health_events.snapshot_damage().is_empty());
    assert!(logic.health_events.snapshot_heal().is_empty());
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert!(
        logic.host_object(id).is_none(),
        "original admitted death completes physical cleanup once"
    );
}

#[test]
fn fatal_owner_damage_reacts_to_zero_health_and_cannot_write_back_stale_hp() {
    let (mut logic, mut shadow, id) = injured_world();
    let eid = shadow.entity_for_host(id).unwrap();
    let reaction_target = install_actual_reactions(&mut logic, id);

    {
        logic
            .with_host_logic_step(|owner| {
                perform_owner_damage(owner, id, 100.0, None);
                assert_eq!(owner.host_authoritative_health(id), Some(0.0));
            })
            .unwrap();
    };
    let damage: Vec<_> = logic
        .health_events
        .snapshot_damage()
        .into_iter()
        .filter(|event| event.target == id)
        .collect();
    assert_eq!(damage.len(), 1);
    assert!(damage[0].destroyed);
    assert_eq!(damage[0].body_observation().current, 0.0);
    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(
        logic.host_object(reaction_target).unwrap().health.current,
        99.375,
        "TankArmor POISON25% rubble reaction executes before source removal"
    );
    assert!(logic.host_object(id).is_none());
    assert!(shadow.entity_for_host(id).is_none());
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    if let Some(object) = logic.host_object(id) {
        assert_eq!(object.health.current, 0.0);
        assert!(!object.is_alive());
    }
    logic.health_events.clear_damage();
    logic.health_events.clear_heal();
}

#[test]
fn eager_receipt_does_not_claim_unmapped_health_or_experience_was_applied() {
    let (mut logic, mut shadow, mapped) = injured_world();
    // Ordinary production creation inside an active coupled shadow maps
    // synchronously. This exercises the helper's incomplete-mapping boundary:
    // creation occurred before the coupled scope, so the batch must retain its
    // actual admission status when eager spawn mapping runs at its tail.
    let new_id = logic
        .create_object("OwnerHealingVehicle", Team::USA, Vec3::ZERO)
        .unwrap();
    assert!(shadow.entity_for_host(new_id).is_none());
    perform_owner_health(&mut logic, mapped, 80.0);
    perform_owner_damage(&mut logic, new_id, 10.0, None);
    perform_owner_damage(&mut logic, mapped, 10.0, None);
    logic.host_object_mut(new_id).unwrap().gain_experience(12.0);

    observe_completed_owner(&mut shadow, &mut logic);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
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
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(new_id).unwrap().health.current, 90.0);
    assert_eq!(logic.host_object(mapped).unwrap().health.current, 70.0);
    logic.health_events.clear_damage();
    logic.health_events.clear_heal();
}

#[test]
fn newly_mapped_body_does_not_replay_completed_damage() {
    for eager in [true, false] {
        let (mut logic, _initial_shadow, id) = injured_world();
        let _ = owner_damage(&mut logic, id, 10.0);
        assert_eq!(logic.host_object(id).unwrap().health.current, 30.0);
        let mut shadow = GameWorldShadow::new(64);

        if eager {
            observe_completed_owner(&mut shadow, &mut logic);
        }
        let _ = observe_completed_owner(&mut shadow, &mut logic);
        assert_eq!(
            logic.host_object(id).unwrap().health.current,
            30.0,
            "eager={eager}: mapping from post-damage owner state must not subtract again"
        );
    }
}

#[test]
fn unmapped_owner_fallback_does_not_replay_completed_damage() {
    let (mut logic, mut shadow, id) = injured_world();
    owner_damage(&mut logic, id, 10.0);
    let events = logic.health_events.snapshot_damage();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].body_observation().current, 30.0);
    shadow.host_to_entity.clear();
    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 30.0);
    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 30.0);
}

#[test]
fn completed_damage_reaction_retains_the_cap_at_the_damage_point() {
    let (mut logic, mut shadow, id) = injured_world();
    logic.host_object_mut(id).unwrap().fire_weapon_when_damaged = Some(
        crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData {
            reaction_pristine: Some("HealthyReaction".into()),
            reaction_damaged: Some("DamagedReaction".into()),
            ..Default::default()
        },
    );
    owner_damage(&mut logic, id, 1.0);
    assert_eq!(
        logic
            .host_object(id)
            .unwrap()
            .pending_fire_when_damaged_weapon
            .as_deref(),
        Some("DamagedReaction")
    );
    // A later definition/cap observation must not recalculate the body reaction.
    let eid = shadow.entity_for_host(id).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .max_health = 50.0;
    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 39.0);
    assert_eq!(
        logic
            .host_object(id)
            .unwrap()
            .pending_fire_when_damaged_weapon
            .as_deref(),
        Some("DamagedReaction")
    );
}

#[test]
fn pending_destroy_transport_does_not_invent_a_body_health_change() {
    let (mut logic, mut shadow, id) = injured_world();
    logic.destroy_object(id);
    let object = logic.host_object(id).unwrap();
    assert!(object.status.destroyed);
    assert_eq!(
        object.health.current, 40.0,
        "direct destruction is not body damage"
    );
    assert!(!object.status.on_die_started);
    assert!(logic.health_events.snapshot_damage().is_empty());
    observe_completed_owner(&mut shadow, &mut logic);
    assert!(logic.host_object(id).is_none());
    assert!(shadow.entity_for_host(id).is_none());
}
