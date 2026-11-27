//! CPP ActiveBody585–614: commit Body first, then observe its live state
//! after synchronous onDamage callbacks. These tests exercise the bounded
//! Body continuation; they do not claim an original-engine playthrough.
use super::*;
use crate::game_logic::host_bone_fx_damage::{
    HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
};
use crate::game_logic::host_enum_table_residual::{MC_BIT_DAMAGED};
use generals_sim::object::BodyDamageType;

fn body() -> Object {
    // Authored ActiveBody (a programmatic template carries no body module, so the
    // retail damage-state thresholds never apply to it).
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        "Object BoundedBodyStateCallbackOwner\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
        "damage_state_callback_owner.ini",
    ).expect("authored receiving ActiveBody");
    let template = GameLogic::build_template_from_object_definition(
        "BoundedBodyStateCallbackOwner",
        parser
            .get_definition("BoundedBodyStateCallbackOwner")
            .unwrap(),
        None,
    );
    let mut object = Object::new(template, ObjectId(947), Team::USA);
    let mut authored = HostBoneFxAuthored::default();
    for state in [
        BodyDamageType::Damaged,
        BodyDamageType::ReallyDamaged,
    ] {
        authored.fx[state.ordinal() as usize][0] = Some(HostBoneFxSlot {
            bone: "Body".into(),
            only_once: true,
            delay_min: 0.0,
            delay_max: 0.0,
            name: format!("ObservedState{}", state.ordinal()),
        });
    }
    object.bone_fx_damage = Some(Box::new(HostBoneFxDamageData::from_authored(authored)));
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
    assert_eq!(object.body_damage_state, BodyDamageType::Damaged);
    assert_ne!(object.model_condition_bits & (1u128 << MC_BIT_DAMAGED), 0);
    assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
    assert!(object.bone_fx_damage.as_ref().unwrap().pending.is_empty());
    application.complete_body_state_callbacks(&mut object, None);
    application.complete_body_state_callbacks(&mut object, None);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 1);
    assert_eq!(bone.pending.len(), 1);
    // A BoneFX event names the state whose authored slot fired (BoneFXUpdate
    // fires the current state's slots), not the transition endpoints.
    assert_eq!(bone.pending[0].old_state, BodyDamageType::Damaged);
    assert_eq!(bone.pending[0].new_state, BodyDamageType::Damaged);
}

#[test]
fn state_callback_rereads_body_after_intervening_health_change() {
    let mut object = body();
    let application = begin(&mut object);
    // The owner can run a synchronous onDamage effect after releasing Body.
    // Exercise its real health operation, without fabricating a callback log.
    object.internal_change_health(-40.0);
    assert_eq!(object.body_damage_state, BodyDamageType::ReallyDamaged);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 1);
    assert_eq!(bone.pending.len(), 1);
    assert_eq!(bone.pending[0].old_state, BodyDamageType::ReallyDamaged);
    assert_eq!(bone.pending[0].new_state, BodyDamageType::ReallyDamaged);
}

#[test]
fn restored_body_does_not_consume_the_obsolete_predicted_transition() {
    let mut object = body();
    let application = begin(&mut object);
    object.internal_change_health(40.0);
    assert_eq!(object.body_damage_state, BodyDamageType::Pristine);
    application.finish_standalone(&mut object, None, &false);
    let bone = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(bone.transitions, 0);
    assert!(bone.pending.is_empty());
}

#[test]
fn receiving_transition_callback_runs_inline_and_is_consumed_once() {
    use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData;
    let mut object = body();
    let mut transition = HostTransitionDamageFxData::default();
    transition.enabled = true;
    transition.fx_lists_for_state[BodyDamageType::Damaged.ordinal() as usize] =
        vec!["FX_ReceivingBodyTransition".into()];
    object.transition_damage_fx = Some(transition);
    let mut application = begin(&mut object);
    let mut observations = Vec::new();
    let mut receive = |victim: &mut Object, event: crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent, source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>| {
        observations.push((victim.id, victim.health.current, event.fx_name.clone()));
        assert!(source.is_none());
        assert_eq!(victim.body_damage_state, BodyDamageType::Damaged);
        // The callback is a synchronous borrowed phase, not an end-of-frame
        // queue: the caller sees its mutation before continuing this impact.
        victim.name = "ObservedDuringBodyCallback".into();
    };
    application.complete_body_state_callbacks_with_transition_fx(&mut object, None, &mut receive);
    assert_eq!(object.name, "ObservedDuringBodyCallback");
    application.complete_body_state_callbacks_with_transition_fx(&mut object, None, &mut receive);
    assert_eq!(
        observations,
        vec![(
            ObjectId(947),
            60.0,
            Some("FX_ReceivingBodyTransition".into())
        )]
    );
    assert!(object.pending_transition_damage_fx.is_empty());
}

#[test]
fn ordinary_damage_within_the_same_body_band_does_not_dispatch_state_effects() {
    use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData;
    let mut object = body();
    object.internal_change_health(-40.0);
    assert_eq!(object.body_damage_state, BodyDamageType::Damaged);
    let mut transition = HostTransitionDamageFxData::default();
    transition.enabled = true;
    transition.attached_ids[BodyDamageType::Damaged.ordinal() as usize] = vec![77];
    object.transition_damage_fx = Some(transition);
    let mut application = object.begin_damage_with_context(
        5.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        124,
        &DamageHitContext::default(),
        &mut crate::game_logic::HostHealthEvents::default(),
        None,
    );
    let mut count = 0;
    application.complete_body_state_callbacks_with_transition_fx(
        &mut object,
        None,
        &mut |_, _, _| count += 1,
    );
    assert_eq!(object.health.current, 55.0);
    assert_eq!(object.body_damage_state, BodyDamageType::Damaged);
    assert_eq!(
        count, 0,
        "ActiveBody only emits the state callback when the damage band changes"
    );
    assert_eq!(
        object.transition_damage_fx.as_ref().unwrap().attached_ids
            [BodyDamageType::Damaged.ordinal() as usize],
        [77]
    );
}
