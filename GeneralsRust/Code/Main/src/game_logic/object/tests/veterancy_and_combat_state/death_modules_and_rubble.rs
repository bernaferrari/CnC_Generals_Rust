use super::*;

#[test]
fn fire_sound_loop_extends_and_stops() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "fire_sound_loop_extends_and_stops",
        || {
            use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
            // CPP FiringTracker.cpp:138 reads the fired template's authored timer.
            assert_eq!(
                crate::assets::ini_template_loader::register_weapons_from_ini_text(
                    r#"
Weapon FixtureDragonLoopWeapon
  FireSound = DragonTankWeaponLoop
  FireSoundLoopTime = 80
End
"#
                ),
                1
            );
            assert_eq!(
                crate::game_logic::weapon_bootstrap::host_fire_sound_loop_frames_for_weapon_name(
                    "FixtureDragonLoopWeapon"
                ),
                3
            );
            let mut tmpl = ThingTemplate::new("FlameLoop");
            tmpl.primary_weapon_name = Some("FixtureDragonLoopWeapon".into());
            tmpl.set_health(100.0);
            tmpl.add_kind_of(KindOf::Vehicle);
            tmpl.add_kind_of(KindOf::Attackable);
            let mut o = Object::new(tmpl, ObjectId(7), Team::China);
            o.weapon = Some(Weapon {
                damage: 5.0,
                range: 50.0,
                reload_time: 0.1,
                last_fire_time: -100.0,
                ..Weapon::default()
            });
            let mut combat = crate::game_logic::CombatSystem::new();
            let loop_events = &mut combat.fire_sound_loop_events;
            o.stamp_fire_sound_loop_after_shot(10, Some("FixtureDragonLoopWeapon"), loop_events);
            assert!(o.fire_sound_loop_until_frame > 10);
            loop_events.complete_boundary();
            let start = loop_events.take_completed();
            assert_eq!(start.len(), 1);
            assert!(start[0].start);
            // refresh should not re-emit start while still active
            o.stamp_fire_sound_loop_after_shot(11, Some("FixtureDragonLoopWeapon"), loop_events);
            assert!(loop_events.pending().is_empty());
            let stop_at = o.fire_sound_loop_until_frame;
            o.tick_fire_sound_loop(stop_at, loop_events);
            loop_events.complete_boundary();
            let stop = loop_events.take_completed();
            assert_eq!(stop.len(), 1);
            assert!(!stop[0].start);
            assert_eq!(o.fire_sound_loop_until_frame, 0);

            // Real same-ID objects use different receiving match output owners.
            // A peer's admission, restore, reset, construction and destruction
            // must not drain or reset A's already admitted transitions.
            let mut peer = Object::new(ThingTemplate::new("PeerLoop"), ObjectId(7), Team::China);
            let mut peer_combat = crate::game_logic::CombatSystem::new();
            o.stamp_fire_sound_loop_after_shot(20, Some("FixtureDragonLoopWeapon"), loop_events);
            loop_events.complete_boundary();
            o.tick_fire_sound_loop(23, loop_events);
            loop_events.complete_boundary();
            peer.stamp_fire_sound_loop_after_shot(
                30,
                Some("FixtureDragonLoopWeapon"),
                &mut peer_combat.fire_sound_loop_events,
            );
            peer_combat.fire_sound_loop_events.complete_boundary();
            let peer_start = peer_combat.fire_sound_loop_events.take_completed();
            assert_eq!(peer_start.len(), 1);
            assert_eq!(peer_start[0].unit, ObjectId(7));
            peer_combat.restore_pending_combat(&Default::default());
            peer_combat.reset_pending_projectiles();
            drop(peer_combat);
            let cold_peer = crate::game_logic::CombatSystem::new();
            assert!(cold_peer.fire_sound_loop_events.completed().is_empty());
            let fifo = loop_events.take_completed();
            assert_eq!(fifo.len(), 2);
            assert_eq!(
                fifo.iter().map(|event| event.start).collect::<Vec<_>>(),
                vec![true, false]
            );
            assert!(fifo.iter().all(|event| event.unit == ObjectId(7) && event.sound == "DragonTankWeaponLoop"));
            assert!(loop_events.take_completed().is_empty());

            // CPP UnsignedInt now + authored delay wraps, not saturates.
            o.stamp_fire_sound_loop_after_shot(
                u32::MAX - 1,
                Some("FixtureDragonLoopWeapon"),
                loop_events,
            );
            assert_eq!(o.fire_sound_loop_until_frame, 1);
            o.tick_fire_sound_loop(1, loop_events);
            assert_eq!(o.fire_sound_loop_until_frame, 0);
            loop_events.complete_boundary();
            assert_eq!(loop_events.take_completed().len(), 2);
            o.stamp_fire_sound_loop_after_shot(40, Some("FixtureDragonLoopWeapon"), loop_events);
            loop_events.complete_boundary();
            combat.restore_pending_combat(&Default::default());
            assert!(combat.fire_sound_loop_events.pending().is_empty());
            assert!(combat.fire_sound_loop_events.take_completed().is_empty());
            o.tick_fire_sound_loop(43, &mut combat.fire_sound_loop_events);
            combat.reset_pending_projectiles();
            assert!(combat.fire_sound_loop_events.pending().is_empty());
        },
    );
}

#[test]
fn height_die_kills_when_low() {
    use crate::game_logic::host_height_die::HostHeightDieData;
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaAuroraBomb");
    t.set_health(10.0);
    let mut o = Object::new(t, ObjectId(1), Team::USA);
    o.height_die = Some(HostHeightDieData::with_target(5.0, true, 0));
    o.set_position(glam::Vec3::new(0.0, 100.0, 0.0));
    assert!(!o.tick_height_die(1, 0.0));
    o.set_position(glam::Vec3::new(0.0, 50.0, 0.0));
    assert!(!o.tick_height_die(2, 0.0));
    o.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
    assert!(o.tick_height_die(3, 0.0));
    assert!(o.status.destroyed);
}

#[test]
fn squish_requires_velocity_toward_victim() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::host_squish_collide::velocity_toward_victim;
    assert!(velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (2.0, 0.0)));
    assert!(!velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (-2.0, 0.0)));

    let mut vt = ThingTemplate::new("CrusherTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(101), Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    // Moving toward infantry (+X).
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    tank.selection_radius = 8.0;

    // SquishCollide surface (retail crates author Behavior = SquishCollide,
    // Crate.ini:18297; C++ SquishCollide.cpp:79-93 gates the kill on velocity
    // toward the victim). Dealer lands inside the victim center zone so
    // CrushDie::onDie crushLocationCheck picks TOTAL (both body flags,
    // CrushDie.cpp:60-72 + 171-172).
    let mut it = ThingTemplate::new("CrushableInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(102), Team::GLA);
    inf.crushable_level = 0;
    inf.has_squish_collide = true;
    inf.selection_radius = 5.0;
    inf.set_position(glam::Vec3::new(1.0, 0.0, 0.0));
    inf.health.current = 100.0;
    inf.health.maximum = 100.0;

    assert!(
        tank.check_for_overlap_collision(0, &mut inf, false, &mut health_events),
        "squish must kill when moving toward infantry in tight radius"
    );
    assert!(inf.front_crushed && inf.back_crushed);
    assert!(!inf.is_alive() || inf.health.current <= 0.0);
}

#[test]
fn defection_timer_expires_and_blows_on_fire() {
    use crate::game_logic::host_defection_helper::DEFAULT_DEFECTION_PROTECTION_FRAMES;
    let mut t = ThingTemplate::new("AmericaInfantryPilot");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(301), Team::USA);
    o.begin_undetected_defection(0, 30, true);
    assert!(o.is_undetected_defector());
    for f in 0..29 {
        o.tick_defection_helper(f);
    }
    assert!(o.is_undetected_defector());
    o.tick_defection_helper(30);
    assert!(!o.is_undetected_defector());

    o.begin_undetected_defection(0, DEFAULT_DEFECTION_PROTECTION_FRAMES, true);
    assert!(o.is_undetected_defector());
    o.status.is_firing_weapon = true;
    o.tick_defection_helper(5);
    assert!(!o.is_undetected_defector());
}

#[test]
fn fire_weapon_power_queues_shots() {
    let mut t = ThingTemplate::new("SpectreHowitzerMarker");
    t.set_health(100.0);
    let mut o = Object::new(t, ObjectId(302), Team::USA);
    assert!(o.activate_fire_weapon_power(Some((100.0, 200.0))));
    let req = o.fire_weapon_power.as_ref().unwrap();
    assert_eq!(req.shots_remaining, 3);
    assert!(req.has_location);
}

#[test]
fn poisoned_behavior_dots_after_toxin() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    let mut t = ThingTemplate::new("TestInfantry");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(201), Team::USA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;

    let _ = o.take_damage_from_typed(20.0, None, DamageType::Toxin, 10, &mut health_events);
    // HP reduced by initial hit
    let after_hit = o.health.current;
    assert!(after_hit < 100.0);
    assert!(
        o.poisoned_behavior
            .as_ref()
            .map(|p| p.is_active())
            .unwrap_or(false)
    );
    assert!(o.is_poison_tinted());
    // Advance DoT ticks

    let mut total_dot = 0.0;
    for f in 11..100 {
        if let Some((d, _)) = o.tick_poisoned_behavior(f) {
            total_dot += d;
            let _ = o.take_damage_from_typed_death(
                d,
                None,
                DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
                f,
                &mut health_events,
            );
        }
    }
    assert!(total_dot > 0.0, "poison DoT must tick");
    assert!(o.health.current < after_hit || !o.is_alive());
}

#[test]
fn healing_clears_poison() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    let mut t = ThingTemplate::new("TestInfantry");
    t.set_health(100.0);
    let mut o = Object::new(t, ObjectId(202), Team::USA);
    o.health.current = 80.0;
    o.health.maximum = 100.0;

    let _ = o.take_damage_from_typed(10.0, None, DamageType::Toxin, 5, &mut health_events);
    assert!(o.is_poison_tinted());
    o.heal(5.0, &mut health_events, 5);
    assert!(!o.is_poison_tinted());
}

#[test]
fn bone_fx_fires_on_body_damage_worsen() {
    use crate::game_logic::host_bone_fx_damage::{
        HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
    };
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut t = ThingTemplate::new("GLAVehicleScudLauncher");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(103), Team::GLA);
    let mut authored = HostBoneFxAuthored::default();
    authored.fx[HostBodyDamageType::Damaged.ordinal() as usize][0] = Some(HostBoneFxSlot {
        bone: "FXBone01".into(),
        only_once: true,
        delay_min: 0.0,
        delay_max: 0.0,
        name: "FX_ScudLauncherDamageTransition".into(),
    });
    o.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    o.health.current = 1000.0;
    o.health.maximum = 1000.0;
    o.refresh_model_condition_bits();
    // Actual Body damage owns the transition and its operation-local clock.
    o.take_damage_from_typed_death_at_frame(
        600.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        41,
        &mut crate::game_logic::HostHealthEvents::default(),
    );
    assert!(
        o.bone_fx_damage
            .as_ref()
            .map(|b| b.transitions > 0)
            .unwrap_or(false),
        "BoneFX must fire on damage transition"
    );
    assert_eq!(
        o.bone_fx_damage.as_ref().and_then(|b| b.last_fx.as_deref()),
        Some("FX_ScudLauncherDamageTransition")
    );
}

#[test]
fn bone_fx_pristine_slots_fire_without_state_change() {
    use crate::game_logic::host_bone_fx_damage::{
        HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
    };
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut t = ThingTemplate::new("GLAVehicleScudLauncher");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(104), Team::GLA);
    let mut authored = HostBoneFxAuthored::default();
    authored.fx[HostBodyDamageType::Pristine.ordinal() as usize][0] = Some(HostBoneFxSlot {
        bone: "FXBone01".into(),
        only_once: true,
        delay_min: 0.0,
        delay_max: 0.0,
        name: "FX_ScudLauncherPristine".into(),
    });
    o.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    o.health.current = 1000.0;
    o.health.maximum = 1000.0;
    o.body_damage_state = HostBodyDamageType::Pristine;
    if let Some(bfx) = o.bone_fx_damage.as_mut() {
        bfx.tick(10);
    }
    assert_eq!(
        o.bone_fx_damage.as_ref().and_then(|b| b.last_fx.as_deref()),
        Some("FX_ScudLauncherPristine"),
        "C++ BoneFXUpdate ctor BODY_PRISTINE; first update fires authored pristine slots"
    );
}

#[test]
fn crush_die_sets_model_condition_bits() {
    use crate::game_logic::host_neutron_missile_slow_death::{
        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("TestInfantry");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(4), Team::USA);
    o.front_crushed = true;
    o.apply_crush_die_model_conditions();
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
    assert_eq!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
    o.back_crushed = true;
    o.apply_crush_die_model_conditions();
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
}

#[test]
fn keep_object_die_leaves_rubble() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("TechHospital");
    t.set_health(500.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(9), Team::Neutral);
    o.health.current = 0.0;
    assert!(o.begin_keep_object_die(
        10,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(o.status.keep_as_rubble);
    assert!(o.status.effectively_dead);
    assert!(!o.status.destroyed);
    assert!(!o.is_alive());
    assert!(
        o.status.no_collisions,
        "C++ ActiveBody rubble stamps OBJECT_STATUS_NO_COLLISIONS"
    );
}

#[test]
fn structure_rubble_collapses_geometry_z_and_stamps_no_collisions() {
    // C++ ActiveBody.cpp:189-208 setGeometryInfoZ + OBJECT_STATUS_NO_COLLISIONS.
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaCommandCenter");
    t.set_health(400.0);
    t.add_kind_of(KindOf::Structure);
    t.structure_rubble_height = 8;
    t.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 40.0,
        major_radius: 30.0,
        minor_radius: 25.0,
        authored: true,
    };
    let mut o = Object::new(t, ObjectId(11), Team::USA);
    assert!(((*o.geometry_info()).height - 40.0).abs() < 1e-4);
    assert!(!o.status.no_collisions);
    o.internal_change_health(-o.health.current);
    assert_eq!(
        o.body_damage_state,
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
    );
    assert!(o.status.no_collisions);
    assert!(
        ((*o.geometry_info()).height - 8.0).abs() < 1e-4,
        "rubble must collapse authored box Z to StructureRubbleHeight"
    );
    assert!((o.thing.geometry.bounds_max.y - o.thing.geometry.bounds_min.y - 8.0).abs() < 1e-4);
}

#[test]
fn structure_rubble_uses_the_objects_own_physics_rules() {
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};

    let mut template = ThingTemplate::new("SameStructure");
    template.add_kind_of(KindOf::Structure);
    template.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 40.0,
        major_radius: 30.0,
        minor_radius: 25.0,
        authored: true,
    };
    let mut first = Object::new(template.clone(), ObjectId(11), Team::USA);
    let mut second = Object::new(template, ObjectId(11), Team::USA);
    let mut first_rules = crate::game_logic::physics_rules::default_host_physics_rules();
    first_rules.default_structure_rubble_height = 4.0;
    let mut second_rules = first_rules;
    second_rules.default_structure_rubble_height = 9.0;
    first.bind_physics_rules(first_rules, first_rules.gravity);
    second.bind_physics_rules(second_rules, second_rules.gravity);

    first.apply_structure_rubble_collision_effects();
    assert_eq!(first.geometry_info().height, 4.0);
    assert_eq!(second.geometry_info().height, 40.0);
    second.apply_structure_rubble_collision_effects();
    assert_eq!(second.geometry_info().height, 9.0);
    assert_eq!(first.geometry_info().height, 4.0);
}

#[test]
fn ground_or_structure_height_uses_bounding_circle_not_major() {
    // C++ PartitionManager.cpp:4686-4687 FROM_BOUNDINGSPHERE_2D subtracts
    // bounding-circle (sqrt(major^2+minor^2)), so rooftop corners stay in range.
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("TestWideFactory");
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 25.0,
        major_radius: 20.0,
        minor_radius: 10.0,
        authored: true,
    };
    logic.templates.insert("TestWideFactory".into(), tmpl);
    logic
        .create_object("TestWideFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("spawn factory");
    // Corner of the box is ~22.36wu from center. major_radius=20 would miss
    // RANGE=1; bounding circle includes it.
    let corner = Vec3::new(20.5, 0.0, 10.5);
    let h = logic.ground_or_structure_height_at(corner, 0.0);
    assert!(
        (h - 25.0).abs() < 0.01,
        "rooftop corner must ride structure height, got {h}"
    );
}

#[test]
fn jet_slow_death_begins_for_raptor() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(3), Team::USA);
    o.health.current = 0.0;
    o.set_position(glam::Vec3::new(0.0, 80.0, 0.0));
    let start =
        o.begin_jet_slow_death(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    assert!(start.defer_destruction);
    assert!(start.enter_idle, "the owning World performs the Idle entry");
    assert!(o.jet_slow_death.as_ref().unwrap().is_active());
}

#[test]
fn jet_grounded_and_deck_death_destroys_same_frame_without_delay() {
    // C++ JetSlowDeathBehavior::onDie: ground/deck -> FXOnGroundDeath, OCL,
    // destroyObject immediately (no slow-death timer), then clear deck offset.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    for (y, deck) in [(0.0_f32, false), (80.0, true)] {
        let mut t = ThingTemplate::new("AmericaJetRaptor");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Aircraft);
        let mut o = Object::new(t, ObjectId(3), Team::USA);
        o.health.current = 0.0;
        o.set_position(glam::Vec3::new(0.0, y, 0.0));
        o.status.deck_height_offset = deck;
        let start =
            o.begin_jet_slow_death(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
        assert!(
            !start.defer_destruction,
            "deck={deck}: destroy is not deferred"
        );
        assert!(!start.enter_idle);
        let j = o.jet_slow_death.as_mut().unwrap();
        assert!(j.started_on_ground && j.done && !j.is_active());
        assert!(j.tick(0, 0.0).2, "any later tick reports destroy at once");
        assert!(
            !o.status.deck_height_offset,
            "onDie clears DECK_HEIGHT_OFFSET"
        );
    }
}

#[test]
fn helicopter_slow_death_begins() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaComanche");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(2), Team::USA);
    o.health.current = 0.0;
    let start =
        o.begin_helicopter_slow_death(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    assert!(start.defer_destruction);
    assert!(start.enter_idle, "the owning World performs the Idle entry");
    assert!(o.helicopter_slow_death.as_ref().unwrap().is_active());
}

#[test]
fn slow_death_infantry_defers_and_sinks() {
    use crate::game_logic::host_slow_death::HostSlowDeathIni;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(1), Team::USA);
    o.health.current = 0.0;
    // C++ SlowDeathBehavior.cpp:191 beginSlowDeath uses authored module data,
    // not KindOf::Infantry. Bare fixtures must pass INI SinkDelay/SinkRate/DestructionDelay.
    assert!(o.begin_slow_death_from_ini(
        0,
        &HostSlowDeathIni::infantry_retail(),
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(!o.status.destroyed);
    assert!(o.slow_death.as_ref().unwrap().is_active());
    let body_before_completion = o.health.current;
    let mut done = false;
    for f in 0..400 {
        if o.tick_slow_death(f) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(!o.status.destroyed, "timer reports deletion to its owner");
    assert_eq!(o.health.current, body_before_completion);
    assert!(o.presentation_slow_death_sink_offset() <= 0.0);
}

#[test]
fn create_object_die_queues_spawns() {
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("GLASneakAttackTunnelNetworkStart");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(1), Team::GLA);
    o.create_object_die = Some(HostCreateObjectDieData {
        ocl_name: "OCL_CreateSneakAttackTunnel".into(),
        spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
        transfer_previous_health: true,
        fired: false,
    });
    o.take_damage_from_typed_death_at_frame(
        100.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        41,
        &mut crate::game_logic::HostHealthEvents::default(),
    );
    let (spawns, dmg, transfer, _sub, _src) = o.take_pending_create_object_die_spawns();
    assert_eq!(spawns, vec!["GLASneakAttackTunnelNetwork".to_string()]);
    assert!(transfer);
    assert!(dmg >= 0.0);
}

#[test]
fn create_object_die_uses_previous_health_not_current() {
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("GLASneakAttackTunnelNetworkStart");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(3), Team::GLA);
    o.create_object_die = Some(HostCreateObjectDieData {
        ocl_name: "OCL_CreateSneakAttackTunnel".into(),
        spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
        transfer_previous_health: true,
        fired: false,
    });
    o.health.current = 0.0;
    o.previous_health = 100.0;
    o.subdual_damage = 25.0;
    o.last_damage_source = Some(ObjectId(9));
    o.fire_create_object_die();
    let (_spawns, dmg, transfer, sub, src) = o.take_pending_create_object_die_spawns();
    assert!(transfer);
    assert!(
        dmg.abs() < 1e-3,
        "lifetime kill at full previous health transfers 0, got {dmg}"
    );
    assert!((sub - 25.0).abs() < 1e-3);
    assert_eq!(src, Some(ObjectId(9)));
}

#[test]
fn crush_die_queues_sound_on_crushed_death() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(4), Team::USA);
    o.status.death_type = HostDeathType::Crushed;
    o.front_crushed = true;
    o.back_crushed = true;
    o.fire_crush_die();
    let (_, audio) = o.take_pending_death_fx_audio();
    assert_eq!(audio.as_deref(), Some("InfantryCrush"));
}

#[test]
fn crush_die_zero_flags_defaults_to_total() {
    use crate::game_logic::host_neutron_missile_slow_death::{
        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
    };
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(5), Team::USA);
    o.status.death_type = HostDeathType::Crushed;
    assert!(!o.front_crushed && !o.back_crushed);
    o.fire_crush_die();
    assert!(o.front_crushed && o.back_crushed);
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
    let (_, audio) = o.take_pending_death_fx_audio();
    assert_eq!(audio.as_deref(), Some("InfantryCrush"));
}

#[test]
fn crush_die_recomputes_front_from_crusher_pose() {
    use crate::game_logic::host_neutron_missile_slow_death::{
        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
    };
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(6), Team::USA);
    o.status.death_type = HostDeathType::Crushed;
    o.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    o.set_orientation(0.0);
    let mut geometry = *o.geometry_info();
    geometry.authored = true;
    geometry.major_radius = 10.0;
    o.set_geometry_shape(geometry);
    o.fire_crush_die_from_crusher(Some((5.0, 0.0)));
    assert!(o.front_crushed);
    assert!(!o.back_crushed);
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
    assert_eq!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
}

#[test]
fn instant_death_under_construction_building() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaParticleUplinkCannon");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(5), Team::USA);
    o.status.under_construction = true;
    assert!(o.fire_instant_death(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new())));
    let (fx, _) = o.take_pending_death_fx_audio();
    assert_eq!(fx.as_deref(), Some("FX_StructureMediumDeath"));
}

#[test]
fn lifetime_update_expires() {
    use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("PoisonFieldMedium");
    t.set_health(10.0);
    let mut o = Object::new(t, ObjectId(2), Team::Neutral);
    o.lifetime_update = Some(HostLifetimeUpdateData::from_delay_frames(0, 3));
    assert!(!o.tick_lifetime_update(2));
    assert!(o.tick_lifetime_update(3));
}

#[test]
fn transition_damage_fx_queues_on_worse_state() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaWarFactory");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(1), Team::USA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    o.transition_damage_fx = Some(HostTransitionDamageFxData::generic_structure_residual());
    o.body_damage_state = HostBodyDamageType::Pristine;
    o.take_damage_from_typed_death_at_frame(
        60.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        41,
        &mut crate::game_logic::HostHealthEvents::default(),
    );
    assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
    let ev = o.take_pending_transition_damage_fx();
    assert!(!ev.is_empty());
    assert_eq!(ev[0].new_state, HostBodyDamageType::Damaged.ordinal());
}

#[test]
fn fx_list_die_queues_on_rubble() {
    use crate::game_logic::host_fx_list_die::HostFxListDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaTankCrusader");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(2), Team::USA);
    o.fx_list_die = Some(HostFxListDieData {
        death_fx: Some("FX_VehicleDie".into()),
        death_audio: Some("VehicleDestroyed".into()),
        ..Default::default()
    });
    o.take_damage_from_typed_death_at_frame(
        100.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        41,
        &mut crate::game_logic::HostHealthEvents::default(),
    );
    let (fx, audio) = o.take_pending_death_fx_audio();
    assert_eq!(fx.as_deref(), Some("FX_VehicleDie"));
    assert_eq!(audio.as_deref(), Some("VehicleDestroyed"));
}

#[test]
fn fx_list_die_extra_module_uses_last_damage_source() {
    use crate::game_logic::host_fx_list_die::HostFxListDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaTankCrusader");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(2), Team::USA);
    o.fx_list_die = Some(HostFxListDieData {
        death_fx: Some("FX_VehicleDie".into()),
        more: vec![HostFxListDieData::with_fx("FX_RicochetDie")],
        ..Default::default()
    });
    o.last_damage_source = Some(ObjectId(99));
    o.fire_fx_list_die();
    let (fx, _) = o.take_pending_death_fx_audio();
    assert_eq!(fx.as_deref(), Some("FX_VehicleDie"));
    assert_eq!(o.last_damage_source, Some(ObjectId(99)));
}

#[test]
fn fx_list_die_exempt_status_skips_burned_object() {
    use crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaTankCrusader");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(3), Team::USA);
    o.fx_list_die = Some(fx_list_die_from_behavior_attrs_for_test());
    o.object_status_bits = object_status_mask_from_names(&["BURNED"]);
    o.fire_fx_list_die();
    let (fx, _) = o.take_pending_death_fx_audio();
    assert!(fx.is_none(), "ExemptStatus BURNED must skip FXListDie");
}

#[test]
fn structure_collapse_on_lethal() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("CivilianBarn01");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    t.authored_structure_collapse = true;
    let mut b = Object::new(t, ObjectId(1), Team::Neutral);
    b.health.current = 100.0;
    assert!(b.begin_structure_collapse(
        5,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(b.structure_collapse_data.as_ref().unwrap().is_active());
    let mut done = false;
    for f in 5..800 {
        if b.tick_structure_collapse(f) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(
        !b.status.destroyed,
        "animation DONE must not manufacture destruction"
    );
    assert!(b.presentation_collapse_height_offset() < -1.0);
}

#[test]
fn structure_collapse_binds_and_plays_phase_fx() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let definition = |delay: u32, phase_fx: &str| {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                &format!(
                    "Object CivilianBarn01\n  KindOf = STRUCTURE\n  Behavior = StructureCollapseUpdate ModuleTag_Sink\n    MinBurstDelay = {delay}\n    MaxBurstDelay = {delay}\n    FXList = {phase_fx}\n  End\nEnd\n"
                ),
                "collapse-owner.ini",
            )
            .unwrap();
        parser.get_definition("CivilianBarn01").unwrap().clone()
    };
    let make_object = |definition| {
        let mut template = ThingTemplate::new("CivilianBarn01");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Structure);
        template.authored_structure_collapse = true;
        template.receiving_object_definition = Some(std::sync::Arc::new(definition));
        Object::new(template, ObjectId(1), Team::Neutral)
    };
    let mut b = make_object(definition(
        66,
        "INITIAL FX_DieInitial\nDELAY FX_DieDelay\nBURST FX_DieBurst\nFINAL FX_DieFinal",
    ));
    let mut other = make_object(definition(
        132,
        "INITIAL FX_OtherInitial\nDELAY FX_OtherDelay\nBURST FX_OtherBurst\nFINAL FX_OtherFinal",
    ));
    assert!(b.begin_structure_collapse(
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(other.begin_structure_collapse(
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    {
        let sc = b.structure_collapse_data.as_ref().unwrap();
        assert!(sc.initial_played);
        assert_eq!(sc.min_burst_delay, 66);
        assert_eq!(sc.fx_initial.as_deref(), Some("FX_DieInitial"));
        assert_eq!(sc.fx_burst.as_deref(), Some("FX_DieBurst"));
        assert_eq!(sc.fx_delay.as_deref(), Some("FX_DieDelay"));
        assert_eq!(sc.fx_final.as_deref(), Some("FX_DieFinal"));
        assert!(
            sc.pending_phase_fx.is_empty(),
            "INITIAL dispatched at begin"
        );
    }
    {
        let sc = other.structure_collapse_data.as_ref().unwrap();
        assert_eq!(sc.min_burst_delay, 132);
        assert_eq!(sc.fx_initial.as_deref(), Some("FX_OtherInitial"));
        assert_eq!(sc.fx_burst.as_deref(), Some("FX_OtherBurst"));
    }
    let mut saw_start_burst = false;
    let mut done = false;
    for f in 0..800 {
        if b.tick_structure_collapse(f) {
            done = true;
            break;
        }
        if b.structure_collapse_data
            .as_ref()
            .map(|d| d.start_burst_played)
            .unwrap_or(false)
        {
            saw_start_burst = true;
        }
    }
    assert!(saw_start_burst);
    assert!(done);
    assert!(
        b.structure_collapse_data
            .as_ref()
            .map(|d| d.final_played)
            .unwrap_or(false)
    );
}

#[test]
fn structure_topple_on_lethal_damage() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaWarFactory");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Structure);
    t.authored_structure_topple = true;
    let mut b = Object::new(t, ObjectId(1), Team::USA);
    b.health.current = 200.0;
    assert!(b.begin_structure_topple(
        10,
        Some((0.0, 0.0)),
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(
        b.structure_topple_data.as_ref().unwrap().is_active()
            || !b.structure_topple_data.as_ref().unwrap().is_standing()
    );
    assert!(!b.status.destroyed);
    let mut done = false;
    for f in 10..800 {
        if b.tick_structure_topple(f) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(
        !b.status.destroyed,
        "animation DONE must not manufacture destruction"
    );
    assert_eq!(
        b.structure_topple_data.as_ref().unwrap().state,
        crate::game_logic::host_structure_topple::HostStructureToppleState::Done
    );
    assert!(b.presentation_topple_lean_radians() > 1.0);
}

#[test]
fn topple_residual_falls_and_dies() {
    use crate::game_logic::host_topple::{HostToppleData, TOPPLE_OPTIONS_NO_BOUNCE};
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("TreeOak");
    t.set_health(50.0);
    let mut tree = Object::new(t, ObjectId(1), Team::Neutral);
    tree.health.current = 50.0;
    tree.topple_data = Some(HostToppleData::default());
    assert!(!tree.apply_topple(1.0, 0.0, 2.0, TOPPLE_OPTIONS_NO_BOUNCE));
    assert!(tree.is_alive());
    let mut died = false;
    for _ in 0..600 {
        if tree.tick_topple() {
            died = true;
            break;
        }
    }
    assert!(died);
    assert!(tree.status.destroyed);
    assert_eq!(
        tree.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Toppled
    );
}

#[test]
fn crate_and_chemsuit_set_terrain_decal_types() {
    use crate::game_logic::host_battlemaster::{
        TERRAIN_DECAL_CHEMSUIT, TERRAIN_DECAL_CRATE, TERRAIN_DECAL_SHADOW_TEXTURE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut ct = ThingTemplate::new("SalvageCrate");
    ct.add_kind_of(KindOf::Crate);
    let mut crate_obj = Object::new(ct, ObjectId(1), Team::Neutral);
    crate_obj.apply_crate_terrain_decal();
    assert_eq!(crate_obj.terrain_decal_type, TERRAIN_DECAL_CRATE);
    assert!(crate_obj.terrain_decal_size > 0.0);
    assert!(crate_obj.terrain_decal_fade_rate > 0.0);
    crate_obj.tick_terrain_decal_fade();
    assert!(crate_obj.terrain_decal_opacity > 0.0);

    let mut it = ThingTemplate::new("AmericaInfantryRanger");
    it.add_kind_of(KindOf::Infantry);
    let mut ranger = Object::new(it, ObjectId(2), Team::USA);
    ranger.set_terrain_decal_chemsuit(true);
    assert!(ranger.terrain_decal_chemsuit);
    assert_eq!(ranger.terrain_decal_type, TERRAIN_DECAL_CHEMSUIT);

    let mut ft = ThingTemplate::new("GLAFakeBarracks");
    ft.add_kind_of(KindOf::Structure);
    ft.add_kind_of(KindOf::FSFake);
    let fake = Object::new(ft, ObjectId(3), Team::GLA);
    assert_eq!(fake.terrain_decal_type, TERRAIN_DECAL_SHADOW_TEXTURE);
}

#[test]
fn tree_topple_queues_stump_and_crush_direction() {
    use crate::game_logic::host_topple::TOPPLE_OPTIONS_NONE;
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("TreeOak");
    t.set_health(50.0);
    let mut tree = Object::new(t, ObjectId(4), Team::Neutral);
    tree.health.current = 50.0;
    assert!(!tree.apply_topple(0.0, 1.0, 2.0, TOPPLE_OPTIONS_NONE));
    let td = tree.topple_data.as_ref().expect("topple");
    assert!(!td.shadows_enabled);
    assert!((td.dir_y - 1.0).abs() < 1e-5);
    assert_eq!(td.pending_stump_name, "TreeOakStump");
    assert_eq!(td.pending_topple_fx, "");
    // FX already dispatched (pending cleared).
}
