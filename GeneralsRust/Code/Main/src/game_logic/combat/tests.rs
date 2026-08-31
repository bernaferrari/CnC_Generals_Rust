// Characterization coverage for the live C++ weapon/projectile behavior seams.

#[cfg(test)]
mod tests {
    /// CombatSystem unit tests apply damage without a GameWorld shadow session,
    /// so host HP must mutate directly (opt out of damage authority last-writer).
    fn ensure_unit_test_direct_damage() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");
    }

    use super::*;
    use crate::game_logic::{KindOf, Object, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    /// The characterization tests in this module share process globals (the
    /// pending-projectile queue, the WeaponStore delayed-damage list, the
    /// global logic frame, and env-authority flags), so the default parallel
    /// test harness must not run them concurrently inside one process.
    /// Mirrors the host_rng_residual RNG_TEST_LOCK precedent.
    static COMBAT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn combat_test_guard() -> std::sync::MutexGuard<'static, ()> {
        COMBAT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }


    fn make_obj(
        name: &str,
        id: ObjectId,
        team: Team,
        pos: Vec3,
        kinds: &[KindOf],
        radius: f32,
    ) -> Object {
        let mut tmpl = ThingTemplate::new(name);
        tmpl.set_health(200.0);
        for k in kinds {
            tmpl.add_kind_of(*k);
        }
        let mut obj = Object::new(tmpl, id, team);
        obj.set_position(pos);
        obj.selection_radius = radius;
        obj
    }

    /// A long coordinate flight keeps lifecycle tests away from ordinary
    /// target/ground collision, so the assertion exercises the parsed Object
    /// behavior which owns the C++ timeout result.
    fn lifecycle_test_pending_projectile(
        projectile_object_name: &str,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> PendingProjectile {
        PendingProjectile {
            shooter_id: ObjectId(9_001),
            shooter_pos: Vec3::ZERO,
            source_context: Some(ProjectileLaunchContext {
                source_team: Team::China,
                source_owner_player_id: None,
                source_veterancy: crate::game_logic::VeterancyLevel::Rookie,
                source_orientation: 0.0,
                source_velocity: Vec3::ZERO,
            }),
            target_id,
            target_pos: Some(target_pos),
            damage: 25.0,
            speed: 1.0,
            splash_radius: 8.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: projectile_object_name.into(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: "FX_AuthoredLifecycleImpact".into(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: 0,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        }
    }

    #[test]
    fn retail_dumb_projectile_expiry_detonates_through_pending_host_path() {
        let _combat_serial = combat_test_guard();
        clear_pending_projectile_queue_for_test();
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        queue_projectile_direct(lifecycle_test_pending_projectile(
            "RangerFlashBangGrenade",
            None,
            Vec3::new(10_000.0, 0.0, 0.0),
        ));
        drain_pending_projectiles(&mut combat, &objects);

        let projectile = combat
            .projectiles_snapshot()
            .into_iter()
            .next()
            .expect("parsed DumbProjectile must materialize");
        assert_eq!(
            projectile.projectile_lifecycle,
            Some(
                crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::DumbProjectile {
                    max_lifespan_frames: 300,
                }
            )
        );

        for _ in 0..299 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            assert_eq!(combat.projectile_count(), 1);
            assert!(combat.take_impact_fx().is_empty());
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        assert_eq!(combat.projectile_count(), 0);
        let impacts = combat.take_impact_fx();
        assert_eq!(impacts.len(), 1);
        assert_eq!(impacts[0].detonation_fx_name, "FX_AuthoredLifecycleImpact");
        assert!(
            impacts[0].position.x < 20.0,
            "expiry must detonate at its in-flight pose rather than the distant target"
        );
    }


    #[test]
    fn retail_missile_fuel_detonation_and_target_loss_use_distinct_authored_paths() {
        let _combat_serial = combat_test_guard();
        clear_pending_projectile_queue_for_test();

        // DragonTankFlameProjectile has FuelLifetime=350ms and
        // DetonateOnNoFuel=Yes in retail WeaponObjects.ini. C++ rounds that
        // duration up to 11 logic frames and invokes its detonation weapon.
        let mut fuel_combat = CombatSystem::new();
        let mut fuel_objects = HashMap::new();
        queue_projectile_direct(lifecycle_test_pending_projectile(
            "DragonTankFlameProjectile",
            None,
            Vec3::new(10_000.0, 0.0, 0.0),
        ));
        drain_pending_projectiles(&mut fuel_combat, &fuel_objects);
        for _ in 0..10 {
            let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
            assert_eq!(fuel_combat.projectile_count(), 1);
            assert!(fuel_combat.take_impact_fx().is_empty());
        }
        let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
        assert_eq!(
            fuel_combat.projectile_count(),
            1,
            "C++ MissileAIUpdate keeps the detonated object for KillSelfDelay"
        );
        assert_eq!(fuel_combat.take_impact_fx().len(), 1);
        for _ in 0..2 {
            let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
            assert_eq!(fuel_combat.projectile_count(), 1);
            assert!(fuel_combat.take_impact_fx().is_empty());
        }
        let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
        assert_eq!(fuel_combat.projectile_count(), 0);

        // PatriotMissile follows an object and has DetonateOnNoFuel=No. Its
        // C++ airborneTargetGone transition instead enters KILL_SELF for the
        // parsed three-frame delay, producing neither a guessed explosion nor
        // impact FX when that target vanishes.
        let target = ObjectId(9_002);
        let mut target_loss_combat = CombatSystem::new();
        let mut target_loss_objects = HashMap::new();
        target_loss_objects.insert(
            target,
            make_obj(
                "LifecycleTarget",
                target,
                Team::GLA,
                Vec3::new(10_000.0, 0.0, 0.0),
                &[KindOf::Aircraft, KindOf::Attackable],
                5.0,
            ),
        );
        queue_projectile_direct(lifecycle_test_pending_projectile(
            "PatriotMissile",
            Some(target),
            Vec3::new(10_000.0, 0.0, 0.0),
        ));
        drain_pending_projectiles(&mut target_loss_combat, &target_loss_objects);
        let projectile = target_loss_combat
            .projectiles_snapshot()
            .into_iter()
            .next()
            .expect("parsed MissileAIUpdate must materialize");
        assert!(projectile.is_homing);
        target_loss_objects.remove(&target);
        for _ in 0..3 {
            let _ = target_loss_combat.update_projectiles(1.0 / 30.0, &mut target_loss_objects);
            assert_eq!(target_loss_combat.projectile_count(), 1);
            assert!(target_loss_combat.take_impact_fx().is_empty());
        }
        let _ = target_loss_combat.update_projectiles(1.0 / 30.0, &mut target_loss_objects);
        assert_eq!(target_loss_combat.projectile_count(), 0);
        assert!(target_loss_combat.take_impact_fx().is_empty());
    }

    #[test]
    fn coupled_missile_kill_self_removal_publishes_inactive_shadow_residual() {
        let _combat_serial = combat_test_guard();
        let _authority_guard = crate::gameworld_shadow::authority_env_lock();
        let prior_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
        let prior_projectile = std::env::var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", "1");
        crate::game_logic::host_projectile_log::clear();

        let mut combat = CombatSystem::new();
        let id = combat.fire_projectile(
            Vec3::ZERO,
            Vec3::new(1_000.0, 0.0, 0.0),
            &Weapon::default(),
            ObjectId(17),
            None,
            100.0,
        );
        let projectile = combat.projectile_mut(id).expect("projectile");
        projectile.set_projectile_lifecycle(Some(
            crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile {
                try_to_follow_target: false,
                fuel_lifetime_frames: 0,
                detonate_on_no_fuel: false,
                kill_self_delay_frames: 3,
            },
        ));
        projectile.missile_kill_self_started_frame = Some(0);
        projectile.lifetime = 3.0 / 30.0;

        {
            let _couple = crate::gameworld_shadow::ShadowCoupleGuard::enter();
            let mut objects = HashMap::new();
            let removed =
                combat.update_projectiles_with_countermeasures(0.0, &mut objects, None, 0);
            assert_eq!(removed, vec![id]);
        }
        let events = crate::game_logic::host_projectile_log::drain();
        assert!(
            events
                .iter()
                .any(|event| event.host_id == id.0 && !event.active),
            "actual KILL_SELF completion must retire its GameWorld flight residual: {events:?}"
        );

        match prior_shadow {
            Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        match prior_projectile {
            Some(value) => {
                crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", value)
            }
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY"),
        }
    }

    #[test]
    fn projectile_hits_intervening_structure() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(1);
        let wall = ObjectId(2);
        let tgt = ObjectId(3);
        objects.insert(
            atk,
            make_obj(
                "PrAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            wall,
            make_obj(
                "PrWall",
                wall,
                Team::Neutral,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "PrTgt",
                tgt,
                Team::GLA,
                Vec3::new(80.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 40.0,
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            40.0,
        );
        let wall_hp0 = objects.get(&wall).unwrap().health.current;
        let tgt_hp0 = objects.get(&tgt).unwrap().health.current;
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let wall_hp1 = objects.get(&wall).unwrap().health.current;
        let tgt_hp1 = objects.get(&tgt).unwrap().health.current;
        assert!(
            wall_hp1 < wall_hp0 - 1.0,
            "intervening structure must take projectile damage (wall {wall_hp0}->{wall_hp1})"
        );
        assert!(
            (tgt_hp1 - tgt_hp0).abs() < 0.01,
            "target behind wall must not be hit (tgt {tgt_hp0}->{tgt_hp1})"
        );
    }

    #[test]
    fn aa_projectile_detonates_on_intervening_building() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(21);
        let factory = ObjectId(22);
        let jet = ObjectId(23);
        objects.insert(
            atk,
            make_obj(
                "AaAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Structure, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            factory,
            make_obj(
                "AaFactory",
                factory,
                Team::Neutral,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            jet,
            make_obj(
                "AaJet",
                jet,
                Team::GLA,
                Vec3::new(80.0, 20.0, 0.0),
                &[KindOf::Aircraft, KindOf::Attackable],
                5.0,
            ),
        );
        objects.get_mut(&jet).unwrap().status.airborne_target = true;
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 40.0,
            can_target_air: true,
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 20.0, 0.0),
            &w,
            atk,
            Some(jet),
            40.0,
        );
        let factory_hp0 = objects.get(&factory).unwrap().health.current;
        let jet_hp0 = objects.get(&jet).unwrap().health.current;
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let factory_hp1 = objects.get(&factory).unwrap().health.current;
        let jet_hp1 = objects.get(&jet).unwrap().health.current;
        assert!(
            factory_hp1 < factory_hp0 - 1.0,
            "AA shot must detonate on intervening building ({factory_hp0}->{factory_hp1})"
        );
        assert!(
            (jet_hp1 - jet_hp0).abs() < 0.01,
            "jet behind factory must not be hit ({jet_hp0}->{jet_hp1})"
        );
    }

    #[test]
    fn projectile_structure_intercept_cpp_surface() {
        let _combat_serial = combat_test_guard();
        let src = concat!(include_str!("projectile.rs"), include_str!("resolution.rs"));
        assert!(
            src.contains("Intervening structure residual")
                && src.contains("KindOf::Structure")
                && src.contains("CONTROLLED_STRUCTURES"),
            "update_projectiles must gate own structures on CONTROLLED_STRUCTURES"
        );
    }

    #[test]
    fn projectile_skips_own_structure_unless_controlled_bit() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(91);
        let factory = ObjectId(92);
        let tgt = ObjectId(93);
        objects.insert(
            atk,
            make_obj(
                "OwnAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            factory,
            make_obj(
                "OwnFactory",
                factory,
                Team::USA,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "OwnTgt",
                tgt,
                Team::GLA,
                Vec3::new(80.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.get_mut(&atk).unwrap().owner_player_id = Some(1);
        objects.get_mut(&factory).unwrap().owner_player_id = Some(1);
        objects.get_mut(&tgt).unwrap().owner_player_id = Some(2);

        let factory0 = objects.get(&factory).unwrap().health.current;
        let tgt0 = objects.get(&tgt).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 40.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            40.0,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.projectile_collides = crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT;
            p.source_owner_player_id = Some(1);
            p.source_team = Team::USA;
        }
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let factory1 = objects.get(&factory).unwrap().health.current;
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        assert_eq!(
            factory1, factory0,
            "own factory must not intercept without CONTROLLED_STRUCTURES"
        );
        assert!(
            tgt1 < tgt0 - 1.0,
            "shot must pass through own factory to the target"
        );

        objects.get_mut(&factory).unwrap().health.current = factory0;
        objects.get_mut(&tgt).unwrap().health.current = tgt0;
        let pid = combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            40.0,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.projectile_collides =
                crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES;
            p.source_owner_player_id = Some(1);
            p.source_team = Team::USA;
        }
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let factory2 = objects.get(&factory).unwrap().health.current;
        let tgt2 = objects.get(&tgt).unwrap().health.current;
        assert!(
            factory2 < factory0 - 1.0,
            "CONTROLLED_STRUCTURES must intercept own factory"
        );
        assert!(
            (tgt2 - tgt0).abs() < 0.01,
            "target behind own factory must not be hit when collide-controlled"
        );
    }

    #[test]
    fn projectile_reaches_target_without_wall() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(10);
        let tgt = ObjectId(11);
        objects.insert(
            atk,
            make_obj(
                "PrAtk2",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "PrTgt2",
                tgt,
                Team::GLA,
                Vec3::new(30.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 200.0,
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(30.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            200.0,
        );
        let tgt_hp0 = objects.get(&tgt).unwrap().health.current;
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let tgt_hp1 = objects.get(&tgt).unwrap().health.current;
        assert!(
            tgt_hp1 < tgt_hp0 - 1.0,
            "open-field projectile must still hit target ({tgt_hp0}->{tgt_hp1})"
        );
    }

    #[test]
    fn projectile_splash_damages_nearby() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(20);
        let tgt = ObjectId(21);
        let near = ObjectId(22);
        objects.insert(
            atk,
            make_obj(
                "SpAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "SpTgt",
                tgt,
                Team::GLA,
                Vec3::new(20.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            near,
            make_obj(
                "SpNear",
                near,
                Team::GLA,
                Vec3::new(25.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            range: 200.0,
            projectile_speed: 500.0,
            splash_radius: 15.0,
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(20.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            500.0,
        );
        let tgt0 = objects.get(&tgt).unwrap().health.current;
        let near0 = objects.get(&near).unwrap().health.current;
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        let near1 = objects.get(&near).unwrap().health.current;
        assert!(tgt1 < tgt0 - 1.0, "splash center must damage target");
        assert!(
            near1 < near0 - 1.0,
            "nearby unit within splash_radius must take area damage ({near0}->{near1})"
        );
    }

    /// C++ Weapon.cpp:1438 dealDamageInternal: amount is primaryDamage inside
    /// primaryRadius and secondaryDamage outside it. No quadratic falloff.
    #[test]
    fn projectile_splash_is_flat_primary_not_quadratic() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(70);
        let tgt = ObjectId(71);
        let edge = ObjectId(72);
        objects.insert(
            atk,
            make_obj(
                "StepAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "StepTgt",
                tgt,
                Team::GLA,
                Vec3::new(20.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            edge,
            make_obj(
                "StepEdge",
                edge,
                Team::GLA,
                Vec3::new(24.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            range: 200.0,
            projectile_speed: 500.0,
            splash_radius: 10.0,
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(20.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            500.0,
        );
        let tgt0 = objects.get(&tgt).unwrap().health.current;
        let edge0 = objects.get(&edge).unwrap().health.current;
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        let edge1 = objects.get(&edge).unwrap().health.current;
        let tgt_lost = tgt0 - tgt1;
        let edge_lost = edge0 - edge1;
        assert!(
            (tgt_lost - 50.0).abs() < 0.1,
            "center of primary ring takes full PrimaryDamage, lost={tgt_lost}"
        );
        assert!(
            (edge_lost - 50.0).abs() < 0.1,
            "edge of primary ring still takes full PrimaryDamage (Weapon.cpp:1438), lost={edge_lost}"
        );
        assert_eq!(
            objects.get(&tgt).unwrap().last_damage_source,
            Some(atk),
            "splash must stamp launcher as last_damage_source"
        );
    }

    #[test]
    fn instant_hit_laser_damages_same_frame() {
        let _combat_serial = combat_test_guard();
        let mut objects = HashMap::new();
        let atk = ObjectId(30);
        let tgt = ObjectId(31);
        objects.insert(
            atk,
            make_obj(
                "LasAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "LasTgt",
                tgt,
                Team::GLA,
                Vec3::new(50.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 0.0, // instant residual
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(50.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            0.0,
        );
        let hp0 = objects.get(&tgt).unwrap().health.current;
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let hp1 = objects.get(&tgt).unwrap().health.current;
        assert!(
            hp1 < hp0 - 1.0,
            "instant laser must damage on first projectile step ({hp0}->{hp1})"
        );
        assert_eq!(
            combat.projectile_count(),
            0,
            "instant projectile should resolve and clear"
        );
    }

    #[test]
    fn homing_projectile_tracks_moving_target() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(40);
        let tgt = ObjectId(41);
        objects.insert(
            atk,
            make_obj(
                "HomAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
                &[KindOf::Vehicle, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "HomTgt",
                tgt,
                Team::GLA,
                Vec3::new(30.0, 0.0, 0.0),
                &[KindOf::Aircraft, KindOf::Attackable],
                5.0,
            ),
        );
        objects.get_mut(&tgt).unwrap().status.airborne_target = true;
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 30.0,
            range: 200.0,
            projectile_speed: 80.0,
            can_target_air: true,
            can_target_ground: false,
            ..Weapon::default()
        };
        // Aim at stale point (origin line); target will drift +Z so ballistic would miss.
        combat.fire_projectile_ex(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(30.0, 0.0, 0.0),
            &w,
            atk,
            Some(tgt),
            80.0,
            true,
        );
        assert!(
            combat
                .get_projectiles()
                .values()
                .next()
                .map(|p| p.is_homing)
                .unwrap_or(false),
            "projectile must be marked homing"
        );
        // Drift target off the initial aim line.
        for step in 0..120 {
            if let Some(o) = objects.get_mut(&tgt) {
                // Move +Z so a non-homing shot at (30,0,0) would miss.
                o.set_position(Vec3::new(30.0, 0.0, (step as f32) * 0.35));
            }
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let hp = objects.get(&tgt).unwrap().health.current;
        assert!(
            hp < 200.0 - 1.0,
            "homing missile must hit target that drifted off aim line (hp={hp})"
        );
    }

    #[test]
    fn instant_and_homing_cpp_surface() {
        let _combat_serial = combat_test_guard();
        let src = concat!(include_str!("projectile.rs"), include_str!("resolution.rs"));
        assert!(src.contains("is_instant_speed"));
        assert!(src.contains("fire_projectile_ex"));
        assert!(src.contains("is_homing"));
        assert!(
            src.contains("Instant residual") || src.contains("instant-hit"),
            "must document instant laser residual"
        );
    }

    #[test]
    fn projectile_impact_queues_detonation_fx() {
        let _combat_serial = combat_test_guard();
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        let shooter = ObjectId(1);
        let target = ObjectId(2);
        let mut t = Object::new_simple(
            target,
            crate::game_logic::ObjectType::Infantry,
            "GLARebel".to_string(),
        );
        t.set_position(Vec3::new(5.0, 0.0, 0.0));
        objects.insert(target, t);

        // Instant residual: same-frame impact (ProjectileDetonationFX at hit).
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &Weapon {
                damage: 10.0,
                range: 100.0,
                min_range: 0.0,
                reload_time: 1.0,
                last_fire_time: 0.0,
                ammo: None,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                suspend_fx_frame: 0,
                reloading_clip: false,
                last_bonus_rof: 0.0,
            },
            shooter,
            Some(target),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.detonation_fx_name = "FX_GenericTankShellDetonation".into();
            p.detonation_ocl_name = "OCL_FireFieldSmall".into();
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let fx = combat.take_impact_fx();
        assert_eq!(fx.len(), 1, "impact must queue detonation fx");
        assert_eq!(fx[0].detonation_fx_name, "FX_GenericTankShellDetonation");
        assert_eq!(fx[0].detonation_ocl_name, "OCL_FireFieldSmall");
        assert_eq!(fx[0].target_id, Some(target));
    }

    #[test]
    fn garrison_hit_kill_fx_uses_do_fx_obj_not_detonation_pos() {
        let _combat_serial = combat_test_guard();
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        let shooter = ObjectId(1);
        let building = ObjectId(2);
        let occupant = ObjectId(3);

        let mut rider = Object::new_simple(
            occupant,
            crate::game_logic::ObjectType::Infantry,
            "GLARebel".to_string(),
        );
        rider.set_position(Vec3::new(5.0, 0.0, 0.0));
        objects.insert(occupant, rider);

        let mut bunker = Object::new_simple(
            building,
            crate::game_logic::ObjectType::Building,
            "CivilianBuilding".to_string(),
        );
        bunker.set_position(Vec3::new(5.0, 0.0, 0.0));
        bunker.thing.template.contain_module.kind = crate::game_logic::ContainModuleKind::Garrison;
        bunker.thing.template.garrison_contain_max = Some(5);
        let mut bd = crate::game_logic::buildings::BuildingData::new(
            crate::game_logic::buildings::BuildingType::Bunker,
        );
        bd.garrisoned_units = vec![occupant];
        bunker.building_data = Some(bd);
        objects.insert(building, bunker);

        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &Weapon {
                damage: 10.0,
                range: 100.0,
                min_range: 0.0,
                reload_time: 1.0,
                last_fire_time: 0.0,
                ammo: None,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                suspend_fx_frame: 0,
                reloading_clip: false,
                last_bonus_rof: 0.0,
            },
            shooter,
            Some(building),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.flight = Some(
                crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(
                    crate::game_logic::weapon_bootstrap::HostDumbProjectileFlight {
                        garrison_hit_kill_count: 1,
                        garrison_hit_kill_fx: "FX_FlashBang".into(),
                        ..Default::default()
                    },
                ),
            );
            p.detonation_fx_name = "FX_ShouldNotPlay".into();
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        assert_eq!(
            combat.projectile_count(),
            0,
            "garrison clear must consume the projectile without detonation"
        );
        let fx = combat.take_impact_fx();
        assert!(
            fx.is_empty(),
            "GarrisonHitKillFX must not queue Weapon.cpp detonation pos form, got {fx:?}"
        );
        let rider = objects.get(&occupant).expect("occupant");
        assert!(
            !rider.is_alive(),
            "GarrisonHitKillCount must still kill the occupant"
        );
        assert!(
            objects
                .get(&building)
                .expect("building")
                .contained_units()
                .is_empty()
        );
    }

    #[test]
    fn missile_calls_on_die_fires_missile_fx_not_victim_death() {
        let _combat_serial = combat_test_guard();
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        let shooter = ObjectId(1);
        let target = ObjectId(2);
        let mut t = Object::new_simple(
            target,
            crate::game_logic::ObjectType::Infantry,
            "GLARebel".to_string(),
        );
        t.set_position(Vec3::new(5.0, 0.0, 0.0));
        t.health.current = 1.0;
        t.health.maximum = 1.0;
        objects.insert(target, t);

        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &Weapon {
                damage: 50.0,
                range: 100.0,
                min_range: 0.0,
                reload_time: 1.0,
                last_fire_time: 0.0,
                ammo: None,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                suspend_fx_frame: 0,
                reloading_clip: false,
                last_bonus_rof: 0.0,
            },
            shooter,
            Some(target),
            0.0,
            false,
        );
        crate::game_logic::host_fx_list_die::set_test_fx_list_die(
            "ScudStormMissile",
            crate::game_logic::host_fx_list_die::HostFxListDieData::with_fx(
                "FX_AuthoredMissileDie",
            ),
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Exploded;
            p.die_on_detonate = true;
            p.projectile_object_name = "ScudStormMissile".into();
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        crate::game_logic::host_fx_list_die::clear_test_fx_list_die();
        let victim = objects.get(&target).expect("victim");
        assert_eq!(
            victim.status.death_type,
            crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
            "MissileCallsOnDie must not remap victim DeathType"
        );
        let fx = combat.take_impact_fx();
        assert!(
            fx.iter()
                .any(|ev| ev.detonation_fx_name == "FX_AuthoredMissileDie"),
            "authored missile FXListDie must fire on die_on_detonate, got {fx:?}"
        );
        assert!(
            fx.iter()
                .all(|ev| ev.detonation_fx_name != "FX_ScudMissileDie"),
            "must not invent FX_ScudMissileDie, got {fx:?}"
        );
    }

    #[test]
    fn pending_projectile_preserves_exhaust_and_frozen_fire_effect_context() {
        let _combat_serial = combat_test_guard();
        let mut combat = CombatSystem::new();
        let objects = HashMap::new();
        queue_projectile(PendingProjectile {
            shooter_id: ObjectId(1),
            shooter_pos: Vec3::ZERO,
            source_context: Some(ProjectileLaunchContext {
                source_team: Team::China,
                source_owner_player_id: None,
                source_veterancy: crate::game_logic::VeterancyLevel::Heroic,
                source_orientation: std::f32::consts::FRAC_PI_2,
                source_velocity: Vec3::new(12.0, 0.0, -4.0),
            }),
            target_id: Some(ObjectId(2)),
            target_pos: Some(Vec3::new(10.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 100.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: "GenericTankShell".into(),
            projectile_lifecycle: None,
            fire_fx_name: "FX_HeroicGenericTankGunNoTracer".into(),
            fire_ocl_name: "OCL_FireFieldSmall".into(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: "MissileExhaust".into(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        // Need a dummy target for drain to resolve? target_pos is Some so OK.
        drain_pending_projectiles(&mut combat, &objects);
        let snaps: Vec<_> = combat.projectiles_snapshot();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].exhaust_name, "MissileExhaust");
        assert_eq!(snaps[0].source_team, Team::China);
        assert_eq!(
            snaps[0].source_veterancy,
            crate::game_logic::VeterancyLevel::Heroic
        );

        // There is intentionally no source object in `objects`: the queued
        // FireOCL must retain the fire-time transform rather than sampling a
        // deleted/re-owned object during the later combat drain.
        let fire_ocls = combat.take_fire_ocl();
        assert_eq!(fire_ocls.len(), 1);
        assert_eq!(fire_ocls[0].fire_fx_name, "FX_HeroicGenericTankGunNoTracer");
        assert_eq!(fire_ocls[0].fire_ocl_name, "OCL_FireFieldSmall");
        assert_eq!(fire_ocls[0].source_team, Team::China);
        assert_eq!(
            fire_ocls[0].source_veterancy,
            crate::game_logic::VeterancyLevel::Heroic
        );
        assert!((fire_ocls[0].source_orientation - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(fire_ocls[0].source_velocity, Vec3::new(12.0, 0.0, -4.0));
    }

    #[test]
    fn dual_ring_secondary_damage_residual() {
        let _combat_serial = combat_test_guard();
        let mut objects = HashMap::new();
        let atk = ObjectId(40);
        let near = ObjectId(41);
        let far = ObjectId(42);
        objects.insert(
            near,
            Object::new_simple(
                near,
                crate::game_logic::ObjectType::Infantry,
                "GLARebel".into(),
            ),
        );
        objects.insert(
            far,
            Object::new_simple(
                far,
                crate::game_logic::ObjectType::Infantry,
                "GLARebel".into(),
            ),
        );
        objects
            .get_mut(&near)
            .unwrap()
            .set_position(Vec3::new(5.0, 0.0, 0.0));
        objects
            .get_mut(&far)
            .unwrap()
            .set_position(Vec3::new(18.0, 0.0, 0.0));
        // Pin pick-radius so this case tests ring amounts, not hull slack.
        objects.get_mut(&near).unwrap().selection_radius = 0.0;
        objects.get_mut(&far).unwrap().selection_radius = 0.0;
        let near0 = objects.get(&near).unwrap().health.current;
        let far0 = objects.get(&far).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 100.0,
            splash_radius: 10.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &w,
            atk,
            Some(near),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.secondary_damage = 25.0;
            p.secondary_damage_radius = 25.0;
            // Primary ring uses explosion_radius from splash_radius.
            p.explosion_radius = 10.0;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let near1 = objects.get(&near).unwrap().health.current;
        let far1 = objects.get(&far).unwrap().health.current;
        assert!(
            near1 <= near0 - 99.0,
            "inner ring must take primary damage ({near0}->{near1})"
        );
        assert!(
            far1 <= far0 - 24.0 && far1 > far0 - 99.0,
            "outer ring must take secondary only ({far0}->{far1})"
        );
    }

    #[test]
    fn shock_wave_pushes_mobile_units_outward() {
        let _combat_serial = combat_test_guard();
        let mut objects = HashMap::new();
        let atk = ObjectId(50);
        let tgt = ObjectId(51);
        let mut unit = make_obj(
            "GLARebel",
            tgt,
            Team::GLA,
            Vec3::new(5.0, 0.0, 0.0),
            &[KindOf::Infantry, KindOf::Attackable],
            5.0,
        );
        objects.insert(tgt, unit);

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 5.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &w,
            atk,
            Some(tgt),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.shock_wave_amount = 50.0;
            p.shock_wave_radius = 30.0;
            p.shock_wave_taper_off = 0.5;
        }
        let pos0 = objects.get(&tgt).unwrap().get_position();
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let victim = objects.get(&tgt).unwrap();
        assert!(
            victim.is_shock_stunned(),
            "C++ attemptDamage must stun / STUNNED_FLAILING"
        );
        assert!(
            victim.movement.velocity.length() > 0.0,
            "shockwave must toss via apply_shock_wave_impulse"
        );
        assert!(
            (victim.get_position() - pos0).length() < 0.01,
            "detonation frame must not nudge position ({pos0:?} -> {:?})",
            victim.get_position()
        );
    }

    #[test]
    fn radius_damage_affects_skips_allies_by_default() {
        let _combat_serial = combat_test_guard();
        let mut objects = HashMap::new();
        let atk = ObjectId(60);
        let ally = ObjectId(61);
        let enemy = ObjectId(62);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ally,
            make_obj(
                "USA_Ranger",
                ally,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            enemy,
            make_obj(
                "GLARebel",
                enemy,
                Team::GLA,
                Vec3::new(4.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let ally0 = objects.get(&ally).unwrap().health.current;
        let enemy0 = objects.get(&enemy).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            &w,
            atk,
            Some(enemy),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ally1 = objects.get(&ally).unwrap().health.current;
        let enemy1 = objects.get(&enemy).unwrap().health.current;
        assert_eq!(ally1, ally0, "ENEMIES|NEUTRALS INI must skip allies");
        assert!(enemy1 < enemy0 - 1.0, "enemies must take splash");
    }

    #[test]
    fn radius_damage_affects_cpp_default_hits_allies() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(70);
        let ally = ObjectId(71);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ally,
            make_obj(
                "USA_Ranger",
                ally,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let ally0 = objects.get(&ally).unwrap().health.current;
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            &w,
            atk,
            Some(ally),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects = crate::game_logic::weapon_bootstrap::WEAPON_AFFECTS_DEFAULT;
            p.target_id = None; // splash neighbor, not primary victim
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ally1 = objects.get(&ally).unwrap().health.current;
        assert!(
            ally1 < ally0 - 1.0,
            "C++ default ALLIES|ENEMIES|NEUTRALS must friendly-fire"
        );
    }

    #[test]
    fn radius_damage_affects_not_airborne_skips_significantly_above() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(72);
        let ground = ObjectId(73);
        let high = ObjectId(74);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ground,
            make_obj(
                "GLARebel",
                ground,
                Team::GLA,
                Vec3::new(5.0, 0.0, 0.0),
                &[KindOf::Vehicle, KindOf::Attackable],
                5.0,
            ),
        );
        let mut flyer = make_obj(
            "GLAQuad",
            high,
            Team::GLA,
            Vec3::new(5.0, 12.0, 0.0),
            &[KindOf::Vehicle, KindOf::Attackable],
            5.0,
        );
        flyer.ground_height = 0.0;
        flyer.selection_radius = 0.0;
        flyer.thing.template.geometry_info.authored = true;
        flyer.thing.template.geometry_info.major_radius = 0.0;
        flyer.thing.template.geometry_info.geom_type = crate::game_logic::HostGeometryType::Sphere;
        flyer.thing.template.geometry_info.height = 0.0;
        objects.insert(high, flyer);
        let ground0 = objects.get(&ground).unwrap().health.current;
        let high0 = objects.get(&high).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &w,
            atk,
            Some(ground),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_DOESNT_AFFECT_AIRBORNE;
            p.target_id = None;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ground1 = objects.get(&ground).unwrap().health.current;
        let high1 = objects.get(&high).unwrap().health.current;
        assert!(
            ground1 < ground0 - 1.0,
            "grounded enemy must take splash ({ground0}->{ground1})"
        );
        assert_eq!(
            high1, high0,
            "NOT_AIRBORNE must use isSignificantlyAboveTerrain, not KindOf::Aircraft"
        );
    }

    #[test]
    fn splash_uses_from_bounding_sphere_3d() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(80);
        let building = ObjectId(81);
        let mut bldg = make_obj(
            "ChinaWarFactory",
            building,
            Team::GLA,
            Vec3::new(15.0, 0.0, 0.0),
            &[KindOf::Structure, KindOf::Attackable],
            1.0,
        );
        bldg.thing.template.geometry_info.authored = true;
        bldg.thing.template.geometry_info.geom_type = crate::game_logic::HostGeometryType::Sphere;
        bldg.thing.template.geometry_info.major_radius = 8.0;
        objects.insert(building, bldg);

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            splash_radius: 10.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(15.0, 0.0, 0.0),
            &w,
            atk,
            Some(building),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 10.0;
        }
        let hp0 = objects.get(&building).unwrap().health.current;
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let hp1 = objects.get(&building).unwrap().health.current;
        assert!(
            hp1 <= hp0 - 49.0,
            "hull inside ring must take splash ({hp0}->{hp1})"
        );
    }

    #[test]
    fn splash_primary_victim_skips_radius_damage_affects() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(90);
        let ally = ObjectId(91);
        let bystander = ObjectId(92);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ally,
            make_obj(
                "USA_Ranger",
                ally,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            bystander,
            make_obj(
                "USA_MissileDefender",
                bystander,
                Team::USA,
                Vec3::new(4.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let ally0 = objects.get(&ally).unwrap().health.current;
        let by0 = objects.get(&bystander).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            &w,
            atk,
            Some(ally),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ally1 = objects.get(&ally).unwrap().health.current;
        let by1 = objects.get(&bystander).unwrap().health.current;
        assert!(
            ally1 < ally0 - 1.0,
            "intended ally must take splash ({ally0}->{ally1})"
        );
        assert_eq!(by1, by0, "bystander ally must still be skipped");
    }

    #[test]
    fn splash_damage_dealt_at_self_position_recenters_and_clears_victim() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        const NAME: &str = "HuntLiveDamageAtSelfSplash";
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            template.damage_dealt_at_self_position = true;
            store.add_weapon_template(template);
        });

        let mut objects = HashMap::new();
        let atk = ObjectId(190);
        let near = ObjectId(191);
        let far = ObjectId(192);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            near,
            make_obj(
                "GLA_Rebel",
                near,
                Team::GLA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            far,
            make_obj(
                "GLA_Rebel",
                far,
                Team::GLA,
                Vec3::new(80.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let near0 = objects.get(&near).unwrap().health.current;
        let far0 = objects.get(&far).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(80.0, 0.0, 0.0),
            &w,
            atk,
            Some(far),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.historic_weapon_key = NAME.to_string();
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let near1 = objects.get(&near).unwrap().health.current;
        let far1 = objects.get(&far).unwrap().health.current;
        assert!(
            near1 < near0 - 1.0,
            "splash must recenter on shooter ({near0}->{near1})"
        );
        assert_eq!(
            far1, far0,
            "cleared victim at impact must not take splash ({far0}->{far1})"
        );
    }

    #[test]
    fn splash_kills_self_deals_huge_damage() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(100);
        let enemy = ObjectId(101);
        objects.insert(
            atk,
            make_obj(
                "GLADemoTruck",
                atk,
                Team::GLA,
                Vec3::ZERO,
                &[KindOf::Vehicle, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            enemy,
            make_obj(
                "USA_Ranger",
                enemy,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 10.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            &w,
            atk,
            Some(enemy),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        assert!(
            !objects.get(&atk).unwrap().is_alive(),
            "WEAPON_KILLS_SELF must destroy the shooter"
        );
    }

    #[test]
    fn projectile_collides_mask_gates_structure_intercept() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(70);
        let wall = ObjectId(71);
        let tgt = ObjectId(72);
        objects.insert(
            atk,
            make_obj(
                "Atk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            wall,
            make_obj(
                "Wall",
                wall,
                Team::Neutral,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "Tgt",
                tgt,
                Team::GLA,
                Vec3::new(80.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let wall0 = objects.get(&wall).unwrap().health.current;
        let tgt0 = objects.get(&tgt).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            projectile_speed: 500.0,
            ..Weapon::default()
        };
        // No structure collide residual (laser-like).
        let pid = combat.fire_projectile_ex(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            500.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.projectile_collides = 0;
        }
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let wall1 = objects.get(&wall).unwrap().health.current;
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        assert_eq!(wall1, wall0, "mask=0 must not intercept structure");
        assert!(
            tgt1 < tgt0 - 1.0,
            "projectile must reach target when collides mask empty"
        );
    }

    #[test]
    fn scatter_radius_offsets_aim_and_clears_target() {
        let _combat_serial = combat_test_guard();
        let mut objects = HashMap::new();
        let atk = ObjectId(80);
        let tgt = ObjectId(81);
        objects.insert(
            tgt,
            make_obj(
                "GLARebel",
                tgt,
                Team::GLA,
                Vec3::new(50.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        queue_projectile(PendingProjectile {
            shooter_id: atk,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: Some(tgt),
            target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 200.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: "GenericTankShell".into(),

            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 10.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        drain_pending_projectiles(&mut combat, &objects);
        let snaps: Vec<_> = combat.projectiles_snapshot();
        assert_eq!(snaps.len(), 1);
        // Target cleared when scatter applied.
        assert!(snaps[0].target_id.is_none());
        // Aim point moved off exact target.
        let aim = snaps[0].target_position;
        let d = (aim - Vec3::new(50.0, 0.0, 0.0)).length();
        assert!(d > 0.01 && d <= 10.0 + 1e-2, "scatter offset length {d}");
    }

    #[test]
    fn scale_weapon_speed_slows_close_shots() {
        let _combat_serial = combat_test_guard();
        let mut objects = HashMap::new();
        let atk = ObjectId(90);
        let tgt = ObjectId(91);
        // Place target at firebase min range (50).
        objects.insert(
            tgt,
            make_obj(
                "GLATunnelNetwork",
                tgt,
                Team::GLA,
                Vec3::new(50.0, 0.0, 0.0),
                &[KindOf::Structure, KindOf::Attackable],
                20.0,
            ),
        );
        let mut combat = CombatSystem::new();
        queue_projectile(PendingProjectile {
            shooter_id: atk,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: Some(tgt),
            target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
            damage: 50.0,
            speed: 300.0,
            splash_radius: 10.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: "GenericTankShell".into(),

            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 375.0,
            min_attack_range: 50.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        drain_pending_projectiles(&mut combat, &objects);
        let snaps: Vec<_> = combat.projectiles_snapshot();
        assert_eq!(snaps.len(), 1);
        assert!(
            (snaps[0].speed - 75.0).abs() < 1e-2,
            "close lob speed {}, want ~75",
            snaps[0].speed
        );

        // Far shot at max range → full speed.
        let mut combat2 = CombatSystem::new();
        queue_projectile(PendingProjectile {
            shooter_id: atk,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: None,
            target_pos: Some(Vec3::new(375.0, 0.0, 0.0)),
            damage: 50.0,
            speed: 300.0,
            splash_radius: 10.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: "GenericTankShell".into(),

            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 375.0,
            min_attack_range: 50.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        drain_pending_projectiles(&mut combat2, &objects);
        let snaps2: Vec<_> = combat2.projectiles_snapshot();
        assert_eq!(snaps2.len(), 1);
        assert!(
            (snaps2[0].speed - 300.0).abs() < 1e-2,
            "far lob speed {}, want ~300",
            snaps2[0].speed
        );
    }

    fn projectileless_pending(
        speed: f32,
        target_pos: Vec3,
        historic_weapon_key: &str,
    ) -> PendingProjectile {
        PendingProjectile {
            shooter_id: ObjectId(501),
            shooter_pos: Vec3::ZERO,
            source_context: Some(ProjectileLaunchContext {
                source_team: Team::USA,
                source_owner_player_id: Some(1),
                source_veterancy: crate::game_logic::VeterancyLevel::Rookie,
                source_orientation: 0.0,
                source_velocity: Vec3::ZERO,
            }),
            target_id: Some(ObjectId(502)),
            target_pos: Some(target_pos),
            damage: 40.0,
            speed,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 200.0,
            min_attack_range: 0.0,
            historic_weapon_key: historic_weapon_key.into(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        }
    }

    #[test]
    fn projectileless_finite_speed_queues_leftover_delayed_damage() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        clear_pending_projectile_queue_for_test();
        clear_live_projectileless_delayed_for_test();
        crate::game_logic::host_historic_bonus::set_logic_frame(20);
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        const NAME: &str = "Hq0c9b4CombatRifleDelayed";
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            template.weapon_speed = 10.0;
            template.projectile_name.clear();
            template.primary_damage = 40.0;
            store.add_weapon_template(template);
        });
        let leftover_before = leftover_delayed_damage_count_for_test();

        let mut objects = HashMap::new();
        objects.insert(
            ObjectId(502),
            make_obj(
                "GLARebel",
                ObjectId(502),
                Team::GLA,
                Vec3::new(100.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let hp0 = objects.get(&ObjectId(502)).unwrap().health.current;
        let mut combat = CombatSystem::new();
        queue_projectile_direct(projectileless_pending(
            10.0,
            Vec3::new(100.0, 0.0, 0.0),
            NAME,
        ));
        drain_pending_projectiles(&mut combat, &objects);
        assert_eq!(
            combat.projectile_count(),
            0,
            "projectileless fire must not spawn a dummy CombatSystem projectile"
        );
        assert!(
            leftover_delayed_damage_count_for_test() > leftover_before,
            "leftover WeaponStore setDelayedDamage must be queued"
        );
        assert_eq!(live_projectileless_delayed_count_for_test(), 1);

        apply_ready_projectileless_delayed_damage(&mut combat, &mut objects, 20, None);
        let hp_mid = objects.get(&ObjectId(502)).unwrap().health.current;
        assert_eq!(hp_mid, hp0, "damage waits travel frames");

        apply_ready_projectileless_delayed_damage(&mut combat, &mut objects, 30, None);
        let hp1 = objects.get(&ObjectId(502)).unwrap().health.current;
        assert!(
            hp1 < hp0 - 1.0,
            "leftover delayed damage must apply on the live host ({hp0}->{hp1})"
        );
        assert_eq!(live_projectileless_delayed_count_for_test(), 0);
        clear_pending_projectile_queue_for_test();
        clear_live_projectileless_delayed_for_test();
    }

    #[test]
    fn projectileless_subframe_delay_applies_same_frame() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        clear_pending_projectile_queue_for_test();
        clear_live_projectileless_delayed_for_test();
        crate::game_logic::host_historic_bonus::set_logic_frame(7);
        let leftover_before = leftover_delayed_damage_count_for_test();

        let mut objects = HashMap::new();
        objects.insert(
            ObjectId(502),
            make_obj(
                "GLARebel",
                ObjectId(502),
                Team::GLA,
                Vec3::new(5.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let hp0 = objects.get(&ObjectId(502)).unwrap().health.current;
        let mut combat = CombatSystem::new();
        queue_projectile_direct(projectileless_pending(
            10.0,
            Vec3::new(5.0, 0.0, 0.0),
            "Hq0c9b4CombatRifleNow",
        ));
        drain_pending_projectiles(&mut combat, &objects);
        assert_eq!(combat.projectile_count(), 0);
        assert_eq!(
            leftover_delayed_damage_count_for_test(),
            leftover_before,
            "sub-frame travel must dealDamageInternal now, not setDelayedDamage"
        );
        apply_ready_projectileless_delayed_damage(&mut combat, &mut objects, 7, None);
        let hp1 = objects.get(&ObjectId(502)).unwrap().health.current;
        assert!(hp1 < hp0 - 1.0, "sub-frame projectileless must apply now");
        clear_pending_projectile_queue_for_test();
        clear_live_projectileless_delayed_for_test();
    }

    #[test]
    fn fire_at_projectileless_weapon_skips_dummy_projectile() {
        let _combat_serial = combat_test_guard();
        ensure_unit_test_direct_damage();
        clear_pending_projectile_queue_for_test();
        crate::game_logic::host_historic_bonus::set_logic_frame(3);
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        const NAME: &str = "Hq0c9b4CombatRifleFireAt";
        let leftover_before = leftover_delayed_damage_count_for_test();
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            template.weapon_speed = 10.0;
            template.projectile_name.clear();
            template.primary_damage = 15.0;
            store.add_weapon_template(template);
        });

        let mut tmpl = ThingTemplate::new("Hq0c9b4Shooter");
        tmpl.set_primary_weapon_name(NAME);
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        // C++ fireWeaponTemplate always fires at a live victim whose position
        // feeds the travel vector (Weapon.cpp:998-1003).  The Object-only
        // fire path resolves that stand-in from prev_victim_pos.
        atk.set_position(Vec3::ZERO);
        atk.prev_victim_pos = Some(Vec3::new(100.0, 0.0, 0.0));
        atk.weapon = Some(Weapon {
            damage: 15.0,
            range: 200.0,
            projectile_speed: 10.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        assert!(atk.fire_at(ObjectId(2), 1.0));
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        objects.insert(
            ObjectId(2),
            make_obj(
                "GLARebel",
                ObjectId(2),
                Team::GLA,
                Vec3::new(100.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        drain_pending_projectiles(&mut combat, &objects);

        assert_eq!(combat.projectile_count(), 0);
        assert!(leftover_delayed_damage_count_for_test() > leftover_before);
        clear_pending_projectile_queue_for_test();
        clear_live_projectileless_delayed_for_test();
    }

    #[test]
    fn missile_ai_ignition_fx_plays_on_delay_zero_and_after_delay() {
        let _combat_serial = combat_test_guard();
        use crate::game_logic::weapon_bootstrap::{
            HostMissileFlight, HostMissilePhase, HostProjectileFlight,
        };

        let mut delay0 = Projectile::new(
            ObjectId(100_001),
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            10.0,
            DamageType::Explosive,
            ObjectId(1),
            None,
        );
        delay0.speed = 100.0;
        delay0.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
            ignition_delay_frames: 0,
            ignition_fx: "FX_MissileIgnition".into(),
            ..HostMissileFlight::default()
        }));
        delay0.bind_authored_flight(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), 100.0);
        assert!(
            !delay0.is_warhead_armed(),
            "C++ delay 0 still starts LAUNCH so IgnitionFX can play"
        );
        assert_eq!(
            delay0.flight_runtime.missile_phase,
            HostMissilePhase::Launch
        );
        let _ = delay0.update(1.0 / 30.0, true);
        assert!(delay0.is_warhead_armed());
        assert_eq!(
            delay0.flight_runtime.missile_phase,
            HostMissilePhase::Attack
        );

        let mut delayed = Projectile::new(
            ObjectId(100_002),
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            10.0,
            DamageType::Explosive,
            ObjectId(1),
            None,
        );
        delayed.speed = 100.0;
        delayed.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
            ignition_delay_frames: 2,
            ignition_fx: "FX_MissileIgnition".into(),
            ..HostMissileFlight::default()
        }));
        delayed.bind_authored_flight(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), 100.0);
        assert!(!delayed.is_warhead_armed());
        let _ = delayed.update(1.0 / 30.0, true);
        assert!(
            !delayed.is_warhead_armed(),
            "frame 1 must stay in LAUNCH when IgnitionDelay is 2"
        );
        let _ = delayed.update(1.0 / 30.0, true);
        assert!(delayed.is_warhead_armed());
        assert_eq!(
            delayed.flight_runtime.missile_phase,
            HostMissilePhase::Attack
        );

        let src = concat!(include_str!("projectile.rs"), include_str!("resolution.rs"));
        let start = src
            .find("fn try_enter_missile_ignition")
            .expect("ignition helper");
        // Window spans try_enter_missile_ignition plus its
        // play_missile_ignition helper, where C++ MissileAIUpdate.cpp:462
        // FXList::doFXObj(d->m_ignitionFX, getObject()) actually runs.
        let body = &src[start..start + 2600];
        assert!(
            body.contains("dispatch_fx_list_at_object(ignition_fx, self.id.0, None)"),
            "live MissileAI ignition must play authored IgnitionFX",
        );
        assert!(body.contains("missile.ignition_fx"));
    }
}
