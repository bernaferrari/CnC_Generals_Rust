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

    let frame = PresentationFrame::build_from_logic(&logic, 0);
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
    damage_parity_probe(&mut logic, &mut shadow, id, 35.0).expect("parity");
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
    assert_eq!(view.entities.len(), logic.host_objects().len().min(4096));
}

#[test]
fn presentation_overlay_uses_shadow_health() {
    use crate::presentation_frame::PresentationFrame;
    crate::game_logic::host_damage_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresOverlay");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "OverlayUnit", 100.0);
    let id = logic
        .create_object("OverlayUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 0.0))
        .expect("u");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.queue_damage_for_host(id, 40.0));
    let _ = shadow.apply_pending();
    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
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
    assert!(shadow.queue_set_transform_for_host(id, [42.0, 1.0, 7.0], 0.5));
    let _ = shadow.apply_pending();
    // Host still at origin until writeback.
    {
        let p = logic.host_objects().get(&id).unwrap().get_position();
        assert!(p.x.abs() < 0.1, "pre-writeback host x={}", p.x);
    }
    let n = shadow.writeback_transforms_to_host(&mut logic);
    let _ = crate::game_logic::host_transform_ready_log::drain();
    assert!(n >= 1, "writeback count {n}");
    let p = logic.host_objects().get(&id).unwrap().get_position();
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("e.engine_bridged = false"),
        "sync must force engine_bridged false"
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("fow_visibility_alpha")
            && src.contains("ground_height_from_terrain")
            && src.contains("FOWRenderingBridge::get_object_visibility"),
        "sync must copy FOW/ground residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("model_key_from_template") && src.contains("mesh_scale_from_template"),
        "sync must copy mesh residual via resolve helpers"
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("production_queue_items") && src.contains("EntityProductionItem"),
        "sync must copy full production queue residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("applied_upgrade_names") && src.contains("MAX_UPGRADES"),
        "sync must copy upgrade name residual"
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("host_kind_of_bits") && src.contains("kind_of_bits"),
        "sync must copy kind_of residual"
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("garrisoned_host_ids") && src.contains("garrisoned_units"),
        "sync must copy garrison residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("path_waypoints") && src.contains("secondary_weapon_range"),
        "sync must copy path/secondary residual"
    );
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
        obj.continuous_fire_consecutive = 4;
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("weapon_bonus_frenzy_until_frame")
            && src.contains("continuous_fire_coast_until_frame")
            && src.contains("battle_plan_sight_scalar_applied"),
        "sync must copy combat-timing residual"
    );
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
        obj.continuous_fire_consecutive = 7;
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
    let src_txt = GAMEWORLD_SHADOW_SRC;
    assert!(
        src_txt.contains("weapon_bonus_horde")
            && src_txt.contains("turret_angle_deg")
            && src_txt.contains("disguise_as_template"),
        "sync must copy combat-bonus residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("is_detector")
            && src.contains("special_power_ready")
            && src.contains("applied_upgrade_count"),
        "sync must copy detector/sp residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("overlord_bunker_capacity")
            && src.contains("contained_by_host")
            && src.contains("is_battle_bus_transport"),
        "sync must copy transport residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("weapon_damage") && src.contains("move_max_speed") && src.contains("path_len"),
        "sync must copy weapon/movement residual"
    );
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
    let src = GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("production_queue_len")
            && src.contains("building_type_ordinal")
            && src.contains("rally_point"),
        "sync must copy building residual"
    );
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
    crate::game_logic::host_production_log::clear();
    let mut logic = GameLogic::new();
    logic.set_production_authority(true);
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
    let n = shadow.writeback_production_to_host(&mut logic);
    let _ = shadow.writeback_production_door_to_host(&mut logic);
    let _ = shadow.writeback_body_damage_to_host(&mut logic);
    let _ = shadow.writeback_death_type_to_host(&mut logic);
    let _ = crate::game_logic::host_death_type_ready_log::drain();
    let _ = shadow.writeback_radar_extend_to_host(&mut logic);
    let _ = shadow.writeback_shock_stun_to_host(&mut logic);
    let _ = crate::game_logic::host_shock_stun_ready_log::drain();
    let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    assert!(n >= 1, "writeback must touch building");
    let obj = logic.host_objects().get(&id).expect("o");
    let bd = obj.building_data.as_ref().expect("bd");
    assert_eq!(bd.rally_point, Some(glam::Vec3::new(9.0, 0.0, 8.0)));
    assert!((bd.production_queue[0].progress - 0.75).abs() < 1e-5);
}

#[test]
fn production_authority_sole_ticks_queue_progress() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.set_production_authority(true);
    assert!(gameworld_production_authority_enabled());
    {
        let mut t = ThingTemplate::new("SoleTickFact");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("SoleTickFact".into(), t);
    }
    let oid = logic
        .create_object("SoleTickFact", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    host_production_progress_log::record(
        oid,
        vec![HostProductionQueueItem {
            template_name: "Ranger".into(),
            progress: 0.0,
            total_time: 10.0,
            construction_frames: 0,
            cost_supplies: 100,
            is_upgrade: false,
            quantity_total: 1,
            quantity_produced: 0,
        }],
        0.0,
        0.5, // half power
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let events = host_production_progress_log::drain();
    assert_eq!(shadow.apply_host_production_progress_events(&events), 1);
    let n = shadow.tick_production_queues(2.0);
    assert!(n >= 1, "sole-tick must advance at least one queue");
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
    let wb = shadow.writeback_production_to_host(&mut logic);
    assert!(wb >= 1);
    let obj = logic.host_object(oid).expect("obj");
    let b = obj.building_data.as_ref().expect("bd");
    let hp = b.production_queue.first().expect("hq");
    assert!((hp.progress - expected).abs() < 1e-4);
}

#[test]
fn completed_production_waits_for_open_door_before_entity_first_spawn() {
    let _env_guard = authority_env_lock();
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let _coupled = ShadowCoupleGuard::enter();

    use crate::game_logic::host_production_ready_log;
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources,
    };

    host_production_ready_log::clear();
    let mut logic = GameLogic::new();
    logic.set_production_authority(true);
    let cfg = golden_skirmish_config("DoorGateProduction");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "DoorGateRanger", 100.0);
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
        .create_object(
            "AmericaBarracks",
            Team::USA,
            glam::Vec3::new(8.0, 0.0, 8.0),
        )
        .expect("producer");
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            template_name: "DoorGateRanger".to_string(),
            progress: 1.0,
            total_time: 1.0,
            construction_frames: 0,
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
    shadow.writeback_production_to_host(&mut logic);
    shadow.writeback_production_to_host(&mut logic);
    assert_eq!(
        shadow.world().world().entity_count(),
        before,
        "a closed production door must not accumulate unmapped entity-first spawns"
    );
    assert!(
        host_production_ready_log::drain().is_empty(),
        "host must not receive a ready event before the door is waiting open"
    );

    // WAITING_OPEN: exactly one entity-first ready event is now valid. The
    // host completion phase binds it to the newly-created host object.
    logic
        .host_object_mut(producer_id)
        .expect("producer object")
        .production_door_phase = 2;
    shadow.sync_from_host(&logic);
    shadow.writeback_production_to_host(&mut logic);
    assert_eq!(shadow.world().world().entity_count(), before + 1);
    assert_eq!(host_production_ready_log::drain().len(), 1);
    // Process-global thread-local logs must not leak into the next producer
    // test (established clear-at-boundary pattern).
    crate::game_logic::host_production_log::clear();

    match prev_shadow {
        Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn completed_quantity_batch_emits_every_entity_first_unit_after_open_door() {
    let _env_guard = authority_env_lock();
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let _coupled = ShadowCoupleGuard::enter();

    use crate::game_logic::host_production_ready_log;
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionExitMetadata, ProductionExitStyle,
        ProductionItem, ProductionKind, Resources,
    };

    host_production_ready_log::clear();
    let mut logic = GameLogic::new();
    logic.set_production_authority(true);
    let cfg = golden_skirmish_config("DoorBatchProduction");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "DoorBatchRanger", 100.0);
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
            template_name: "DoorBatchRanger".to_string(),
            progress: 1.0,
            total_time: 1.0,
            construction_frames: 0,
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
        producer.production_door_phase_end_frame = hold_open_until;
        producer.building_data = Some(building);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let before = shadow.world().world().entity_count();
    shadow.writeback_production_to_host(&mut logic);
    let ready = host_production_ready_log::drain();
    assert_eq!(
        ready.len(),
        2,
        "one terminal QuantityModifier update must record every ready unit"
    );
    assert!(
        ready.iter().all(|event| {
            event.producer == producer_id
                && event.template_name == "DoorBatchRanger"
                && !event.is_upgrade
                && event.gw_entity_raw.is_some()
        }),
        "every batch member must have an entity-first bind: {ready:?}"
    );
    assert_eq!(
        shadow.world().world().entity_count(),
        before + 2,
        "one open-door terminal update must create both shadow entities"
    );

    host_production_ready_log::clear();
    match prev_shadow {
        Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn queue_exit_sole_tick_releases_one_then_waits_exact_nine_frames() {
    let _env_guard = authority_env_lock();
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let _coupled = ShadowCoupleGuard::enter();

    use crate::game_logic::host_production_progress_log;
    use crate::game_logic::host_production_ready_log;
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionExitMetadata, ProductionExitStyle,
        ProductionItem, ProductionKind, Resources,
    };

    host_production_progress_log::clear();
    host_production_ready_log::clear();
    let mut logic = GameLogic::new();
    logic.set_production_authority(true);
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
        producer.production_door_phase_end_frame = hold_open_until;
        producer.building_data = Some(building);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.writeback_production_to_host(&mut logic);
    let initial_ready = host_production_ready_log::drain();
    assert_eq!(
        initial_ready.len(),
        1,
        "fresh Queue InitialBurst=0 must admit only the first completed member"
    );

    // Model the real host-side successful exit/bind, then carry that exact
    // per-Object counter through the sole-tick progress event.
    let runtime = {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let exit = producer.thing.template.production_exit_metadata;
        let building = producer.building_data.as_mut().expect("building");
        building.record_successful_production_exit(exit.as_ref());
        building.production_exit_runtime_state()
    };
    host_production_progress_log::record_exit_runtime_only(
        producer_id,
        runtime.delay_frames as f32 / 30.0,
        runtime,
    );
    let events = host_production_progress_log::drain();
    assert_eq!(shadow.apply_host_production_progress_events(&events), 1);

    for _ in 0..8 {
        shadow.tick_production_queues(1.0 / 30.0);
    }
    shadow.writeback_production_to_host(&mut logic);
    assert!(
        host_production_ready_log::drain().is_empty(),
        "Queue delay must still hold the second member after eight logic frames"
    );

    shadow.tick_production_queues(1.0 / 30.0);
    shadow.writeback_production_to_host(&mut logic);
    assert_eq!(
        host_production_ready_log::drain().len(),
        1,
        "the ninth Queue update reopens the exit for exactly one member"
    );

    match prev_shadow {
        Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn construction_authority_sole_ticks_percent() {
    let _env_guard = authority_env_lock();

    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    // Tick path is authority-gated; sole-tick freeze is host-side (coupled frame).
    let mut logic = GameLogic::new();
    logic.set_construction_authority(true);
    assert!(gameworld_construction_authority_enabled());
    use crate::game_logic::host_construction_progress_log::{self};
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
    host_construction_progress_log::record(oid, 0.0, true, 0.25);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let events = host_construction_progress_log::drain();
    assert!(shadow.apply_host_construction_progress_events(&events) >= 1);
    let n = shadow.tick_construction_progress(2.0);
    assert!(n >= 1, "sole-tick must advance construction");
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let ent = shadow.world().entity(eid).expect("ent");
    // 0 + 0.25*2 = 0.5
    assert!(
        (ent.construction_percent - 0.5).abs() < 1e-4,
        "got {}",
        ent.construction_percent
    );
    assert!(shadow.writeback_construction_to_host(&mut logic) >= 1);
    let obj = logic.host_object(oid).expect("obj");
    assert!((obj.construction_percent - 0.5).abs() < 1e-4);
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn special_power_authority_sole_ticks_cooldown() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();
    use crate::game_logic::host_special_power_log::{self};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.set_special_power_authority(true);
    assert!(gameworld_special_power_sole_tick_enabled());
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
    host_special_power_log::record(oid, false, 4.0, 10.0, false);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let events = host_special_power_log::drain();
    assert!(shadow.apply_host_special_power_events(&events) >= 1);
    let n = shadow.tick_special_power_cooldowns(1.5);
    assert!(n >= 1, "sole-tick must advance SP cooldown");
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let ent = shadow.world().entity(eid).expect("ent");
    assert!(
        (ent.special_power_cooldown_remaining - 2.5).abs() < 1e-4,
        "got {}",
        ent.special_power_cooldown_remaining
    );
    assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
    let o = logic.host_object(oid).expect("obj");
    assert!((o.special_power_cooldown_remaining - 2.5).abs() < 1e-4);
    // Frozen residual: no advance while disabled.
    host_special_power_log::record(oid, false, 2.5, 10.0, true);
    let events = host_special_power_log::drain();
    assert!(shadow.apply_host_special_power_events(&events) >= 1);
    assert_eq!(shadow.tick_special_power_cooldowns(1.0), 0);
}

#[test]
fn shared_special_power_sole_ticks_player_cds() {
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    // Sole-tick tick path requires coupled engine frame (same as host freeze).
    begin_shadow_coupled_tick();
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::GameLogic;
    use crate::game_logic::host_player_cooldown_log;
    let mut logic = GameLogic::new();
    logic.set_special_power_authority(true);
    assert!(gameworld_special_power_sole_tick_enabled());
    let pid = 0u32;
    let Some(p) = logic.get_player_mut(pid) else {
        end_shadow_coupled_tick();
        match prev_s {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        return;
    };
    p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 5.0);
    p.record_host_cooldowns();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let events = host_player_cooldown_log::drain();
    assert!(
        shadow.apply_host_player_cooldown_events(&events) >= 1,
        "cooldown events applied"
    );
    let n = shadow.tick_player_shared_special_power_cooldowns(2.0);
    assert!(n >= 1, "player shared SP must sole-tick");
    let _ = shadow.writeback_shared_special_power_cooldowns_to_host(&mut logic);
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
    end_shadow_coupled_tick();
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn writeback_construction_percent_to_host() {
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
        e.under_construction = true;
        e.sold = true;
        e.reconstructing = true;
        e.unselectable = true;
        // Combat/status flags are NOT owned by construction writeback.
        e.stealthed = true;
        e.selected = true;
    }
    let n = shadow.writeback_construction_to_host(&mut logic);
    assert!(n >= 1);
    let obj = logic.host_objects().get(&id).expect("o");
    assert!((obj.construction_percent - 0.85).abs() < 1e-5);
    assert!(obj.status.under_construction);
    assert!(obj.status.sold);
    assert!(obj.status.reconstructing);
    assert!(obj.status.unselectable);
    // Construction writeback must not touch combat-status residual.
    assert!(!obj.status.stealthed);
    assert!(!obj.status.selected);
    // Dedicated combat-status writeback restores those flags.
    {
        let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
        e.stealthed = true;
        e.selected = true;
    }
    assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    let obj = logic.host_objects().get(&id).expect("o");
    assert!(obj.status.stealthed);
    assert!(obj.status.selected);
    // Complete residual
    {
        let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
        e.construction_percent = 1.0;
        e.under_construction = false;
    }
    let _ = shadow.writeback_construction_to_host(&mut logic);
    let obj = logic.host_objects().get(&id).expect("o");
    assert!((obj.construction_percent - 1.0).abs() < 1e-5);
    assert!(!obj.status.under_construction);
}
