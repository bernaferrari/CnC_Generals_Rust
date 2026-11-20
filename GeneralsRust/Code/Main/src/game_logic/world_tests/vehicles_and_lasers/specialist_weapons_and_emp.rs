//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

fn admit_test_sticky_bomb_charge(game: &mut GameLogic) {
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "booby-trap-charge.ini".into(),
            "Object BoobyTrap\nKindOf = IMMOBILE BOOBY_TRAP\nBody = ActiveBody BodyTag\nMaxHealth = 1\nInitialHealth = 1\nEnd\nBehavior = StickyBombUpdate Sticky\nEnd\nEnd\n".into(),
        )])
        .expect("typed StickyBombUpdate charge content"),
    )
    .expect("admit typed StickyBombUpdate charge content");
}

#[test]
fn hero_spawn_waits_stealth_delay() {
    use crate::game_logic::host_colonel_burton::BURTON_STEALTH_DELAY_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut burton_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    burton_tpl.stealth_update = Some(crate::game_logic::object::StealthUpdateData {
        stealth_delay_frames: BURTON_STEALTH_DELAY_FRAMES,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".to_string(), burton_tpl);

    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    {
        let b = game_logic.host_object(burton_id).expect("burton");
        assert!(b.innate_stealth);
        assert!(
            !b.status.stealthed,
            "C++ ctor sets CAN_STEALTH only; STEALTHED waits StealthDelay"
        );
        assert_eq!(
            b.stealth_update_data().map(|d| d.stealth_delay_frames),
            Some(BURTON_STEALTH_DELAY_FRAMES)
        );
        assert_eq!(b.stealth_allowed_frame, BURTON_STEALTH_DELAY_FRAMES);
    }
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        !game_logic.host_object(burton_id).unwrap().status.stealthed,
        "must stay visible during StealthDelay"
    );
    game_logic.frame = BURTON_STEALTH_DELAY_FRAMES;
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        game_logic.host_object(burton_id).unwrap().status.stealthed,
        "hero cloaks after StealthDelay"
    );
}

#[test]
fn supw_patriot_emp_residual_dual_slot_and_disable() {
    use crate::game_logic::game_logic::accepted_projectile_owner_tests::
        patriot_retail_battery_tests::superweapon_world;
    use crate::game_logic::host_base_defense::{
        SUPW_PATRIOT_AIR_DAMAGE, SUPW_PATRIOT_GROUND_DAMAGE, SUPW_PATRIOT_GROUND_RANGE,
        is_supw_patriot_template,
    };

    for airborne in [false, true] {
        let (mut world, battery, target) = superweapon_world(airborne);
        let patriot = world.host_object(battery).unwrap();
        assert!(is_supw_patriot_template(&patriot.template_name));
        let ground = patriot.weapon.as_ref().expect("authored ground weapon");
        assert_eq!(ground.damage, SUPW_PATRIOT_GROUND_DAMAGE);
        assert_eq!(ground.range, SUPW_PATRIOT_GROUND_RANGE);
        // The second slot is the assisting weapon. Aircraft use the third.
        let aa = patriot.tertiary_weapon.as_ref().expect("authored AA weapon");
        assert_eq!(aa.damage, SUPW_PATRIOT_AIR_DAMAGE);
        assert!(aa.can_target_air && !aa.can_target_ground);

        let assets = world.application_assets().expect("retail W3D owner");
        world.objects.get_mut(&battery).unwrap().set_max_shots_to_fire(1);
        let initial_health = world.host_object(target).unwrap().health.current;
        let expected_damage = if airborne {
            SUPW_PATRIOT_AIR_DAMAGE
        } else {
            SUPW_PATRIOT_GROUND_DAMAGE
        };
        let tick = |world: &mut GameLogic| {
            world.tick_logic_frame_with_script_effects(
                LOGIC_FRAME_TIMESTEP,
                None,
                None,
                crate::game_logic::ScriptPresentationEffects {
                    pristine_bones: Some(assets.as_ref()),
                    ..Default::default()
                },
                |_| {},
                |_| {},
            );
        };
        let mut observed_owned_flight = false;
        for _ in 0..90 {
            tick(&mut world);
            observed_owned_flight |= world.combat_system.projectiles_snapshot().iter().any(|shot| {
                shot.target_id == Some(target)
                    && shot.projectile_object_name == "PatriotMissileEMP"
                    && shot.missile_motion.is_some()
                    && shot.damage == expected_damage
                    && shot.detonation_ocl_name == "SupW_OCL_PatriotEMPEffectSpheroid"
            });
            if world.host_object(target).is_none_or(|object| object.health.current < initial_health) {
                break;
            }
        }
        assert!(observed_owned_flight, "retail EMP round must fly before impact");
        assert_eq!(world.base_defense_residual_fires(), 1);
        let victim_at_impact = world.host_object(target).expect("direct damage alone is nonlethal");
        assert_eq!(victim_at_impact.health.current, initial_health - expected_damage);
        let effect = {
            let mut spheroids = world
                .objects
                .registered_values()
                .filter(|object| object.template_name == "EMPPatriotEffectSpheroid");
            let effect = spheroids.next().expect("authored detonation effect").id;
            assert!(spheroids.next().is_none(), "one detonation creates one effect");
            effect
        };
        // A completed tick exposes the next frame to execute.
        let spawn_frame = world.frame - 1;
        let position = world.objects[&effect].get_position();
        assert!(!world.objects[&effect].status.off_map);
        assert!(!world.objects[&target].status.off_map);
        let hardened = if airborne {
            let id = world.create_object_for_player("EmpTestHardened", 1, position).unwrap();
            let airframe = world.objects.get_mut(&id).unwrap();
            airframe.stamp_airborne_target_from_locomotor();
            assert!(airframe.status.airborne_target && airframe.is_kind_of(KindOf::EmpHardened));
            Some(id)
        } else {
            None
        };
        let builder = crate::save_load::SnapshotBuilder::new();
        let mut restored = None;

        // EMPUpdate.cpp applies the disable when the authored 100 ms fade starts.
        for elapsed in 0..3 {
            let victim = world.host_object(target).expect("the EMP fade has not started");
            assert!(victim.is_alive() && !victim.is_emp_disabled() && !victim.status.disabled_emp);
            assert_eq!(victim.status.airborne_target, airborne);
            if elapsed == 1 {
                let saved = builder.create_world_snapshot(&world).unwrap();
                let bytes = bincode_legacy::serialize(&saved).unwrap();
                let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes)
                    .unwrap();
                let (mut receiving, _, _) = superweapon_world(airborne);
                builder.restore_from_snapshot(&decoded, &mut receiving).unwrap();
                restored = Some(receiving);
            }
            if let Some(receiving) = &mut restored {
                assert_eq!(
                    receiving.objects[&effect].native_lifecycle,
                    world.objects[&effect].native_lifecycle,
                );
                assert_eq!(
                    receiving.objects[&effect].drawable_instance_scale.to_bits(),
                    world.objects[&effect].drawable_instance_scale.to_bits(),
                );
                tick(receiving);
            }
            tick(&mut world);
        }
        let mut restored = restored.expect("mid-pulse receiving world");
        let pulse_outcome = |world: &GameLogic| {
            world.host_object(target).map(|object| {
                (object.is_alive(), object.status.disabled_emp_until_frame)
            })
        };
        assert_eq!(pulse_outcome(&world), pulse_outcome(&restored));
        if airborne {
            assert!(
                world.host_object(target).is_none_or(|aircraft| !aircraft.is_alive()),
                "the pulse at {position:?} destroys the airborne target at fade start, got {:?}",
                world.host_object(target).map(|aircraft| (
                    aircraft.get_position(),
                    aircraft.health.current,
                    aircraft.status.disabled_emp_until_frame,
                )),
            );
        } else {
            let vehicle = world.host_object(target).expect("EMP leaves the ground vehicle alive");
            assert!(vehicle.is_emp_disabled() || vehicle.status.disabled_emp,
                "the authored pulse disables the ground vehicle at fade start");
            assert_eq!(vehicle.status.disabled_emp_until_frame, spawn_frame + 3 + 300);
        }
        for _ in 3..15 {
            let pulse = world.host_object(effect).expect("the authored pulse is still alive");
            assert!(pulse.is_alive());
            assert_eq!(
                pulse.native_lifecycle,
                restored.objects[&effect].native_lifecycle,
            );
            tick(&mut world);
            tick(&mut restored);
        }
        for receiving in [&world, &restored] {
            if let Some(hardened) = hardened {
                let immune = receiving.host_object(hardened).expect("hardened airframe survives");
                assert!(immune.is_alive() && !immune.is_emp_disabled() && immune.status.airborne_target);
            }
            assert!(
                receiving.host_object(effect).is_none_or(|pulse| !pulse.is_alive()),
                "the authored pulse dies at its 15-frame deadline",
            );
        }
    }
}

#[test]
fn supw_emp_scatter_misses_infantry_residual() {
    use crate::game_logic::host_base_defense::{
        SUPW_EMP_SCATTER_VS_INFANTRY, SUPW_PATRIOT_EMP_RADIUS,
    };

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut tpl = ThingTemplate::new("SupW_AmericaPatriotBattery");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), tpl);

    let bat = logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(5.0, 0.0, 0.0));
    logic.apply_supw_patriot_emp_residual_at(impact, bat, Team::USA, Some(inf));
    assert!(
        logic.supw_emp_scatter_applied > 0
            || logic.supw_emp_scatter_misses > 0
            || logic.honesty_supw_emp_scatter_ok(),
        "supw emp scatter residual must peel vs infantry"
    );
    assert!((SUPW_EMP_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);
    assert!((SUPW_PATRIOT_EMP_RADIUS - 10.0).abs() < 0.01);

    // Vehicle EMP center without infantry intended still grants in radius.
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(3.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(3.0, 0.0, 0.0));
    let before = logic.supw_patriot_emp_residual_grants;
    logic.apply_supw_patriot_emp_residual_at(impact, bat, Team::USA, Some(tank));
    assert!(
        logic.supw_patriot_emp_residual_grants > before || logic.honesty_supw_patriot_emp_ok(),
        "vehicle EMP grant residual"
    );
}

#[test]
fn supw_patriot_emp_intended_victim_near_miss_disables() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);

    let mut pat_tpl = crate::game_logic::ThingTemplate::new("SupW_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), pat_tpl);

    let mut air_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    air_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic.templates.insert("TestJet".to_string(), air_tpl);

    let bat = logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    // 30 units from blast: outside EffectRadius 10, inside leftover 40 near-miss.
    let jet = logic
        .create_object("TestJet", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("jet");
    if let Some(a) = logic.host_object_mut(jet) {
        a.status.airborne_target = true;
    }

    logic.apply_supw_patriot_emp_residual_at(Vec3::new(0.0, 0.0, 0.0), bat, Team::USA, Some(jet));
    let victim = logic.host_object(jet).expect("jet");
    assert!(
        victim.is_emp_disabled() || victim.status.disabled_emp,
        "intended aircraft near-miss must DISABLED_EMP"
    );

    // Farther than 40 and radius*2: leftover miss, stay mobile.
    let far = logic
        .create_object("TestJet", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("far jet");
    if let Some(a) = logic.host_object_mut(far) {
        a.status.airborne_target = true;
    }
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(far));
    let far_v = logic.host_object(far).expect("far jet");
    assert!(
        !far_v.is_emp_disabled() && !far_v.status.disabled_emp,
        "aircraft farther than 40 and radius*2 must not near-miss disable"
    );

    // Ground vehicle outside EffectRadius: C++ fallback requires KINDOF_AIRCRAFT.
    let tank = logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("tank");
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(tank));
    let tank_v = logic.host_object(tank).expect("tank");
    assert!(
        !tank_v.is_emp_disabled() && !tank_v.status.disabled_emp,
        "non-aircraft intended victim must not near-miss disable"
    );

    // EMP_HARDENED name marker (cargo plane) — leftover skips fallback.
    let mut cargo_tpl = crate::game_logic::ThingTemplate::new("AmericaJetCargoPlane");
    cargo_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaJetCargoPlane".to_string(), cargo_tpl);
    let cargo = logic
        .create_object("AmericaJetCargoPlane", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("cargo");
    if let Some(a) = logic.host_object_mut(cargo) {
        a.status.airborne_target = true;
    }
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(cargo));
    let cargo_v = logic.host_object(cargo).expect("cargo");
    assert!(
        !cargo_v.is_emp_disabled() && !cargo_v.status.disabled_emp,
        "EMP_HARDENED aircraft must not near-miss disable"
    );
}
