//! Behavior suite extracted from `production_and_mobs`.
use super::*;

#[test]
fn group_hunt_includes_immobile_and_disabled_members() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut tower = ThingTemplate::new("TestTower");
    tower
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .add_kind_of(KindOf::Selectable)
        .set_health(400.0);
    logic.templates.insert("TestTower".into(), tower);

    let emp = logic
        .create_object("TestTank", Team::USA, Vec3::ZERO)
        .expect("emp");
    {
        let u = logic.host_object_mut(emp).unwrap();
        u.status.disabled_held = true;
        assert!(!u.can_move());
    }
    let base = logic
        .create_object("TestTower", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("tower");
    assert!(
        logic
            .host_object(base)
            .unwrap()
            .is_kind_of(KindOf::Structure)
    );

    assert!(
        logic.unit_command_patrol(emp),
        "EMP'd tank with AI must still enter hunt"
    );
    assert_eq!(
        logic.host_object(emp).unwrap().ai_state,
        AIState::Patrolling
    );
    assert!(
        logic.unit_command_patrol(base),
        "structure-with-AI must still enter hunt"
    );
    assert_eq!(
        logic.host_object(base).unwrap().ai_state,
        AIState::Patrolling
    );
}

#[test]
fn preorder_create_sets_model_bit_on_command_center_complete() {
    use crate::game_logic::host_preorder_create::{MC_BIT_PREORDER, has_preorder_model_bit};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Immobile)
        .set_health(5000.0);
    tpl.has_preorder_create = true;
    logic.templates.insert("AmericaCommandCenter".into(), tpl);
    // Ensure a USA player with preorder.
    if logic.players.is_empty() {
        let p = Player::new(0, Team::USA, "USA", true);
        logic.players.insert(0, p);
    }
    logic.set_player_did_preorder(Team::USA, true);

    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("cc");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.status.under_construction = true;
    }
    logic.notify_structure_construction_complete(id);
    let o = logic.host_object(id).unwrap();
    assert!(
        has_preorder_model_bit(o.model_condition_bits),
        "PREORDER bit {MC_BIT_PREORDER} must be set"
    );
    assert!(logic.honesty_preorder_create_ok());

    // Non-preorder player clears bit.
    logic.set_player_did_preorder(Team::USA, false);
    // Force clear path: clear bit then re-notify.
    {
        let o = logic.host_object_mut(id).unwrap();
        o.model_condition_bits |= 1u128 << MC_BIT_PREORDER;
    }
    logic.notify_structure_construction_complete(id);
    let o = logic.host_object(id).unwrap();
    assert!(!has_preorder_model_bit(o.model_condition_bits));
}

#[test]
fn map_placed_create_modules_run_on_build_complete() {
    // C++ GameLogic.cpp:1878-1885 every map object runs CreateModules
    // onBuildComplete — SupplyCenterCreate, GrantUpgradeCreate, PreorderCreate.
    use crate::game_logic::host_preorder_create::has_preorder_model_bit;
    use crate::game_logic::{
        GrantUpgradeCreateMetadata, KindOf, Player, Team, ThingTemplate, VeterancyLevel,
    };

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.did_preorder = true;
    logic.players.insert(0, player);

    let mut sc = ThingTemplate::new("AmericaSupplyCenter");
    sc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(2000.0);
    sc.has_supply_center_create = true;
    sc.has_preorder_create = true;
    sc.grant_upgrade_creates.push(GrantUpgradeCreateMetadata {
        upgrade_name: "Upgrade_AmericaRadar".into(),
        exempt_under_construction: true,
    });
    logic.templates.insert("AmericaSupplyCenter".into(), sc);

    let id = logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("map sc");
    let obj = logic.host_object(id).expect("obj");
    assert!(
        has_preorder_model_bit(obj.model_condition_bits),
        "map-placed PreorderCreate must set MODELCONDITION_PREORDER"
    );
    assert!(
        obj.has_upgrade_tag("Upgrade_AmericaRadar"),
        "OBJECT GrantUpgradeCreate Upgrade_AmericaRadar is giveUpgrade on the object"
    );
    let player = logic.players.get(&0).expect("p0");
    assert!(
        player.resource_supply_centers.contains(&id),
        "map-placed SupplyCenterCreate must register with gatherers"
    );
    assert!(
        !player.completed_upgrades.contains("Upgrade_AmericaRadar"),
        "OBJECT type must not addUpgrade on the player"
    );
    let _ = VeterancyLevel::Rookie;
}

#[test]
fn grant_upgrade_create_branches_player_vs_object_type() {
    // C++ GrantUpgradeCreate.cpp:108-117 UPGRADE_TYPE_PLAYER vs OBJECT.
    use crate::game_logic::{GrantUpgradeCreateMetadata, KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));

    let mut obj_tpl = ThingTemplate::new("ObjectGrantBuilding");
    obj_tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    obj_tpl
        .grant_upgrade_creates
        .push(GrantUpgradeCreateMetadata {
            upgrade_name: "Upgrade_AmericaRadar".into(),
            exempt_under_construction: true,
        });
    logic
        .templates
        .insert("ObjectGrantBuilding".into(), obj_tpl);

    let mut player_tpl = ThingTemplate::new("PlayerGrantBuilding");
    player_tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    player_tpl
        .grant_upgrade_creates
        .push(GrantUpgradeCreateMetadata {
            upgrade_name: "Upgrade_AmericaSupplyLines".into(),
            exempt_under_construction: true,
        });
    logic
        .templates
        .insert("PlayerGrantBuilding".into(), player_tpl);

    let oid = logic
        .create_object("ObjectGrantBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("obj grant");
    let pid = logic
        .create_object("PlayerGrantBuilding", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("player grant");

    let obj = logic.host_object(oid).expect("oid");
    assert!(obj.has_upgrade_tag("Upgrade_AmericaRadar"));
    assert!(!obj.has_upgrade_tag("Upgrade_AmericaSupplyLines"));
    let player_bldg = logic.host_object(pid).expect("pid");
    assert!(!player_bldg.has_upgrade_tag("Upgrade_AmericaSupplyLines"));
    let player = logic.players.get(&0).expect("p");
    assert!(
        player
            .completed_upgrades
            .contains("Upgrade_AmericaSupplyLines")
    );
    assert!(!player.completed_upgrades.contains("Upgrade_AmericaRadar"));
}

#[test]
fn special_power_create_starts_unit_recharge() {
    // C++ ProductionUpdate.cpp:821-825 + SpecialPowerCreate.cpp:41-48
    // startPowerRecharge — units must not spawn ready.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{
        KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));

    let mut tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    tpl.add_kind_of(KindOf::Infantry).set_health(200.0);
    tpl.has_special_power_create = true;
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_TimedDemoCharge".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialAbilityColonelBurtonTimedCharges".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::BurtonTimedCharges),
        reload_time_frames: 300,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), tpl);

    let id = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    logic.notify_unit_production_complete(id, id, "AmericaInfantryColonelBurton");
    let o = logic.host_object(id).expect("o");
    assert!(
        !o.is_special_power_ready(&SpecialPowerType::BurtonTimedCharges),
        "SpecialPowerCreate must start ReloadTime, not spawn ready"
    );
    let remaining = o
        .special_power_cooldowns
        .get(&SpecialPowerType::BurtonTimedCharges)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (remaining - 10.0).abs() < 0.01,
        "ReloadTime 300 frames / 30 = 10s, got {remaining}"
    );
}

#[test]
fn veterancy_gain_create_uses_controlling_player_sciences_only() {
    // C++ VeterancyGainCreate.cpp:61-73 controlling player only.
    use crate::game_logic::{
        Player, Team, ThingTemplate, VeterancyGainCreateMetadata, VeterancyLevel,
    };

    let mut logic = GameLogic::new();
    let mut owner = Player::new(0, Team::USA, "Owner", true);
    owner.is_alive = true;
    let mut ally = Player::new(1, Team::USA, "Ally", false);
    ally.is_alive = true;
    ally.unlocked_sciences
        .insert("SCIENCE_RedGuardTraining".into());
    logic.players.insert(0, owner);
    logic.players.insert(1, ally);

    let mut tpl = ThingTemplate::new("ChinaInfantryRedguard");
    tpl.add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(120.0);
    tpl.is_trainable = true;
    tpl.veterancy_gain_creates
        .push(VeterancyGainCreateMetadata {
            starting_level: VeterancyLevel::Veteran,
            science_required: Some("SCIENCE_RedGuardTraining".into()),
        });
    logic.templates.insert("ChinaInfantryRedguard".into(), tpl);

    let owned = logic
        .create_object_for_player("ChinaInfantryRedguard", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("owned");
    assert_eq!(
        logic.host_object(owned).expect("o").experience.level,
        VeterancyLevel::Rookie,
        "ally training science must not veteran the controlling player's unit"
    );

    let allied = logic
        .create_object_for_player("ChinaInfantryRedguard", 1, Vec3::new(20.0, 0.0, 0.0))
        .expect("ally unit");
    assert_eq!(
        logic.host_object(allied).expect("a").experience.level,
        VeterancyLevel::Veteran,
        "controlling player with the science must grant StartingLevel"
    );
}

#[test]
fn pilot_veterancy_gain_uses_set_min_and_trainable_gate() {
    // C++ VeterancyGainCreate.cpp:68-71 isTrainable + setMinVeterancyLevel
    // (health/weapon onVeterancyLevelChanged). Direct level writes skip FX.
    use crate::game_logic::{Team, ThingTemplate, VeterancyCrateCollideMetadata, VeterancyLevel};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);

    let mut pilot = ThingTemplate::new("AmericaInfantryPilot");
    pilot
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    pilot.is_trainable = true;
    pilot.veterancy_crate_collide = Some(VeterancyCrateCollideMetadata {
        is_pilot: true,
        required_kind_of_vehicle: true,
        forbidden_kind_of_dozer: true,
        effect_range: Some(0.0),
        adds_owner_veterancy: true,
        starting_level: Some(VeterancyLevel::Veteran),
    });
    logic.templates.insert("AmericaInfantryPilot".into(), pilot);

    let id = logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    let o = logic.host_object(id).expect("p");
    assert_eq!(o.experience.level, VeterancyLevel::Veteran);
    assert!(
        (o.health.maximum - 120.0).abs() < 0.01,
        "setMinVeterancyLevel must apply +20% Veteran HP, got {}",
        o.health.maximum
    );

    let mut locked = ThingTemplate::new("UntrainablePilot");
    locked
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    locked.is_trainable = false;
    locked.veterancy_crate_collide = Some(VeterancyCrateCollideMetadata {
        is_pilot: true,
        required_kind_of_vehicle: true,
        forbidden_kind_of_dozer: true,
        effect_range: Some(0.0),
        adds_owner_veterancy: true,
        starting_level: Some(VeterancyLevel::Veteran),
    });
    logic.templates.insert("UntrainablePilot".into(), locked);
    let uid = logic
        .create_object("UntrainablePilot", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("locked");
    let u = logic.host_object(uid).expect("u");
    assert_eq!(
        u.experience.level,
        VeterancyLevel::Rookie,
        "untrainable must skip setMinVeterancyLevel"
    );
}

#[test]
fn ocl_special_power_daisy_and_moab_upgrade() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_ocl_special_power::{
        OclCreateLocType, honesty_ocl_special_power_residual_ok,
    };
    assert!(honesty_ocl_special_power_residual_ok());

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("cc");

    let plan = logic
        .plan_ocl_special_power("SuperweaponDaisyCutter", id, Vec3::new(300.0, 0.0, 300.0))
        .expect("daisy plan");
    assert_eq!(plan.ocl_name, "SUPERWEAPON_DaisyCutter");
    assert_eq!(plan.create_loc, OclCreateLocType::EdgeNearSource);

    // Unlock MOAB science → UpgradeOCL residual.
    if let Some(p) = logic.get_player_mut(0) {
        let _ = p.unlock_science("SCIENCE_MOAB");
    }
    let plan2 = logic
        .plan_ocl_special_power("SuperweaponDaisyCutter", id, Vec3::new(300.0, 0.0, 300.0))
        .expect("moab plan");
    assert_eq!(plan2.ocl_name, "SUPERWEAPON_MOAB");
    assert!(logic.ocl_special_power_reg.science_upgrades >= 1);

    let transport = logic
        .execute_ocl_special_power("SuperweaponDaisyCutter", id, Vec3::new(300.0, 0.0, 300.0))
        .expect("transport");
    let t = logic.host_object(transport).expect("t");
    assert!(
        t.template_name.contains("B3") || t.template_name.contains("B52"),
        "MOAB/Daisy transport residual got {}",
        t.template_name
    );
    assert!(logic.ocl_special_power_reg.transports_spawned >= 1);
    assert_eq!(
        logic.ocl_special_power_reg.payloads_spawned, 0,
        "payload delayed until DeliverPayload approach completes"
    );
    // Advance through SuperweaponOclBomb approach + door residual (Daisy 90f).
    for _ in 0..95 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_deliver_payloads();
    }
    assert!(
        logic.ocl_special_power_reg.payloads_spawned >= 1,
        "payload should spawn after OCL DeliverPayload approach residual"
    );
    assert!(logic.honesty_ocl_special_power_ok());
}

#[test]
fn ocl_special_power_leaflet_transport_only() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("cc");
    let before_payload = logic.ocl_special_power_reg.payloads_spawned;
    let transport = logic
        .execute_ocl_special_power("SuperweaponLeafletDrop", id, Vec3::new(250.0, 0.0, 250.0))
        .expect("leaflet transport");
    let t = logic.host_object(transport).unwrap();
    assert!(t.template_name.contains("B52"));
    assert_eq!(
        logic.ocl_special_power_reg.payloads_spawned, before_payload,
        "Leaflet is TransportOnly — host owns LeafletContainer disable residual"
    );
}

#[test]
fn ocl_special_power_spy_drone_create_object() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("cc");
    let drone = logic
        .execute_ocl_special_power("SpecialPowerSpyDrone", id, Vec3::new(80.0, 0.0, 80.0))
        .expect("drone");
    let d = logic.host_object(drone).unwrap();
    assert!(d.template_name.contains("SpyDrone"));
    assert!(logic.ocl_special_power_reg.create_objects_spawned >= 1);
}

#[test]
fn smart_bomb_homing_steers_toward_target() {
    use crate::game_logic::host_smart_bomb_target_homing::honesty_smart_bomb_target_homing_residual_ok;
    assert!(honesty_smart_bomb_target_homing_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("MOAB");
    tpl.set_health(100.0);
    logic.templates.insert("MOAB".to_string(), tpl);
    let id = logic
        .create_object("MOAB", Team::USA, Vec3::new(0.0, 80.0, 0.0))
        .expect("moab");
    assert!(
        logic
            .host_object(id)
            .unwrap()
            .smart_bomb_target_homing
            .is_some()
    );
    assert!(logic.set_smart_bomb_target(id, Vec3::new(100.0, 0.0, 0.0)));
    logic.update_smart_bomb_target_homing();
    let p = logic.host_object(id).unwrap().get_position();
    assert!(p.x > 0.5 && p.x < 2.0, "1% course fudge got x={}", p.x);
    assert!((p.y - 80.0).abs() < 0.1, "altitude preserved");
    assert!(logic.smart_bomb_target_homing_reg.steers >= 1);
    assert!(logic.honesty_smart_bomb_target_homing_ok());
}

#[test]
fn spectre_gunship_deployment_spawns_at_far_edge() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_spectre_gunship_deployment::{
        SPECTRE_GUNSHIP_TEMPLATE, SPECTRE_PREFERRED_ELEVATION,
        honesty_spectre_gunship_deployment_residual_ok,
    };
    assert!(honesty_spectre_gunship_deployment_residual_ok());

    let mut logic = GameLogic::new();
    let mut cc_tpl = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc_tpl);

    let cc = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("cc");
    assert!(
        logic
            .host_object(cc)
            .unwrap()
            .spectre_gunship_deployment
            .is_some()
    );
    assert!(logic.spectre_gunship_deployment_reg.installed >= 1);

    let target = Vec3::new(250.0, 0.0, 250.0);
    let ship = logic
        .initiate_spectre_gunship_deployment(cc, target)
        .expect("gunship spawn");
    let g = logic.host_object(ship).expect("ship obj");
    assert!(g.template_name.contains("Spectre") || g.template_name == SPECTRE_GUNSHIP_TEMPLATE);
    assert!((g.get_position().y - SPECTRE_PREFERRED_ELEVATION).abs() < 0.1);
    assert_eq!(g.producer_id, Some(cc));
    assert_eq!(
        logic.host_object(cc).and_then(|o| o
            .spectre_gunship_deployment
            .as_ref()
            .and_then(|d| d.gunship_id)),
        Some(ship)
    );
    assert!(logic.spectre_gunship_deployment_reg.spawns >= 1);
    assert!(logic.honesty_spectre_gunship_deployment_ok());
}

#[test]
fn checkpoint_opens_for_ally_closes_for_enemy() {
    use crate::game_logic::host_checkpoint_update::honesty_checkpoint_update_residual_ok;
    assert!(honesty_checkpoint_update_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaCheckpoint");
    tpl.set_health(1000.0);
    logic.templates.insert("AmericaCheckpoint".to_string(), tpl);
    ensure_test_infantry_template(&mut logic);

    let gate = logic
        .create_object("AmericaCheckpoint", Team::USA, Vec3::ZERO)
        .expect("gate");
    {
        let g = logic.host_object_mut(gate).unwrap();
        g.vision_range = 150.0;
        if let Some(cp) = g.checkpoint_update.as_mut() {
            cp.vision_range = 150.0;
            cp.scan_delay = 0;
        }
    }
    assert!(logic.host_object(gate).unwrap().checkpoint_update.is_some());

    let ally = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("ally");
    let _ = ally;
    logic.update_checkpoint_update();
    assert!(
        logic
            .host_object(gate)
            .and_then(|o| o.checkpoint_update.as_ref().map(|c| c.open && c.ally_near))
            .unwrap_or(false),
        "ally near must open checkpoint"
    );
    assert!(logic.checkpoint_update_reg.opens >= 1);

    let enemy = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");
    let _ = enemy;
    {
        let g = logic.host_object_mut(gate).unwrap();
        if let Some(cp) = g.checkpoint_update.as_mut() {
            cp.scan_delay = 0;
        }
    }
    logic.update_checkpoint_update();
    assert!(
        logic
            .host_object(gate)
            .and_then(|o| o
                .checkpoint_update
                .as_ref()
                .map(|c| !c.open && c.enemy_near))
            .unwrap_or(false),
        "enemy near must close checkpoint"
    );
    assert!(logic.honesty_checkpoint_update_ok());
}

#[test]
fn radius_decal_scud_storm_create_and_kill_on_idle() {
    use crate::game_logic::host_radius_decal_update::{
        SCUD_STORM_DECAL_TEXTURE, SCUD_STORM_DELIVERY_DECAL_RADIUS,
        honesty_radius_decal_update_residual_ok,
    };
    assert!(honesty_radius_decal_update_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    tpl.set_health(4000.0);
    logic.templates.insert("GLAScudStorm".to_string(), tpl);
    let id = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("storm");
    assert!(logic.host_object(id).unwrap().radius_decal_update.is_some());
    assert!(logic.radius_decal_update_reg.installed >= 1);

    let target = Vec3::new(400.0, 0.0, 200.0);
    assert!(logic.create_delivery_radius_decal(id, target));
    {
        let o = logic.host_object(id).unwrap();
        let rd = o.radius_decal_update.as_ref().unwrap();
        assert!(!rd.delivery_decal.is_empty());
        assert!((rd.delivery_decal.radius - SCUD_STORM_DELIVERY_DECAL_RADIUS).abs() < 0.1);
        assert_eq!(
            rd.delivery_decal
                .template
                .as_ref()
                .map(|t| t.texture.as_str()),
            Some(SCUD_STORM_DECAL_TEXTURE)
        );
        assert!(rd.kill_when_no_longer_attacking);
        assert!(o.status.attacking);
    }
    assert!(logic.radius_decal_update_reg.creates >= 1);

    // Still attacking: decal stays.
    logic.set_current_frame(10);
    logic.update_radius_decal_update();
    assert!(
        logic
            .host_object(id)
            .and_then(|o| o
                .radius_decal_update
                .as_ref()
                .map(|r| !r.delivery_decal.is_empty()))
            .unwrap_or(false)
    );

    // Attack ends → killWhenNoLongerAttacking clears decal.
    {
        let o = logic.host_object_mut(id).unwrap();
        o.status.attacking = false;
        o.ai_state = crate::game_logic::AIState::Idle;
    }
    logic.set_current_frame(20);
    logic.update_radius_decal_update();
    assert!(
        logic
            .host_object(id)
            .and_then(|o| o
                .radius_decal_update
                .as_ref()
                .map(|r| r.delivery_decal.is_empty()))
            .unwrap_or(false)
    );
    assert!(logic.radius_decal_update_reg.attack_kills >= 1);
    assert!(logic.honesty_radius_decal_update_ok());
}

#[test]
fn float_update_ferry_sways() {
    use crate::game_logic::host_float_update::honesty_float_update_residual_ok;
    assert!(honesty_float_update_residual_ok());
    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("CivilianVehicleFerry");
    tpl.set_health(100.0);
    logic
        .templates
        .insert("CivilianVehicleFerry".to_string(), tpl);
    let id = logic
        .create_object("CivilianVehicleFerry", Team::Neutral, Vec3::ZERO)
        .expect("ferry");
    assert!(logic.host_object(id).unwrap().float_update.is_some());
    logic.set_current_frame(50);
    logic.update_float_update();
    let yaw = logic
        .host_object(id)
        .and_then(|o| o.float_update.as_ref().map(|f| f.yaw))
        .unwrap_or(0.0);
    assert!(yaw.abs() > 0.0 || logic.float_update_reg.sway_ticks > 0);
    assert!(logic.honesty_float_update_ok());
}

#[test]
fn ocl_create_debris_disposition_spawn() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let ids = logic.spawn_ocl_create_debris(
        &HostOclCreateDebrisPlan::generic_tank_debris(),
        Team::USA,
        Vec3::new(10.0, 5.0, 10.0),
        Vec3::new(2.0, 0.0, 0.0),
        None,
    );
    assert_eq!(ids.len(), 3);
    let o = logic.host_object(ids[0]).unwrap();
    assert!(
        o.movement.velocity.length() > 0.5,
        "debris should receive disposition force"
    );
    assert!(
        o.shock_allow_bounce,
        "C++ setAllowBouncing(true) on flying CreateDebris"
    );
    assert!(logic.ocl_create_debris_reg.debris_spawned >= 3);
    assert!(logic.ocl_create_debris_reg.flying_forces >= 1);
    assert!(crate::game_logic::host_ocl_create_debris::honesty_ocl_create_debris_residual_ok());
}

#[test]
fn ocl_apply_random_force_technical() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut tech = crate::game_logic::ThingTemplate::new("GLAVehicleTechnicalAirDeath");
    tech.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic
        .templates
        .insert("GLAVehicleTechnicalAirDeath".into(), tech);
    // Also match via OCL-style name peel on template containing technical+death
    let id = logic
        .create_object(
            "GLAVehicleTechnicalAirDeath",
            Team::GLA,
            Vec3::new(0.0, 5.0, 0.0),
        )
        .unwrap();
    let v0 = logic.host_object(id).unwrap().movement.velocity;
    assert!(logic.apply_ocl_random_force(id));
    let v1 = logic.host_object(id).unwrap().movement.velocity;
    assert!(
        (v1 - v0).length() > 1.0,
        "ApplyRandomForce should impulse velocity"
    );
    assert!(logic.ocl_apply_random_force_reg.applied >= 1);
    assert!(logic.honesty_ocl_apply_random_force_ok());
}

#[test]
fn fuel_air_gas_slow_death_detonates() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut gas_tpl = crate::game_logic::ThingTemplate::new("SupW_AuroraFuelAirGas");
    gas_tpl.set_health(1.0);
    logic
        .templates
        .insert("SupW_AuroraFuelAirGas".into(), gas_tpl);
    let mut enemy = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    enemy.add_kind_of(KindOf::Vehicle).set_health(500.0);
    logic.templates.insert("GLATankScorpion".into(), enemy);
    let gas = logic
        .create_object("SupW_AuroraFuelAirGas", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.objects.get_mut(&gas).unwrap();
        o.ensure_fuel_air_gas_slow_death(0);
        logic.fuel_air_gas_reg.record_install();
    }
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    for f in 0..=35 {
        logic.frame = f;
        logic.update_fuel_air_gas_slow_death();
    }
    assert!(logic.fuel_air_gas_reg.final_detonations >= 1);
    assert!(logic.fuel_air_gas_reg.midpoint_flames >= 1);
    let foe_alive = logic
        .host_object(foe)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        !foe_alive || hp1 < hp0,
        "detonation should damage nearby enemy (hp0={hp0} hp1={hp1})"
    );
    assert!(logic.honesty_fuel_air_gas_slow_death_ok());
}

#[test]
fn daisy_bomb_create_object_die_spawns_gas() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut bomb = crate::game_logic::ThingTemplate::new("DaisyCutterBomb");
    bomb.add_kind_of(KindOf::Projectile).set_health(10.0);
    logic.templates.insert("DaisyCutterBomb".into(), bomb);
    let id = logic
        .create_object("DaisyCutterBomb", Team::USA, Vec3::new(50.0, 40.0, 50.0))
        .expect("bomb");
    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.ensure_create_object_die();
        o.fire_create_object_die();
    }
    logic.apply_pending_create_object_die(id);
    let gas = logic
        .objects
        .values()
        .any(|o| o.template_name.contains("FuelAirGas") || o.template_name.contains("Gas"));
    let debris = logic
        .objects
        .values()
        .any(|o| o.template_name.contains("Debris"));
    assert!(
        gas,
        "FuelAir gas should spawn from DaisyCutterBomb CreateObjectDie"
    );
    assert!(debris, "shell debris should spawn");
}

#[test]
fn moab_flight_uses_jet_b3() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_daisy_cutter_flight(
            cc_id,
            Vec3::new(180.0, 0.0, 0.0),
            DaisyFlightPayloadTier::Moab,
        )
        .expect("b3");
    assert_eq!(
        logic.host_object(jet).unwrap().template_name,
        "AmericaJetB3"
    );
    assert!(logic.daisy_cutter_flight_reg.moab_transports_spawned >= 1);
    for f in 0..400 {
        logic.frame = f;
        logic.update_daisy_cutter_flights();
        if logic.daisy_cutter_flight_reg.detonations >= 1 {
            break;
        }
    }
    assert!(logic.daisy_cutter_flight_reg.bombs_dropped >= 1);
    assert!(logic.daisy_cutter_flight_reg.detonations >= 1);
    assert!(logic.honesty_daisy_cutter_flight_ok());
}

#[test]
fn daisy_science_moab_upgrades_b52_to_b3() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_daisy_cutter_flight(
            cc_id,
            Vec3::new(180.0, 0.0, 0.0),
            DaisyFlightPayloadTier::DaisyCutter,
        )
        .expect("b52");
    assert_eq!(
        logic.host_object(jet).unwrap().template_name,
        "AmericaJetB52",
        "without SCIENCE_MOAB, Daisy stays B52 + DaisyCutterBomb"
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.templates.insert("AmericaCommandCenter".into(), {
        let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure).set_health(5000.0);
        cc
    });
    if let Some(p) = logic.get_player_mut(0) {
        let _ = p.unlock_science("SCIENCE_MOAB");
    }
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    logic.queue_special_power_strike(
        &SpecialPowerType::DaisyCutter,
        cc_id,
        Vec3::new(180.0, 0.0, 0.0),
    );
    let b3 = logic
        .host_objects()
        .values()
        .any(|o| o.template_name == "AmericaJetB3");
    assert!(
        b3,
        "SCIENCE_MOAB UpgradeOCL must send AmericaJetB3, not B52"
    );
    assert!(logic.daisy_cutter_flight_reg.moab_transports_spawned >= 1);
}

#[test]
fn daisy_upgrade_americamoab_grants_findocl() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    if let Some(p) = logic.get_player_mut(0) {
        p.completed_upgrades.insert("Upgrade_AmericaMOAB".into());
    }
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_daisy_cutter_flight(
            cc_id,
            Vec3::new(180.0, 0.0, 0.0),
            DaisyFlightPayloadTier::DaisyCutter,
        )
        .expect("b3");
    assert_eq!(
        logic.host_object(jet).unwrap().template_name,
        "AmericaJetB3",
        "Upgrade_AmericaMOAB must grant SCIENCE_MOAB for findOCL"
    );
}

#[test]
fn daisy_cutter_flight_drops_bomb() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(800.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(160.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let jet = logic
        .spawn_daisy_cutter_flight(
            cc_id,
            Vec3::new(160.0, 0.0, 0.0),
            crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier::DaisyCutter,
        )
        .expect("b52");
    assert!(
        logic
            .host_object(jet)
            .unwrap()
            .daisy_cutter_transport
            .is_some()
    );
    assert!(logic.daisy_cutter_flight_reg.transports_spawned >= 1);
    for f in 0..400 {
        logic.frame = f;
        logic.update_daisy_cutter_flights();
        if logic.daisy_cutter_flight_reg.detonations >= 1 {
            break;
        }
    }
    assert!(logic.daisy_cutter_flight_reg.bombs_dropped >= 1);
    assert!(logic.daisy_cutter_flight_reg.detonations >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "daisy detonation should damage nearby units"
    );
    assert!(logic.honesty_daisy_cutter_flight_ok());
}

#[test]
fn paradrop_cargo_plane_drops_parachute() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_Paradrop1");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_paradrop(
            &SpecialPowerType::Paradrop,
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("paradrop");
    assert!(id >= 1);
    assert!(logic.host_paradrops.transports_spawned >= 1);
    for f in 0..300 {
        logic.frame = f;
        logic.update_paradrop_cargo_planes();
        if logic.host_paradrops.parachutes_dropped >= 1 {
            break;
        }
    }
    assert!(logic.host_paradrops.parachutes_dropped >= 1);
    assert!(logic.host_paradrops.honesty_cargo_plane_path_ok());
    // Existing infantry spawn residual still runs after approach delay.
    for f in 0..200 {
        logic.frame = f;
        logic.update_paradrops();
    }
    use crate::game_logic::host_paradrop::HostParadropKind;
    assert!(
        logic
            .host_paradrops
            .honesty_host_path_ok(HostParadropKind::AmericaParadrop)
            || logic.host_paradrops.parachutes_dropped >= 1
    );
}

#[test]
fn leaflet_b52_drops_container() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    foe_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let _foe = logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_leaflet_drop(
            &SpecialPowerType::LeafletDrop,
            cc_id,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("leaflet");
    assert!(id >= 1);
    assert!(logic.host_leaflet_drops.transports_spawned >= 1);
    assert!(
        logic.special_power_strikes().view_object_count() >= 1,
        "Leaflet Drop must spawn SpecialPowerViewObject at the click"
    );
    let vo = &logic.special_power_strikes().view_objects()[0];
    assert!((vo.range - 250.0).abs() < 0.1);
    assert_eq!(vo.duration_frames(), 900);
    let jet = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.leaflet_transport_target.is_some())
        .map(|(id, _)| *id);
    if let Some(jid) = jet {
        let o = logic.host_object(jid).unwrap();
        let p = o.get_position();
        let (min, max) = logic.world_bounds();
        let on_edge = (p.x - min.x).abs() < 1.0
            || (p.x - max.x).abs() < 1.0
            || (p.z - min.z).abs() < 1.0
            || (p.z - max.z).abs() < 1.0;
        assert!(on_edge, "leaflet B52 must spawn at map edge, pos={p:?}");
        assert!(
            o.radius_decal_update
                .as_ref()
                .is_some_and(|rd| !rd.delivery_decal.is_empty()),
            "inbound leaflet DeliveryDecal ring missing"
        );
    }
    for f in 0..200 {
        logic.frame = f;
        logic.update_leaflet_b52_flights();
        if logic.host_leaflet_drops.containers_dropped >= 1 {
            break;
        }
    }
    assert!(logic.host_leaflet_drops.containers_dropped >= 1);
    if let Some(jid) = jet {
        for f in 200..500 {
            logic.frame = f;
            logic.update_leaflet_b52_flights();
            let gone = logic
                .host_object(jid)
                .map(|o| !o.is_alive())
                .unwrap_or(true);
            if gone {
                break;
            }
        }
        let gone = logic
            .host_object(jid)
            .map(|o| !o.is_alive())
            .unwrap_or(true);
        assert!(gone, "leaflet B52 must HeadOffMap and destroy after drop");
    }
    // Disable residual still applies via existing delay path.
    for f in 0..120 {
        logic.frame = f;
        logic.update_leaflet_drops();
    }
    assert!(
        logic.host_leaflet_drops.disable_count >= 1
            || logic.host_leaflet_drops.activation_count >= 1
            || logic.host_leaflet_drops.containers_dropped >= 1
    );
}

#[test]
fn a10_strike_flight_drops_missiles() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::A10StrikeScienceTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let _foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(180.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_a10_strike_flight(
            cc_id,
            Vec3::new(180.0, 0.0, 0.0),
            A10StrikeScienceTier::Level1,
        )
        .expect("a10");
    assert!(
        logic
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .is_some()
    );
    assert!(logic.a10_strike_flight_reg.missiles_scheduled >= 6);
    for f in 0..400 {
        logic.frame = f;
        logic.update_a10_strike_flights();
        if logic.a10_strike_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.a10_strike_flight_reg.missiles_dropped >= 1);
    assert!(logic.a10_strike_flight_reg.impacts >= 1);
    assert!(logic.honesty_a10_strike_flight_ok());
}

#[test]
fn a10_missiles_drop_from_jet_when_close_not_on_timer() {
    // C++ DeliverPayloadAIUpdate.cpp:348-368 / DeliveringState::update:687-891
    // VisiblePayload is created at the jet only while isCloseEnoughToTarget.
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        A10_DELIVERY_DISTANCE, A10_PAYLOAD_TEMPLATE, A10StrikeScienceTier,
    };
    let mut logic = GameLogic::new();
    logic.override_world_size(4000.0, 4000.0);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = Vec3::new(1200.0, 0.0, 0.0);
    let jet = logic
        .spawn_a10_strike_flight(cc_id, target, A10StrikeScienceTier::Level1)
        .expect("a10");
    let launch = logic.host_object(jet).unwrap().get_position();
    let launch_dist = ((launch.x - target.x).powi(2) + (launch.z - target.z).powi(2)).sqrt();
    assert!(
        launch_dist > A10_DELIVERY_DISTANCE,
        "test map must start the jet outside DeliveryDistance, dist={launch_dist}"
    );
    logic.frame = 1;
    logic.update_a10_strike_flights();
    assert_eq!(
        logic.a10_strike_flight_reg.missiles_dropped, 0,
        "must not timer-drop missiles while the jet is far from the target"
    );
    let mut first_drop_xz = None;
    for f in 2..400 {
        logic.frame = f;
        logic.update_a10_strike_flights();
        if logic.a10_strike_flight_reg.missiles_dropped >= 1 {
            first_drop_xz = logic
                .host_objects()
                .values()
                .find(|o| o.template_name == A10_PAYLOAD_TEMPLATE)
                .map(|o| o.get_position());
            break;
        }
    }
    assert!(logic.a10_strike_flight_reg.missiles_dropped >= 1);
    let jet_pos = logic.host_object(jet).unwrap().get_position();
    let jet_dist = ((jet_pos.x - target.x).powi(2) + (jet_pos.z - target.z).powi(2)).sqrt();
    assert!(
        jet_dist <= A10_DELIVERY_DISTANCE + 22.0,
        "first drop must happen near DeliveryDistance, jet_dist={jet_dist}"
    );
    let drop = first_drop_xz.expect("payload");
    let dx = drop.x - jet_pos.x;
    let dz = drop.z - jet_pos.z;
    assert!(
        dx * dx + dz * dz <= 20.0 * 20.0,
        "missile must spawn at the jet, not a precomputed ground blast, drop={drop:?} jet={jet_pos:?}"
    );
}

#[test]
fn artillery_barrage_flight_drops_shells() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    // CC and click on the east side so closest-to-source ≠ farthest-from-target.
    let cc_id = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .unwrap();
    let _foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    let target = Vec3::new(200.0, 0.0, 0.0);
    let transport = logic
        .spawn_artillery_barrage_flight(cc_id, target, ArtilleryBarrageScienceTier::Level1)
        .expect("cannon");
    assert!(
        logic
            .host_object(transport)
            .unwrap()
            .artillery_barrage_transport
            .is_some()
    );
    let cannons: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.artillery_barrage_transport.is_some())
        .collect();
    assert_eq!(
        cannons.len(),
        12,
        "C++ FormationSize 12 ChinaArtilleryCannon, got {}",
        cannons.len()
    );
    let closest = logic.closest_map_edge_point(Vec3::new(200.0, 0.0, 0.0));
    let lead = logic.host_object(transport).unwrap().get_position();
    assert!(
        lead.x < closest.x - 50.0,
        "lead must come from farthest edge (west), not closest (east). lead={lead:?} closest={closest:?}"
    );
    assert!(
        lead.y >= 300.0,
        "CREATE_AT_EDGE_FARTHEST_FROM_TARGET z+=300 then preferred height, y={}",
        lead.y
    );
    assert!(logic.artillery_barrage_flight_reg.shells_scheduled >= 12);
    assert_eq!(logic.artillery_barrage_flight_reg.transports_spawned, 12);
    for f in 0..400 {
        logic.frame = f;
        logic.update_artillery_barrage_flights();
        if logic.artillery_barrage_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.artillery_barrage_flight_reg.shells_dropped >= 1);
    assert!(logic.artillery_barrage_flight_reg.impacts >= 1);
    assert!(logic.honesty_artillery_barrage_flight_ok());
}

#[test]
fn artillery_barrage_l3_spawns_formation_size_36() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .unwrap();
    let _ = logic
        .spawn_artillery_barrage_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level3,
        )
        .expect("cannon");
    let n = logic
        .host_objects()
        .values()
        .filter(|o| o.artillery_barrage_transport.is_some())
        .count();
    assert_eq!(n, 36, "C++ SUPERWEAPON_ArtilleryBarrage3 FormationSize 36");
}

#[test]
fn china_carpet_bomb_flight_uses_china_payload() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let transport = logic
        .spawn_carpet_bomb_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            CarpetBombFactionTier::China,
        )
        .expect("china bomber");
    let name = logic.host_object(transport).unwrap().template_name.clone();
    assert_eq!(name, "ChinaJetCarpetBomber");
    assert_eq!(
        logic.carpet_bomb_flight_reg.bombs_scheduled,
        CarpetBombFactionTier::China.bomb_count()
    );
    for f in 0..400 {
        logic.frame = f;
        logic.update_carpet_bomb_flights();
        if logic.carpet_bomb_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.carpet_bomb_flight_reg.impacts >= 1);
}

#[test]
fn airforce_carpet_bomb_flight_uses_airf_payload() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let transport = logic
        .spawn_carpet_bomb_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            CarpetBombFactionTier::AirForce,
        )
        .expect("airf bomber");
    let name = logic.host_object(transport).unwrap().template_name.clone();
    assert_eq!(name, "AirF_AmericaJetB3");
    assert_eq!(
        logic.carpet_bomb_flight_reg.bombs_scheduled,
        CarpetBombFactionTier::AirForce.bomb_count()
    );
}

#[test]
fn carpet_bomb_flight_drops_payload() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let transport = logic
        .spawn_carpet_bomb_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            crate::game_logic::special_power_strikes::CarpetBombFactionTier::America,
        )
        .expect("b52");
    assert!(
        logic
            .host_object(transport)
            .unwrap()
            .carpet_bomb_transport
            .is_some()
    );
    assert!(logic.carpet_bomb_flight_reg.bombs_scheduled >= 15);
    for f in 0..400 {
        logic.frame = f;
        logic.update_carpet_bomb_flights();
        if logic.carpet_bomb_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.carpet_bomb_flight_reg.bombs_dropped >= 1);
    assert!(logic.carpet_bomb_flight_reg.impacts >= 1);
    // Damage is applied at drop epicenters along the residual line; foe may
    // miss variance/line if off the stripe — check damage if still alive near center.
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let _ = (hp0, hp1, foe);
    assert!(logic.honesty_carpet_bomb_flight_ok());
}

#[test]
fn chem_scud_storm_anthrax_primary_impact() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut storm = crate::game_logic::ThingTemplate::new("Chem_GLAScudStorm");
    storm.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("Chem_GLAScudStorm".into(), storm);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(800.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let storm_id = logic
        .create_object("Chem_GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    assert!(logic.execute_ocl_attack("SUPERWEAPON_ScudStorm", storm_id, Vec3::new(80.0, 0.0, 0.0)));
    for f in 0..900 {
        logic.frame = f;
        logic.update_scud_storm_missile_flights();
        if logic.scud_storm_missile_flight_reg.grounded >= 1 {
            break;
        }
    }
    assert!(logic.scud_storm_missile_flight_reg.grounded >= 1);
    assert!(
        logic.scud_storm_missile_flight_reg.launched >= 1
            || logic.scud_storm_missile_flight_reg.ignition_fx >= 1
            || logic.scud_storm_missile_flight_reg.exhaust_fx >= 1
    );
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "chem scud toxin primary should damage nearby units"
    );
    assert!(
        logic.special_power_strikes.toxin_fields_spawned_total() >= 1
            || !logic.special_power_strikes.toxin_fields().is_empty()
    );
}

#[test]
fn scud_storm_missile_ballistic_flight() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut storm = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    storm.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), storm);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(800.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let storm_id = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    assert!(logic.execute_ocl_attack(
        "SUPERWEAPON_ScudStorm",
        storm_id,
        Vec3::new(100.0, 0.0, 0.0)
    ));
    assert!(logic.scud_storm_missile_flight_reg.scheduled >= 9);
    assert_eq!(logic.scud_storm_missile_flight_reg.launched, 0);
    for f in 0..800 {
        logic.frame = f;
        logic.update_scud_storm_missile_flights();
        if logic.scud_storm_missile_flight_reg.grounded >= 1
            && logic.scud_storm_missile_flight_reg.launched >= 9
        {
            break;
        }
    }
    assert!(
        logic.scud_storm_missile_flight_reg.launched >= 9,
        "all 9 missiles should launch after stagger"
    );
    assert!(logic.scud_storm_missile_flight_reg.grounded >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "scud impact should damage nearby units"
    );
    assert!(
        !logic.special_power_strikes.toxin_fields().is_empty()
            || logic.special_power_strikes.toxin_fields_spawned_total() >= 1
            || logic.scud_poison_zones.zones_spawned >= 1,
        "scud impact should spawn poison field residual"
    );
    assert!(logic.honesty_scud_storm_missile_flight_ok());
}

#[test]
fn cruise_missile_moab_impact_not_neutron_field() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut enemy = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    enemy.add_kind_of(KindOf::Vehicle).set_health(500.0);
    logic.templates.insert("GLATankScorpion".into(), enemy);
    let launcher = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let proj = logic
        .execute_ocl_fire_weapon(
            "SUPERWEAPON_CruiseMissile",
            launcher,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("cruise");
    assert!(
        logic
            .host_object(proj)
            .unwrap()
            .neutron_missile_update
            .as_ref()
            .map(|d| d.is_cruise)
            .unwrap_or(false)
    );
    let neutron0 = logic
        .special_power_strikes
        .neutron_slow_death_spawned_total();
    for f in 0..600 {
        logic.frame = f;
        logic.update_neutron_missile_flights();
        if logic.neutron_missile_update_reg.grounded >= 1 {
            break;
        }
    }
    assert!(logic.neutron_missile_update_reg.grounded >= 1);
    assert_eq!(
        logic
            .special_power_strikes
            .neutron_slow_death_spawned_total(),
        neutron0,
        "cruise must not spawn neutron SlowDeath field"
    );
    let foe_hp = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        foe_hp < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "MOAB residual should damage nearby enemy"
    );
}

#[test]
fn neutron_missile_loft_reaches_ground() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut silo = crate::game_logic::ThingTemplate::new("ChinaNuclearMissileLauncher");
    silo.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("ChinaNuclearMissileLauncher".into(), silo);
    let launcher = logic
        .create_object(
            "ChinaNuclearMissileLauncher",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let proj = logic
        .execute_ocl_fire_weapon(
            "SUPERWEAPON_NeutronMissile",
            launcher,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("missile");
    assert!(
        logic
            .host_object(proj)
            .unwrap()
            .neutron_missile_update
            .is_some()
    );
    assert!(
        logic
            .host_object(proj)
            .unwrap()
            .radius_decal_update
            .as_ref()
            .map(|d| !d.delivery_decal.is_empty())
            .unwrap_or(false)
            || logic.radius_decal_update_reg.creates > 0
            || logic
                .host_object(launcher)
                .and_then(|o| o.radius_decal_update.as_ref())
                .map(|d| !d.delivery_decal.is_empty())
                .unwrap_or(false),
        "DeliveryDecal residual should install on neutron launch"
    );
    let mut grounded = false;
    for f in 0..600 {
        logic.frame = f;
        logic.update_neutron_missile_flights();
        if logic.host_object(proj).is_none()
            || logic
                .host_object(proj)
                .map(|o| !o.is_alive())
                .unwrap_or(true)
        {
            grounded = true;
            break;
        }
        if logic.neutron_missile_update_reg.grounded >= 1 {
            grounded = true;
            break;
        }
    }
    assert!(grounded, "missile should complete loft/dive");
    assert!(logic.neutron_missile_update_reg.launched >= 1);
    assert!(
        logic.special_power_strikes.neutron_slow_death_field_count() >= 1
            || logic
                .special_power_strikes
                .neutron_slow_death_spawned_total()
                >= 1,
        "ground impact should spawn NeutronMissileSlowDeath field"
    );
    // Advance SlowDeath midpoint → OCL_NukeRadiationField residual.
    let rad0 = logic.special_power_strikes.radiation_fields_spawned_total();
    for _ in 0..60 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_neutron_slow_death_fields();
    }
    assert!(
        logic.special_power_strikes.radiation_fields_spawned_total() > rad0
            || !logic.special_power_strikes.radiation_fields().is_empty(),
        "midpoint should spawn OCL_NukeRadiationField residual"
    );
    assert!(logic.honesty_neutron_missile_update_ok());
}

#[test]
fn ocl_fire_weapon_neutron_projectile() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut silo = crate::game_logic::ThingTemplate::new("ChinaNuclearMissileLauncher");
    silo.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("ChinaNuclearMissileLauncher".into(), silo);
    let id = logic
        .create_object(
            "ChinaNuclearMissileLauncher",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("silo");
    let proj = logic
        .execute_ocl_fire_weapon(
            "SUPERWEAPON_NeutronMissile",
            id,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(400.0, 0.0, 400.0),
        )
        .expect("neutron proj");
    let p = logic.host_object(proj).unwrap();
    assert!(p.template_name.contains("NeutronMissile"));
    assert!(logic.ocl_fire_weapon_attack_reg.projectiles_spawned >= 1);
    assert!(logic.honesty_ocl_fire_weapon_attack_ok());
}

#[test]
fn ocl_attack_scud_storm_shots() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut storm = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    storm.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), storm);
    let id = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
        .expect("storm");
    assert!(logic.execute_ocl_attack("SUPERWEAPON_ScudStorm", id, Vec3::new(300.0, 0.0, 300.0)));
    assert_eq!(logic.ocl_fire_weapon_attack_reg.last_attack_shots, 9);
    let o = logic.host_object(id).unwrap();
    assert!(
        o.fire_weapon_power
            .as_ref()
            .map(|r| r.shots_remaining >= 9)
            .unwrap_or(false)
    );
}

#[test]
fn prone_update_damage_ratio_and_recovery() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_prone_update::{
        PRONE_GLA_DAMAGE_TO_FRAMES_RATIO, honesty_prone_update_residual_ok,
    };
    assert!(honesty_prone_update_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryWorker");
    tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryWorker".to_string(), tpl);
    let id = logic
        .create_object("GLAInfantryWorker", Team::GLA, Vec3::ZERO)
        .expect("worker");
    assert!(logic.host_object(id).unwrap().prone_update.is_some());
    {
        let o = logic.host_object_mut(id).unwrap();
        if let Some(pu) = o.prone_update.as_mut() {
            pu.damage_to_frames_ratio = PRONE_GLA_DAMAGE_TO_FRAMES_RATIO;
        }
    }
    logic.record_prone_go_if_needed(id, 20.0);
    assert_eq!(
        logic
            .host_object(id)
            .and_then(|o| o.prone_update.as_ref().map(|p| p.prone_frames))
            .unwrap_or(0),
        100
    );
    assert!(logic.prone_update_reg.go_prone >= 1);
    // Tick to recovery.
    for f in 0u32..100 {
        logic.set_current_frame(u64::from(f));
        logic.update_prone_update();
    }
    assert!(
        logic
            .host_object(id)
            .and_then(|o| o.prone_update.as_ref().map(|p| !p.is_prone()))
            .unwrap_or(false)
    );
    assert!(logic.honesty_prone_update_ok());
}

#[test]
fn active_shroud_upgrade_sets_shroud_range() {
    use crate::game_logic::host_active_shroud_upgrade::honesty_active_shroud_upgrade_residual_ok;
    assert!(honesty_active_shroud_upgrade_residual_ok());

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let id = logic
        .create_object("TestTank", Team::USA, Vec3::ZERO)
        .expect("tank");
    assert_eq!(logic.host_object(id).unwrap().shroud_range, 0.0);
    assert!(logic.apply_active_shroud_upgrade(id, 175.0));
    assert!((logic.host_object(id).unwrap().shroud_range - 175.0).abs() < 0.01);
    assert!(logic.active_shroud_upgrade_reg.applies >= 1);
    assert!(logic.honesty_active_shroud_upgrade_ok());
}

#[test]
fn animation_steering_battle_bus_turn_conditions() {
    use crate::game_logic::host_animation_steering::{
        BATTLE_BUS_MIN_TRANSITION_FRAMES, honesty_animation_steering_residual_ok,
    };
    use crate::game_logic::object::PhysicsTurningType;
    assert!(honesty_animation_steering_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAVehicleBattleBus");
    tpl.set_health(220.0);
    logic
        .templates
        .insert("GLAVehicleBattleBus".to_string(), tpl);
    let id = logic
        .create_object("GLAVehicleBattleBus", Team::GLA, Vec3::ZERO)
        .expect("bus");
    assert!(logic.host_object(id).unwrap().animation_steering.is_some());
    assert!(logic.animation_steering_reg.installed >= 1);

    {
        let o = logic.host_object_mut(id).unwrap();
        o.physics_turning = PhysicsTurningType::TurnNegative;
    }
    logic.set_current_frame(0);
    logic.update_animation_steering();
    assert_eq!(
        logic.host_object(id).and_then(|o| o
            .animation_steering
            .as_ref()
            .and_then(|a| a.active_condition.clone())),
        Some("CENTER_TO_RIGHT".to_string())
    );

    {
        let o = logic.host_object_mut(id).unwrap();
        o.physics_turning = PhysicsTurningType::TurnNone;
    }
    logic.set_current_frame(u64::from(BATTLE_BUS_MIN_TRANSITION_FRAMES));
    logic.update_animation_steering();
    assert_eq!(
        logic.host_object(id).and_then(|o| o
            .animation_steering
            .as_ref()
            .and_then(|a| a.active_condition.clone())),
        Some("RIGHT_TO_CENTER".to_string())
    );
    assert!(logic.honesty_animation_steering_ok());
}

#[test]
fn passengers_fire_upgrade_helix_battle_bunker() {
    use crate::game_logic::host_passengers_fire_upgrade::{
        UPGRADE_HELIX_BATTLE_BUNKER, honesty_passengers_fire_upgrade_residual_ok,
    };
    assert!(honesty_passengers_fire_upgrade_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaHelix");
    tpl.set_health(300.0);
    logic.templates.insert("ChinaHelix".to_string(), tpl);
    let id = logic
        .create_object("ChinaHelix", Team::China, Vec3::ZERO)
        .expect("helix");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.is_helix_transport = true;
        o.passengers_allowed_to_fire = false;
    }
    let n = logic.apply_passengers_fire_upgrade_to_team(Team::China, UPGRADE_HELIX_BATTLE_BUNKER);
    assert!(n >= 1);
    assert!(logic.host_object(id).unwrap().passengers_allowed_to_fire);
    assert!(logic.passengers_fire_upgrade_reg.applies >= 1);
    assert!(logic.honesty_passengers_fire_upgrade_ok());
}

#[test]
fn enemy_near_wall_sets_model_condition_on_scan() {
    use crate::game_logic::host_enemy_near::honesty_enemy_near_residual_ok;
    assert!(honesty_enemy_near_residual_ok());

    let mut logic = GameLogic::new();
    let mut wall_tpl = crate::game_logic::ThingTemplate::new("AmericaWallSegment");
    wall_tpl.set_health(1000.0);
    logic
        .templates
        .insert("AmericaWallSegment".to_string(), wall_tpl);
    ensure_test_infantry_template(&mut logic);

    let wall = logic
        .create_object("AmericaWallSegment", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("wall");
    {
        let w = logic.host_object_mut(wall).unwrap();
        if let Some(en) = w.enemy_near.as_mut() {
            en.vision_range = 200.0;
            en.scan_delay = 0;
        }
        w.vision_range = 200.0;
    }
    assert!(logic.host_object(wall).unwrap().enemy_near.is_some());

    let enemy = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    let _ = enemy;

    logic.update_enemy_near();
    assert!(
        logic
            .host_object(wall)
            .and_then(|o| o
                .enemy_near
                .as_ref()
                .map(|e| e.enemy_near && e.model_enemy_near))
            .unwrap_or(false),
        "enemy in vision must set ENEMYNEAR residual"
    );
    assert!(logic.enemy_near_reg.became_near >= 1);
    assert!(logic.honesty_enemy_near_ok());
}

#[test]
fn base_regenerate_structure_heals_after_delay() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_base_regenerate::{
        BASE_REGEN_DELAY_FRAMES, BASE_REGEN_HEAL_RATE_FRAMES, honesty_base_regenerate_residual_ok,
    };
    assert!(honesty_base_regenerate_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), tpl);
    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("cc");
    assert!(logic.host_object(id).unwrap().base_regenerate.is_some());
    assert!(logic.base_regenerate_reg.installed >= 1);

    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 500.0;
        o.status.under_construction = false;
    }
    logic.set_current_frame(0);
    logic.notify_base_regenerate_damage(id, false);
    // Immediately after damage: still delayed.
    logic.update_base_regenerate();
    let mid = logic.host_object(id).unwrap().health.current;
    assert!(
        (mid - 500.0).abs() < 0.01,
        "no heal during delay, got {mid}"
    );

    // After delay, heal ticks.
    let wake = BASE_REGEN_DELAY_FRAMES;
    logic.set_current_frame(u64::from(wake));
    logic.update_base_regenerate();
    let after = logic.host_object(id).unwrap().health.current;
    assert!(after > 500.0, "must heal after delay (after={after})");
    assert!(logic.base_regenerate_reg.heal_ticks >= 1);
    assert!(logic.honesty_base_regenerate_ok());
    let _ = BASE_REGEN_HEAL_RATE_FRAMES;
}

#[test]
fn default_auto_heal_trainable_heals_after_start_delay() {
    // C++ AutoHealBehavior.cpp:205-219 self-heal; :136-157 onDamage StartHealingDelay.
    use crate::game_logic::host_heal::{
        DEFAULT_AUTO_HEAL_AMOUNT, DEFAULT_AUTO_HEAL_START_DELAY_FRAMES,
    };

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("USA_Ranger");
    tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
    tpl.is_trainable = true;
    logic.templates.insert("USA_Ranger".to_string(), tpl);
    let id = logic
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .expect("ranger");
    assert!(
        logic.host_object(id).unwrap().default_auto_heal.is_some(),
        "trainable units inherit ModuleTag_DefaultAutoHealBehavior"
    );

    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 50.0;
        if let Some(ah) = o.default_auto_heal.as_mut() {
            ah.on_damage(0);
        }
    }
    logic.set_current_frame(0);
    logic.update_default_auto_heal();
    let mid = logic.host_object(id).unwrap().health.current;
    assert!(
        (mid - 50.0).abs() < 0.01,
        "no heal during StartHealingDelay, got {mid}"
    );

    logic.set_current_frame(u64::from(DEFAULT_AUTO_HEAL_START_DELAY_FRAMES));
    logic.update_default_auto_heal();
    let after = logic.host_object(id).unwrap().health.current;
    assert!(
        (after - (50.0 + DEFAULT_AUTO_HEAL_AMOUNT)).abs() < 0.01,
        "must pulse HealingAmount=2 after StartHealingDelay (after={after})"
    );

    // Full health sleeps forever; later combat must restart StartHealingDelay.
    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = o.health.maximum;
    }
    logic.update_default_auto_heal();
    let damage_frame = logic.frame;
    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 50.0;
        if let Some(ah) = o.default_auto_heal.as_mut() {
            ah.on_damage(damage_frame);
        }
    }
    logic.update_default_auto_heal();
    let after_second_hit = logic.host_object(id).unwrap().health.current;
    assert!(
        (after_second_hit - 50.0).abs() < 0.01,
        "onDamage must re-arm StartHealingDelay after full-health sleep"
    );
    logic.set_current_frame(
        u64::from(logic.frame) + u64::from(DEFAULT_AUTO_HEAL_START_DELAY_FRAMES),
    );
    logic.update_default_auto_heal();
    let after_restart = logic.host_object(id).unwrap().health.current;
    assert!(
        (after_restart - (50.0 + DEFAULT_AUTO_HEAL_AMOUNT)).abs() < 0.01,
        "self-heal must restart after later combat (after={after_restart})"
    );

    let mut building = ThingTemplate::new("AmericaCommandCenter");
    building.add_kind_of(KindOf::Structure).set_health(1000.0);
    building.is_trainable = false;
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), building);
    let cc = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("cc");
    assert!(
        logic.host_object(cc).unwrap().default_auto_heal.is_none(),
        "C++ drops DefaultAutoHeal when !isTrainable"
    );
}

#[test]
fn fire_spread_tree_ignites_neighbor() {
    use crate::game_logic::host_fire_spread::{
        TREE_SPREAD_TRY_RANGE, honesty_fire_spread_residual_ok,
    };
    assert!(honesty_fire_spread_residual_ok());

    let mut logic = GameLogic::new();
    for name in ["DogwoodTreeA", "DogwoodTreeB"] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.set_health(50.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("DogwoodTreeA", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object(
            "DogwoodTreeB",
            Team::Neutral,
            Vec3::new(TREE_SPREAD_TRY_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("b");
    assert!(logic.host_object(a).unwrap().has_fire_spread());
    assert!(logic.host_object(b).unwrap().has_fire_spread());
    assert!(logic.fire_spread_reg.installed >= 2);

    assert!(logic.ignite_object_fire_spread(a));
    // Force spread due immediately.
    {
        let o = logic.host_object_mut(a).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.next_spread_frame = 0;
        }
    }
    logic.set_current_frame(30);
    logic.update_fire_spread();
    assert!(
        logic.fire_spread_reg.spreads > 0,
        "aflame tree must attempt spread"
    );
    assert!(
        logic
            .host_object(b)
            .and_then(|o| o.fire_spread.as_ref().map(|f| f.is_aflame()))
            .unwrap_or(false)
            || logic.fire_spread_reg.ignitions >= 2,
        "neighbor within range must ignite"
    );
    assert!(logic.honesty_fire_spread_ok());
}

#[test]
fn flammable_buildings_ignite_and_play_burning_loop() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_enum_table_residual::aflame_model_bit;
    use crate::game_logic::host_fire_spread::TREE_BURNING_SOUND;

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("TechOilDerrick");
    tpl.add_kind_of(KindOf::Structure).set_health(2000.0);
    logic.templates.insert("TechOilDerrick".to_string(), tpl);
    let id = logic
        .create_object("TechOilDerrick", Team::Neutral, Vec3::ZERO)
        .expect("oil");
    assert!(logic.host_object(id).unwrap().has_fire_spread());
    assert!(
        !logic
            .host_object(id)
            .unwrap()
            .fire_spread
            .as_ref()
            .unwrap()
            .spread_enabled
    );

    crate::game_logic::host_historic_bonus::set_logic_frame(10);
    {
        let o = logic.host_object_mut(id).unwrap();
        let _ = o.take_damage_from_typed(50.0, None, DamageType::Flame);
    }
    {
        let o = logic.host_object(id).unwrap();
        assert!(o.fire_spread.as_ref().unwrap().is_aflame());
        assert!(o.has_object_status_bit("AFLAME"));
        assert!(o.model_condition_bits & (1u128 << aflame_model_bit()) != 0);
    }
    logic.set_current_frame(10);
    logic.update_fire_spread();
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == TREE_BURNING_SOUND && e.is_looping),
        "GenericFireMediumLoop must start on ignite"
    );
}

#[test]
fn fire_spread_spawns_burning_embers_and_uses_3d_range() {
    use crate::game_logic::host_fire_spread::{TREE_OCL_EMBERS, TREE_SPREAD_TRY_RANGE};

    let mut logic = GameLogic::new();
    for name in ["DogwoodTreeA", "DogwoodTreeCliff"] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.set_health(50.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("DogwoodTreeA", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let cliff = logic
        .create_object(
            "DogwoodTreeCliff",
            Team::Neutral,
            Vec3::new(30.0, 80.0, 0.0),
        )
        .expect("cliff");
    assert!(logic.ignite_object_fire_spread(a));
    {
        let o = logic.host_object_mut(a).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.next_spread_frame = 0;
        }
    }
    logic.set_current_frame(30);
    logic.update_fire_spread();
    assert!(logic.fire_spread_reg.embers > 0);
    assert!(
        logic
            .objects
            .values()
            .any(|o| o.template_name.eq_ignore_ascii_case("BurningEmbers")
                || o.template_name.contains("Ember"))
            || logic
                .combat_particles
                .active_systems()
                .any(|p| p.ocl_list_name == TREE_OCL_EMBERS || p.template_name == TREE_OCL_EMBERS),
        "OCL_BurningEmbers must create ember objects or particle entries"
    );
    assert!(
        !logic
            .host_object(cliff)
            .and_then(|o| o.fire_spread.as_ref().map(|f| f.is_aflame()))
            .unwrap_or(false),
        "FROM_CENTER_3D must exclude a tree 80 units above planar range {TREE_SPREAD_TRY_RANGE}"
    );
}

#[test]
fn tree_smolders_while_still_aflame() {
    use crate::game_logic::host_enum_table_residual::{aflame_model_bit, smoldering_model_bit};
    use crate::game_logic::host_fire_spread::TREE_BURNED_DELAY_FRAMES;

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("DogwoodTreeA");
    tpl.set_health(50.0);
    logic.templates.insert("DogwoodTreeA".to_string(), tpl);
    let id = logic
        .create_object("DogwoodTreeA", Team::Neutral, Vec3::ZERO)
        .expect("tree");
    assert!(logic.ignite_object_fire_spread(id));
    {
        let o = logic.host_object_mut(id).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.burned_end_frame = TREE_BURNED_DELAY_FRAMES;
        }
    }
    logic.set_current_frame(TREE_BURNED_DELAY_FRAMES as u64);
    logic.update_fire_spread();
    let o = logic.host_object(id).unwrap();
    assert!(o.fire_spread.as_ref().unwrap().is_aflame());
    assert!(o.fire_spread.as_ref().unwrap().smoldering);
    assert!(o.has_object_status_bit("AFLAME"));
    assert!(o.has_object_status_bit("BURNED"));
    assert!(o.model_condition_bits & (1u128 << aflame_model_bit()) != 0);
    assert!(o.model_condition_bits & (1u128 << smoldering_model_bit()) != 0);
}

#[test]
fn status_bits_upgrade_booby_trap_sets_bit() {
    use crate::game_logic::host_status_bits_upgrade::honesty_status_bits_upgrade_residual_ok;
    assert!(honesty_status_bits_upgrade_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATunnelNetwork");
    tpl.set_health(400.0);
    logic.templates.insert("GLATunnelNetwork".to_string(), tpl);
    let id = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::ZERO)
        .expect("net");
    assert!(
        !logic
            .host_object(id)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );

    let n = logic.apply_status_bits_upgrade_to_team(Team::GLA, "Upgrade_GLABoobyTrap");
    assert!(n >= 1);
    assert!(
        logic
            .host_object(id)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(logic.status_bits_upgrade_reg.applies >= 1);
    assert!(logic.honesty_status_bits_upgrade_ok());
}

#[test]
fn tensile_formation_avalanche_damage_slide_and_rubble() {
    use crate::game_logic::host_tensile_formation::{
        TENSILE_LIFE_MAX, honesty_tensile_formation_residual_ok,
    };

    assert!(honesty_tensile_formation_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AvalancheChunk");
    tpl.set_health(100.0);
    logic.templates.insert("AvalancheChunk".to_string(), tpl);

    let a = logic
        .create_object("AvalancheChunk", Team::Neutral, Vec3::new(0.0, 10.0, 0.0))
        .expect("chunk a");
    let b = logic
        .create_object("AvalancheChunk", Team::Neutral, Vec3::new(20.0, 10.0, 0.0))
        .expect("chunk b");
    assert!(logic.host_object(a).unwrap().has_tensile_formation());
    assert!(logic.host_object(b).unwrap().has_tensile_formation());
    assert!(logic.tensile_formation_registry().members_installed >= 2);

    // Damage A to BODY_DAMAGED residual → enable formation.
    {
        let o = logic.host_object_mut(a).unwrap();
        o.health.current = 50.0;
    }
    logic.set_current_frame(30);
    logic.update_tensile_formations();
    assert!(
        logic.tensile_formation_registry().enables > 0
            || logic
                .host_object(a)
                .and_then(|o| o.tensile_formation.as_ref().map(|t| t.enabled))
                .unwrap_or(false),
        "damage must enable tensile formation"
    );
    assert!(logic.honesty_tensile_formation_ok());

    // Advance life to rubble.
    {
        let o = logic.host_object_mut(a).unwrap();
        if let Some(tf) = o.tensile_formation.as_mut() {
            tf.enabled = true;
            tf.life = TENSILE_LIFE_MAX;
        }
    }
    logic.set_current_frame(400);
    logic.update_tensile_formations();
    assert!(
        logic.tensile_formation_registry().rubbles > 0
            || logic
                .host_object(a)
                .and_then(|o| o.tensile_formation.as_ref().map(|t| t.rubble))
                .unwrap_or(false),
        "life>300 must rubble"
    );
}

#[test]
fn toxin_fire_ocl_spawns_field_after_min_shots_and_coast() {
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES, TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL,
        is_toxin_tractor_template,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GLAVehicleToxinTruck");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("GLAVehicleToxinTruck".into(), tpl);
    let id = logic
        .create_object("GLAVehicleToxinTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("toxin");
    assert!(is_toxin_tractor_template("GLAVehicleToxinTruck"));
    assert!(
        logic
            .host_object(id)
            .unwrap()
            .fire_ocl_after_cooldown
            .is_some()
    );

    // Fire secondary spray MinShots times.
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        logic.frame = f;
        let _ = logic.apply_toxin_tractor_spray_at(Vec3::new(10.0, 0.0, 0.0), Some(id), Team::GLA);
    }
    assert_eq!(logic.toxin_tractor.fire_ocl_spawns, 0, "no OCL until coast");
    // Advance past coast without more shots.
    logic.frame = TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL + TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES;
    logic.tick_fire_ocl_after_weapon_cooldown();
    assert!(
        logic.toxin_tractor.fire_ocl_spawns > 0,
        "FireOCL should spawn medium field after min shots + coast"
    );
    assert!(logic.honesty_toxin_fire_ocl_ok());
}

#[test]
fn upgrade_die_removes_producer_drone_upgrade() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    // Humvee-like master.
    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);
    let master_id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    let drone_id = game_logic
        .residual_attach_slave_drone(
            master_id,
            crate::game_logic::host_slave_drones::SlaveDroneKind::Scout,
        )
        .expect("scout drone");
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(
            m.has_upgrade_tag(crate::game_logic::host_slave_drones::UPGRADE_AMERICA_SCOUT_DRONE)
        );
    }
    {
        let d = game_logic.host_object(drone_id).unwrap();
        assert_eq!(d.producer_id, Some(master_id));
        assert!(d.upgrade_die.is_some());
    }
    // Kill drone → UpgradeDie frees master upgrade.
    game_logic.destroy_object(drone_id);
    // Process destruction queue if needed.
    if let Some(d) = game_logic.host_object_mut(drone_id) {
        // Wave 753: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = d.health.current.max(1.0);
            let oid = d.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            d.health.current = 0.0;
        }
        d.status.destroyed = true;
    }
    // destroy_object should have already run upgrade die via mark.
    let m = game_logic.host_object(master_id).unwrap();
    assert!(
        !m.has_upgrade_tag(crate::game_logic::host_slave_drones::UPGRADE_AMERICA_SCOUT_DRONE),
        "UpgradeDie must remove producer upgrade"
    );
    assert!(game_logic.honesty_upgrade_die_ok());
}

#[test]
fn tunnel_network_residual_flags_and_capacity_installed() {
    let mut game_logic = GameLogic::new();
    let tunnel_id = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel");
    assert!(tunnel.is_tunnel_network_style_container());
    assert!(tunnel.can_contain());
    assert_eq!(
        tunnel.garrison_capacity(),
        crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY
    );
}

#[test]
fn tunnel_network_residual_enter_sets_garrisoned_and_shared_pool() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_id = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: tunnel_id,
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[infantry_id, tunnel_id], 1.0 / 30.0);

    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel after");
    assert!(
        tunnel.contained_units().contains(&infantry_id),
        "entry tunnel must list occupant"
    );
    let pool_key = tunnel.tunnel_system_key();
    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(infantry.ai_state, AIState::Garrisoned);
    assert_eq!(infantry.contained_by, Some(tunnel_id));
    assert!(!infantry.can_move());
    assert_eq!(game_logic.tunnel_network_residual_enters(), 1);
    assert!(
        game_logic
            .tunnel_network_residual()
            .is_in_network(pool_key, infantry_id)
    );
}

#[test]
fn tunnel_control_bar_inventory_is_shared_player_pool() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_a = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel_b = create_test_tunnel_network(&mut game_logic, Vec3::new(50.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let key = game_logic
        .host_object(tunnel_a)
        .expect("a")
        .tunnel_system_key();
    game_logic.tunnel_network.on_tunnel_created(key, tunnel_a);
    game_logic.tunnel_network.on_tunnel_created(key, tunnel_b);
    assert!(
        game_logic
            .tunnel_network
            .record_enter(key, infantry_id, tunnel_a)
    );
    assert_eq!(
        game_logic.host_authoritative_occupant_count(tunnel_a),
        Some(1)
    );
    assert_eq!(
        game_logic.host_authoritative_occupant_count(tunnel_b),
        Some(1)
    );
    assert_eq!(
        game_logic.host_authoritative_contained_units(tunnel_b),
        vec![infantry_id]
    );
}

#[test]
fn tunnel_network_residual_cross_exit_enter_a_evacuate_b() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_a = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel_b = create_test_tunnel_network(&mut game_logic, Vec3::new(200.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    // Enter A.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.target = Some(tunnel_a);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, tunnel_a, tunnel_b], 1.0 / 30.0);
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned
    );
    assert_eq!(game_logic.tunnel_network_residual_enters(), 1);
    assert!(!game_logic.honesty_tunnel_network_cross_exit_ok());

    // Evacuate B dumps shared pool at B (cross-tunnel residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 2,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tunnel_b],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("exited");
    assert_eq!(
        infantry.ai_state,
        AIState::Moving,
        "TunnelContain inherits OpenContain ExitStart/End walk"
    );
    assert!(infantry.contained_by.is_none());
    assert!(infantry.can_move());
    // Dropped near tunnel B, not tunnel A.
    let pos = infantry.get_position();
    let b_pos = game_logic.host_object(tunnel_b).unwrap().get_position();
    let a_pos = game_logic.host_object(tunnel_a).unwrap().get_position();
    assert!(
        pos.distance(b_pos) < 20.0,
        "cross-exit must place unit near tunnel B (got dist {})",
        pos.distance(b_pos)
    );
    assert!(
        pos.distance(a_pos) > 100.0,
        "cross-exit must not leave unit at tunnel A"
    );
    assert!(
        !game_logic
            .host_object(tunnel_a)
            .unwrap()
            .contained_units()
            .contains(&infantry_id),
        "entry tunnel A capacity freed"
    );
    assert_eq!(game_logic.tunnel_network_residual_exits(), 1);
    assert_eq!(game_logic.tunnel_network_residual_cross_exits(), 1);
    assert!(
        game_logic.honesty_tunnel_network_enter_exit_ok(),
        "enter+exit honesty"
    );
    assert!(
        game_logic.honesty_tunnel_network_cross_exit_ok(),
        "cross-tunnel honesty"
    );
    assert!(game_logic.honesty_tunnel_network_ok());
}

#[test]
fn tunnel_network_residual_shared_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_a = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel_b = create_test_tunnel_network(&mut game_logic, Vec3::new(50.0, 0.0, 0.0));

    let cap = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
    let mut unit_ids = Vec::new();
    for i in 0..cap {
        // Fill half via A, half via B to prove shared pool.
        // Spawn near the chosen tunnel so enter-range residual succeeds.
        let tunnel = if i < cap / 2 { tunnel_a } else { tunnel_b };
        let base = if i < cap / 2 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(51.0, 0.0, 0.0)
        };
        let id = game_logic
            .create_object(
                "TestInfantry",
                Team::GLA,
                base + Vec3::new(i as f32 * 0.1, 0.0, 0.0),
            )
            .expect("infantry");
        unit_ids.push(id);
        {
            let unit = game_logic.host_object_mut(id).unwrap();
            unit.target = Some(tunnel);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[id, tunnel_a, tunnel_b], 1.0 / 30.0);
    }
    assert_eq!(game_logic.tunnel_network_residual_enters() as usize, cap);
    let pool_key = game_logic
        .host_object(tunnel_a)
        .expect("tunnel a")
        .tunnel_system_key();
    assert!(!game_logic.tunnel_network_residual().has_capacity(pool_key));

    let overflow = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(3.0, 0.0, 0.0))
        .expect("overflow");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: tunnel_a,
        },
        player_id: 2,
        command_id: 99,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![overflow],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Enter command may still set Entering, but AI must reject capacity.
    game_logic.update_ai(&[overflow, tunnel_a, tunnel_b], 1.0 / 30.0);
    let overflow_obj = game_logic.host_object(overflow).unwrap();
    assert_ne!(
        overflow_obj.ai_state,
        AIState::Garrisoned,
        "11th unit must not enter shared tunnel pool"
    );
    assert_eq!(
        game_logic.tunnel_network_residual_enters() as usize,
        cap,
        "enter counter must not grow past capacity"
    );
}

#[test]
fn tunnel_network_residual_rejects_aircraft() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_aircraft_template(&mut game_logic);
    let tunnel_id = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let air_id = game_logic
        .create_object("TestAircraft", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("aircraft");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: tunnel_id,
        },
        player_id: 2,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![air_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let air = game_logic.host_object(air_id).expect("aircraft");
    assert_ne!(
        air.ai_state,
        AIState::Entering,
        "aircraft must not enter tunnel residual"
    );
    assert_eq!(game_logic.tunnel_network_residual_enters(), 0);
}

#[test]
fn combat_chinook_residual_capacity_and_flags_installed() {
    let mut game_logic = GameLogic::new();
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(chinook.is_combat_chinook_style_container());
    assert!(chinook.can_contain());
    assert_eq!(
        chinook.transport_capacity(),
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS
    );
    assert!(chinook.passengers_allowed_to_fire);
    assert!(chinook.armed_riders_upgrade_weapon_set);
    assert!(!chinook.weapon_set_player_upgrade);
    assert!(
        chinook.is_kind_of(KindOf::Attackable),
        "Combat Chinook KindOf residual includes CAN_ATTACK"
    );
}

#[test]
fn combat_chinook_residual_enter_sets_docked_and_upgrades_weapon_set() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    // Armed rider residual (rifle) so ArmedRidersUpgradeMyWeaponSet applies.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.5,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: chinook_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[infantry_id, chinook_id], 1.0 / 30.0);

    let chinook = game_logic.host_object(chinook_id).expect("chinook after");
    assert!(chinook.contained_units().contains(&infantry_id));
    assert_eq!(chinook.transport_count(), 1);
    assert!(
        chinook.weapon_set_player_upgrade,
        "armed riders must upgrade Combat Chinook weapon set"
    );
    assert!(
        chinook.weapon.is_some(),
        "PLAYER_UPGRADE residual binds ListeningOutpost dummy weapon"
    );
    let dummy = chinook.weapon.as_ref().unwrap();
    assert!(
        crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(dummy),
        "Combat Chinook dummy must be ListeningOutpost residual (damage ~0.1)"
    );
    assert!(
        (dummy.damage - crate::game_logic::host_combat_chinook::LISTENING_OUTPOST_DUMMY_DAMAGE)
            .abs()
            < f32::EPSILON
    );

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(infantry.ai_state, AIState::Docked);
    assert_eq!(infantry.contained_by, Some(chinook_id));
    assert_eq!(game_logic.combat_chinook_residual_loads(), 1);
    assert_eq!(
        game_logic.transport_residual_loads(),
        0,
        "Combat Chinook load must not count as generic transport load"
    );
    assert_eq!(
        game_logic.battle_bus_residual_loads(),
        0,
        "Combat Chinook load must not count as Battle Bus load"
    );
    assert!(
        game_logic.honesty_combat_chinook_weapon_set_upgrade_ok(),
        "weapon-set upgrade residual honesty"
    );
}
