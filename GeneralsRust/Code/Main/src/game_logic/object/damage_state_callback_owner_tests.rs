//! CPP ActiveBody585–614: commit Body first, then observe its live state
//! after synchronous onDamage callbacks. These tests exercise the bounded
//! Body continuation; they do not claim an original-engine playthrough.
use super::*;
use crate::game_logic::host_bone_fx_damage::{
    HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
};
use crate::game_logic::host_enum_table_residual::{HostBodyDamageType, MC_BIT_DAMAGED};

fn body() -> Object {
    let mut template = ThingTemplate::new("BoundedBodyStateCallbackOwner");
    template.set_health(100.0);
    let mut object = Object::new(template, ObjectId(947), Team::USA);
    let mut authored = HostBoneFxAuthored::default();
    for state in [
        HostBodyDamageType::Damaged,
        HostBodyDamageType::ReallyDamaged,
    ] {
        authored.fx[state.ordinal() as usize][0] = Some(HostBoneFxSlot {
            bone: "Body".into(),
            only_once: true,
            delay_min: 0.0,
            delay_max: 0.0,
            name: format!("ObservedState{}", state.ordinal()),
        });
    }
    object.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    object
}

fn begin(object: &mut Object) -> damage::DamageApplication {
    object.begin_damage_with_context(
        40.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        123,
        &DamageHitContext::default(),
        &mut crate::game_logic::HostHealthEvents::default(),
        None,
    )
}

#[test]
fn accepted_damage_commits_body_and_model_before_consuming_state_callback() {
    let mut object = body();
    let mut application = begin(&mut object);
    assert_eq!(object.health.current, 60.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::Damaged);
    assert_ne!(object.model_condition_bits & (1u128 << MC_BIT_DAMAGED), 0);
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
    assert!(object.bone_fx_damage.as_ref().unwrap().pending.is_empty());
    application.complete_body_state_callbacks(&mut object, None);
    application.complete_body_state_callbacks(&mut object, None);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 1);
    assert_eq!(bone.pending.len(), 1);
    assert_eq!(bone.pending[0].old_state, HostBodyDamageType::Pristine);
    assert_eq!(bone.pending[0].new_state, HostBodyDamageType::Damaged);
}

#[test]
fn state_callback_rereads_body_after_intervening_health_change() {
    let mut object = body();
    let application = begin(&mut object);
    // The owner can run a synchronous onDamage effect after releasing Body.
    // Exercise its real health operation, without fabricating a callback log.
    object.internal_change_health(-40.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::ReallyDamaged);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 1);
    assert_eq!(bone.pending.len(), 1);
    assert_eq!(bone.pending[0].old_state, HostBodyDamageType::Pristine);
    assert_eq!(bone.pending[0].new_state, HostBodyDamageType::ReallyDamaged);
}

#[test]
fn restored_body_does_not_consume_the_obsolete_predicted_transition() {
    let mut object = body();
    let application = begin(&mut object);
    object.internal_change_health(40.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 0);
    assert!(bone.pending.is_empty());
}
