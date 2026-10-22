//! CPP ActiveBody785–848: positive healing commits/stamps before onHealing,
//! then state callbacks reread the installed Body before DamageFX. These bounded
//! continuation tests do not claim a rendered or original-engine playthrough.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_bone_fx_damage::{
    HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
};
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;

fn body() -> Object {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        "Object BoundedHealingStateCallbackOwner\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
        "healing_state_callback_owner.ini",
    ).expect("authored receiving ActiveBody");
    let template = GameLogic::build_template_from_object_definition(
        "BoundedHealingStateCallbackOwner",
        parser
            .get_definition("BoundedHealingStateCallbackOwner")
            .unwrap(),
        None,
    );
    let mut object = Object::new(template, ObjectId(953), Team::USA);
    assert_eq!(
        (object.health.current, object.health.maximum),
        (100.0, 100.0)
    );
    assert_eq!(object.body_initial_health(), 100.0);
    assert!(object.is_alive());
    object.internal_change_health(-80.0);
    object.last_damage_info_type = Some(DamageType::Bullet);
    object.last_damage_timestamp = Some(10);
    let mut authored = HostBoneFxAuthored::default();
    authored.damage_fx_types = gamelogic::damage::DamageTypeFlags::HEALING.bits();
    for state in [HostBodyDamageType::Damaged, HostBodyDamageType::Pristine] {
        authored.fx[state.ordinal() as usize][0] = Some(HostBoneFxSlot {
            bone: "Body".into(),
            only_once: true,
            delay_min: 0.0,
            delay_max: 0.0,
            name: format!("HealingState{}", state.ordinal()),
        });
    }
    object.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    object
}

fn poison(object: &mut Object) {
    let mut state = crate::game_logic::host_poisoned_behavior::HostPoisonedBehaviorData::default();
    state.start_poisoned_effects(100, 3.0, HostDeathType::Poisoned);
    object.poisoned_behavior = Some(state);
}

fn begin(object: &mut Object, amount: f32) -> damage::DamageApplication {
    begin_with_context(object, amount, &DamageHitContext::default())
}

fn begin_with_context(
    object: &mut Object,
    amount: f32,
    context: &DamageHitContext<'_>,
) -> damage::DamageApplication {
    object.begin_damage_with_context(
        amount,
        Some(ObjectId(954)),
        DamageType::Healing,
        HostDeathType::Normal,
        None,
        123,
        context,
        &mut crate::game_logic::HostHealthEvents::default(),
        None,
    )
}

#[test]
fn healing_commits_and_stamps_before_exactly_one_healing_filtered_state_callback() {
    let mut object = body();
    let mut application = begin(&mut object, 30.0);
    assert_eq!(
        (object.previous_health, object.health.current),
        (20.0, 50.0)
    );
    assert_eq!(object.body_damage_state, HostBodyDamageType::Damaged);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(ObjectId(954)));
    assert_eq!(object.last_damage_timestamp, Some(123));
    assert_eq!(object.last_healing_timestamp, Some(123));
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 0);
    assert!(bone.pending.is_empty());
    application.complete_body_state_callbacks(&mut object, None);
    application.complete_body_state_callbacks(&mut object, None);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 1);
    assert_eq!(bone.pending.len(), 1);
    assert_eq!(bone.pending[0].old_state, HostBodyDamageType::ReallyDamaged);
    assert_eq!(bone.pending[0].new_state, HostBodyDamageType::Damaged);
    assert_eq!(bone.pending[0].fx_list.as_deref(), Some("HealingState1"));
}

#[test]
fn healing_state_callback_observes_intervening_body_operation_and_ignores_returned_state() {
    for return_to_prior in [false, true] {
        let mut object = body();
        let application = begin(&mut object, 30.0);
        object.internal_change_health(if return_to_prior { -30.0 } else { 40.0 });
        application.finish_standalone(&mut object, None, &false);
        let bone = object.bone_fx_damage.as_ref().unwrap();
        if return_to_prior {
            assert_eq!(object.body_damage_state, HostBodyDamageType::ReallyDamaged);
            assert_eq!(bone.transitions, 0);
            assert!(bone.pending.is_empty());
        } else {
            assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
            assert_eq!(bone.transitions, 1);
            assert_eq!(bone.pending.len(), 1);
            assert_eq!(bone.pending[0].old_state, HostBodyDamageType::ReallyDamaged);
            assert_eq!(bone.pending[0].new_state, HostBodyDamageType::Pristine);
            assert_eq!(bone.pending[0].fx_list.as_deref(), Some("HealingState0"));
        }
    }
}

#[test]
fn positive_healing_at_max_stamps_history_without_on_healing_poison_clear() {
    let mut object = body();
    object.internal_change_health(80.0);
    poison(&mut object);
    begin(&mut object, 5.0).finish_standalone(&mut object, None, &false);
    assert_eq!(
        (object.previous_health, object.health.current),
        (100.0, 100.0)
    );
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(ObjectId(954)));
    assert_eq!(object.last_damage_timestamp, Some(123));
    assert_eq!(object.last_healing_timestamp, Some(123));
    assert!(object.poisoned_behavior.as_ref().unwrap().is_active());
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
}

#[test]
fn positive_healing_above_max_clamps_down_without_on_healing_poison_clear() {
    let mut object = body();
    object.internal_change_health(80.0);
    // Exercise original internalChangeHealth's upper clamp on preexisting
    // above-maximum state; setup is intentionally not another health operation.
    object.health.current = 150.0;
    poison(&mut object);
    begin(&mut object, 5.0).finish_standalone(&mut object, None, &false);
    assert_eq!(
        (object.previous_health, object.health.current),
        (150.0, 100.0)
    );
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(ObjectId(954)));
    assert_eq!(object.last_damage_timestamp, Some(123));
    assert_eq!(object.last_healing_timestamp, Some(123));
    assert!(object.poisoned_behavior.as_ref().unwrap().is_active());
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
}

#[test]
fn receiving_world_healing_keeps_same_id_peer_body_and_callbacks_unchanged() {
    let mut world = crate::game_logic::GameLogic::new();
    let mut peer = crate::game_logic::GameLogic::new();
    let object = body();
    let id = object.id;
    world.objects.insert(id, object);
    peer.objects.insert(id, body());
    world.frame = 123;
    let result = world
        .apply_owned_damage(
            id,
            30.0,
            None,
            DamageType::Healing,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .expect("receiving native healing");
    assert!(!result.destroyed());
    let object = &world.objects[&id];
    assert_eq!(object.health.current, 50.0);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_healing_timestamp, Some(123));
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 1);
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().pending.len(), 1);
    let foreign = &peer.objects[&id];
    assert_eq!(foreign.health.current, 20.0);
    assert_eq!(foreign.last_damage_info_type, Some(DamageType::Bullet));
    assert_eq!(foreign.last_healing_timestamp, None);
    assert_eq!(foreign.bone_fx_damage.as_ref().unwrap().transitions, 0);
    assert!(foreign.bone_fx_damage.as_ref().unwrap().pending.is_empty());
}

#[test]
fn nonpositive_or_nan_healing_preserves_body_history_poison_and_retains_fx_phase() {
    for amount in [0.0, -5.0, f32::NAN] {
        let mut object = body();
        poison(&mut object);
        let previous = object.previous_health;
        let application = begin(&mut object, amount);
        assert!(
            matches!(&application, damage::DamageApplication::Healing(_)),
            "CPP attemptHealing always reaches doDamageFX after nonpositive armor result"
        );
        assert_eq!(
            (object.previous_health, object.health.current),
            (previous, 20.0)
        );
        assert_eq!(object.last_damage_info_type, Some(DamageType::Bullet));
        assert_eq!(object.last_damage_timestamp, Some(10));
        assert_eq!(object.last_damage_source, None);
        assert_eq!(object.last_healing_timestamp, None);
        assert!(object.poisoned_behavior.as_ref().unwrap().is_active());
        assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
        application.finish_standalone(&mut object, None, &false);
        assert!(object.poisoned_behavior.as_ref().unwrap().is_active());
        assert!(object.bone_fx_damage.as_ref().unwrap().pending.is_empty());
        // No ArmorSet/DamageFX is authored here: continuation admission proves
        // ordering, not a dispatched renderer effect or DamageFX-store coverage.
        assert_eq!(object.last_damage_fx_done, None);
    }
}

#[test]
fn positive_infinite_healing_uses_original_upper_clamp_and_positive_history() {
    let mut object = body();
    poison(&mut object);
    let application = begin(&mut object, f32::INFINITY);
    assert_eq!(
        (object.previous_health, object.health.current),
        (20.0, 100.0)
    );
    assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(ObjectId(954)));
    assert_eq!(object.last_damage_timestamp, Some(123));
    assert_eq!(object.last_healing_timestamp, Some(123));
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
    application.finish_standalone(&mut object, None, &false);
    assert!(!object.poisoned_behavior.as_ref().unwrap().is_active());
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 1);
    assert_eq!(bone.pending.len(), 1);
    assert_eq!(bone.pending[0].new_state, HostBodyDamageType::Pristine);
    assert_eq!(bone.pending[0].fx_list.as_deref(), Some("HealingState0"));
}

#[test]
fn zero_maximum_bridge_healing_keeps_canonical_dead_body_and_retained_husk() {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        "Object BoundedZeroMaximumBridge\n KindOf = BRIDGE STRUCTURE\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 0\n InitialHealth = 0\n End\nEnd\n",
        "healing_zero_maximum_bridge.ini",
    ).expect("authored receiving bridge ActiveBody");
    let template = GameLogic::build_template_from_object_definition(
        "BoundedZeroMaximumBridge",
        parser.get_definition("BoundedZeroMaximumBridge").unwrap(),
        None,
    );
    let mut object = Object::new(template, ObjectId(955), Team::USA);
    assert_eq!((object.health.current, object.health.maximum), (0.0, 0.0));
    assert_eq!(object.body_initial_health(), 0.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::Rubble);
    // Existing retained-husk adapter state is explicit fixture setup, not a
    // claim that this bounded test executed all original bridge death modules.
    object.status.keep_as_rubble = true;
    object.status.destroyed = true;
    object.status.on_die_started = true;
    // CPP constructor classifies damage state without setting effectivelyDead;
    // this flag belongs to the prior lethal operation represented by the husk.
    object.status.effectively_dead = true;
    assert!(object.status.effectively_dead);
    begin_with_context(
        &mut object,
        5.0,
        &DamageHitContext::default().with_healing_entry(),
    )
    .finish_standalone(&mut object, None, &false);
    assert_eq!((object.previous_health, object.health.current), (0.0, 0.0));
    assert_eq!(object.health.maximum, 0.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::Rubble);
    assert!(object.status.effectively_dead);
    assert!(object.status.keep_as_rubble);
    assert!(object.status.destroyed);
    assert!(object.status.on_die_started);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_healing_timestamp, Some(123));
}

#[test]
fn receiving_world_above_max_healing_does_not_consume_a_pending_damage_reaction() {
    let mut world = GameLogic::new();
    let mut peer = GameLogic::new();
    let mut object = body();
    object.internal_change_health(80.0);
    // A preexisting over-maximum state exercises original upper clamping;
    // assignment is explicitly setup, not a supported ordinary Body operation.
    object.health.current = 150.0;
    poison(&mut object);
    object.pending_fire_when_damaged_weapon = Some("PriorDamageReactionWitness".into());
    let id = object.id;
    let mut foreign = body();
    foreign.internal_change_health(80.0);
    foreign.health.current = 150.0;
    poison(&mut foreign);
    foreign.pending_fire_when_damaged_weapon = Some("PeerDamageReactionWitness".into());
    world.objects.insert(id, object);
    peer.objects.insert(id, foreign);
    world.frame = 123;
    let result = world
        .apply_owned_damage(
            id,
            5.0,
            None,
            DamageType::Healing,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .expect("receiving native Healing upper clamp");
    assert!(!result.destroyed());
    let object = &world.objects[&id];
    assert_eq!(
        (object.previous_health, object.health.current),
        (150.0, 100.0)
    );
    assert_eq!(
        object.pending_fire_when_damaged_weapon.as_deref(),
        Some("PriorDamageReactionWitness")
    );
    assert!(object.poisoned_behavior.as_ref().unwrap().is_active());
    assert!(!object.status.on_die_started);
    assert!(!object.status.destroyed);
    assert_eq!(object.experience.current, 0.0);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_healing_timestamp, Some(123));
    let foreign = &peer.objects[&id];
    assert_eq!(foreign.health.current, 150.0);
    assert_eq!(
        foreign.pending_fire_when_damaged_weapon.as_deref(),
        Some("PeerDamageReactionWitness")
    );
    assert!(foreign.poisoned_behavior.as_ref().unwrap().is_active());
    assert_eq!(foreign.last_damage_info_type, Some(DamageType::Bullet));
    assert_eq!(foreign.last_healing_timestamp, None);
    assert!(!foreign.status.on_die_started);
    assert_eq!(foreign.experience.current, 0.0);
}

#[test]
fn direct_healing_bypasses_indestructible_but_damage_healing_does_not() {
    let mut world = GameLogic::new();
    let mut peer = GameLogic::new();
    let mut object = body();
    object.set_indestructible(true);
    let id = object.id;
    let mut foreign = body();
    foreign.set_indestructible(true);
    world.objects.insert(id, object);
    peer.objects.insert(id, foreign);
    let mut benefactor = body();
    benefactor.id = ObjectId(954);
    world.objects.insert(benefactor.id, benefactor);
    world.frame = 123;
    for direct in [false, true] {
        let result = if direct {
            world.apply_owned_healing(id, 30.0, Some(ObjectId(954)))
        } else {
            world.apply_owned_damage(
                id,
                30.0,
                None,
                DamageType::Healing,
                HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
            )
        }
        .expect("installed receiving Body");
        assert!(!result.destroyed());
        let object = &world.objects[&id];
        assert_eq!(object.health.current, if direct { 50.0 } else { 20.0 });
        assert_eq!(
            object.last_damage_info_type,
            Some(if direct {
                DamageType::Healing
            } else {
                DamageType::Bullet
            })
        );
        assert_eq!(object.last_healing_timestamp, direct.then_some(123));
        assert_eq!(object.last_damage_source, direct.then_some(ObjectId(954)));
        assert_eq!(
            object.bone_fx_damage.as_ref().unwrap().transitions,
            u32::from(direct)
        );
        let foreign = &peer.objects[&id];
        assert_eq!(foreign.health.current, 20.0);
        assert_eq!(foreign.last_damage_info_type, Some(DamageType::Bullet));
        assert_eq!(foreign.last_healing_timestamp, None);
        assert_eq!(foreign.bone_fx_damage.as_ref().unwrap().transitions, 0);
    }
}

#[test]
fn healing_damage_with_kill_input_does_not_mutate_body_or_history() {
    let mut object = body();
    poison(&mut object);
    begin_with_context(&mut object, 30.0, &DamageHitContext::for_kill()).finish_standalone(
        &mut object,
        None,
        &false,
    );
    assert_eq!(
        (object.previous_health, object.health.current),
        (100.0, 20.0)
    );
    assert_eq!(object.last_damage_info_type, Some(DamageType::Bullet));
    assert_eq!(object.last_damage_timestamp, Some(10));
    assert_eq!(object.last_healing_timestamp, None);
    assert!(object.poisoned_behavior.as_ref().unwrap().is_active());
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
    assert!(!object.status.on_die_started);
}

#[test]
fn dead_non_bridge_rejects_both_healing_entry_kinds() {
    for direct in [false, true] {
        let mut object = body();
        object.internal_change_health(-20.0);
        assert!(object.status.effectively_dead);
        let context = if direct {
            DamageHitContext::default().with_healing_entry()
        } else {
            DamageHitContext::default()
        };
        begin_with_context(&mut object, 30.0, &context).finish_standalone(
            &mut object,
            None,
            &false,
        );
        assert_eq!((object.previous_health, object.health.current), (20.0, 0.0));
        assert_eq!(object.last_damage_info_type, Some(DamageType::Bullet));
        assert_eq!(object.last_healing_timestamp, None);
        assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
        assert!(object.status.effectively_dead);
    }
}

#[test]
fn dead_bridge_rejects_damage_healing_and_accepts_direct_healing() {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        "Object BoundedHealingEntryBridge\n KindOf = BRIDGE STRUCTURE\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
        "healing_entry_bridge.ini",
    ).expect("authored receiving bridge ActiveBody");
    let template = GameLogic::build_template_from_object_definition(
        "BoundedHealingEntryBridge",
        parser.get_definition("BoundedHealingEntryBridge").unwrap(),
        None,
    );
    let mut object = Object::new(template, ObjectId(955), Team::USA);
    object.internal_change_health(-100.0);
    object.status.keep_as_rubble = true;
    object.status.destroyed = true;
    object.status.on_die_started = true;
    assert!(object.status.effectively_dead);
    begin(&mut object, 30.0).finish_standalone(&mut object, None, &false);
    assert_eq!(
        (object.previous_health, object.health.current),
        (100.0, 0.0)
    );
    assert_eq!(object.last_healing_timestamp, None);
    assert!(object.status.effectively_dead);
    assert!(object.status.keep_as_rubble);
    assert!(object.status.destroyed);
    assert!(object.status.on_die_started);
    begin_with_context(
        &mut object,
        30.0,
        &DamageHitContext::default().with_healing_entry(),
    )
    .finish_standalone(&mut object, None, &false);
    assert_eq!((object.previous_health, object.health.current), (0.0, 30.0));
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_healing_timestamp, Some(123));
    assert!(!object.status.effectively_dead);
    assert!(!object.status.keep_as_rubble);
    assert!(!object.status.destroyed);
    assert!(!object.status.on_die_started);
}

#[test]
fn receiving_positive_healing_preserves_prior_damage_reaction_without_firing_it() {
    let mut world = GameLogic::new();
    let mut object = body();
    assert_eq!(object.health.current, 20.0);
    object.pending_fire_when_damaged_weapon = Some("PriorDamageReactionWitness".into());
    let id = object.id;
    world.objects.insert(id, object);
    let result = world
        .apply_owned_damage(
            id,
            10.0,
            None,
            DamageType::Healing,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
    assert!(!result.destroyed());
    let object = world.host_object(id).unwrap();
    assert_eq!(object.health.current, 30.0);
    assert_eq!(
        object.pending_fire_when_damaged_weapon.as_deref(),
        Some("PriorDamageReactionWitness")
    );
    assert!(!object.status.on_die_started);
}
