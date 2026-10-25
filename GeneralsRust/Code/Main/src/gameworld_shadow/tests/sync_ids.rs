//! Stable ids, host sync residuals, damage mutation, production/construction/SP sole-tick.

use super::*;

#[test]
fn shadow_stable_ids_across_sync() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StableIdMap");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "ShadowUnit", 100.0);
    let a = logic
        .create_object("ShadowUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("ShadowUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("b");

    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    let ea = shadow.entity_for_host(a).expect("map a");
    let eb = shadow.entity_for_host(b).expect("map b");
    assert_ne!(ea.get(), eb.get());

    // Second sync must keep the same EntityIds.
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.entity_for_host(a), Some(ea));
    assert_eq!(shadow.entity_for_host(b), Some(eb));

    let probe = shadow.probe(&mut logic);
    assert!(probe.full_match(), "{}", probe.format_report());
}

#[test]
fn shadow_world_boundary_reset_clears_previous_identity_map() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WorldBoundaryReset");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "BoundaryUnit", 100.0);
    let id = logic
        .create_object("BoundaryUnit", Team::USA, Vec3::ZERO)
        .expect("unit");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).expect("entity");
    assert_eq!(shadow.mapped_count(), 1);

    // A reset/map-load/save-restore may reuse host ObjectIds.  No old
    // GameWorld entity or host map entry may survive that boundary.
    shadow.reset_for_world_boundary();
    assert_eq!(shadow.mapped_count(), 0);
    assert!(shadow.entity_for_host(id).is_none());
    assert!(shadow.world().entity(entity).is_none());

    // The next authoritative sync starts a fresh mapping in the new world.
    shadow.sync_from_host(&logic);
    assert!(shadow.entity_for_host(id).is_some());
}

#[test]
fn sync_grows_beyond_initial_entity_hint_without_dropping_lifecycle_mappings() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ShadowEntityGrowth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "GrowthUnit", 100.0);

    let ids: Vec<_> = (0..3)
        .map(|index| {
            logic
                .create_object(
                    "GrowthUnit",
                    Team::USA,
                    Vec3::new(index as f32 * 10.0, 0.0, 0.0),
                )
                .expect("growth unit")
        })
        .collect();

    // The hint is deliberately smaller than the live host object set. C++
    // expands its ObjectID lookup storage instead of omitting registrations.
    let mut shadow = GameWorldShadow::new(1);
    shadow.sync_from_host(&logic);

    assert_eq!(shadow.mapped_count(), ids.len());
    for id in ids {
        assert!(
            shadow.entity_for_host(id).is_some(),
            "host object {id:?} must retain a lifecycle mapping"
        );
    }
}

#[test]
fn same_faction_slots_keep_owner_authority_through_shadow_and_presentation() {
    use crate::game_logic::Player;
    use crate::presentation_frame::PresentationFrame;
    use gamelogic::common::Relationship;

    let mut logic = GameLogic::new();
    logic.clear_all_players();
    let mut local = Player::new(0, Team::USA, "USA local", true);
    local.alliance_team = 0;
    let mut opponent = Player::new(1, Team::USA, "USA opponent", false);
    opponent.alliance_team = 1;
    logic.add_player(local);
    logic.add_player(opponent);

    ensure_template(&mut logic, "OwnerParityUnit", 100.0);
    logic
        .templates
        .get_mut("OwnerParityUnit")
        .expect("template")
        .add_kind_of(KindOf::Vehicle);
    let mine = logic
        .create_object_for_player("OwnerParityUnit", 0, Vec3::ZERO)
        .expect("local unit");
    let theirs = logic
        .create_object_for_player("OwnerParityUnit", 1, Vec3::new(20.0, 0.0, 0.0))
        .expect("opponent unit");

    let mine_host = logic.host_object(mine).expect("mine host");
    let theirs_host = logic.host_object(theirs).expect("opponent host");
    assert_eq!(mine_host.team, Team::USA);
    assert_eq!(theirs_host.team, Team::USA);
    assert_eq!(mine_host.owner_player_id, Some(0));
    assert_eq!(theirs_host.owner_player_id, Some(1));
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
    assert_eq!(
        logic.object_relationship(mine_host, theirs_host),
        Relationship::Enemies
    );

    // A stale client may hold another player's ObjectId in its selection
    // (selection is mask-only); the direct-order gate must still refuse it.
    logic.select_objects(0, vec![mine, theirs]);
    assert_eq!(
        logic.get_player(0).expect("local player").selected_objects,
        vec![mine, theirs]
    );
    logic
        .get_player_mut(0)
        .expect("local player")
        .selected_objects = vec![theirs];
    logic.command_move(0, Vec3::new(100.0, 0.0, 0.0));
    assert!(
        logic
            .host_object(theirs)
            .expect("opponent host")
            .movement
            .target_position
            .is_none(),
        "player 0 must not move player 1's same-faction unit"
    );
    logic
        .get_player_mut(0)
        .expect("local player")
        .selected_objects = vec![mine];
    logic.command_move(0, Vec3::new(100.0, 0.0, 0.0));
    assert!(
        logic
            .host_object(mine)
            .expect("local host")
            .movement
            .target_position
            .is_some(),
        "the owning player retains command authority"
    );

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let mine_entity = shadow.entity_for_host(mine).expect("mine entity");
    let theirs_entity = shadow.entity_for_host(theirs).expect("opponent entity");
    assert_ne!(
        shadow
            .world()
            .entity(mine_entity)
            .expect("mine entity")
            .owner,
        shadow
            .world()
            .entity(theirs_entity)
            .expect("opponent entity")
            .owner,
        "shadow must not collapse two USA owners"
    );

    let frame = PresentationFrame::build_from_logic(&mut logic, 0);
    let mine_frame = frame
        .objects
        .iter()
        .find(|o| o.id == mine)
        .expect("mine frame");
    let theirs_frame = frame
        .objects
        .iter()
        .find(|o| o.id == theirs)
        .expect("opponent frame");
    assert_eq!(mine_frame.owner_player_id, Some(0));
    assert_eq!(theirs_frame.owner_player_id, Some(1));
    assert!(frame.is_owned_by_local(mine_frame));
    assert!(!frame.is_owned_by_local(theirs_frame));
    assert!(frame.is_enemy_of_local(theirs_frame));
}

#[test]
fn shadow_damage_mutation_matches_host() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DamageParity");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "DmgUnit", 200.0);
    let id = logic
        .create_object("DmgUnit", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("unit");

    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    logic.apply_host_damage(id, 35.0);
    assert_eq!(logic.host_object(id).unwrap().health.current, 165.0);
    shadow.sync_from_host(&logic);
    // ID remains stable after damage.
    assert!(shadow.entity_for_host(id).is_some());
    let probe = shadow.probe(&mut logic);
    assert!(probe.health_match, "{}", probe.format_report());
}

#[test]
fn shadow_counts_and_economy_match_after_skirmish_config() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GameWorldShadowMap");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let (shadow, probe) = probe_host_vs_gameworld(&mut logic);
    assert!(
        probe.full_match() || probe.host_objects > 4096,
        "{}",
        probe.format_report()
    );
    let view = presentation_view_from_shadow(&shadow, 0);
    assert_eq!(view.frame, logic.get_frame() as u64);
    assert_eq!(
        view.entities.len(),
        logic.host_objects().registered_len().min(4096)
    );
}

#[test]
fn presentation_overlay_uses_shadow_health() {
    use crate::presentation_frame::PresentationFrame;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresOverlay");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "OverlayUnit", 100.0);
    let id = logic
        .create_object("OverlayUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 0.0))
        .expect("u");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .health = 60.0;
    let mut pres = PresentationFrame::build_from_logic(&mut logic, 0);
    let before = pres
        .objects
        .iter()
        .find(|o| o.id == id)
        .map(|o| o.health_current)
        .unwrap();
    let n = pres.overlay_gameworld_shadow(&shadow);
    assert!(n >= 1);
    let after = pres
        .objects
        .iter()
        .find(|o| o.id == id)
        .map(|o| o.health_current)
        .unwrap();
    assert!(
        after < before,
        "overlay should pull lower shadow HP {after} vs {before}"
    );
}

#[test]
fn pose_writeback_is_last_writer() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PoseWB");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "PoseU", 80.0);
    let id = logic
        .create_object("PoseU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .transform
        .position = glam::Vec3::new(42.0, 1.0, 7.0);
    // Host still at origin until writeback.
    {
        let p = logic.host_objects().lookup(&id).unwrap().get_position();
        assert!(p.x.abs() < 0.1, "pre-writeback host x={}", p.x);
    }
    logic.set_fixture_object_position(id, glam::Vec3::new(42.0, 1.0, 7.0));
    logic.set_fixture_object_orientation(id, 0.5);
    shadow.sync_from_host(&logic);

    let p = logic.host_objects().lookup(&id).unwrap().get_position();
    assert!((p.x - 42.0).abs() < 0.01, "host x={}", p.x);
    assert!((p.z - 7.0).abs() < 0.01, "host z={}", p.z);
}

#[test]
fn sync_from_host_copies_entity_engine_bridged_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityEngineBridged");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "BrU", 100.0);
    let id = logic
        .create_object("BrU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(
        !e.engine_bridged,
        "dual-id bridge retired — engine_bridged stays false"
    );
}

fn sync_from_host_copies_entity_fow_ground_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityFowGround");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "FowU", 100.0);
    let id = logic
        .create_object("FowU", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.fow_visibility_alpha - 1.0).abs() < 1e-5);
    assert!((e.fow_is_explored - 1.0).abs() < 1e-5);
    assert!(e.ground_height.is_finite());
}

fn sync_from_host_copies_entity_model_key_mesh_scale_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityMeshKey");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MeshU", 100.0);
    {
        let t = logic.templates.get_mut("MeshU").expect("t");
        t.model_name = Some("AVTank".into());
    }
    let id = logic
        .create_object("MeshU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    // Prove host object carries template model residual before shadow sync.
    {
        let obj = logic.host_object(id).expect("host obj");
        let key = crate::assets::mesh_asset_resolve::model_key_from_template(obj.get_template());
        assert_eq!(
            key.to_ascii_lowercase(),
            "avtank",
            "host template model key"
        );
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(
        e.model_key.to_ascii_lowercase(),
        "avtank",
        "model_key residual got {:?}",
        e.model_key
    );
    assert!(
        e.mesh_scale.is_finite() && e.mesh_scale > 0.0,
        "mesh_scale residual"
    );
}

fn sync_from_host_copies_entity_production_queue_items_residual() {
    use crate::game_logic::{BuildingData, BuildingType, ProductionItem, Resources};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityProdQueue");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "Fact", 500.0);
    ensure_template(&mut logic, "UnitA", 100.0);
    ensure_template(&mut logic, "UnitB", 100.0);
    let id = logic
        .create_object("Fact", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("o");
        let mut bd = BuildingData::new(BuildingType::WarFactory);
        bd.production_queue = vec![
            ProductionItem {
                runtime_entry_identity: 0,
                template_name: "UnitA".into(),
                progress: 0.25,
                total_time: 10.0,
                construction_frames: 0,
                cost: Resources {
                    supplies: 300,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: crate::game_logic::buildings::ProductionKind::Unit,
            },
            ProductionItem {
                runtime_entry_identity: 0,
                template_name: "UnitB".into(),
                progress: 0.0,
                total_time: 12.0,
                construction_frames: 0,
                cost: Resources {
                    supplies: 400,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: crate::game_logic::buildings::ProductionKind::Unit,
            },
        ];
        obj.building_data = Some(bd);
        obj.object_type = crate::game_logic::ObjectType::Building;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.production_queue_len, 2);
    assert_eq!(e.production_queue_items.len(), 2);
    assert_eq!(e.production_queue_items[0].template_name, "UnitA");
    assert!((e.production_queue_items[0].progress - 0.25).abs() < 1e-5);
    assert_eq!(e.production_queue_items[0].cost_supplies, 300);
    assert_eq!(e.production_queue_items[1].template_name, "UnitB");
    assert_eq!(e.production_queue_items[1].total_time, 12.0);
    assert_eq!(e.production_template, "UnitA");
}

fn sync_from_host_copies_entity_applied_upgrade_names_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityUpgrades");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "UpU", 100.0);
    let id = logic
        .create_object("UpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("o");
        obj.apply_upgrade_tag("UpgradeA");
        obj.apply_upgrade_tag("UpgradeB");
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.applied_upgrade_count, 2);
    assert_eq!(
        e.applied_upgrade_names,
        vec!["UpgradeA".to_string(), "UpgradeB".to_string()]
    );
}

fn sync_from_host_copies_entity_kind_of_bits_residual() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityKindOf");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "KindU", 100.0);
    {
        let t = logic.templates.get_mut("KindU").expect("t");
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        t.add_kind_of(KindOf::Unattackable);
    }
    let id = logic
        .create_object("KindU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    // ORDER: Structure=0 Infantry=1 ... Selectable=6 Attackable=7
    assert!(e.kind_of_bits & (1 << 1) != 0, "Infantry bit");
    assert!(e.kind_of_bits & (1 << 6) != 0, "Selectable bit");
    assert!(e.kind_of_bits & (1 << 7) != 0, "Attackable bit");
    assert!(
        e.unattackable,
        "the full compact KindOf bank has no spare bit, so the C++ WeaponSet victim override travels on its dedicated shadow channel"
    );
}

fn sync_from_host_copies_entity_garrison_contain_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityGarrisonContain");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "GarBldg", 500.0);
    ensure_template(&mut logic, "GarInf", 100.0);
    let bldg = logic
        .create_object("GarBldg", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("bldg");
    let inf = logic
        .create_object("GarInf", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("inf");
    {
        use crate::game_logic::{BuildingData, BuildingType};
        let obj = logic.host_object_mut(bldg).expect("b");
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.garrisoned_units = vec![inf];
        bd.max_garrison = 5;
        obj.building_data = Some(bd);
        obj.object_type = crate::game_logic::ObjectType::Building;
    }
    {
        let obj = logic.host_object_mut(inf).expect("i");
        obj.contained_by = Some(bldg);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let be = shadow.entity_for_host(bldg).expect("bm");
    let b = shadow.world().entity(be).expect("b");
    assert_eq!(b.garrisoned_host_ids, vec![inf.0]);
    assert_eq!(b.max_garrison, 5);
    let ie = shadow.entity_for_host(inf).expect("im");
    let i = shadow.world().entity(ie).expect("i");
    assert_eq!(i.contained_by_host, bldg.0);
}

fn sync_from_host_copies_entity_path_waypoints_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityPathWp");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "PathWpU", 100.0);
    let id = logic
        .create_object("PathWpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        use crate::game_logic::Weapon;
        let obj = logic.host_object_mut(id).expect("obj");
        obj.movement.path = vec![
            glam::Vec3::new(1.0, 0.0, 1.0),
            glam::Vec3::new(2.0, 0.0, 2.0),
            glam::Vec3::new(3.0, 0.0, 3.0),
        ];
        obj.movement.current_path_index = 1;
        obj.secondary_weapon = Some(Weapon {
            damage: 8.0,
            range: 90.0,
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
        });
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.path_len, 3);
    assert_eq!(e.path_index, 1);
    assert_eq!(e.path_waypoints.len(), 3);
    assert!((e.path_waypoints[2][0] - 3.0).abs() < 0.01);
    assert!(e.has_secondary_weapon || e.secondary_weapon_range > 0.0);
    assert!((e.secondary_weapon_range - 90.0).abs() < 0.01);
    assert!((e.secondary_weapon_damage - 8.0).abs() < 0.01);
}

fn sync_from_host_copies_entity_combat_timing_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityCombatTiming");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "CbtTimeU", 100.0);
    let id = logic
        .create_object("CbtTimeU", Team::USA, glam::Vec3::new(13.0, 0.0, 13.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.weapon_bonus_frenzy_until_frame = 90;
        obj.continuous_fire_coast_until_frame = 33;
        obj.battle_plan_sight_scalar_applied = 1.5;
        obj.consecutive_shots_at_target = 4;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.frenzy_until_entity_count(), 1);
    assert_eq!(shadow.battle_plan_sight_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.weapon_bonus_frenzy_until_frame, 90);
    assert_eq!(e.continuous_fire_coast_until_frame, 33);
    assert!((e.battle_plan_sight_scalar_applied - 1.5).abs() < 0.001);
    assert_eq!(e.continuous_fire_consecutive, 4);
}

fn sync_from_host_copies_entity_combat_bonus_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityCombatBonus");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "BonusU", 100.0);
    let id = logic
        .create_object("BonusU", Team::China, glam::Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    let src = logic
        .create_object("BonusU", Team::GLA, glam::Vec3::new(20.0, 0.0, 12.0))
        .expect("src");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.weapon_bonus_enthusiastic = true;
        obj.weapon_bonus_subliminal = true;
        obj.weapon_bonus_horde = true;
        obj.weapon_bonus_nationalism = true;
        obj.weapon_bonus_frenzy = true;
        obj.weapon_bonus_frenzy_level = 2;
        obj.weapon_bonus_battle_plan_bombardment = true;
        obj.weapon_bonus_battle_plan_hold_the_line = true;
        obj.weapon_bonus_battle_plan_search_and_destroy = true;
        obj.continuous_fire_level = 3;
        obj.consecutive_shots_at_target = 7;
        obj.faerie_fire_until_frame = 99;
        obj.is_humvee_transport = true;
        obj.is_listening_outpost_transport = true;
        obj.is_troop_crawler_transport = true;
        obj.is_helix_transport = true;
        obj.has_overlord_gattling_addon = true;
        obj.has_overlord_propaganda_addon = true;
        obj.demo_suicided_detonating = true;
        obj.hive_slave_count = 3;
        obj.hive_slave_hp = 40.0;
        obj.turret_angle_deg = 45.0;
        obj.turret_pitch_deg = 15.0;
        obj.turret_idle_scanning = true;
        obj.turret_holding = true;
        obj.ai_attitude = 2;
        obj.last_damage_source = Some(src);
        obj.command_set_override = Some("Command_ChinaTankOverlord".into());
        obj.disguise_as_template = Some("AmericaVehicleHumvee".into());
        obj.disguise_as_team = Some(Team::USA);
        obj.vision_spied_mask = 0b101;
        obj.camo_friendly_opacity = 0.4;
        // camo_stealth_look left default
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.horde_bonus_entity_count(), 1);
    assert_eq!(shadow.humvee_transport_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.weapon_bonus_enthusiastic && e.weapon_bonus_horde && e.weapon_bonus_frenzy);
    assert_eq!(e.weapon_bonus_frenzy_level, 2);
    assert_eq!(e.continuous_fire_level, 3);
    assert_eq!(e.continuous_fire_consecutive, 7);
    assert_eq!(e.faerie_fire_until_frame, 99);
    assert!(e.is_humvee_transport && e.is_helix_transport);
    assert!(e.has_overlord_gattling_addon && e.has_overlord_propaganda_addon);
    assert!(e.demo_suicided_detonating);
    assert_eq!(e.hive_slave_count, 3);
    assert!((e.turret_angle_deg - 45.0).abs() < 0.01);
    assert_eq!(e.ai_attitude, 2);
    assert_eq!(e.last_damage_source_host, src.0);
    assert_eq!(e.command_set_override, "Command_ChinaTankOverlord");
    assert_eq!(e.disguise_as_template, "AmericaVehicleHumvee");
    assert_eq!(e.disguise_as_team_ordinal, 0); // USA
    assert_eq!(e.vision_spied_mask, 0b101);
    assert!((e.camo_friendly_opacity - 0.4).abs() < 0.01);
}

fn sync_from_host_copies_entity_detector_sp_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityDetectorSp");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "DetectU", 100.0);
    let id = logic
        .create_object("DetectU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.cheer_timer = 2.5;
        obj.overcharge_enabled = true;
        obj.active_weapon_slot = 2;
        obj.guard_radius = 120.0;
        obj.applied_upgrades.insert("UpgradeChemicalSuits".into());
        obj.applied_upgrades.insert("UpgradeCompositeArmor".into());
        obj.special_power_ready = true;
        obj.special_power_cooldown = 60.0;
        obj.special_power_cooldown_remaining = 12.0;
        obj.is_detector = true;
        obj.detection_range = 200.0;
        obj.detection_rate_frames = 15;
        obj.stealth_breaks_on_attack = true;
        obj.stealth_breaks_on_move = true;
        obj.innate_stealth = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.detector_entity_count(), 1);
    assert_eq!(shadow.special_power_ready_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.cheer_timer - 2.5).abs() < 0.01);
    assert!(e.overcharge_enabled);
    assert_eq!(e.active_weapon_slot, 2);
    assert!((e.guard_radius - 120.0).abs() < 0.01);
    assert_eq!(e.applied_upgrade_count, 2);
    assert!(e.special_power_ready);
    assert!((e.special_power_cooldown - 60.0).abs() < 0.01);
    assert!((e.special_power_cooldown_remaining - 12.0).abs() < 0.01);
    assert!(e.is_detector);
    assert!((e.detection_range - 200.0).abs() < 0.01);
    assert_eq!(e.detection_rate_frames, 15);
    assert!(e.stealth_breaks_on_attack && e.stealth_breaks_on_move && e.innate_stealth);
}

fn sync_from_host_copies_entity_transport_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityTransport");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "OverlordX", 800.0);
    ensure_template(&mut logic, "RiderX", 100.0);
    let bus = logic
        .create_object("OverlordX", Team::China, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("bus");
    let rider = logic
        .create_object("RiderX", Team::China, glam::Vec3::new(10.0, 0.0, 9.0))
        .expect("rider");
    {
        let obj = logic.host_object_mut(bus).expect("obj");
        obj.name = "OL-1".into();
        obj.overlord_bunker_capacity = Some(5);
        obj.passengers_allowed_to_fire = true;
        obj.armed_riders_upgrade_weapon_set = true;
        obj.weapon_set_player_upgrade = true;
        obj.is_battle_bus_transport = true;
        obj.is_technical_transport = false;
        obj.is_combat_cycle_transport = false;
        obj.combat_cycle_rider = 0;
        obj.is_tunnel_network = false;
        obj.is_combat_chinook_transport = true;
    }
    {
        let obj = logic.host_object_mut(rider).expect("rider");
        obj.contained_by = Some(bus);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.battle_bus_entity_count(), 1);
    assert_eq!(shadow.contained_entity_count(), 1);
    let eid = shadow.entity_for_host(bus).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.display_name, "OL-1");
    assert_eq!(e.overlord_bunker_capacity, 5);
    assert!(e.passengers_allowed_to_fire);
    assert!(e.armed_riders_upgrade_weapon_set);
    assert!(e.weapon_set_player_upgrade);
    assert!(e.is_battle_bus_transport);
    assert!(e.is_combat_chinook_transport);
    let rid = shadow.entity_for_host(rider).expect("rmap");
    let r = shadow.world().entity(rid).expect("r");
    assert_eq!(r.contained_by_host, bus.0);
}

#[test]
fn sync_from_host_copies_entity_weapon_move_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityWeaponMove");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "WpnMoveU", 100.0);
    let id = logic
        .create_object("WpnMoveU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        use crate::game_logic::Weapon;
        let obj = logic.host_object_mut(id).expect("obj");
        obj.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            min_range: 5.0,
            reload_time: 1.5,
            last_fire_time: 0.0,
            ammo: Some(30),
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 200.0,
            pre_attack_delay: 0.1,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
        obj.secondary_weapon = Some(Weapon::default());
        obj.movement.max_speed = 12.5;
        obj.movement.velocity = glam::Vec3::new(1.0, 0.0, 2.0);
        obj.movement.path = vec![
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::new(10.0, 0.0, 10.0),
        ];
        obj.movement.current_path_index = 1;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.armed_entity_count(), 1);
    assert_eq!(shadow.pathing_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.has_weapon && e.has_secondary_weapon);
    assert!((e.weapon_damage - 25.0).abs() < 0.01);
    assert!((e.weapon_range - 150.0).abs() < 0.01);
    assert!((e.weapon_min_range - 5.0).abs() < 0.01);
    assert_eq!(e.weapon_ammo, 30);
    assert!(e.weapon_can_target_air);
    assert!((e.move_max_speed - 12.5).abs() < 0.01);
    assert!((e.velocity[2] - 2.0).abs() < 0.01);
    assert_eq!(e.path_len, 2);
    assert_eq!(e.path_index, 1);
}

#[test]
fn sync_from_host_copies_entity_building_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityBuilding");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "BarracksRes", 500.0);
    let id = logic
        .create_object("BarracksRes", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    {
        use crate::game_logic::{BuildingData, BuildingType, ProductionItem, Resources};
        let obj = logic.host_object_mut(id).expect("obj");
        obj.object_type = crate::game_logic::ObjectType::Building;
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "AmericaInfantryRanger".into(),
            progress: 0.35,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 225,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: crate::game_logic::buildings::ProductionKind::Unit,
        });
        bd.rally_point = Some(glam::Vec3::new(20.0, 0.0, 20.0));
        bd.garrisoned_units = vec![crate::game_logic::ObjectId(99)];
        bd.max_garrison = 5;
        obj.building_data = Some(bd);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.building_data_entity_count(), 1);
    assert_eq!(shadow.producing_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.is_building);
    assert_eq!(e.building_type_ordinal, 1); // Barracks
    assert_eq!(e.production_queue_len, 1);
    assert!((e.production_progress - 0.35).abs() < 0.001);
    assert_eq!(e.production_template, "AmericaInfantryRanger");
    assert_eq!(e.garrison_count, 1);
    assert_eq!(e.max_garrison, 5);
    let rp = e.rally_point.expect("rally");
    assert!((rp[0] - 20.0).abs() < 0.01);
}

#[test]
fn writeback_production_and_rally_to_host() {
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
        ThingTemplate,
    };
    // writeback_production_to_host is the GameWorld->GameLogic production
    // last-writer channel; C++ ProductionUpdate (ProductionUpdate.cpp) makes
    // TheGameLogic the sole production writer, so the channel is opt-in via
    // GameLogic::set_production_authority(true) (hq-e84zk retired the
    // GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY env flag). Arm it per-instance.
    // The ready/production logs are process-global thread-locals; clear at the
    // boundary so a prior producer test's pending entries cannot feed the
    // writeback skip-guard (writeback_production.rs:43-47).
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("ProdRallyWb");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WarFact") {
        let mut t = ThingTemplate::new("WarFact");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("WarFact".into(), t);
    }
    let id = logic
        .create_object("WarFact", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");
    {
        let obj = /* Wave 946 */ logic.host_object_mut(id).expect("o");
        let mut bd = BuildingData::new(BuildingType::WarFactory);
        bd.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "USACrusaderTank".into(),
            progress: 0.1,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 900,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        bd.rally_point = Some(glam::Vec3::new(1.0, 0.0, 2.0));
        obj.building_data = Some(bd);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    {
        let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
        e.rally_point = Some([9.0, 0.0, 8.0]);
        if let Some(item) = e.production_queue_items.get_mut(0) {
            item.progress = 0.75;
        }
    }
    let original = logic
        .host_object(id)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert_eq!(original.rally_point, Some(glam::Vec3::new(1.0, 0.0, 2.0)));
    assert_eq!(original.production_queue[0].progress, 0.1);
    let data = logic
        .host_object_mut(id)
        .unwrap()
        .building_data
        .as_mut()
        .unwrap();
    data.rally_point = Some(glam::Vec3::new(9.0, 0.0, 8.0));
    data.production_queue[0].progress = 0.75;
    shadow.sync_from_host(&logic);
    let obj = logic.host_objects().lookup(&id).expect("o");
    let bd = obj.building_data.as_ref().expect("bd");
    assert_eq!(bd.rally_point, Some(glam::Vec3::new(9.0, 0.0, 8.0)));
    assert!((bd.production_queue[0].progress - 0.75).abs() < 1e-5);
}

#[test]
fn production_authority_sole_ticks_queue_progress() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    {
        let mut t = ThingTemplate::new("SoleTickFact");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("SoleTickFact".into(), t);
    }
    let oid = logic
        .create_object("SoleTickFact", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    {
        use crate::game_logic::{
            BuildingData, BuildingType, ProductionItem, ProductionKind, Resources,
        };
        let mut data = BuildingData::new(BuildingType::Barracks);
        data.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "Ranger".into(),
            progress: 0.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        logic.host_object_mut(oid).unwrap().building_data = Some(data);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue[0]
            .construction_frames,
        1
    );
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let ent = shadow.world().entity(eid).expect("ent");
    let head = ent.production_queue_items.first().expect("head");
    // C++ ProductionUpdate.cpp:687 increments m_framesUnderConstruction once
    // per logic update, then percent = frames / calcTimeToBuild.
    // GameWorld sole-tick mirrors that one-frame step (dt is unused).
    let before = 0.0;
    assert!(
        head.progress > before,
        "expected one construction-frame advance, got {}",
        head.progress
    );
    let expected = head.progress;
    shadow.sync_from_host(&logic);
    let obj = logic.host_object(oid).expect("obj");
    let b = obj.building_data.as_ref().expect("bd");
    let hp = b.production_queue.first().expect("hq");
    assert!((hp.progress - expected).abs() < 1e-4);
}

#[test]
fn completed_production_waits_for_open_door_before_entity_first_spawn() {
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources,
    };

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DoorGateProduction");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Isolate authored door behavior from the genuine low-power build penalty.
    for player in logic.get_players_mut().values_mut() {
        player.power_produced = 1;
        player.power_consumed = 0;
    }
    ensure_template(&mut logic, "DoorGateRanger", 100.0);
    logic
        .templates
        .get_mut("DoorGateRanger")
        .unwrap()
        .add_kind_of(KindOf::Infantry);
    // Retail producers author ProductionUpdate NumDoorAnimations (C++
    // ProductionUpdate door cycle gates spawn on WAITING_OPEN); the host
    // door table resolves retail names (AmericaBarracks: 1 door). The
    // invented name resolved 0 doors and bypassed the door gate entirely.
    let mut producer = ThingTemplate::new("AmericaBarracks");
    producer.set_health(500.0);
    producer.add_kind_of(KindOf::Structure);
    producer.add_kind_of(KindOf::FSBarracks);
    logic
        .templates
        .insert("AmericaBarracks".to_string(), producer);
    let producer_id = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("producer");
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "DoorGateRanger".to_string(),
            progress: 1.0,
            total_time: 1.0,
            construction_frames: 30,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        // CLOSED: C++ ProductionUpdate must start its door cycle before spawn.
        producer.production_door_phase = 0;
        producer.building_data = Some(building);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let before = shadow.world().world().entity_count();
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow.world().world().entity_count(),
        before,
        "a closed production door must not accumulate unmapped entity-first spawns"
    );
    assert_eq!(
        logic.host_objects().registered_len(),
        before,
        "closed doors must not create host objects"
    );

    // WAITING_OPEN: exactly one entity-first ready event is now valid. The
    // host completion phase binds it to the newly-created host object.
    logic
        .host_object_mut(producer_id)
        .expect("producer object")
        .production_door_phase = 2;
    logic
        .host_object_mut(producer_id)
        .unwrap()
        .production_door_phases[0] = 2;
    shadow.sync_from_host(&logic);
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().world().entity_count(), before + 1);
    assert_eq!(
        logic
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "DoorGateRanger")
            .count(),
        1
    );
    assert!(
        logic
            .host_object(producer_id)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    // Process-global thread-local logs must not leak into the next producer
    // test (established clear-at-boundary pattern).

    match prev_shadow {
        Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn completed_quantity_batch_emits_every_entity_first_unit_after_open_door() {
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionExitMetadata, ProductionExitStyle,
        ProductionItem, ProductionKind, Resources,
    };

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DoorBatchProduction");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Isolate authored door behavior from the genuine low-power build penalty.
    for player in logic.get_players_mut().values_mut() {
        player.power_produced = 1;
        player.power_consumed = 0;
    }
    ensure_template(&mut logic, "DoorBatchRanger", 100.0);
    logic
        .templates
        .get_mut("DoorBatchRanger")
        .unwrap()
        .add_kind_of(KindOf::Infantry);
    let mut producer = ThingTemplate::new("DoorBatchBarracks");
    producer.set_health(500.0);
    producer.add_kind_of(KindOf::Structure);
    producer.add_kind_of(KindOf::FSBarracks);
    // This is specifically DefaultProductionExitUpdate semantics: C++ has no
    // Queue delay/burst busy state, so a completed QuantityModifier batch can
    // reserve every member in one terminal update.
    producer.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Default,
        unit_create_point: [0.0, 0.0, 0.0],
        natural_rally_point: [0.0, 0.0, 0.0],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic
        .templates
        .insert("DoorBatchBarracks".to_string(), producer);
    let producer_id = logic
        .create_object(
            "DoorBatchBarracks",
            Team::USA,
            glam::Vec3::new(8.0, 0.0, 8.0),
        )
        .expect("producer");
    let hold_open_until = logic.get_frame().saturating_add(100);
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "DoorBatchRanger".to_string(),
            progress: 1.0,
            total_time: 1.0,
            construction_frames: 30,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 2,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        // C++ QuantityModifier loop is only reached after an exit is available.
        producer.production_door_phase = 2;
        producer.production_door_phases[0] = 2;
        producer.production_door_phase_end_frame = hold_open_until;
        producer.production_door_phase_end_frames[0] = hold_open_until;
        producer.building_data = Some(building);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let before = shadow.world().world().entity_count();
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    let spawned: Vec<_> = logic
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "DoorBatchRanger")
        .collect();
    assert_eq!(
        spawned.len(),
        2,
        "one terminal QuantityModifier update must create every member"
    );
    assert!(
        spawned
            .iter()
            .all(|object| object.team == logic.host_object(producer_id).unwrap().team)
    );
    assert!(
        logic
            .host_object(producer_id)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    assert_eq!(
        shadow.world().world().entity_count(),
        before + 2,
        "one open-door terminal update must create both shadow entities"
    );

    match prev_shadow {
        Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn queue_exit_sole_tick_releases_one_then_waits_exact_nine_frames() {
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

    use crate::game_logic::host_production_progress_log;

    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionExitMetadata, ProductionExitStyle,
        ProductionItem, ProductionKind, Resources,
    };

    host_production_progress_log::clear();

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("QueueSoleTickProduction");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "QueueSoleRanger", 100.0);
    let mut producer = ThingTemplate::new("QueueSoleProducer");
    producer
        .set_health(500.0)
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks);
    producer.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic
        .templates
        .insert("QueueSoleProducer".to_string(), producer);
    let producer_id = logic
        .create_object("QueueSoleProducer", Team::China, glam::Vec3::ZERO)
        .expect("producer");
    let hold_open_until = logic.get_frame().saturating_add(100);
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "QueueSoleRanger".to_string(),
            progress: 1.0,
            total_time: 1.0,
            construction_frames: 30,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 2,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        producer.production_door_phase = 2;
        producer.production_door_phases[0] = 2;
        producer.production_door_phase_end_frame = hold_open_until;
        producer.production_door_phase_end_frames[0] = hold_open_until;
        producer.building_data = Some(building);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    let spawned = |logic: &GameLogic| {
        logic
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "QueueSoleRanger")
            .count()
    };
    assert_eq!(
        spawned(&logic),
        1,
        "fresh Queue InitialBurst=0 releases one member"
    );
    let runtime = logic
        .host_object(producer_id)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_exit_runtime_state();
    assert_eq!(runtime.delay_frames, 9);
    for _ in 0..8 {
        logic.set_current_frame(u64::from(logic.get_frame()) + 1);
        logic.update_production(1.0 / 30.0);
    }
    shadow.sync_from_host(&logic);
    assert_eq!(
        spawned(&logic),
        1,
        "second member remains held after eight logic frames"
    );
    logic.set_current_frame(u64::from(logic.get_frame()) + 1);
    logic.update_production(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    assert_eq!(
        spawned(&logic),
        2,
        "ninth Queue frame releases exactly one member"
    );
    assert!(
        logic
            .host_object(producer_id)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    match prev_shadow {
        Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn construction_progress_observation_does_not_tick_host_or_shadow() {
    let mut logic = GameLogic::new();

    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    {
        let mut t = ThingTemplate::new("SoleTickBuild");
        t.add_kind_of(KindOf::Structure);
        t.build_time = 10.0;
        logic.templates.insert("SoleTickBuild".into(), t);
    }
    let oid = logic
        .create_object("SoleTickBuild", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        o.construction_percent = 0.0;
        o.set_status_under_construction(true);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let ent = shadow.world().entity(eid).expect("ent");
    // 0 + 0.25*2 = 0.5
    assert!(
        ent.construction_percent == 0.0,
        "got {}",
        ent.construction_percent
    );
    assert_eq!(shadow.compare_construction_to_host(&logic), 0);
    let obj = logic.host_object(oid).expect("obj");
    assert_eq!(obj.construction_percent, 0.0);
}

#[test]
fn special_power_authority_sole_ticks_cooldown() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    {
        let mut t = ThingTemplate::new("SoleTickSp");
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("SoleTickSp".into(), t);
    }
    let oid = logic
        .create_object("SoleTickSp", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        o.special_power_cooldown = 10.0;
        o.special_power_cooldown_remaining = 4.0;
        o.special_power_ready = false;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.host_object_mut(oid).unwrap().tick_timers(
        1.5,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let ent = shadow.world().entity(eid).expect("ent");
    assert!(
        (ent.special_power_cooldown_remaining - 2.5).abs() < 1e-4,
        "got {}",
        ent.special_power_cooldown_remaining
    );
    shadow.sync_from_host(&logic);
    let o = logic.host_object(oid).expect("obj");
    assert!((o.special_power_cooldown_remaining - 2.5).abs() < 1e-4);
    logic.host_object_mut(oid).unwrap().status.disabled_emp = true;
    logic.host_object_mut(oid).unwrap().tick_timers(
        1.0,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .special_power_cooldown_remaining,
        2.5
    );
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow
            .world()
            .entity(eid)
            .unwrap()
            .special_power_cooldown_remaining,
        2.5
    );
}

#[test]
fn shared_special_power_sole_ticks_player_cds() {
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    // Sole-tick tick path requires coupled engine frame (same as host freeze).

    use crate::command_system::SpecialPowerType;
    use crate::game_logic::GameLogic;

    let mut logic = GameLogic::new();

    logic.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "OwnedCooldown",
        true,
    ));
    let pid = 0u32;
    let Some(p) = logic.get_player_mut(pid) else {
        match prev_s {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        return;
    };
    p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 5.0);

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.tick_shared_special_power_timers(2.0);
    shadow.sync_from_host(&logic);
    let p = logic.get_player(pid).expect("player");
    let rem = p
        .shared_special_power_cooldowns
        .get(&SpecialPowerType::Airstrike)
        .copied()
        .unwrap_or(-1.0);
    assert!(
        (rem - 3.0).abs() < 1e-3,
        "expected ~3.0 remaining after 2s tick, got {rem}"
    );

    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn construction_shadow_disagreement_cannot_mutate_live_host() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ConstrWb");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("BuildPad") {
        let mut t = ThingTemplate::new("BuildPad");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("BuildPad".into(), t);
    }
    let id = logic
        .create_object("BuildPad", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let obj = /* Wave 946 */ logic.host_object_mut(id).expect("o");
        obj.construction_percent = 0.2;
        obj.status.under_construction = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    {
        let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
        e.construction_percent = 0.85;
        e.under_construction = false;
        e.sold = true;
        e.reconstructing = true;
        e.unselectable = true;
        // Combat/status flags are NOT owned by construction writeback.
        e.stealthed = true;
        e.selected = true;
    }
    let n = shadow.compare_construction_to_host(&logic);
    assert!(n >= 1);
    let obj = logic.host_objects().lookup(&id).expect("o");
    assert!((obj.construction_percent - 0.2).abs() < 1e-5);
    assert!(obj.status.under_construction);
    assert!(!obj.status.sold);
    assert!(!obj.status.reconstructing);
    assert!(!obj.status.unselectable);
    // Construction writeback must not touch combat-status residual.
    assert!(!obj.status.stealthed);
    assert!(!obj.status.selected);
    // Poisoned observations cannot select or reveal a live object.
    {
        let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
        e.stealthed = true;
        e.selected = true;
    }
    shadow.sync_from_host(&logic);

    let obj = logic.host_objects().lookup(&id).expect("o");
    assert!(!obj.status.stealthed);
    assert!(!obj.status.selected);
    // Complete residual
    {
        let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
        e.construction_percent = 1.0;
        e.under_construction = false;
    }
    let _ = shadow.compare_construction_to_host(&logic);
    let obj = logic.host_objects().lookup(&id).expect("o");
    assert!((obj.construction_percent - 0.2).abs() < 1e-5);
    assert!(obj.status.under_construction);
}

#[test]
fn construction_observation_uses_the_supplied_world_even_with_same_id_peer() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    fn world(percent: f32) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("OwnedObservationPad");
        template.add_kind_of(KindOf::Structure);
        logic
            .templates
            .insert("OwnedObservationPad".into(), template);
        let id = logic
            .create_object("OwnedObservationPad", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = logic.host_object_mut(id).unwrap();
        object.construction_percent = percent;
        object.status.under_construction = true;
        (logic, id)
    }
    let (owner, id) = world(0.25);
    let (mut peer, peer_id) = world(0.75);
    assert_eq!(id, peer_id);
    let mut projection = GameWorldShadow::new(64);
    projection.sync_from_host(&owner);
    peer.host_object_mut(peer_id).unwrap().construction_percent = 0.9;
    projection.sync_from_host(&owner);
    let entity = projection.entity_for_host(id).unwrap();
    assert_eq!(
        projection
            .world()
            .entity(entity)
            .unwrap()
            .construction_percent,
        0.25
    );
    assert_eq!(owner.host_object(id).unwrap().construction_percent, 0.25);
    assert_eq!(peer.host_object(peer_id).unwrap().construction_percent, 0.9);
    assert_eq!(projection.compare_construction_to_host(&owner), 0);
}
