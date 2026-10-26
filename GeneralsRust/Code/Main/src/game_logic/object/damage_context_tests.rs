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
    let mut health_events = crate::game_logic::HostHealthEvents::default();

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
        &context,
        &mut health_events,
        None,
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
    let mut health_events = crate::game_logic::HostHealthEvents::default();

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
    let impact = 123;
    victim.last_damage_fx_done = Some(DamageType::Unresistable);
    victim.next_damage_fx_time = impact - 1;
    let _ = take_dispatched_armor_damage_fx();
    assert!(!victim.take_damage_from_typed_death_at_frame(
        1.0,
        None,
        DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        impact,
        &mut health_events
    ));
    assert!(
        take_dispatched_armor_damage_fx()
            .iter()
            .any(|name| name == "FX_ExplicitImpactFrame")
    );
    assert_eq!(victim.next_damage_fx_time, impact);
    assert_eq!(victim.last_damage_timestamp, Some(impact));
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
    a.set_fixture_object_position(source_a, glam::Vec3::new(at_a.0, 0.0, at_a.1));
    let context_a = DamageHitContext::new(a.objects.get(&source_a), None, DamageType::Bullet);
    b.set_fixture_object_position(source_b, glam::Vec3::new(at_b.0, 0.0, at_b.1));
    let context_b = DamageHitContext::new(b.objects.get(&source_b), None, DamageType::Bullet);
    // A's live pose changes after capture too: delivery must use its frozen
    // operation input, not either world's current object with this ID.
    a.set_fixture_object_position(source_a, glam::Vec3::new(at_b.0, 0.0, at_b.1));
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
                &context_a,
                &mut a.health_events,
                Some(&mut a.ownership_events),
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
                &context_b,
                &mut b.health_events,
                Some(&mut b.ownership_events),
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
                43,
                &mut a.health_events
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
                &context_a,
                &mut a.health_events,
                Some(&mut a.ownership_events),
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
                &context_b,
                &mut b.health_events,
                Some(&mut b.ownership_events),
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
                53,
                &mut b.health_events
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

fn history_world(incoming_kind: KindOf) -> (crate::game_logic::GameLogic, [ObjectId; 4]) {
    use crate::game_logic::{GameLogic, Player};
    let mut world = GameLogic::new();
    for (id, team) in [
        (1, Team::USA),
        (2, Team::GLA),
        (3, Team::China),
        (4, Team::GLA),
    ] {
        world.add_player(Player::new(id, team, "HistoryOwner", id == 1));
    }
    let mut ids = Vec::new();
    for (name, player, kind) in [
        ("HistoryVictim", 1, KindOf::Infantry),
        ("HistoryFirstSource", 2, KindOf::Infantry),
        ("HistoryIncomingSource", 3, incoming_kind),
        ("HistoryFactionSource", 4, KindOf::FSBarracks),
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(1000.0).add_kind_of(kind);
        world.templates.insert(name.into(), template);
        ids.push(
            world
                .create_object_for_player(name, player, glam::Vec3::ZERO)
                .unwrap(),
        );
    }
    (world, ids.try_into().unwrap())
}

fn history_hit(
    world: &mut crate::game_logic::GameLogic,
    victim: ObjectId,
    source: Option<ObjectId>,
    frame: u32,
    kind: DamageType,
) {
    world.frame = frame;
    let context = DamageHitContext::new(source.and_then(|id| world.objects.get(&id)), None, kind);
    world
        .apply_owned_damage(
            victim,
            1.0,
            source,
            kind,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
}

#[test]
fn last_damage_selects_current_receiving_source_kinds_and_disappearance() {
    let (mut a, [victim, first, incoming, faction]) = history_world(KindOf::Projectile);
    let (mut b, peer_ids) = history_world(KindOf::Vehicle);
    assert_eq!([victim, first, incoming, faction], peer_ids);
    history_hit(&mut a, victim, Some(first), 100, DamageType::Unresistable);
    history_hit(&mut b, victim, Some(first), 100, DamageType::Unresistable);
    // Same IDs, different actual classifications in different owners.
    history_hit(&mut a, victim, Some(incoming), 100, DamageType::Falling);
    history_hit(&mut b, victim, Some(incoming), 100, DamageType::Falling);
    assert_eq!(a.objects[&victim].last_damage_source, Some(first));
    assert_eq!(
        a.objects[&victim].last_damage_info_type,
        Some(DamageType::Unresistable)
    );
    assert!(a.get_player(1).unwrap().get_attacked_by(2));
    assert!(!a.get_player(1).unwrap().get_attacked_by(3));
    assert_eq!(b.objects[&victim].last_damage_source, Some(incoming));
    assert_eq!(
        b.objects[&victim].last_damage_info_type,
        Some(DamageType::Falling)
    );
    assert!(b.get_player(1).unwrap().get_attacked_by(3));
    history_hit(&mut a, victim, Some(faction), 101, DamageType::Falling);
    assert_eq!(a.objects[&victim].last_damage_source, Some(faction));
    // A non-null missing ID is not an existing attacker.
    history_hit(
        &mut a,
        victim,
        Some(ObjectId(999)),
        101,
        DamageType::Unresistable,
    );
    assert_eq!(a.objects[&victim].last_damage_source, Some(faction));
    assert_eq!(
        a.objects[&victim].last_damage_info_type,
        Some(DamageType::Falling)
    );
    a.objects.remove(&faction);
    history_hit(
        &mut a,
        victim,
        Some(incoming),
        102,
        DamageType::Unresistable,
    );
    assert_eq!(a.objects[&victim].last_damage_source, Some(incoming));
    assert_eq!(a.objects[&victim].last_damage_timestamp, Some(102));
    history_hit(&mut a, victim, None, 103, DamageType::Falling);
    assert_eq!(a.objects[&victim].last_damage_source, Some(incoming));
    assert_eq!(
        a.objects[&victim].last_damage_info_type,
        Some(DamageType::Unresistable)
    );
    assert_eq!(a.objects[&victim].last_damage_timestamp, Some(102));
    history_hit(&mut a, victim, None, 104, DamageType::Falling);
    assert_eq!(a.objects[&victim].last_damage_source, None);
    assert_eq!(a.objects[&victim].last_damage_timestamp, Some(104));
    assert_eq!(b.objects[&victim].last_damage_source, Some(incoming));
}

#[test]
fn last_damage_same_next_frame_relation_wraps_original_u32_clock() {
    let (mut world, [victim, first, incoming, _]) = history_world(KindOf::Projectile);
    history_hit(
        &mut world,
        victim,
        Some(first),
        u32::MAX,
        DamageType::Unresistable,
    );
    history_hit(&mut world, victim, Some(incoming), 0, DamageType::Falling);
    assert_eq!(world.objects[&victim].last_damage_source, Some(first));
    assert_eq!(world.objects[&victim].last_damage_timestamp, Some(u32::MAX));
    assert_eq!(
        world.objects[&victim].last_damage_info_type,
        Some(DamageType::Unresistable)
    );
    history_hit(&mut world, victim, Some(incoming), 1, DamageType::Falling);
    assert_eq!(world.objects[&victim].last_damage_source, Some(incoming));
    assert_eq!(world.objects[&victim].last_damage_timestamp, Some(1));
}

#[test]
fn initial_frame_zero_uses_cpp_uintmax_history_sentinel() {
    let (mut world, [victim, _, incoming, _]) = history_world(KindOf::Projectile);
    history_hit(
        &mut world,
        victim,
        Some(ObjectId(999)),
        0,
        DamageType::Unresistable,
    );
    assert_eq!(world.objects[&victim].last_damage_timestamp, None);
    assert_eq!(world.objects[&victim].last_damage_source, None);
    assert_eq!(world.objects[&victim].last_damage_info_type, None);
    history_hit(&mut world, victim, None, 0, DamageType::Falling);
    assert_eq!(world.objects[&victim].last_damage_timestamp, None);
    assert_eq!(world.objects[&victim].last_damage_info_type, None);
    history_hit(&mut world, victim, Some(incoming), 0, DamageType::Falling);
    assert_eq!(world.objects[&victim].last_damage_timestamp, Some(0));
    assert_eq!(world.objects[&victim].last_damage_source, Some(incoming));
    assert_eq!(
        world.objects[&victim].last_damage_info_type,
        Some(DamageType::Falling)
    );
    assert!(world.get_player(1).unwrap().get_attacked_by(3));
}

fn typed_history_world() -> (crate::game_logic::GameLogic, [ObjectId; 4]) {
    use crate::game_logic::{GameLogic, Player};
    let mut world = GameLogic::new();
    for (id, team) in [(0, Team::Neutral), (1, Team::USA), (2, Team::GLA)] {
        let mut player = Player::new(id, team, "TypedHistory", id == 1);
        if team == Team::Neutral {
            player.map_side.role = crate::game_logic::game_logic::PlayerSideRole::Neutral;
        }
        world.add_player(player);
    }
    let mut ids = Vec::new();
    for (name, player, kind, cap) in [
        ("TypedHistoryVehicle", 1, KindOf::Vehicle, 100.0),
        ("TypedHistoryInfantry", 1, KindOf::Infantry, 100.0),
        ("TypedHistoryImmune", 1, KindOf::Infantry, 0.0),
        ("TypedHistorySource", 2, KindOf::Infantry, 0.0),
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(100.0).add_kind_of(kind);
        template.subdual_damage_cap = cap;
        world.templates.insert(name.into(), template);
        ids.push(
            world
                .create_object_for_player(name, player, glam::Vec3::ZERO)
                .unwrap(),
        );
    }
    (world, ids.try_into().unwrap())
}

#[test]
fn typed_special_effects_notify_selected_player_without_hp_damage_or_peer_writes() {
    let (mut a, [vehicle, infantry, immune, source]) = typed_history_world();
    let (b, peer_ids) = typed_history_world();
    assert_eq!([vehicle, infantry, immune, source], peer_ids);
    for victim in [vehicle, infantry] {
        a.frame = 20;
        let context = DamageHitContext::new(
            a.objects.get(&source),
            None,
            DamageType::SubdualUnresistable,
        );
        a.apply_owned_damage(
            victim,
            10.0,
            Some(source),
            DamageType::SubdualUnresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
        assert_eq!(a.objects[&victim].health.current, 100.0);
        assert_eq!(a.objects[&victim].subdual_damage, 10.0);
        assert_eq!(a.objects[&victim].last_damage_source, Some(source));
        assert!(a.get_player(1).unwrap().get_attacked_by(2));
        assert_eq!(a.get_player(1).unwrap().get_attacked_frame(), 20);
    }
    a.get_player_mut(1).unwrap().attacked_by[2] = false;
    a.frame = 21;
    let context = DamageHitContext::for_kill();
    a.apply_owned_damage(
        immune,
        10.0,
        Some(source),
        DamageType::SubdualUnresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        &context,
    )
    .unwrap();
    assert_eq!(a.objects[&immune].subdual_damage, 0.0);
    assert_eq!(a.objects[&immune].last_damage_source, None);
    assert!(!a.get_player(1).unwrap().get_attacked_by(2));
    // STATUS changes the authored status, then notifies at the same frame;
    // it is duration input and never HP damage.
    let context = DamageHitContext::with_status(gamelogic::common::ObjectStatusTypes::FaerieFire);
    a.apply_owned_damage(
        vehicle,
        1000.0,
        Some(source),
        DamageType::Status,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        &context,
    )
    .unwrap();
    assert!(a.objects[&vehicle].status.faerie_fire);
    assert_eq!(a.objects[&vehicle].health.current, 100.0);
    assert_eq!(
        a.objects[&vehicle].last_damage_info_type,
        Some(DamageType::Status)
    );
    assert!(a.get_player(1).unwrap().get_attacked_by(2));
    assert_eq!(a.get_player(1).unwrap().get_attacked_frame(), 21);
    a.get_player_mut(1).unwrap().attacked_by[2] = false;
    a.apply_owned_damage(
        vehicle,
        0.0,
        Some(source),
        DamageType::Status,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        &DamageHitContext::default(),
    )
    .unwrap();
    assert!(!a.get_player(1).unwrap().get_attacked_by(2));
    a.frame = 22;
    a.apply_owned_damage(
        vehicle,
        0.0,
        Some(source),
        DamageType::Status,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        &DamageHitContext::for_kill(),
    )
    .unwrap();
    assert!(a.get_player(1).unwrap().get_attacked_by(2));
    assert_eq!(a.get_player(1).unwrap().get_attacked_frame(), 22);
    assert_eq!(a.objects[&vehicle].health.current, 100.0);
    assert!(!b.get_player(1).unwrap().get_attacked_by(2));
    assert_eq!(b.objects[&infantry].subdual_damage, 0.0);
}

#[test]
fn kill_pilot_notifies_actual_post_effect_neutral_owner_before_fx() {
    let (mut world, [vehicle, _, _, source]) = typed_history_world();
    world.frame = 31;
    let context = DamageHitContext::new(world.objects.get(&source), None, DamageType::KillPilot);
    world
        .apply_owned_damage(
            vehicle,
            1.0,
            Some(source),
            DamageType::KillPilot,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
    assert!(world.objects[&vehicle].status.disabled_unmanned);
    assert_eq!(world.objects[&vehicle].team, Team::Neutral);
    assert_eq!(world.objects[&vehicle].owner_player_id, Some(0));
    assert_eq!(world.objects[&vehicle].health.current, 100.0);
    assert!(world.get_player(0).unwrap().get_attacked_by(2));
    assert_eq!(world.get_player(0).unwrap().get_attacked_frame(), 31);
    assert!(!world.get_player(1).unwrap().get_attacked_by(2));
}

/// ActiveBody.cpp uses this match's frame for every hit. Constructors, peer
/// impacts/reset/drop and same object IDs cannot retime another receiver.
#[test]
fn damage_convenience_calls_use_each_driving_world_frame_after_peer_reset() {
    fn world(frame: u32) -> (crate::game_logic::GameLogic, ObjectId) {
        let mut world = crate::game_logic::GameLogic::new();
        world.frame = frame;
        let mut template = ThingTemplate::new("OwnedDamageFrameVictim");
        template.set_health(100.0).add_kind_of(KindOf::Vehicle);
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object("OwnedDamageFrameVictim", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        (world, id)
    }
    fn impact(world: &mut crate::game_logic::GameLogic, id: ObjectId) {
        let frame = world.frame;
        let (object, events) = world.host_object_and_health_events_mut(id).unwrap();
        assert!(!object.take_damage_from_typed(
            10.0,
            None,
            DamageType::Unresistable,
            frame,
            events
        ));
    }
    let (mut baseline, id) = world(37);
    let (mut owner, owner_id) = world(37);
    let (mut peer, peer_id) = world(901);
    assert_eq!(id, owner_id);
    assert_eq!(owner_id, peer_id);
    impact(&mut baseline, id);
    impact(&mut peer, peer_id);
    assert_eq!(
        peer.get_object(peer_id).unwrap().last_damage_timestamp,
        Some(901)
    );
    impact(&mut owner, owner_id);
    assert_eq!(
        owner.get_object(owner_id).unwrap().last_damage_timestamp,
        Some(37)
    );
    assert_eq!(
        owner.get_object(owner_id).unwrap().health.current,
        baseline.get_object(id).unwrap().health.current
    );
    peer.reset();
    drop(peer);
    let (mut replacement, replacement_id) = world(u32::MAX);
    impact(&mut replacement, replacement_id);
    assert_eq!(
        replacement
            .get_object(replacement_id)
            .unwrap()
            .last_damage_timestamp,
        None
    );
    assert_eq!(
        replacement
            .get_object(replacement_id)
            .unwrap()
            .health
            .current,
        90.0
    );
    replacement.frame = 1;
    impact(&mut replacement, replacement_id);
    assert_eq!(
        replacement
            .get_object(replacement_id)
            .unwrap()
            .last_damage_timestamp,
        Some(1)
    );
    assert_eq!(
        replacement
            .get_object(replacement_id)
            .unwrap()
            .health
            .current,
        80.0
    );
    owner.frame = 38;
    baseline.frame = 38;
    impact(&mut owner, owner_id);
    impact(&mut baseline, id);
    assert_eq!(
        owner.get_object(owner_id).unwrap().last_damage_timestamp,
        Some(37)
    );
    assert_eq!(
        owner.get_object(owner_id).unwrap().health.current,
        baseline.get_object(id).unwrap().health.current
    );
    // C++ retains a sourceless hit's timestamp on the same or next frame.
    owner.frame = 39;
    baseline.frame = 39;
    impact(&mut owner, owner_id);
    impact(&mut baseline, id);
    assert_eq!(
        owner.get_object(owner_id).unwrap().last_damage_timestamp,
        Some(39)
    );
    assert_eq!(
        baseline.get_object(id).unwrap().last_damage_timestamp,
        Some(39)
    );
    assert_eq!(owner.get_object(owner_id).unwrap().health.current, 70.0);
    assert_eq!(
        owner.get_object(owner_id).unwrap().health.current,
        baseline.get_object(id).unwrap().health.current
    );
}
