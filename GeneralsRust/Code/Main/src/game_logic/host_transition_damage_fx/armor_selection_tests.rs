//! Original ActiveBody throttle admission precedes a synchronous FX callback.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::{HostArmorSet, Object, ObjectId, Team, ThingTemplate};
use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_damage_fx::{self, DamageFX};

fn body() -> Object {
    body_named("ArmorCommitVictim")
}
fn body_named(name: &str) -> Object {
    ini_damage_fx::init_global_damage_fx_store();
    let mut data = DamageFX::new();
    ini_damage_fx::parse_time(&mut INI::new(), &mut data, &["Default", "1000"]).unwrap();
    for kind in [
        ini_damage_fx::DamageType::Explosion,
        ini_damage_fx::DamageType::SmallArms,
    ] {
        data.set_major_minor_fx(
            kind,
            Some("FX_ArmorCommitMajor".into()),
            Some("FX_ArmorCommitMinor".into()),
            10.,
        );
    }
    ini_damage_fx::get_damage_fx_store_mut()
        .unwrap()
        .add_damage_fx("ArmorCommitRules".into(), data);
    let mut template = ThingTemplate::new(name);
    template.set_health(100.);
    template.armor_sets.push(HostArmorSet {
        conditions: 0,
        armor: None,
        damage_fx: Some("ArmorCommitRules".into()),
    });
    Object::new(template, ObjectId(9821), Team::USA)
}
fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

#[test]
fn selected_callback_sees_committed_throttle_and_reentry_without_definition_guard() {
    isolated(
        "selected_callback_sees_committed_throttle_and_reentry_without_definition_guard",
        || {
            let mut victim = body();
            let mut calls = 0;
            let result = dispatch_armor_damage_fx_with_dispatch(
                &mut victim,
                DamageType::Explosive,
                10.,
                100,
                None,
                &mut |victim, name, source| {
                    calls += 1;
                    assert_eq!(name, "FX_ArmorCommitMajor");
                    assert_eq!(victim.last_damage_fx_done, Some(DamageType::Explosive));
                    assert_eq!(
                        victim.next_damage_fx_time, 130,
                        "CPP commits before executing nuggets"
                    );
                    assert!(source.is_none());
                    let nested = dispatch_armor_damage_fx_with_dispatch(
                        victim,
                        DamageType::Explosive,
                        10.,
                        101,
                        None,
                        &mut |_, _, _| {
                            panic!("same-type reentry must observe the committed throttle")
                        },
                    );
                    assert!(nested.is_none());
                    // This would deadlock if the authored DamageFX read guard
                    // survived into the selected-list callback.
                    drop(ini_damage_fx::get_damage_fx_store_mut().unwrap());
                },
            );
            assert_eq!(result.as_deref(), Some("FX_ArmorCommitMajor"));
            assert_eq!(calls, 1);
        },
    );
}

#[test]
fn zero_amount_commits_without_dispatch_and_type_and_exact_expiry_follow_cpp() {
    isolated(
        "zero_amount_commits_without_dispatch_and_type_and_exact_expiry_follow_cpp",
        || {
            let mut victim = body();
            let mut names = Vec::new();
            let mut receive = |_: &mut Object, name: &str, _: Option<&HostDamageFxVictim>| {
                names.push(name.to_string())
            };
            let zero = dispatch_armor_damage_fx_with_dispatch(
                &mut victim,
                DamageType::Explosive,
                0.,
                100,
                None,
                &mut receive,
            );
            assert_eq!(zero.as_deref(), Some("ArmorCommitRules"));
            assert_eq!(victim.next_damage_fx_time, 130);
            assert!(
                dispatch_armor_damage_fx_with_dispatch(
                    &mut victim,
                    DamageType::Explosive,
                    1.,
                    101,
                    None,
                    &mut receive
                )
                .is_none()
            );
            assert!(
                dispatch_armor_damage_fx_with_dispatch(
                    &mut victim,
                    DamageType::Bullet,
                    1.,
                    101,
                    None,
                    &mut receive
                )
                .is_some()
            );
            assert_eq!(victim.next_damage_fx_time, 131);
            assert!(
                dispatch_armor_damage_fx_with_dispatch(
                    &mut victim,
                    DamageType::Bullet,
                    20.,
                    130,
                    None,
                    &mut receive
                )
                .is_none()
            );
            assert!(
                dispatch_armor_damage_fx_with_dispatch(
                    &mut victim,
                    DamageType::Bullet,
                    20.,
                    131,
                    None,
                    &mut receive
                )
                .is_some()
            );
            assert_eq!(names, ["FX_ArmorCommitMinor", "FX_ArmorCommitMajor"]);
        },
    );
}

#[test]
fn unsigned_throttle_wrap_uses_original_addition_and_original_comparison() {
    isolated(
        "unsigned_throttle_wrap_uses_original_addition_and_original_comparison",
        || {
            let mut victim = body();
            let mut count = 0;
            let mut receive = |victim: &mut Object, _: &str, _: Option<&HostDamageFxVictim>| {
                count += 1;
                assert!(
                    victim.next_damage_fx_time < 30,
                    "unsigned time must wrap, not saturate"
                );
            };
            dispatch_armor_damage_fx_with_dispatch(
                &mut victim,
                DamageType::Explosive,
                10.,
                u32::MAX - 4,
                None,
                &mut receive,
            );
            assert_eq!(victim.next_damage_fx_time, 25);
            // CPP compares the unsigned values directly; do not silently replace
            // it with a different wrap-aware deadline policy during this port.
            assert!(
                dispatch_armor_damage_fx_with_dispatch(
                    &mut victim,
                    DamageType::Explosive,
                    10.,
                    0,
                    None,
                    &mut receive
                )
                .is_none()
            );
            assert!(
                dispatch_armor_damage_fx_with_dispatch(
                    &mut victim,
                    DamageType::Explosive,
                    10.,
                    u32::MAX - 3,
                    None,
                    &mut receive
                )
                .is_some()
            );
            assert_eq!(victim.next_damage_fx_time, 26);
            assert_eq!(count, 2);
        },
    );
}

#[test]
fn consumed_hive_forwarding_and_swallow_do_not_emit_or_throttle_hull_armor_fx() {
    isolated(
        "consumed_hive_forwarding_and_swallow_do_not_emit_or_throttle_hull_armor_fx",
        || {
            use crate::game_logic::{DamageHitContext, HostHealthEvents};
            for forwarded in [true, false] {
                let mut hive = body_named("GLAStingerSite");
                if forwarded {
                    hive.hive_slaves =
                        crate::game_logic::host_base_defense::init_stinger_hive_slave_roster();
                }
                let before: f32 = hive.hive_slaves.iter().map(|slave| slave.hp).sum();
                let source = Object::new(
                    ThingTemplate::new("HullFxSource"),
                    ObjectId(9833),
                    Team::China,
                );
                let kind = if forwarded {
                    DamageType::Bullet
                } else {
                    DamageType::Sniper
                };
                let context = DamageHitContext::new(Some(&source), None, kind);
                let mut events = HostHealthEvents::default();
                let mut calls = 0;
                let mut receive =
                    |_: &mut Object,
                     _: crate::game_logic::object::BodyStateFx,
                     _: Option<&HostDamageFxVictim>| calls += 1;
                let _ = take_dispatched_armor_damage_fx();
                let application = hive.begin_damage_with_context_and_effects(
                    5.,
                    Some(source.id),
                    kind,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    100,
                    &context,
                    &mut events,
                    None,
                    |_| {},
                    &mut receive,
                );
                application.finish_standalone_with_body_fx(&mut hive, None, &false, &mut receive);
                assert_eq!(hive.health.current, 100.);
                assert_eq!(
                    calls, 0,
                    "consumed request does not reach the hive hull's ArmorFX"
                );
                assert_eq!(hive.last_damage_fx_done, None);
                assert_eq!(hive.next_damage_fx_time, 0);
                assert!(take_dispatched_armor_damage_fx().is_empty());
                let after: f32 = hive.hive_slaves.iter().map(|slave| slave.hp).sum();
                assert_eq!(before - after, if forwarded { 5. } else { 0. });
            }
        },
    );
}
