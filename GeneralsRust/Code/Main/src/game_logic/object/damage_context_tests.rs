//! C++ DamageInfoInput carries context per hit; HiveStructureBody.cpp55–65
//! selects the nearest slave to the shooter, ActiveBody.cpp309–315 uses logic frame.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::{KindOf, Team, ThingTemplate};

fn unit(name: &str, id: u32) -> Object {
    let mut template = ThingTemplate::new(name);
    template.set_health(200.0);
    template.add_kind_of(KindOf::Vehicle);
    Object::new_with_logic_frame(template, ObjectId(id), Team::USA, 0)
}

#[test]
fn live_hit_context_selects_slave_nearest_to_shooter() {
    let mut site = unit("GLAStingerSite", 901);
    site.hive_slaves = crate::game_logic::host_base_defense::init_stinger_hive_slave_roster();
    let mut shooter = unit("Source", 902);
    let closest = site.hive_slaves[2].world_xz(0.0, 0.0);
    shooter.set_position(glam::Vec3::new(closest.0, 0.0, closest.1));
    let initial = site.hive_slaves.map(|slave| slave.hp);
    let context = DamageHitContext::new(Some(&shooter), None, DamageType::Bullet);
    assert!(!site.take_damage_with_context(
        5.0,
        Some(shooter.id),
        DamageType::Bullet,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        23,
        &context
    ));
    assert_eq!(
        site.hive_slaves[0].hp, initial[0],
        "must not pick first alive slave"
    );
    assert_eq!(site.hive_slaves[1].hp, initial[1]);
    assert_eq!(site.hive_slaves[2].hp, initial[2] - 5.0);
}

#[test]
fn impact_frame_drives_damage_fx_throttle() {
    use crate::game_logic::host_transition_damage_fx::take_dispatched_armor_damage_fx;
    use game_engine::common::ini::ini_damage_fx;
    ini_damage_fx::init_global_damage_fx_store();
    let mut fx = ini_damage_fx::DamageFX::new();
    fx.set_major_minor_fx_at_level(
        ini_damage_fx::DamageType::Unresistable,
        0,
        Some("FX_ExplicitImpactFrame".into()),
        None,
        0.0,
    );
    ini_damage_fx::get_damage_fx_store_mut()
        .unwrap()
        .add_damage_fx("ExplicitImpactFrame".into(), fx);
    let mut victim = unit("FrameVictim", 903);
    victim
        .thing
        .template
        .armor_sets
        .push(crate::game_logic::HostArmorSet {
            conditions: 0,
            armor: None,
            damage_fx: Some("ExplicitImpactFrame".into()),
        });
    let ambient = crate::game_logic::host_historic_bonus::logic_frame();
    let impact = ambient.checked_add(123).expect("bounded test clock");
    victim.last_damage_fx_done = Some(DamageType::Unresistable);
    victim.next_damage_fx_time = impact - 1;
    let _ = take_dispatched_armor_damage_fx();
    assert!(!victim.take_damage_from_typed_death_at_frame(
        1.0,
        None,
        DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        impact
    ));
    assert!(
        take_dispatched_armor_damage_fx()
            .iter()
            .any(|name| name == "FX_ExplicitImpactFrame")
    );
    assert_eq!(victim.next_damage_fx_time, impact);
    assert_eq!(
        crate::game_logic::host_historic_bonus::logic_frame(),
        ambient
    );
}

// These are GREEN explicit-input controls, not executable OLD witnesses: the
// retired ambient implementation has no DamageHitContext API. They exercise
// actual admitted Objects and synchronous damage consumers, not whole-game
// isolation of still-shared rules, FX output catalogs or native callbacks.
fn frozen_context_world(hive: bool) -> (crate::game_logic::GameLogic, ObjectId, ObjectId) {
    let mut world = crate::game_logic::GameLogic::new();
    let mut source = ThingTemplate::new("FrozenContextSource");
    source.set_health(200.0).add_kind_of(KindOf::Vehicle);
    let name = if hive {
        "GLAStingerSite"
    } else {
        "FrozenContextVictim"
    };
    let mut victim = ThingTemplate::new(name);
    victim.set_health(200.0).add_kind_of(KindOf::Attackable);
    victim.add_kind_of(if hive {
        KindOf::Structure
    } else {
        KindOf::Vehicle
    });
    world.templates.insert(source.name.clone(), source);
    world.templates.insert(victim.name.clone(), victim);
    let source = world
        .create_object("FrozenContextSource", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object(name, Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    (world, source, victim)
}

#[test]
fn prepared_hive_context_keeps_pose_across_same_id_world_capture_and_generic_hit() {
    let (mut a, source_a, victim_a) = frozen_context_world(true);
    let (mut b, source_b, victim_b) = frozen_context_world(true);
    assert_eq!((source_a, victim_a), (source_b, victim_b));
    let roster = crate::game_logic::host_base_defense::init_stinger_hive_slave_roster();
    a.objects.get_mut(&victim_a).unwrap().hive_slaves = roster;
    b.objects.get_mut(&victim_b).unwrap().hive_slaves = roster;
    let at_a = roster[2].world_xz(0.0, 0.0);
    let at_b = roster[1].world_xz(0.0, 0.0);
    a.objects
        .get_mut(&source_a)
        .unwrap()
        .set_position(glam::Vec3::new(at_a.0, 0.0, at_a.1));
    let context_a = DamageHitContext::new(a.objects.get(&source_a), None, DamageType::Bullet);
    b.objects
        .get_mut(&source_b)
        .unwrap()
        .set_position(glam::Vec3::new(at_b.0, 0.0, at_b.1));
    let context_b = DamageHitContext::new(b.objects.get(&source_b), None, DamageType::Bullet);
    // A's live pose changes after capture too: delivery must use its frozen
    // operation input, not either world's current object with this ID.
    a.objects
        .get_mut(&source_a)
        .unwrap()
        .set_position(glam::Vec3::new(at_b.0, 0.0, at_b.1));
    let death = crate::game_logic::host_usa_pilot::HostDeathType::Normal;
    assert!(
        !a.objects
            .get_mut(&victim_a)
            .unwrap()
            .take_damage_with_context(
                5.0,
                Some(source_a),
                DamageType::Bullet,
                death,
                None,
                41,
                &context_a
            )
    );
    assert_eq!(a.objects[&victim_a].hive_slaves[2].hp, roster[2].hp - 5.0);
    assert_eq!(a.objects[&victim_a].hive_slaves[1].hp, roster[1].hp);
    assert_eq!(
        b.objects[&victim_b].hive_slaves.map(|s| s.hp),
        roster.map(|s| s.hp)
    );
    assert!(
        !b.objects
            .get_mut(&victim_b)
            .unwrap()
            .take_damage_with_context(
                7.0,
                Some(source_b),
                DamageType::Bullet,
                death,
                None,
                42,
                &context_b
            )
    );
    assert_eq!(b.objects[&victim_b].hive_slaves[1].hp, roster[1].hp - 7.0);
    assert_eq!(b.objects[&victim_b].hive_slaves[2].hp, roster[2].hp);
    // The source-free public wrapper intentionally retains first-alive Hive
    // fallback. Neither prepared source may become an ambient current hit.
    assert!(
        !a.objects
            .get_mut(&victim_a)
            .unwrap()
            .take_damage_from_typed_death_at_frame(
                3.0,
                Some(source_a),
                DamageType::Bullet,
                death,
                43
            )
    );
    assert_eq!(a.objects[&victim_a].hive_slaves[0].hp, roster[0].hp - 3.0);
    assert_eq!(a.objects[&victim_a].hive_slaves[1].hp, roster[1].hp);
    assert_eq!(a.objects[&victim_a].hive_slaves[2].hp, roster[2].hp - 5.0);
}

#[test]
fn prepared_context_freezes_source_veterancy_and_does_not_leak_status_or_fx_source() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_transition_damage_fx::take_dispatched_armor_damage_fx;
    use game_engine::common::ini::ini_damage_fx;
    const RULE: &str = "FrozenContextFaerieRule";
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(
            "Weapon FrozenContextFaerieRule\n  PrimaryDamage = 200\n  DamageType = STATUS\n  DamageStatusType = FAERIE_FIRE\nEnd\n"
        ),
        1
    );
    let (mut a, source_a, victim_a) = frozen_context_world(false);
    let (mut b, source_b, victim_b) = frozen_context_world(false);
    assert_eq!((source_a, victim_a), (source_b, victim_b));
    a.objects.get_mut(&source_a).unwrap().experience.level = VeterancyLevel::Elite;
    let context_a = DamageHitContext::new(a.objects.get(&source_a), Some(RULE), DamageType::Status);
    b.objects.get_mut(&source_b).unwrap().experience.level = VeterancyLevel::Heroic;
    let context_b = DamageHitContext::new(b.objects.get(&source_b), None, DamageType::Unresistable);
    a.objects.get_mut(&source_a).unwrap().experience.level = VeterancyLevel::Rookie;
    ini_damage_fx::init_global_damage_fx_store();
    let mut fx = ini_damage_fx::DamageFX::new();
    for (level, name) in [
        (0, "FX_FrozenContextRegular"),
        (2, "FX_FrozenContextElite"),
        (3, "FX_FrozenContextHeroic"),
    ] {
        for kind in [
            ini_damage_fx::DamageType::Status,
            ini_damage_fx::DamageType::Unresistable,
        ] {
            fx.set_major_minor_fx_at_level(kind, level, Some(name.into()), None, 0.0);
        }
    }
    ini_damage_fx::get_damage_fx_store_mut()
        .unwrap()
        .add_damage_fx("FrozenContextArmorFx".into(), fx);
    for victim in [
        a.objects.get_mut(&victim_a).unwrap(),
        b.objects.get_mut(&victim_b).unwrap(),
    ] {
        victim
            .template_mut()
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: None,
                damage_fx: Some("FrozenContextArmorFx".into()),
            });
    }
    let death = crate::game_logic::host_usa_pilot::HostDeathType::Normal;
    let _ = take_dispatched_armor_damage_fx();
    assert!(
        !a.objects
            .get_mut(&victim_a)
            .unwrap()
            .take_damage_with_context(
                200.0,
                Some(source_a),
                DamageType::Status,
                death,
                None,
                51,
                &context_a
            )
    );
    assert!(a.objects[&victim_a].is_faerie_fire());
    assert_eq!(a.objects[&victim_a].health.current, 200.0);
    let events = take_dispatched_armor_damage_fx();
    assert!(
        events.iter().any(|name| name == "FX_FrozenContextElite"),
        "frozen A veterancy: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|name| name == "FX_FrozenContextHeroic" || name == "FX_FrozenContextRegular")
    );
    assert!(!b.objects[&victim_b].is_faerie_fire());
    assert!(
        !b.objects
            .get_mut(&victim_b)
            .unwrap()
            .take_damage_with_context(
                1.0,
                Some(source_b),
                DamageType::Unresistable,
                death,
                None,
                52,
                &context_b
            )
    );
    let events = take_dispatched_armor_damage_fx();
    assert!(
        events.iter().any(|name| name == "FX_FrozenContextHeroic"),
        "B retains its own source: {events:?}"
    );
    assert_eq!(b.objects[&victim_b].health.current, 199.0);
    assert!(!b.objects[&victim_b].is_faerie_fire());
    assert!(
        !b.objects
            .get_mut(&victim_b)
            .unwrap()
            .take_damage_from_typed_death_at_frame(
                200.0,
                Some(source_b),
                DamageType::Status,
                death,
                53
            )
    );
    assert!(
        !b.objects[&victim_b].is_faerie_fire(),
        "generic hit has no prepared status"
    );
    assert_eq!(b.objects[&victim_b].health.current, 199.0);
    let events = take_dispatched_armor_damage_fx();
    assert!(
        events.iter().any(|name| name == "FX_FrozenContextRegular"),
        "generic FX has no leaked source: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|name| name == "FX_FrozenContextElite" || name == "FX_FrozenContextHeroic")
    );
}
