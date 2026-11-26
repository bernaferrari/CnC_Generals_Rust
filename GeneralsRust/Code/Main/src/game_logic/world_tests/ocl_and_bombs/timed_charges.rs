//! Timed demo charge detonation, attribution, and countdown tests.
use super::*;

#[test]
fn timed_demo_charge_residual_detonates_after_delay() {
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);

    // Short delay for test observability (not full 10s retail lifetime).
    let charge_id = game_logic
        .place_timed_demo_charge(Team::USA, Vec3::new(0.0, 0.0, 0.0), None, None, Some(3))
        .expect("charge");
    assert_eq!(game_logic.mine_residual_places(), 1);

    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("building");
    let health_before = game_logic.host_object(building_id).unwrap().health.current;

    // Before deadline: no detonation.
    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_timed_detonations(), 0);
    assert!(game_logic.host_object(charge_id).unwrap().is_alive());

    // At deadline: detonate.
    game_logic.frame = 3;
    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_timed_detonations(), 1);
    assert!(
        game_logic.honesty_timed_demo_charge_ok(),
        "timed charge honesty"
    );

    let building = game_logic.host_object(building_id).unwrap();
    assert!(
        building.health.current < health_before || building.status.destroyed,
        "timed charge must damage nearby structure"
    );
}

#[test]
fn timed_c4_kill_sinks_xp_to_planter() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    // C++ Object::scoreTheKill awards XP only between ENEMIES; fresh test
    // slots default Neutral (Player::new alliance_team=-1) — declare hostility.
    game_logic
        .get_player_mut(0)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    game_logic
        .get_player_mut(2)
        .unwrap()
        .set_map_relationship(0, gamelogic::common::Relationship::Enemies);
    let burton_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("burton");
    {
        let burton = game_logic.host_object_mut(burton_id).unwrap();
        burton.template_mut().is_trainable = true;
        burton.template_mut().veterancy_xp_thresholds = [40.0, 150.0, 300.0];
    }

    let charge_id = game_logic
        .place_timed_demo_charge(
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            Some(burton_id),
            None,
            Some(1),
        )
        .expect("charge");
    {
        let charge = game_logic.host_object(charge_id).unwrap();
        assert_eq!(
            charge.experience_sink,
            Some(burton_id),
            "C4 must sink XP to the planter"
        );
        assert!(!charge.thing().template.is_trainable);
    }

    let victim_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("victim");
    {
        let victim = game_logic.host_object_mut(victim_id).unwrap();
        victim.health.current = 10.0;
        victim.health.maximum = 10.0;
        victim.template_mut().experience_value = 40.0;
        victim.template_mut().experience_values = [40.0, 40.0, 80.0, 120.0];
    }

    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps().unwrap();

    let burton = game_logic.host_object(burton_id).unwrap();
    assert!(
        burton.experience.current + f32::EPSILON >= 40.0,
        "C4 kill XP must sink to planter, got {}",
        burton.experience.current
    );
    assert_eq!(burton.experience.level, VeterancyLevel::Veteran);
    assert!(burton.test_weapon_condition(0));
    assert!(burton.weapon_bonus_veteran);
}

/// StickyBombUpdate.cpp:34-43,83-104 + LifetimeUpdate.cpp:69-74: an admitted
/// StickyBombUpdate+LifetimeUpdate charge keeps only the native die frame for
/// its attach ping countdown. No residual timer is installed, and the authored
/// FireWeaponWhenDeadBehavior owns the blast — fired exactly once at expiry.
#[test]
fn authored_timed_charge_expires_once_through_its_death_weapon() {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(32, 32)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    ensure_test_player_for_team(&mut world, Team::USA);
    ensure_test_player_for_team(&mut world, Team::GLA);
    world
        .get_player_mut(0)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    world
        .get_player_mut(2)
        .unwrap()
        .set_map_relationship(0, gamelogic::common::Relationship::Enemies);
    world
        .admit_object_ini_sources([(
            "timed-witness.ini".to_owned(),
            "Object TimingWitnessCharge\n  KindOf = MINE NO_COLLIDE\n  Body = ActiveBody BodyTag\n    MaxHealth = 1\n    InitialHealth = 1\n  End\n  Behavior = StickyBombUpdate Sticky\n    OffsetZ = 3\n  End\n  Behavior = LifetimeUpdate Life\n    MinLifetime = 1000\n    MaxLifetime = 3000\n  End\n  Behavior = FireWeaponWhenDeadBehavior Dead\n    StartsActive = Yes\n    DeathWeapon = TimedChargeWitness\n    DeathTypes = NONE +NORMAL\n  End\nEnd\nObject WitnessVictim\n  KindOf = VEHICLE\n  Body = ActiveBody BodyTag\n    MaxHealth = 1000\n    InitialHealth = 1000\n  End\nEnd\n".to_owned(),
        )])
        .unwrap();
    let mut weapons = std::collections::HashMap::new();
    assert!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut weapons,
            // initStickyBomb makes the victim the charge's ProducerID, and
            // Weapon.cpp:1333 skips producer victims unless AFFECTS_SELF —
            // retail C4 weapons all author SELF (BurtonC4ChargeWeapon).
            "Weapon TimedChargeWitness\n PrimaryDamage = 31\n PrimaryDamageRadius = 20\n WeaponSpeed = 999999\n DamageType = UNRESISTABLE\n RadiusDamageAffects = SELF ENEMIES\n ProjectileObject = NONE\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ) >= 1,
        "fixture weapons must parse"
    );
    world
        .admit_weapon_content_snapshot(
            weapons
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    let victim = world
        .create_object("WitnessVictim", Team::GLA, Vec3::ZERO)
        .expect("victim");
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();

    world.frame = 123;
    let charge = world
        .place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge,
            "TimingWitnessCharge",
            Team::USA,
            Vec3::ZERO,
            None,
            Some(victim),
            Some(3),
        )
        .expect("authored timed charge");
    let deadline = world
        .host_object(charge)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .and_then(|lifecycle| lifecycle.first_lifetime_deadline())
        .expect("authored LifetimeUpdate die frame");
    assert!(
        (30..=90).contains(&(deadline - world.frame)),
        "MinLifetime=1000 MaxLifetime=3000 is a real 30..90 frame clock, got {deadline}"
    );
    let (residual_timer, first_ping) = {
        let data = world
            .host_object(charge)
            .unwrap()
            .mine_data
            .as_ref()
            .unwrap();
        assert_eq!(
            data.kind,
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
        );
        (data.detonate_at_frame, data.next_ping_frame)
    };

    // StickyBombUpdate.cpp:76-135 initStickyBomb: placement itself lands a
    // mobile target's charge at the authored OffsetZ and stores the target in
    // generic ProducerID — no attachment update needs to run first.
    let victim_pos = world.host_object(victim).unwrap().get_position();
    {
        let placed = world.host_object(charge).unwrap();
        assert_eq!(
            placed.producer_id,
            Some(victim),
            "initStickyBomb stores the target in generic ProducerID"
        );
        assert_eq!(
            placed.get_position(),
            Vec3::new(victim_pos.x, victim_pos.y + 3.0, victim_pos.z),
            "a mobile target is followed at the authored OffsetZ on placement"
        );
    }

    // StickyBombUpdate.cpp:145-175: each attachment update then re-follows a
    // moved victim at the same authored OffsetZ.
    world.set_fixture_object_position(victim, Vec3::new(1.0, 0.0, 2.0));
    world.update_sticky_bomb_attachments();
    assert_eq!(
        world.host_object(charge).unwrap().get_position(),
        Vec3::new(1.0, 3.0, 2.0)
    );

    // Drawable.cpp:3492-3524: the timed icon counts down the StickyBombUpdate
    // detonation frame — here the native LifetimeUpdate deadline — rounded up
    // to whole seconds.
    let presentation =
        crate::presentation_frame::PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let icon = presentation
        .objects
        .iter()
        .find(|object| object.id == charge)
        .expect("charge presentation");
    assert_eq!(icon.bomb_type, 1);
    assert_eq!(
        icon.bomb_timer_seconds,
        (deadline - world.frame).div_ceil(crate::game_logic::host_mines::UNIT_BOMB_PING_FRAMES)
    );

    // Frame 126 is the discarded synthetic delay (3 frames): it must never
    // schedule a blast. LifetimeUpdate kills the charge at its die frame and
    // the authored DeathWeapon fires exactly once.
    for frame in [126, deadline - 1, deadline, deadline + 1] {
        world.frame = frame;
        world.update_mines_and_demo_traps().unwrap();
        world.update_ai(&[charge], LOGIC_FRAME_TIMESTEP).unwrap();
        let before_expiry = frame < deadline;
        if frame == deadline - 1 {
            let late =
                crate::presentation_frame::PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
            let icon = late
                .objects
                .iter()
                .find(|object| object.id == charge)
                .expect("charge presentation");
            assert_eq!(
                icon.bomb_timer_seconds, 1,
                "icon rounds the last partial second up"
            );
        }
        assert_eq!(
            world
                .host_object(charge)
                .is_some_and(|object| object.is_alive()),
            before_expiry,
            "charge at frame {frame}"
        );
        assert_eq!(
            world.host_object(victim).unwrap().health.current,
            if before_expiry { 1000.0 } else { 969.0 },
            "victim HP at frame {frame}"
        );
    }

    assert_eq!(residual_timer, None, "no residual timer");
    assert_eq!(
        first_ping,
        Some(123 + (deadline - 123) % crate::game_logic::host_mines::UNIT_BOMB_PING_FRAMES),
        "pings count down to the native LifetimeUpdate die frame"
    );
}
