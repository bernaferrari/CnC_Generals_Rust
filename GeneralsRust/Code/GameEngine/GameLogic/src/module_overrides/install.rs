//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! install_module_overrides / ensure_module_overrides_installed.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

pub fn install_module_overrides() -> Result<(), String> {
    register_module_override(
        "InactiveBody",
        ModuleType::Body,
        inactive_body_module_factory,
        inactive_body_module_data_factory,
    )?;

    register_module_override(
        "ActiveBody",
        ModuleType::Body,
        active_body_module_factory,
        active_body_module_data_factory,
    )?;

    register_module_override(
        "StructureBody",
        ModuleType::Body,
        structure_body_module_factory,
        structure_body_module_data_factory,
    )?;

    register_module_override(
        "HighlanderBody",
        ModuleType::Body,
        highlander_body_module_factory,
        highlander_body_module_data_factory,
    )?;

    register_module_override(
        "ImmortalBody",
        ModuleType::Body,
        immortal_body_module_factory,
        immortal_body_module_data_factory,
    )?;

    register_module_override(
        "HiveStructureBody",
        ModuleType::Body,
        hive_structure_body_module_factory,
        hive_structure_body_module_data_factory,
    )?;

    register_module_override(
        "UndeadBody",
        ModuleType::Body,
        undead_body_module_factory,
        undead_body_module_data_factory,
    )?;

    register_module_override(
        "OpenContain",
        ModuleType::Behavior,
        open_contain_module_factory,
        open_contain_module_data_factory,
    )?;

    register_module_override(
        "TransportContain",
        ModuleType::Behavior,
        transport_contain_module_factory,
        transport_contain_module_data_factory,
    )?;

    register_module_override(
        "GarrisonContain",
        ModuleType::Behavior,
        garrison_contain_module_factory,
        garrison_contain_module_data_factory,
    )?;

    register_module_override(
        "TunnelContain",
        ModuleType::Behavior,
        tunnel_contain_module_factory,
        tunnel_contain_module_data_factory,
    )?;

    register_module_override(
        "OverlordContain",
        ModuleType::Behavior,
        overlord_contain_module_factory,
        overlord_contain_module_data_factory,
    )?;

    register_module_override(
        "HelixContain",
        ModuleType::Behavior,
        helix_contain_module_factory,
        helix_contain_module_data_factory,
    )?;

    register_module_override(
        "ParachuteContain",
        ModuleType::Behavior,
        parachute_contain_module_factory,
        parachute_contain_module_data_factory,
    )?;

    register_module_override(
        "MobNexusContain",
        ModuleType::Behavior,
        mob_nexus_contain_module_factory,
        mob_nexus_contain_module_data_factory,
    )?;

    register_module_override(
        "RailedTransportContain",
        ModuleType::Behavior,
        railed_transport_contain_module_factory,
        railed_transport_contain_module_data_factory,
    )?;

    register_module_override(
        "RiderChangeContain",
        ModuleType::Behavior,
        rider_change_contain_module_factory,
        rider_change_contain_module_data_factory,
    )?;

    register_module_override(
        "InternetHackContain",
        ModuleType::Behavior,
        internet_hack_contain_module_factory,
        internet_hack_contain_module_data_factory,
    )?;

    register_module_override(
        "HealContain",
        ModuleType::Behavior,
        heal_contain_module_factory,
        heal_contain_module_data_factory,
    )?;

    register_module_override(
        "CaveContain",
        ModuleType::Behavior,
        cave_contain_module_factory,
        cave_contain_module_data_factory,
    )?;

    register_module_override(
        "LockWeaponCreate",
        ModuleType::Behavior,
        lock_weapon_create_module_factory,
        lock_weapon_create_module_data_factory,
    )?;

    register_module_override(
        "PreorderCreate",
        ModuleType::Behavior,
        preorder_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SupplyCenterCreate",
        ModuleType::Behavior,
        supply_center_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SupplyWarehouseCreate",
        ModuleType::Behavior,
        supply_warehouse_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SpecialPowerCreate",
        ModuleType::Behavior,
        special_power_create_module_factory,
        simple_create_module_data_factory,
    )?;

    register_module_override(
        "SpecialPowerModule",
        ModuleType::Behavior,
        special_power_module_factory,
        special_power_module_data_factory,
    )?;

    register_module_override(
        "ProductionUpdate",
        ModuleType::Behavior,
        production_update_module_factory,
        production_update_module_data_factory,
    )?;

    register_module_override(
        "DemoralizeSpecialPower",
        ModuleType::Behavior,
        demoralize_special_power_module_factory,
        demoralize_special_power_module_data_factory,
    )?;

    register_module_override(
        "CashHackSpecialPower",
        ModuleType::Behavior,
        cash_hack_special_power_module_factory,
        cash_hack_special_power_module_data_factory,
    )?;

    register_module_override(
        "SpyVisionSpecialPower",
        ModuleType::Behavior,
        spy_vision_special_power_module_factory,
        spy_vision_special_power_module_data_factory,
    )?;

    register_module_override(
        "DefectorSpecialPower",
        ModuleType::Behavior,
        defector_special_power_module_factory,
        defector_special_power_module_data_factory,
    )?;

    register_module_override(
        "CashBountyPower",
        ModuleType::Behavior,
        cash_bounty_power_module_factory,
        cash_bounty_power_module_data_factory,
    )?;

    register_module_override(
        "CleanupAreaPower",
        ModuleType::Behavior,
        cleanup_area_power_module_factory,
        cleanup_area_power_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponPower",
        ModuleType::Behavior,
        fire_weapon_power_module_factory,
        fire_weapon_power_module_data_factory,
    )?;

    register_module_override(
        "SpecialAbility",
        ModuleType::Behavior,
        special_ability_module_factory,
        special_ability_module_data_factory,
    )?;

    register_module_override(
        "BaikonurLaunchPower",
        ModuleType::Behavior,
        baikonur_launch_power_module_factory,
        baikonur_launch_power_module_data_factory,
    )?;

    register_module_override(
        "OCLSpecialPower",
        ModuleType::Behavior,
        ocl_special_power_module_factory,
        ocl_special_power_module_data_factory,
    )?;

    register_module_override(
        "GrantUpgradeCreate",
        ModuleType::Behavior,
        grant_upgrade_create_module_factory,
        grant_upgrade_create_module_data_factory,
    )?;

    register_module_override(
        "VeterancyGainCreate",
        ModuleType::Behavior,
        veterancy_gain_create_module_factory,
        veterancy_gain_create_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponCollide",
        ModuleType::Behavior,
        fire_weapon_collide_module_factory,
        fire_weapon_collide_module_data_factory,
    )?;

    register_module_override(
        "ShroudCrateCollide",
        ModuleType::Behavior,
        shroud_crate_collide_module_factory,
        shroud_crate_collide_module_data_factory,
    )?;

    register_module_override(
        "W3DModelDraw",
        ModuleType::Draw,
        w3d_model_draw_module_factory,
        module_data_proc_or(
            "W3DModelDraw",
            ModuleType::Draw,
            w3d_model_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DDefaultDraw",
        ModuleType::Draw,
        w3d_default_draw_module_factory,
        module_data_proc_or(
            "W3DDefaultDraw",
            ModuleType::Draw,
            w3d_default_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DDependencyModelDraw",
        ModuleType::Draw,
        w3d_dependency_model_draw_module_factory,
        module_data_proc_or(
            "W3DDependencyModelDraw",
            ModuleType::Draw,
            w3d_dependency_model_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DOverlordAircraftDraw",
        ModuleType::Draw,
        w3d_overlord_aircraft_draw_module_factory,
        module_data_proc_or(
            "W3DOverlordAircraftDraw",
            ModuleType::Draw,
            w3d_overlord_aircraft_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DTankDraw",
        ModuleType::Draw,
        w3d_tank_draw_module_factory,
        module_data_proc_or(
            "W3DTankDraw",
            ModuleType::Draw,
            w3d_tank_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DOverlordTankDraw",
        ModuleType::Draw,
        w3d_overlord_tank_draw_module_factory,
        module_data_proc_or(
            "W3DOverlordTankDraw",
            ModuleType::Draw,
            w3d_overlord_tank_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DOverlordTruckDraw",
        ModuleType::Draw,
        w3d_overlord_truck_draw_module_factory,
        module_data_proc_or(
            "W3DOverlordTruckDraw",
            ModuleType::Draw,
            w3d_overlord_truck_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DPoliceCarDraw",
        ModuleType::Draw,
        w3d_police_car_draw_module_factory,
        module_data_proc_or(
            "W3DPoliceCarDraw",
            ModuleType::Draw,
            w3d_police_car_draw_module_data_factory,
        ),
    )?;

    // C++ W3DModuleFactory.cpp:39-48 registers W3DModelDraw / W3DProjectileStreamDraw
    // (and siblings). There is no W3DProjectileDraw module in GeneralsMD.
    // Do not re-register this invented name if this archival dump is rewired.

    register_module_override(
        "W3DLaserDraw",
        ModuleType::Draw,
        w3d_laser_draw_module_factory,
        w3d_laser_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DRopeDraw",
        ModuleType::Draw,
        w3d_rope_draw_module_factory,
        w3d_rope_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DProjectileStreamDraw",
        ModuleType::Draw,
        w3d_projectile_stream_draw_module_factory,
        w3d_projectile_stream_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DTreeDraw",
        ModuleType::Draw,
        w3d_tree_draw_module_factory,
        w3d_tree_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DTracerDraw",
        ModuleType::Draw,
        w3d_tracer_draw_module_factory,
        w3d_tracer_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DDebrisDraw",
        ModuleType::Draw,
        w3d_debris_draw_module_factory,
        w3d_debris_draw_module_data_factory,
    )?;

    register_module_override(
        "W3DScienceModelDraw",
        ModuleType::Draw,
        w3d_science_model_draw_module_factory,
        module_data_proc_or(
            "W3DScienceModelDraw",
            ModuleType::Draw,
            w3d_science_model_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DSupplyDraw",
        ModuleType::Draw,
        w3d_supply_draw_module_factory,
        module_data_proc_or(
            "W3DSupplyDraw",
            ModuleType::Draw,
            w3d_supply_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DTruckDraw",
        ModuleType::Draw,
        w3d_truck_draw_module_factory,
        module_data_proc_or(
            "W3DTruckDraw",
            ModuleType::Draw,
            w3d_truck_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "W3DTankTruckDraw",
        ModuleType::Draw,
        w3d_tank_truck_draw_module_factory,
        module_data_proc_or(
            "W3DTankTruckDraw",
            ModuleType::Draw,
            w3d_tank_truck_draw_module_data_factory,
        ),
    )?;

    register_module_override(
        "LaserUpdate",
        ModuleType::ClientUpdate,
        laser_update_module_factory,
        laser_update_module_data_factory,
    )?;

    register_module_override(
        "OCLUpdate",
        ModuleType::Behavior,
        ocl_update_module_factory,
        ocl_update_module_data_factory,
    )?;

    register_module_override(
        "SpecialPowerUpdate",
        ModuleType::Behavior,
        special_power_update_module_factory,
        special_power_update_module_data_factory,
    )?;

    register_module_override(
        "BeaconClientUpdate",
        ModuleType::ClientUpdate,
        beacon_client_update_module_factory,
        beacon_client_update_module_data_factory,
    )?;

    register_module_override(
        "SwayClientUpdate",
        ModuleType::ClientUpdate,
        sway_client_update_module_factory,
        sway_client_update_module_data_factory,
    )?;

    register_module_override(
        "AnimatedParticleSysBoneClientUpdate",
        ModuleType::ClientUpdate,
        animated_particle_sys_bone_client_update_module_factory,
        animated_particle_sys_bone_client_update_module_data_factory,
    )?;

    register_module_override(
        "SquishCollide",
        ModuleType::Behavior,
        squish_collide_module_factory,
        squish_collide_module_data_factory,
    )?;

    register_module_override(
        "UpgradeDie",
        ModuleType::Behavior,
        upgrade_die_module_factory,
        upgrade_die_module_data_factory,
    )?;
    register_module_override(
        "DestroyDie",
        ModuleType::Behavior,
        destroy_die_module_factory,
        die_module_data_factory,
    )?;
    register_module_override(
        "KeepObjectDie",
        ModuleType::Behavior,
        keep_object_die_module_factory,
        die_module_data_factory,
    )?;
    register_module_override(
        "CreateObjectDie",
        ModuleType::Behavior,
        create_object_die_module_factory,
        create_object_die_module_data_factory,
    )?;
    register_module_override(
        "CreateCrateDie",
        ModuleType::Behavior,
        create_crate_die_module_factory,
        create_crate_die_module_data_factory,
    )?;
    register_module_override(
        "FXListDie",
        ModuleType::Behavior,
        fx_list_die_module_factory,
        fx_list_die_module_data_factory,
    )?;
    register_module_override(
        "CrushDie",
        ModuleType::Behavior,
        crush_die_module_factory,
        crush_die_module_data_factory,
    )?;
    register_module_override(
        "EjectPilotDie",
        ModuleType::Behavior,
        eject_pilot_die_module_factory,
        eject_pilot_die_module_data_factory,
    )?;
    register_module_override(
        "RebuildHoleExposeDie",
        ModuleType::Behavior,
        rebuild_hole_expose_die_module_factory,
        rebuild_hole_expose_die_module_data_factory,
    )?;
    register_module_override(
        "SpecialPowerCompletionDie",
        ModuleType::Behavior,
        special_power_completion_die_module_factory,
        special_power_completion_die_module_data_factory,
    )?;
    register_module_override(
        "DamDie",
        ModuleType::Behavior,
        dam_die_module_factory,
        dam_die_module_data_factory,
    )?;

    register_module_override(
        "StatusBitsUpgrade",
        ModuleType::Behavior,
        status_bits_upgrade_module_factory,
        status_bits_upgrade_module_data_factory,
    )?;

    register_module_override(
        "PassengersFireUpgrade",
        ModuleType::Behavior,
        passengers_fire_upgrade_module_factory,
        passengers_fire_upgrade_module_data_factory,
    )?;

    register_module_override(
        "SubObjectsUpgrade",
        ModuleType::Behavior,
        subobjects_upgrade_module_factory,
        subobjects_upgrade_module_data_factory,
    )?;

    register_module_override(
        "GrantScienceUpgrade",
        ModuleType::Behavior,
        grant_science_upgrade_module_factory,
        grant_science_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ObjectCreationUpgrade",
        ModuleType::Behavior,
        object_creation_upgrade_module_factory,
        object_creation_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ActiveShroudUpgrade",
        ModuleType::Behavior,
        active_shroud_upgrade_module_factory,
        active_shroud_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ArmorUpgrade",
        ModuleType::Behavior,
        armor_upgrade_module_factory,
        armor_upgrade_module_data_factory,
    )?;

    register_module_override(
        "CommandSetUpgrade",
        ModuleType::Behavior,
        command_set_upgrade_module_factory,
        command_set_upgrade_module_data_factory,
    )?;

    register_module_override(
        "CostModifierUpgrade",
        ModuleType::Behavior,
        cost_modifier_upgrade_module_factory,
        cost_modifier_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ExperienceScalarUpgrade",
        ModuleType::Behavior,
        experience_scalar_upgrade_module_factory,
        experience_scalar_upgrade_module_data_factory,
    )?;

    register_module_override(
        "LocomotorSetUpgrade",
        ModuleType::Behavior,
        locomotor_set_upgrade_module_factory,
        locomotor_set_upgrade_module_data_factory,
    )?;

    register_module_override(
        "MaxHealthUpgrade",
        ModuleType::Behavior,
        max_health_upgrade_module_factory,
        max_health_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ModelConditionUpgrade",
        ModuleType::Behavior,
        model_condition_upgrade_module_factory,
        model_condition_upgrade_module_data_factory,
    )?;

    register_module_override(
        "PowerPlantUpgrade",
        ModuleType::Behavior,
        power_plant_upgrade_module_factory,
        power_plant_upgrade_module_data_factory,
    )?;

    register_module_override(
        "RadarUpgrade",
        ModuleType::Behavior,
        radar_upgrade_module_factory,
        radar_upgrade_module_data_factory,
    )?;

    register_module_override(
        "ReplaceObjectUpgrade",
        ModuleType::Behavior,
        replace_object_upgrade_module_factory,
        replace_object_upgrade_module_data_factory,
    )?;

    register_module_override(
        "StealthUpgrade",
        ModuleType::Behavior,
        stealth_upgrade_module_factory,
        stealth_upgrade_module_data_factory,
    )?;

    register_module_override(
        "UnpauseSpecialPowerUpgrade",
        ModuleType::Behavior,
        unpause_special_power_upgrade_module_factory,
        unpause_special_power_upgrade_module_data_factory,
    )?;

    register_module_override(
        "WeaponBonusUpgrade",
        ModuleType::Behavior,
        weapon_bonus_upgrade_module_factory,
        weapon_bonus_upgrade_module_data_factory,
    )?;

    register_module_override(
        "WeaponSetUpgrade",
        ModuleType::Behavior,
        weapon_set_upgrade_module_factory,
        weapon_set_upgrade_module_data_factory,
    )?;

    register_module_override(
        "TransitionDamageFX",
        ModuleType::Behavior,
        transition_damage_fx_module_factory,
        transition_damage_fx_module_data_factory,
    )?;

    register_module_override(
        "StealthUpdate",
        ModuleType::Behavior,
        stealth_update_module_factory,
        stealth_update_module_data_factory,
    )?;

    register_module_override(
        "StickyBombUpdate",
        ModuleType::Behavior,
        sticky_bomb_update_module_factory,
        sticky_bomb_update_module_data_factory,
    )?;

    register_module_override(
        "ProneUpdate",
        ModuleType::Behavior,
        prone_update_module_factory,
        prone_update_module_data_factory,
    )?;

    register_module_override(
        "ProjectileStreamUpdate",
        ModuleType::Behavior,
        projectile_stream_update_module_factory,
        projectile_stream_update_module_data_factory,
    )?;

    register_module_override(
        "PointDefenseLaserUpdate",
        ModuleType::Behavior,
        point_defense_laser_update_module_factory,
        point_defense_laser_update_module_data_factory,
    )?;

    register_module_override(
        "LaserUpdate",
        ModuleType::Behavior,
        laser_behavior_update_module_factory,
        laser_behavior_update_module_data_factory,
    )?;

    register_module_override(
        "BoneFXUpdate",
        ModuleType::Behavior,
        bone_fx_update_module_factory,
        bone_fx_update_module_data_factory,
    )?;

    register_module_override(
        "DemoTrapUpdate",
        ModuleType::Behavior,
        demo_trap_update_module_factory,
        demo_trap_update_module_data_factory,
    )?;

    register_module_override(
        "SmartBombTargetHomingUpdate",
        ModuleType::Behavior,
        smart_bomb_target_homing_update_module_factory,
        smart_bomb_target_homing_update_module_data_factory,
    )?;

    register_module_override(
        "TensileFormationUpdate",
        ModuleType::Behavior,
        tensile_formation_update_module_factory,
        tensile_formation_update_module_data_factory,
    )?;

    register_module_override(
        "GenerateMinefieldBehavior",
        ModuleType::Behavior,
        generate_minefield_behavior_module_factory,
        generate_minefield_behavior_module_data_factory,
    )?;

    register_module_override(
        "SpecialAbilityUpdate",
        ModuleType::Behavior,
        special_ability_update_module_factory,
        special_ability_update_module_data_factory,
    )?;

    register_module_override(
        "SpectreGunshipUpdate",
        ModuleType::Behavior,
        spectre_gunship_update_module_factory,
        spectre_gunship_update_module_data_factory,
    )?;

    register_module_override(
        "SpectreGunshipDeploymentUpdate",
        ModuleType::Behavior,
        spectre_gunship_deployment_update_module_factory,
        spectre_gunship_deployment_update_module_data_factory,
    )?;

    register_module_override(
        "ParticleUplinkCannonUpdate",
        ModuleType::Behavior,
        particle_uplink_cannon_update_module_factory,
        particle_uplink_cannon_update_module_data_factory,
    )?;

    register_module_override(
        "BattlePlanUpdate",
        ModuleType::Behavior,
        battle_plan_update_module_factory,
        battle_plan_update_module_data_factory,
    )?;

    register_module_override(
        "LifetimeUpdate",
        ModuleType::Behavior,
        lifetime_update_module_factory,
        lifetime_update_module_data_factory,
    )?;

    register_module_override(
        "MissileLauncherBuildingUpdate",
        ModuleType::Behavior,
        missile_launcher_building_update_module_factory,
        missile_launcher_building_update_module_data_factory,
    )?;

    register_module_override(
        "SpyVisionUpdate",
        ModuleType::Behavior,
        spy_vision_update_module_factory,
        spy_vision_update_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponWhenDeadBehavior",
        ModuleType::Behavior,
        fire_weapon_when_dead_behavior_module_factory,
        fire_weapon_when_dead_behavior_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponWhenDamagedBehavior",
        ModuleType::Behavior,
        fire_weapon_when_damaged_behavior_module_factory,
        fire_weapon_when_damaged_behavior_module_data_factory,
    )?;

    register_module_override(
        "FireWeaponUpdate",
        ModuleType::Behavior,
        fire_weapon_update_module_factory,
        fire_weapon_update_module_data_factory,
    )?;

    register_module_override(
        "FireOCLAfterWeaponCooldownUpdate",
        ModuleType::Behavior,
        fire_ocl_after_weapon_cooldown_update_module_factory,
        fire_ocl_after_weapon_cooldown_update_module_data_factory,
    )?;

    register_module_override(
        "WeaponBonusUpdate",
        ModuleType::Behavior,
        weapon_bonus_update_module_factory,
        weapon_bonus_update_module_data_factory,
    )?;

    register_module_override(
        "EMPUpdate",
        ModuleType::Behavior,
        emp_update_module_factory,
        emp_update_module_data_factory,
    )?;

    register_module_override(
        "StructureCollapseUpdate",
        ModuleType::Behavior,
        structure_collapse_update_module_factory,
        structure_collapse_update_module_data_factory,
    )?;

    register_module_override(
        "FloatUpdate",
        ModuleType::Behavior,
        float_update_module_factory,
        float_update_module_data_factory,
    )?;

    register_module_override(
        "EnemyNearUpdate",
        ModuleType::Behavior,
        enemy_near_update_module_factory,
        enemy_near_update_module_data_factory,
    )?;

    register_module_override(
        "AutoFindHealingUpdate",
        ModuleType::Behavior,
        auto_find_healing_update_module_factory,
        auto_find_healing_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyWarehouseCripplingBehavior",
        ModuleType::Behavior,
        supply_warehouse_crippling_behavior_module_factory,
        supply_warehouse_crippling_behavior_module_data_factory,
    )?;

    register_module_override(
        "BaseRegenerateUpdate",
        ModuleType::Behavior,
        base_regenerate_update_module_factory,
        base_regenerate_update_module_data_factory,
    )?;

    register_module_override(
        "AutoDepositUpdate",
        ModuleType::Behavior,
        auto_deposit_update_module_factory,
        auto_deposit_update_module_data_factory,
    )?;

    register_module_override(
        "PowerPlantUpdate",
        ModuleType::Behavior,
        power_plant_update_module_factory,
        power_plant_update_module_data_factory,
    )?;

    register_module_override(
        "TechBuildingBehavior",
        ModuleType::Behavior,
        tech_building_behavior_module_factory,
        tech_building_behavior_module_data_factory,
    )?;

    register_module_override(
        "PropagandaTowerBehavior",
        ModuleType::Behavior,
        propaganda_tower_behavior_module_factory,
        propaganda_tower_behavior_module_data_factory,
    )?;

    register_module_override(
        "AssistedTargetingUpdate",
        ModuleType::Behavior,
        assisted_targeting_update_module_factory,
        assisted_targeting_update_module_data_factory,
    )?;

    register_module_override(
        "DynamicShroudClearingRangeUpdate",
        ModuleType::Behavior,
        dynamic_shroud_clearing_range_update_module_factory,
        dynamic_shroud_clearing_range_update_module_data_factory,
    )?;

    register_module_override(
        "CleanupHazardUpdate",
        ModuleType::Behavior,
        cleanup_hazard_update_module_factory,
        cleanup_hazard_update_module_data_factory,
    )?;

    register_module_override(
        "FireSpreadUpdate",
        ModuleType::Behavior,
        fire_spread_update_module_factory,
        fire_spread_update_module_data_factory,
    )?;

    register_module_override(
        "CommandButtonHuntUpdate",
        ModuleType::Behavior,
        command_button_hunt_update_module_factory,
        command_button_hunt_update_module_data_factory,
    )?;

    register_module_override(
        "SlavedUpdate",
        ModuleType::Behavior,
        slaved_update_module_factory,
        slaved_update_module_data_factory,
    )?;

    register_module_override(
        "MobMemberSlavedUpdate",
        ModuleType::Behavior,
        mob_member_slaved_update_module_factory,
        mob_member_slaved_update_module_data_factory,
    )?;

    register_module_override(
        "AIUpdateInterface",
        ModuleType::Behavior,
        ai_update_interface_module_factory,
        ai_update_interface_module_data_factory,
    )?;

    register_module_override(
        "TransportAIUpdate",
        ModuleType::Behavior,
        transport_ai_update_module_factory,
        transport_ai_update_module_data_factory,
    )?;

    register_module_override(
        "DeployStyleAIUpdate",
        ModuleType::Behavior,
        deploy_style_ai_update_module_factory,
        deploy_style_ai_update_module_data_factory,
    )?;

    register_module_override(
        "WanderAIUpdate",
        ModuleType::Behavior,
        wander_ai_update_module_factory,
        wander_ai_update_module_data_factory,
    )?;

    register_module_override(
        "JetAIUpdate",
        ModuleType::Behavior,
        jet_ai_update_module_factory,
        jet_ai_update_module_data_factory,
    )?;

    register_module_override(
        "RailedTransportAIUpdate",
        ModuleType::Behavior,
        railed_transport_ai_update_module_factory,
        railed_transport_ai_update_module_data_factory,
    )?;

    register_module_override(
        "RailroadBehavior",
        ModuleType::Behavior,
        railroad_behavior_module_factory,
        railroad_behavior_module_data_factory,
    )?;

    register_module_override(
        "AssaultTransportAIUpdate",
        ModuleType::Behavior,
        assault_transport_ai_update_module_factory,
        assault_transport_ai_update_module_data_factory,
    )?;

    register_module_override(
        "DeliverPayloadAIUpdate",
        ModuleType::Behavior,
        deliver_payload_ai_update_module_factory,
        deliver_payload_ai_update_module_data_factory,
    )?;

    register_module_override(
        "HackInternetAIUpdate",
        ModuleType::Behavior,
        hack_internet_ai_update_module_factory,
        hack_internet_ai_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyTruckAIUpdate",
        ModuleType::Behavior,
        supply_truck_ai_update_module_factory,
        supply_truck_ai_update_module_data_factory,
    )?;

    register_module_override(
        "ChinookAIUpdate",
        ModuleType::Behavior,
        chinook_ai_update_module_factory,
        chinook_ai_update_module_data_factory,
    )?;

    register_module_override(
        "WorkerAIUpdate",
        ModuleType::Behavior,
        worker_ai_update_module_factory,
        worker_ai_update_module_data_factory,
    )?;

    register_module_override(
        "DozerAIUpdate",
        ModuleType::Behavior,
        dozer_ai_update_module_factory,
        dozer_ai_update_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "POWTruckAIUpdate",
        ModuleType::Behavior,
        pow_truck_ai_update_module_factory,
        pow_truck_ai_update_module_data_factory,
    )?;

    register_module_override(
        "BridgeScaffoldBehavior",
        ModuleType::Behavior,
        bridge_scaffold_behavior_module_factory,
        bridge_scaffold_behavior_module_data_factory,
    )?;

    register_module_override(
        "BridgeTowerBehavior",
        ModuleType::Behavior,
        bridge_tower_behavior_module_factory,
        bridge_tower_behavior_module_data_factory,
    )?;

    register_module_override(
        "BridgeBehavior",
        ModuleType::Behavior,
        bridge_behavior_module_factory,
        bridge_behavior_module_data_factory,
    )?;

    register_module_override(
        "CountermeasuresBehavior",
        ModuleType::Behavior,
        countermeasures_behavior_module_factory,
        countermeasures_behavior_module_data_factory,
    )?;

    register_module_override(
        "BunkerBusterBehavior",
        ModuleType::Behavior,
        bunker_buster_behavior_module_factory,
        bunker_buster_behavior_module_data_factory,
    )?;

    register_module_override(
        "FlightDeckBehavior",
        ModuleType::Behavior,
        flight_deck_behavior_module_factory,
        flight_deck_behavior_module_data_factory,
    )?;

    register_module_override(
        "ParkingPlaceBehavior",
        ModuleType::Behavior,
        parking_place_behavior_module_factory,
        parking_place_behavior_module_data_factory,
    )?;

    register_module_override(
        "BattleBusSlowDeathBehavior",
        ModuleType::Behavior,
        battle_bus_slow_death_behavior_module_factory,
        battle_bus_slow_death_behavior_module_data_factory,
    )?;

    register_module_override(
        "DumbProjectileBehavior",
        ModuleType::Behavior,
        dumb_projectile_behavior_module_factory,
        dumb_projectile_behavior_module_data_factory,
    )?;

    register_module_override(
        "AutoHealBehavior",
        ModuleType::Behavior,
        auto_heal_behavior_module_factory,
        auto_heal_behavior_module_data_factory,
    )?;

    register_module_override(
        "HordeUpdate",
        ModuleType::Behavior,
        horde_update_module_factory,
        horde_update_module_data_factory,
    )?;

    register_module_override(
        "RadarUpdate",
        ModuleType::Behavior,
        radar_update_module_factory,
        radar_update_module_data_factory,
    )?;

    register_module_override(
        "SpawnBehavior",
        ModuleType::Behavior,
        spawn_behavior_module_factory,
        spawn_behavior_module_data_factory,
    )?;

    register_module_override(
        "StealthDetectorUpdate",
        ModuleType::Behavior,
        stealth_detector_update_module_factory,
        stealth_detector_update_module_data_factory,
    )?;

    register_module_override(
        "RadiusDecalUpdate",
        ModuleType::Behavior,
        radius_decal_update_module_factory,
        radius_decal_update_module_data_factory,
    )?;

    register_module_override(
        "ToppleUpdate",
        ModuleType::Behavior,
        topple_update_module_factory,
        topple_update_module_data_factory,
    )?;

    register_module_override(
        "StructureToppleUpdate",
        ModuleType::Behavior,
        structure_topple_update_module_factory,
        structure_topple_update_module_data_factory,
    )?;

    register_module_override(
        "FiringTracker",
        ModuleType::Behavior,
        firing_tracker_behavior_module_factory,
        firing_tracker_behavior_module_data_factory,
    )?;

    register_module_override(
        "OverchargeBehavior",
        ModuleType::Behavior,
        overcharge_behavior_module_factory,
        overcharge_behavior_module_data_factory,
    )?;

    register_module_override(
        "RebuildHoleBehavior",
        ModuleType::Behavior,
        rebuild_hole_behavior_module_factory,
        rebuild_hole_behavior_module_data_factory,
    )?;

    register_module_override(
        "QueueProductionExitUpdate",
        ModuleType::Behavior,
        queue_production_exit_behavior_module_factory,
        queue_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "DefaultProductionExitUpdate",
        ModuleType::Behavior,
        default_production_exit_behavior_module_factory,
        default_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "SpawnPointProductionExitUpdate",
        ModuleType::Behavior,
        spawn_point_production_exit_behavior_module_factory,
        spawn_point_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "SupplyCenterProductionExitUpdate",
        ModuleType::Behavior,
        supply_center_production_exit_behavior_module_factory,
        supply_center_production_exit_behavior_module_data_factory,
    )?;

    register_module_override(
        "RepairDockUpdate",
        ModuleType::Behavior,
        repair_dock_update_module_factory,
        repair_dock_update_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "PrisonDockUpdate",
        ModuleType::Behavior,
        prison_dock_update_module_factory,
        prison_dock_update_module_data_factory,
    )?;

    register_module_override(
        "RailedTransportDockUpdate",
        ModuleType::Behavior,
        railed_transport_dock_update_module_factory,
        railed_transport_dock_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyCenterDockUpdate",
        ModuleType::Behavior,
        supply_center_dock_update_module_factory,
        supply_center_dock_update_module_data_factory,
    )?;

    register_module_override(
        "SupplyWarehouseDockUpdate",
        ModuleType::Behavior,
        supply_warehouse_dock_update_module_factory,
        supply_warehouse_dock_update_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "POWTruckBehavior",
        ModuleType::Behavior,
        pow_truck_behavior_module_factory,
        pow_truck_behavior_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "PrisonBehavior",
        ModuleType::Behavior,
        prison_behavior_module_factory,
        prison_behavior_module_data_factory,
    )?;

    #[cfg(feature = "allow_surrender")]
    register_module_override(
        "PropagandaCenterBehavior",
        ModuleType::Behavior,
        propaganda_center_behavior_module_factory,
        propaganda_center_behavior_module_data_factory,
    )?;

    // ========================================================================
    // Additional Missing Module Registrations
    // ========================================================================

    register_module_override(
        "SlowDeathBehavior",
        ModuleType::Behavior,
        slow_death_behavior_module_factory,
        slow_death_behavior_module_data_factory,
    )?;

    register_module_override(
        "MinefieldBehavior",
        ModuleType::Behavior,
        minefield_behavior_module_factory,
        minefield_behavior_module_data_factory,
    )?;

    register_module_override(
        "GrantStealthBehavior",
        ModuleType::Behavior,
        grant_stealth_behavior_module_factory,
        grant_stealth_behavior_module_data_factory,
    )?;

    register_module_override(
        "PhysicsUpdate",
        ModuleType::Behavior,
        physics_update_module_factory,
        physics_update_module_data_factory,
    )?;

    register_module_override(
        "HeightDieUpdate",
        ModuleType::Behavior,
        height_die_update_module_factory,
        height_die_update_module_data_factory,
    )?;

    register_module_override(
        "DeletionUpdate",
        ModuleType::Behavior,
        deletion_update_module_factory,
        deletion_update_module_data_factory,
    )?;

    register_module_override(
        "WaveGuideUpdate",
        ModuleType::Behavior,
        wave_guide_update_module_factory,
        wave_guide_update_module_data_factory,
    )?;

    register_module_override(
        "CheckpointUpdate",
        ModuleType::Behavior,
        checkpoint_update_module_factory,
        checkpoint_update_module_data_factory,
    )?;

    register_module_override(
        "AnimationSteeringUpdate",
        ModuleType::Behavior,
        animation_steering_update_module_factory,
        animation_steering_update_module_data_factory,
    )?;

    register_module_override(
        "PilotFindVehicleUpdate",
        ModuleType::Behavior,
        pilot_find_vehicle_update_module_factory,
        pilot_find_vehicle_update_module_data_factory,
    )?;

    register_module_override(
        "HijackerUpdate",
        ModuleType::Behavior,
        hijacker_update_module_factory,
        hijacker_update_module_data_factory,
    )?;

    register_module_override(
        "HelicopterSlowDeathBehavior",
        ModuleType::Behavior,
        helicopter_slow_death_behavior_module_factory,
        helicopter_slow_death_behavior_module_data_factory,
    )?;

    register_module_override(
        "NeutronMissileSlowDeathUpdate",
        ModuleType::Behavior,
        neutron_missile_slow_death_update_module_factory,
        neutron_missile_slow_death_update_module_data_factory,
    )?;

    register_module_override(
        "NeutronMissileUpdate",
        ModuleType::Behavior,
        neutron_missile_update_module_factory,
        neutron_missile_update_module_data_factory,
    )?;

    register_module_override(
        "FirestormDynamicGeometryInfoUpdate",
        ModuleType::Behavior,
        firestorm_dynamic_geometry_info_update_module_factory,
        firestorm_dynamic_geometry_info_update_module_data_factory,
    )?;

    register_module_override(
        "DynamicGeometryInfoUpdate",
        ModuleType::Behavior,
        dynamic_geometry_info_update_module_factory,
        dynamic_geometry_info_update_module_data_factory,
    )?;

    Ok(())
}

static MODULE_OVERRIDES_READY: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_module_overrides_installed() -> Result<(), String> {
    MODULE_OVERRIDES_READY
        .get_or_init(|| {
            install_module_overrides()?;
            apply_module_overrides_to_existing_templates()?;
            Ok(())
        })
        .clone()
}
