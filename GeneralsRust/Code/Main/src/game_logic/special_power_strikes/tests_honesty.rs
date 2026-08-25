use super::types::*;
use super::*;
use crate::command_system::SpecialPowerType;
use crate::game_logic::{ObjectId, Team};
use glam::Vec3;

#[test]
fn particle_uplink_orbital_kindof_segments_residual_honesty() {
    assert_eq!(PARTICLE_ORBITAL_LASER_KIND_OF, "IMMOBILE");
    assert_eq!(PARTICLE_ORBITAL_LASER_SEGMENTS, 1);
    assert!((PARTICLE_ORBITAL_LASER_ARC_HEIGHT - 0.0).abs() < 0.01);
    assert!((PARTICLE_ORBITAL_LASER_SEGMENT_OVERLAP - 0.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    assert!(!reg.honesty_beam_orbital_kindof_segments_ok());
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, Vec3::ZERO, 10, strike_id);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert_eq!(f.orbital_kindof_immobile_armed, 1);
        assert_eq!(f.orbital_segments_armed, 1);
        assert_eq!(f.orbital_arc_height_armed, 1);
    }
    assert!(reg.honesty_beam_orbital_kindof_segments_ok());
    assert!(reg.honesty_beam_vision_shroud_ok());
}

#[test]
fn scud_missile_ai_residual_honesty() {
    assert!(!SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET);
    assert_eq!(SCUD_STORM_MISSILE_FUEL_LIFETIME, 0);
    assert!((SCUD_STORM_MISSILE_INITIAL_VELOCITY - 0.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING - 500.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING - 200.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_MISSILE_IGNITION_FX, "FX_ScudStormIgnition");

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(100.0, 0.0, 100.0),
        0,
    );
    assert!(!reg.honesty_scud_missile_ai_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_missile_ai_applications, 1);
    }
    assert!(reg.honesty_scud_missile_ai_ok());
    assert!(reg.honesty_scud_object_params_ok());
    assert!(reg.honesty_scud_geometry_ok());
}

#[test]
fn spectre_howitzer_shell_death_generic_residual_honesty() {
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX,
        "FX_GenericMissileDeath"
    );

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_shell_death_generic_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_shell_death_generic_applications, 1);
    }
    assert!(reg.honesty_howitzer_shell_death_generic_ok());
    assert!(reg.honesty_howitzer_shell_dumb_projectile_ok());
}

#[test]
fn scud_fire_weapon_when_dead_residual_honesty() {
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_WEAPON_BASE,
        "ScudStormDamageWeapon"
    );
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_WEAPON_UPGRADED,
        "ScudStormDamageWeaponUpgraded"
    );
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_CONFLICTS_WITH,
        "Upgrade_GLAAnthraxBeta"
    );
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_TRIGGERED_BY,
        "Upgrade_GLAAnthraxBeta"
    );
    assert!(SCUD_STORM_MISSILE_DEATH_BASE_STARTS_ACTIVE);
    assert!(!SCUD_STORM_MISSILE_DEATH_UPGRADED_STARTS_ACTIVE);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(100.0, 0.0, 100.0),
        0,
    );
    assert!(!reg.honesty_scud_fire_weapon_when_dead_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_fire_weapon_when_dead_applications, 1);
    }
    assert!(reg.honesty_scud_fire_weapon_when_dead_ok());
    assert!(reg.honesty_scud_missile_ai_ok());
}

#[test]
fn scud_body_draw_and_locomotor_appearance_residual_honesty() {
    assert!((SCUD_STORM_MISSILE_INITIAL_HEALTH - 10000.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_MISSILE_EDITOR_SORTING, "SYSTEM");
    assert!(SCUD_STORM_MISSILE_OK_TO_CHANGE_MODEL_COLOR);
    assert_eq!(SCUD_STORM_MISSILE_DAMAGED_MODEL, "NONE");
    assert_eq!(SCUD_STORM_MISSILE_LOCOMOTOR_SURFACES, "AIR");
    assert_eq!(SCUD_STORM_MISSILE_LOCOMOTOR_APPEARANCE, "THRUST");
    assert!(SCUD_STORM_MISSILE_LOCOMOTOR_ALLOW_AIRBORNE_MOTIVE);
    assert!((SCUD_STORM_MISSILE_LOCOMOTOR_BRAKING - 0.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(50.0, 0.0, 50.0),
        0,
    );
    assert!(!reg.honesty_scud_body_draw_params_ok());
    assert!(!reg.honesty_scud_locomotor_appearance_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(50.0, 0.0, 50.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_body_draw_params_applications, 1);
        assert_eq!(s.scud_locomotor_appearance_applications, 1);
    }
    assert!(reg.honesty_scud_body_draw_params_ok());
    assert!(reg.honesty_scud_locomotor_appearance_ok());
}

#[test]
fn spectre_howitzer_shell_design_params_residual_honesty() {
    assert!(!SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_INCLUDES_STRUCTURES);
    assert!((SPECTRE_HOWITZER_SHELL_INITIAL_HEALTH - 100.0).abs() < 0.01);
    assert_eq!(SPECTRE_HOWITZER_SHELL_DISPLAY_NAME, "OBJECT:Missile");
    assert_eq!(SPECTRE_HOWITZER_SHELL_EDITOR_SORTING, "SYSTEM");
    assert!(SPECTRE_HOWITZER_SHELL_OK_TO_CHANGE_MODEL_COLOR);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_shell_design_params_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_shell_design_params_applications, 1);
    }
    assert!(reg.honesty_howitzer_shell_design_params_ok());
    assert!(reg.honesty_howitzer_shell_death_generic_ok());
}

#[test]
fn particle_uplink_single_beam_premul_residual_honesty() {
    let ia = PARTICLE_ORBITAL_LASER_INNER_COLOR.3;
    let (r, g, b, a) = particle_orbital_single_beam_color_premul();
    assert!((r - ia).abs() < 0.01);
    assert!((g - ia).abs() < 0.01);
    assert!((b - ia).abs() < 0.01);
    assert!((a - ia).abs() < 0.01);
    assert!(
        HostSpecialPowerStrikeRegistry::new().honesty_beam_single_beam_premul_ok() || {
            // honesty is pure constant residual — true without live field
            let reg = HostSpecialPowerStrikeRegistry::new();
            reg.honesty_beam_single_beam_premul_ok()
        }
    );
    assert!((particle_orbital_single_beam_color_premul().0 - ia).abs() < 0.01);
}

#[test]
fn scud_destroy_die_locomotor_name_residual_honesty() {
    assert!(SCUD_STORM_MISSILE_DESTROY_DIE);
    assert_eq!(
        SCUD_STORM_MISSILE_LOCOMOTOR_NAME,
        "SCUDStormMissileLocomotor"
    );
    assert_eq!(SCUD_STORM_MISSILE_DAMAGE_FX, "None");

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(50.0, 0.0, 50.0),
        0,
    );
    assert!(!reg.honesty_scud_destroy_die_locomotor_name_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(50.0, 0.0, 50.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_destroy_die_locomotor_name_applications, 1);
    }
    assert!(reg.honesty_scud_destroy_die_locomotor_name_ok());
    assert!(reg.honesty_scud_locomotor_appearance_ok());
}

#[test]
fn spectre_howitzer_shell_locomotor_template_residual_honesty() {
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_LOCOMOTOR_NAME,
        "SpectreHowitzerShellLocomotor"
    );
    assert_eq!(SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SURFACES, "AIR");
    assert_eq!(SPECTRE_HOWITZER_SHELL_LOCOMOTOR_APPEARANCE, "THRUST");
    assert!((SPECTRE_HOWITZER_SHELL_LOCOMOTOR_MIN_SPEED - 1111.0).abs() < 0.01);
    assert!((SPECTRE_HOWITZER_SHELL_LOCOMOTOR_ACCEL - 9160.0).abs() < 0.01);
    assert!((SPECTRE_HOWITZER_SHELL_LOCOMOTOR_TURN_RATE - 99999.0).abs() < 0.01);
    assert!((SPECTRE_HOWITZER_SHELL_LOCOMOTOR_MAX_THRUST_ANGLE - 90.0).abs() < 0.01);
    assert!(SPECTRE_HOWITZER_SHELL_LOCOMOTOR_ALLOW_AIRBORNE);
    assert_eq!(SPECTRE_HOWITZER_SHELL_DAMAGE_FX, "None");

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_shell_locomotor_template_ok());
    assert!(!reg.honesty_howitzer_shell_damage_fx_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_shell_locomotor_template_applications, 1);
        assert_eq!(f.howitzer_shell_damage_fx_applications, 1);
    }
    assert!(reg.honesty_howitzer_shell_locomotor_template_ok());
    assert!(reg.honesty_howitzer_shell_damage_fx_ok());
    assert!(reg.honesty_howitzer_shell_design_params_ok());
}

#[test]
fn particle_uplink_connector_kindof_defaults_residual_honesty() {
    assert_eq!(PARTICLE_CONNECTOR_KIND_OF, "IMMOBILE");
    assert_eq!(PARTICLE_CONNECTOR_SEGMENTS, 1);
    assert!((PARTICLE_CONNECTOR_ARC_HEIGHT - 0.0).abs() < 0.01);
    assert_eq!(PARTICLE_CONNECTOR_MAX_INTENSITY_FRAMES, 0);
    assert_eq!(PARTICLE_CONNECTOR_FADE_FRAMES, 0);
    assert!(!PARTICLE_CONNECTOR_TILE);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    assert!(!reg.honesty_beam_connector_kindof_defaults_ok());
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, Vec3::ZERO, 10, strike_id);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert_eq!(f.connector_kindof_immobile_armed, 1);
        assert_eq!(f.connector_segments_armed, 1);
        assert_eq!(f.connector_max_intensity_fade_armed, 1);
        assert_eq!(f.connector_tile_no_armed, 1);
    }
    assert!(reg.honesty_beam_connector_kindof_defaults_ok());
    assert!(reg.honesty_beam_orbital_kindof_segments_ok());
}

#[test]
fn particle_uplink_remnant_object_params_residual_honesty() {
    assert_eq!(PARTICLE_REMNANT_KIND_OF, "NO_COLLIDE UNATTACKABLE IMMOBILE");
    assert!((PARTICLE_REMNANT_MAX_HEALTH - 50.0).abs() < 0.01);
    assert!((PARTICLE_REMNANT_INITIAL_HEALTH - 50.0).abs() < 0.01);
    assert_eq!(PARTICLE_REMNANT_EDITOR_SORTING, "SYSTEM");
    assert_eq!(PARTICLE_REMNANT_BODY, "ImmortalBody");

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(!reg.honesty_beam_remnant_object_params_ok());
    let rid = reg.spawn_remnant_field(ObjectId(1), Team::USA, Vec3::new(10.0, 0.0, 10.0), 0, 0, 0);
    {
        let f = reg.remnant_fields().iter().find(|r| r.id == rid).unwrap();
        assert_eq!(f.remnant_object_params_applications, 1);
    }
    assert!(reg.honesty_beam_remnant_object_params_ok());
    assert!(reg.honesty_beam_remnant_ok());
}

#[test]
fn scud_death_fire_ocl_and_speed_table_residual_honesty() {
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_FIRE_OCL_BASE,
        "OCL_PoisonFieldLarge"
    );
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_FIRE_OCL_UPGRADED,
        "OCL_PoisonFieldUpgradedLarge"
    );
    assert!((SCUD_STORM_MISSILE_LOCOMOTOR_SPEED_DAMAGED - 200.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_LOCOMOTOR_MIN_SPEED - 100.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_LOCOMOTOR_MAX_THRUST_ANGLE - 45.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(80.0, 0.0, 80.0),
        0,
    );
    assert!(!reg.honesty_scud_death_fire_ocl_ok());
    assert!(!reg.honesty_scud_locomotor_speed_table_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(80.0, 0.0, 80.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_death_fire_ocl_applications, 1);
        assert_eq!(s.scud_locomotor_speed_table_applications, 1);
    }
    assert!(reg.honesty_scud_death_fire_ocl_ok());
    assert!(reg.honesty_scud_locomotor_speed_table_ok());
    assert!(reg.honesty_scud_destroy_die_locomotor_name_ok());
}

#[test]
fn spectre_howitzer_gun_aim_params_residual_honesty() {
    assert!((SPECTRE_HOWITZER_ACCEPTABLE_AIM_DELTA - 180.0).abs() < 0.01);
    assert!((SPECTRE_HOWITZER_ATTACK_RANGE - 2222.0).abs() < 0.01);
    assert_eq!(
        SPECTRE_HOWITZER_PROJECTILE_COLLIDES_WITH,
        "STRUCTURES WALLS"
    );
    assert!(SPECTRE_HOWITZER_ANTI_GROUND);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_gun_aim_params_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_gun_aim_params_applications, 1);
    }
    assert!(reg.honesty_howitzer_gun_aim_params_ok());
    assert!(reg.honesty_howitzer_shell_locomotor_template_ok());
}

#[test]
fn scud_death_damage_table_residual_honesty() {
    assert!((SCUD_STORM_PRIMARY_DAMAGE - 500.0).abs() < 0.01);
    assert!((SCUD_STORM_PRIMARY_RADIUS - 50.0).abs() < 0.01);
    assert!((SCUD_STORM_SECONDARY_DAMAGE - 150.0).abs() < 0.01);
    assert!((SCUD_STORM_SECONDARY_DAMAGE_UPGRADED - 200.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_MISSILE_DEATH_DAMAGE_TYPE, "EXPLOSION");
    assert_eq!(SCUD_STORM_MISSILE_DEATH_DEATH_TYPE, "EXPLODED");
    assert!((SCUD_STORM_MISSILE_DEATH_WEAPON_SPEED - 600.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_DEATH_ATTACK_RANGE - 200.0).abs() < 0.01);
    assert_eq!(
        SCUD_STORM_MISSILE_DEATH_FIRE_FX,
        "ScudStormMissileDetonation"
    );

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(80.0, 0.0, 80.0),
        0,
    );
    assert!(!reg.honesty_scud_death_damage_table_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(80.0, 0.0, 80.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_death_damage_table_applications, 1);
    }
    assert!(reg.honesty_scud_death_damage_table_ok());
    assert!(reg.honesty_scud_death_fire_ocl_ok());
}

#[test]
fn spectre_howitzer_gun_fire_params_residual_honesty() {
    assert!((SPECTRE_HOWITZER_PRIMARY_DAMAGE - 80.0).abs() < 0.01);
    assert!((SPECTRE_HOWITZER_RADIUS - 25.0).abs() < 0.01);
    assert_eq!(SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_MS, 777);
    assert_eq!(SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_FRAMES, 23);
    assert_eq!(SPECTRE_HOWITZER_DAMAGE_TYPE, "EXPLOSION");
    assert_eq!(SPECTRE_HOWITZER_DEATH_TYPE, "EXPLODED");
    assert_eq!(
        SPECTRE_HOWITZER_RADIUS_DAMAGE_AFFECTS,
        "ALLIES ENEMIES NEUTRALS"
    );
    assert_eq!(SPECTRE_HOWITZER_CLIP_SIZE, 0);
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_LOCOMOTOR_GROUP_PRIORITY,
        "MOVES_BACK"
    );

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_gun_fire_params_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_gun_fire_params_applications, 1);
    }
    assert!(reg.honesty_howitzer_gun_fire_params_ok());
    assert!(reg.honesty_howitzer_gun_aim_params_ok());
}

#[test]
fn particle_uplink_remnant_fire_deletion_residual_honesty() {
    assert!(PARTICLE_REMNANT_FIRE_WEAPON_UPDATE);
    assert!(PARTICLE_REMNANT_DELETION_UPDATE);
    assert_eq!(
        PARTICLE_REMNANT_WEAPON_NAME,
        "ParticleUplinkCannonBeamTrailRemnantWeapon"
    );
    assert_eq!(PARTICLE_REMNANT_DAMAGE_TYPE, "PARTICLE_BEAM");
    assert_eq!(PARTICLE_REMNANT_DEATH_TYPE, "BURNED");
    assert_eq!(PARTICLE_REMNANT_MIN_LIFETIME_MS, 4000);
    assert_eq!(PARTICLE_REMNANT_MAX_LIFETIME_MS, 4000);
    assert_eq!(PARTICLE_REMNANT_DURATION_FRAMES, 120);
    assert_eq!(PARTICLE_REMNANT_TICK_INTERVAL_FRAMES, 7);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(!reg.honesty_beam_remnant_fire_deletion_ok());
    let _id = reg.spawn_remnant_field(ObjectId(1), Team::USA, Vec3::new(10.0, 0.0, 10.0), 0, 0, 0);
    {
        let f = &reg.remnant_fields()[0];
        assert_eq!(f.remnant_fire_deletion_applications, 1);
    }
    assert!(reg.honesty_beam_remnant_fire_deletion_ok());
    assert!(reg.honesty_beam_remnant_object_params_ok());
    assert!(reg.honesty_beam_remnant_ok());
}

#[test]
fn scud_weapon_launch_residual_honesty() {
    assert_eq!(SCUD_STORM_CLIP_SIZE, 9);
    assert_eq!(SCUD_STORM_CLIP_SIZE, SCUD_STORM_MISSILE_COUNT);
    assert_eq!(SCUD_STORM_CLIP_RELOAD_TIME_MS, 10000);
    assert_eq!(SCUD_STORM_CLIP_RELOAD_FRAMES, 300);
    assert!(SCUD_STORM_AUTO_RELOADS_CLIP);
    assert!((SCUD_STORM_SCATTER_SCALAR - 120.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_SCATTER_TARGET_COUNT, 9);
    assert_eq!(SCUD_STORM_SCATTER_TARGETS.len(), 9);
    assert!((SCUD_STORM_ACCEPTABLE_AIM_DELTA - 180.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_PROJECTILE_COLLIDES_WITH, "STRUCTURES");
    assert_eq!(SCUD_STORM_PROJECTILE_OBJECT, "ScudStormMissile");
    assert_eq!(SCUD_STORM_DELAY_BETWEEN_MIN_MS, 100);
    assert_eq!(SCUD_STORM_DELAY_BETWEEN_MAX_MS, 1000);
    assert_eq!(SCUD_STORM_DELAY_BETWEEN_MIN_FRAMES, 3);
    assert_eq!(SCUD_STORM_DELAY_BETWEEN_MAX_FRAMES, 30);
    assert_eq!(SCUD_STORM_MISSILE_DEATH_CLIP_RELOAD_TIME_MS, 0);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(80.0, 0.0, 80.0),
        0,
    );
    assert!(!reg.honesty_scud_weapon_launch_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(80.0, 0.0, 80.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_weapon_launch_applications, 1);
    }
    assert!(reg.honesty_scud_weapon_launch_ok());
    assert!(reg.honesty_scud_death_damage_table_ok());
}

#[test]
fn spectre_howitzer_gun_anti_params_residual_honesty() {
    assert!(!SPECTRE_HOWITZER_ANTI_AIRBORNE_VEHICLE);
    assert!(!SPECTRE_HOWITZER_ANTI_AIRBORNE_INFANTRY);
    assert!(!SPECTRE_HOWITZER_ANTI_SMALL_MISSILE);
    assert!(!SPECTRE_HOWITZER_ANTI_BALLISTIC_MISSILE);
    assert!(SPECTRE_HOWITZER_ANTI_GROUND);
    assert_eq!(SPECTRE_HOWITZER_PROJECTILE_OBJECT, "SpectreHowitzerShell");
    assert_eq!(
        SPECTRE_HOWITZER_PROJECTILE_OBJECT,
        SPECTRE_HOWITZER_SHELL_OBJECT
    );
    assert_eq!(SPECTRE_HOWITZER_CONTINUOUS_FIRE_COAST_MS, 2000);
    assert_eq!(SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES, 60);
    assert_eq!(SPECTRE_HOWITZER_CONTINUOUS_FIRE_ONE, 1);
    assert_eq!(SPECTRE_HOWITZER_CONTINUOUS_FIRE_TWO, 2);
    assert!(SPECTRE_HOWITZER_VETERANCY_FIRE_FX.contains("GenericTankGunNoTracer"));

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_gun_anti_params_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_gun_anti_params_applications, 1);
    }
    assert!(reg.honesty_howitzer_gun_anti_params_ok());
    assert!(reg.honesty_howitzer_gun_fire_params_ok());
}

#[test]
fn scud_weapon_special_residual_honesty() {
    assert!((SCUD_STORM_WEAPON_PRIMARY_DAMAGE - 0.0).abs() < 0.01);
    assert!((SCUD_STORM_WEAPON_PRIMARY_RADIUS - 0.0).abs() < 0.01);
    assert!((SCUD_STORM_WEAPON_ATTACK_RANGE - 999_999.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_WEAPON_DAMAGE_TYPE, "EXPLOSION");
    assert_eq!(SCUD_STORM_WEAPON_DEATH_TYPE, "EXPLODED");
    assert!((SCUD_STORM_WEAPON_SPEED - 99_999.0).abs() < 0.01);
    assert!((SCUD_STORM_SCATTER_RADIUS - 0.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_PRE_ATTACK_TYPE, "PER_CLIP");
    assert_eq!(SCUD_STORM_PRE_ATTACK_DELAY_MS, 3000);
    assert_eq!(SCUD_STORM_PRE_ATTACK_FRAMES, 90);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(80.0, 0.0, 80.0),
        0,
    );
    assert!(!reg.honesty_scud_weapon_special_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(80.0, 0.0, 80.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_weapon_special_applications, 1);
    }
    assert!(reg.honesty_scud_weapon_special_ok());
    assert!(reg.honesty_scud_weapon_launch_ok());
}

#[test]
fn spectre_gattling_gun_params_residual_honesty() {
    assert!(!SPECTRE_GATTLING_ANTI_AIRBORNE_VEHICLE);
    assert!(!SPECTRE_GATTLING_ANTI_AIRBORNE_INFANTRY);
    assert!(!SPECTRE_GATTLING_ANTI_SMALL_MISSILE);
    assert!(!SPECTRE_GATTLING_ANTI_BALLISTIC_MISSILE);
    assert!(SPECTRE_GATTLING_ANTI_GROUND);
    assert_eq!(SPECTRE_GATTLING_PROJECTILE_OBJECT, "NONE");
    assert!((SPECTRE_GATTLING_PRIMARY_RADIUS - 0.0).abs() < 0.01);
    assert_eq!(SPECTRE_GATTLING_DAMAGE_TYPE, "Gattling");
    assert_eq!(SPECTRE_GATTLING_DEATH_TYPE, "NORMAL");
    assert!((SPECTRE_GATTLING_WEAPON_SPEED - 999_999.0).abs() < 0.01);
    assert!((SPECTRE_GATTLING_ATTACK_RANGE - 2222.0).abs() < 0.01);
    assert!(SPECTRE_GATTLING_FIRE_FX.contains("SpectreGattlingMuzzleFlash"));
    assert!(SPECTRE_GATTLING_VETERANCY_FIRE_FX.contains("RedTracers"));
    assert_eq!(SPECTRE_GATTLING_CLIP_SIZE, 0);
    assert_eq!(SPECTRE_GATTLING_CLIP_RELOAD_TIME_MS, 0);
    assert_eq!(SPECTRE_GATTLING_DELAY_BETWEEN_SHOTS_MS, 100);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_gattling_gun_params_ok());
    // Both howitzer + gattling are due at spawn_frame residual.
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.gattling_gun_params_applications, 1);
        assert!(f.gattling_ticks >= 1);
    }
    assert!(reg.honesty_gattling_gun_params_ok());
    assert!(reg.honesty_gattling_ok());
}

#[test]
fn scud_missile_ai_defaults_residual_honesty() {
    assert_eq!(SCUD_STORM_MISSILE_IGNITION_DELAY_FRAMES, 0);
    assert!(!SCUD_STORM_MISSILE_USE_WEAPON_SPEED);
    assert!(!SCUD_STORM_MISSILE_DETONATE_ON_NO_FUEL);
    assert!((SCUD_STORM_MISSILE_DISTANCE_FOR_LOCK - 75.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_DISTANCE_SCATTER_WHEN_JAMMED - 75.0).abs() < 0.01);
    assert!(!SCUD_STORM_MISSILE_DETONATE_CALLS_KILL);
    assert_eq!(SCUD_STORM_MISSILE_KILL_SELF_DELAY_FRAMES, 3);
    assert_eq!(
        SCUD_STORM_PROJECTILE_DETONATION_FX,
        "ScudStormMissileDetonation"
    );
    assert_eq!(
        SCUD_STORM_WEAPON_RADIUS_DAMAGE_AFFECTS,
        "ALLIES ENEMIES NEUTRALS"
    );

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(100.0, 0.0, 100.0),
        0,
    );
    assert!(!reg.honesty_scud_missile_ai_defaults_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_missile_ai_defaults_applications, 1);
    }
    assert!(reg.honesty_scud_missile_ai_defaults_ok());
    assert!(reg.honesty_scud_missile_ai_ok());
    assert!(reg.honesty_scud_weapon_special_ok());
}

#[test]
fn particle_uplink_remnant_immortal_body_residual_honesty() {
    assert!((PARTICLE_REMNANT_IMMORTAL_HEALTH_FLOOR - 1.0).abs() < 0.01);
    assert!(PARTICLE_REMNANT_IMMORTAL_NEVER_DEAD);
    assert_eq!(PARTICLE_REMNANT_BODY, "ImmortalBody");
    assert!((immortal_body_apply_health_delta(50.0, -100.0) - 1.0).abs() < 0.01);
    assert!((immortal_body_apply_health_delta(50.0, -10.0) - 40.0).abs() < 0.01);
    assert!((immortal_body_apply_health_delta(1.0, -5.0) - 1.0).abs() < 0.01);
    assert!((immortal_body_apply_health_delta(10.0, 5.0) - 15.0).abs() < 0.01);
    assert!(honesty_immortal_body_health_floor(50.0, -100.0, 1.0));
    assert!(!honesty_immortal_body_health_floor(50.0, -100.0, 0.0));

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(!reg.honesty_beam_remnant_immortal_body_ok());
    let rid = reg.spawn_remnant_field(ObjectId(1), Team::USA, Vec3::new(10.0, 0.0, 10.0), 0, 0, 0);
    {
        let f = reg.remnant_fields().iter().find(|r| r.id == rid).unwrap();
        assert_eq!(f.remnant_immortal_body_applications, 1);
    }
    assert!(reg.honesty_beam_remnant_immortal_body_ok());
    assert!(reg.honesty_beam_remnant_object_params_ok());
    assert!(reg.honesty_beam_remnant_fire_deletion_ok());
    assert!(reg.honesty_beam_remnant_ok());
}

#[test]
fn particle_supw_outer_color_residual_honesty() {
    assert!(honesty_particle_supw_outer_color());
    let (r, g, b, a) = PARTICLE_SUPW_CONNECTOR_OUTER_COLOR;
    assert!((r - 1.0).abs() < 0.01);
    assert!((g - 0.0).abs() < 0.01);
    assert!((b - 1.0).abs() < 0.01);
    assert!((a - 150.0 / 255.0).abs() < 0.01);
    // Normal residual is blue, SupW is magenta.
    assert!((PARTICLE_CONNECTOR_OUTER_COLOR.2 - 1.0).abs() < 0.01);
    assert!((PARTICLE_CONNECTOR_OUTER_COLOR.0 - 0.0).abs() < 0.01);
    assert_eq!(
        PARTICLE_SUPW_MEDIUM_CONNECTOR,
        "SupW_ParticleUplinkCannon_MediumConnectorLaser"
    );
    assert_eq!(
        PARTICLE_SUPW_ORBITAL_LASER,
        "SupW_ParticleUplinkCannon_OrbitalLaser"
    );
}

#[test]
fn deletion_update_sleep_delay_residual_honesty() {
    assert!(honesty_deletion_update_sleep_delay());
    assert_eq!(particle_remnant_deletion_sleep_frames(), 120);
    assert_eq!(deletion_update_calc_sleep_delay(0, 0, 0), 1);
    assert_eq!(deletion_update_calc_sleep_delay(10, 10, 0), 10);
    let d = deletion_update_calc_sleep_delay(2, 5, 0);
    assert!((2..=5).contains(&d));
    let d = deletion_update_calc_sleep_delay(2, 5, 3);
    assert!((2..=5).contains(&d));
}

#[test]
fn particle_uplink_sound_residual_pack_honesty() {
    // Retail sound/FX name residual pack.
    assert!(honesty_particle_sound_loops());
    assert_eq!(
        PARTICLE_POWERUP_AUDIO,
        "ParticleUplinkCannon_PowerupSoundLoop"
    );
    assert_eq!(
        PARTICLE_UNPACK_AUDIO,
        "ParticleUplinkCannon_UnpackToIdleSoundLoop"
    );
    assert_eq!(
        PARTICLE_FIRING_TO_PACK_AUDIO,
        "ParticleUplinkCannon_FiringToPackSoundLoop"
    );
    assert_eq!(
        PARTICLE_BEAM_AUDIO,
        "ParticleUplinkCannon_GroundAnnihilationSoundLoop"
    );
    assert_eq!(
        PARTICLE_BEAM_LAUNCH_FX,
        "FX_ParticleUplinkCannon_BeamLaunchIteration"
    );
    assert_eq!(PARTICLE_LAUNCH_FX_INTERVAL_FRAMES, 30);
    assert_eq!(
        PARTICLE_GROUND_HIT_FX,
        "FX_ParticleUplinkCannon_BeamHitsGround"
    );

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(!reg.honesty_beam_sound_residual_ok());
    let id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::new(50.0, 0.0, 50.0),
        0,
    );
    // PREPARING residual seeds UnpackToIdle sound on queue.
    {
        let s = reg.get(id).unwrap();
        assert!(s.particle_unpack_audio_applications >= 1);
        assert_eq!(s.particle_status, ParticleUplinkStatus::Preparing);
    }
    {
        let loops = reg.take_puc_loop_audio_this_frame();
        assert!(
            loops
                .iter()
                .any(|(_, _, cue)| *cue == PARTICLE_UNPACK_AUDIO),
            "PREPARING must note UnpackToIdleSoundLoop"
        );
    }
    // Long impact window can also hit CHARGING → PoweringUpSoundLoop.
    // begin_charge = impact - (ReadyDelay+RaiseAntenna+BeginCharge) =
    // impact - 350; use impact_frame 350 so frame 0 is CHARGING.
    // Default impact_delay (BeamTravelTime 75f) only covers PREPARING at activate.
    if let Some(s) = reg.strikes.get_mut(&id) {
        s.impact_frame = 350;
        s.particle_status = ParticleUplinkStatus::Idle;
        s.particle_status_peak = ParticleUplinkStatus::Idle;
    }
    reg.advance_particle_intensity_schedule(0);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.particle_status, ParticleUplinkStatus::Charging);
        assert!(s.particle_powerup_audio_applications >= 1);
    }
    {
        let loops = reg.take_puc_loop_audio_this_frame();
        assert!(
            loops
                .iter()
                .any(|(_, _, cue)| *cue == PARTICLE_POWERUP_AUDIO),
            "CHARGING must note PoweringUpSoundLoop"
        );
    }
    // Beam spawn arms GroundAnnihilation + FiringToPack + sound pack.
    reg.record_impact_complete(id, 0.0, 0, 0);
    {
        let f = &reg.beam_fields()[0];
        assert_eq!(f.ground_annihilation_audio_applications, 1);
        assert_eq!(f.firing_to_pack_audio_applications, 1);
        assert_eq!(f.sound_residual_pack_armed, 1);
        assert!(f.beam_launch_fx_applications >= 1);
    }
    {
        let loops = reg.take_puc_loop_audio_this_frame();
        assert!(
            loops
                .iter()
                .any(|(_, _, cue)| *cue == PARTICLE_FIRING_TO_PACK_AUDIO),
            "FIRING beam spawn must note FiringToPackSoundLoop"
        );
    }
    assert!(reg.honesty_beam_sound_residual_ok());
}

#[test]
fn particle_uplink_scorch_pack_residual_honesty() {
    assert!(honesty_particle_scorch_pack());
    assert!((PARTICLE_SCORCH_MARK_SCALAR - 2.4).abs() < 0.01);
    assert!((PARTICLE_SWATH_OF_DEATH_DISTANCE - 200.0).abs() < 0.01);
    assert!((PARTICLE_SWATH_OF_DEATH_AMPLITUDE - 50.0).abs() < 0.01);
    assert!((PARTICLE_MANUAL_DRIVING_SPEED - 20.0).abs() < 0.01);
    assert!((PARTICLE_MANUAL_FAST_DRIVING_SPEED - 40.0).abs() < 0.01);
    assert_eq!(PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES, 15);
    assert_eq!(PARTICLE_TOTAL_SCORCH_MARKS, 20);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    {
        let f = &reg.beam_fields()[0];
        assert_eq!(f.scorch_scalar_pack_armed, 1);
    }
    let spawn = reg.beam_fields()[0].spawn_frame;
    let events = reg.apply_due_beam_scorch_reveals(spawn);
    assert_eq!(events.len(), 1);
    // Scorch radius = (50 / 3.4) * 2.4 * width_scalar(0) ≈ 0 at spawn grow.
    // Peak scorch at full width: (50/3.4)*2.4 ≈ 35.29.
    let peak_scorch = particle_scorch_radius(spawn, spawn + PARTICLE_WIDTH_GROW_FRAMES);
    assert!(
        (peak_scorch
            - (PARTICLE_BEAM_RADIUS / PARTICLE_DAMAGE_RADIUS_SCALAR * PARTICLE_SCORCH_MARK_SCALAR))
            .abs()
            < 0.1
    );
    assert!(reg.honesty_beam_scorch_ok());
    assert!((particle_manual_speed_per_frame(false) - 20.0 / 30.0).abs() < 0.01);
    assert!((particle_manual_speed_per_frame(true) - 40.0 / 30.0).abs() < 0.01);
}

#[test]
fn point_defense_laser_lifetime_update_residual_honesty() {
    assert!(honesty_point_defense_laser_lifetime());
    assert_eq!(
        POINT_DEFENSE_DRONE_LASER_BEAM,
        "SupW_PointDefenseDroneLaserBeam"
    );
    assert_eq!(POINT_DEFENSE_LASER_BEAM, "PointDefenseLaserBeam");
    assert_eq!(POINT_DEFENSE_LASER_MIN_LIFETIME_MS, 95);
    assert_eq!(POINT_DEFENSE_LASER_MAX_LIFETIME_MS, 95);
    // ceil(95*30/1000) = ceil(2.85) = 3 frames.
    assert_eq!(POINT_DEFENSE_LASER_LIFETIME_FRAMES, 3);
    assert_eq!(duration_ms_to_logic_frames(95), 3);
    assert_eq!(duration_ms_to_logic_frames(0), 0);
    assert_eq!(duration_ms_to_logic_frames(1000), 30);
    assert_eq!(
        lifetime_update_fixed_frames(
            POINT_DEFENSE_LASER_MIN_LIFETIME_MS,
            POINT_DEFENSE_LASER_MAX_LIFETIME_MS
        ),
        3
    );
    let reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_point_defense_laser_lifetime_ok());
}

#[test]
fn particle_uplink_flammable_update_residual_honesty() {
    assert!(honesty_particle_uplink_flammable());
    assert_eq!(PARTICLE_UPLINK_AFLAME_DURATION_MS, 5000);
    assert_eq!(PARTICLE_UPLINK_AFLAME_DURATION_FRAMES, 150);
    assert!((PARTICLE_UPLINK_AFLAME_DAMAGE_AMOUNT - 5.0).abs() < 0.01);
    assert_eq!(PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_MS, 500);
    assert_eq!(PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_FRAMES, 15);
    assert_eq!(duration_ms_to_logic_frames(5000), 150);
    assert_eq!(duration_ms_to_logic_frames(500), 15);
    let reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_particle_uplink_flammable_ok());
}

#[test]
fn particle_uplink_outer_node_flare_pack_residual_honesty() {
    assert!(honesty_particle_outer_node_flare_pack());
    assert_eq!(
        PARTICLE_OUTER_NODE_LIGHT_FLARE,
        "ParticleUplinkCannon_OuterNodeLightFlare"
    );
    assert_eq!(
        PARTICLE_OUTER_NODE_MEDIUM_FLARE,
        "ParticleUplinkCannon_OuterNodeMediumFlare"
    );
    assert_eq!(
        PARTICLE_OUTER_NODE_INTENSE_FLARE,
        "ParticleUplinkCannon_OuterNodeIntenseFlare"
    );
    assert_eq!(
        PARTICLE_LASER_BASE_READY_FLARE,
        "ParticleUplinkCannon_LaserBaseReadyToFire"
    );
    assert_eq!(
        PARTICLE_CONNECTOR_MEDIUM_LASER,
        "ParticleUplinkCannon_MediumConnectorLaser"
    );
    assert_eq!(
        PARTICLE_CONNECTOR_INTENSE_LASER,
        "ParticleUplinkCannon_IntenseConnectorLaser"
    );
    assert_eq!(PARTICLE_OUTER_EFFECT_NUM_BONES, 5);
    // Intensity → flare name residual table.
    assert_eq!(
        ParticleIntensity::Intense.outer_flare_name(),
        PARTICLE_OUTER_NODE_INTENSE_FLARE
    );
    assert_eq!(
        ParticleIntensity::Intense.connector_laser_name(),
        PARTICLE_CONNECTOR_INTENSE_LASER
    );

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(!reg.honesty_beam_outer_node_flare_pack_ok());
    let id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    {
        let f = &reg.beam_fields()[0];
        assert_eq!(f.outer_node_flare_pack_armed, 1);
        assert_eq!(
            f.outer_node_systems_created,
            PARTICLE_OUTER_EFFECT_NUM_BONES
        );
        assert_eq!(f.outer_intensity, ParticleIntensity::Intense);
        assert_eq!(f.laser_base_flare_created, 1);
        assert_eq!(f.connector_lasers_created, PARTICLE_OUTER_EFFECT_NUM_BONES);
    }
    assert!(reg.honesty_beam_outer_node_flare_pack_ok());
}

#[test]
fn particle_uplink_outer_node_flare_name_table_wave81_honesty() {
    assert!(honesty_particle_outer_node_flare_name_table_wave81());
    assert_eq!(PARTICLE_OUTER_NODE_FLARE_NAME_TABLE.len(), 3);
    assert_eq!(PARTICLE_UPLINK_FLARE_LASER_NAME_TABLE.len(), 6);
    assert_eq!(
        PARTICLE_CONNECTOR_MEDIUM_FLARE,
        "ParticleUplinkCannon_InnerConnectorMediumFlare"
    );
    assert_eq!(
        PARTICLE_CONNECTOR_INTENSE_FLARE,
        "ParticleUplinkCannon_InnerConnectorIntenseFlare"
    );
    assert_eq!(particle_outer_node_bone_name(0), "FX01");
    assert_eq!(particle_outer_node_bone_name(4), "FX05");
}

#[test]
fn particle_uplink_slow_death_instant_death_residual_honesty() {
    assert!(honesty_particle_uplink_death_pack());
    assert_eq!(
        PARTICLE_UPLINK_SLOW_DEATH_EXEMPT_STATUS,
        "UNDER_CONSTRUCTION"
    );
    assert_eq!(PARTICLE_UPLINK_SLOW_DEATH_DESTRUCTION_DELAY_MS, 2000);
    assert_eq!(PARTICLE_UPLINK_SLOW_DEATH_DESTRUCTION_DELAY_FRAMES, 60);
    assert_eq!(duration_ms_to_logic_frames(2000), 60);
    assert_eq!(
        PARTICLE_UPLINK_SLOW_DEATH_FX_INITIAL,
        "FX_ParticleUplinkDeathInitial"
    );
    assert_eq!(PARTICLE_UPLINK_SLOW_DEATH_OCL_INITIAL, "OCL_SDILinkLasers");
    assert_eq!(
        PARTICLE_UPLINK_SLOW_DEATH_FX_FINAL,
        "FX_StructureMediumDeath"
    );
    assert_eq!(
        PARTICLE_UPLINK_SLOW_DEATH_OCL_FINAL,
        "OCL_ParticleUplinkDeathFinal"
    );
    assert_eq!(
        PARTICLE_UPLINK_INSTANT_DEATH_REQUIRED_STATUS,
        "UNDER_CONSTRUCTION"
    );
    assert_eq!(PARTICLE_UPLINK_INSTANT_DEATH_OCL, "OCL_ABPowerPlantExplode");
    assert_eq!(PARTICLE_UPLINK_INSTANT_DEATH_FX, "FX_StructureMediumDeath");

    // Constant pack honesty without a beam field.
    let empty = HostSpecialPowerStrikeRegistry::new();
    assert!(empty.honesty_particle_uplink_death_pack_ok());

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    {
        let f = &reg.beam_fields()[0];
        assert_eq!(f.death_pack_armed, 1);
    }
    assert!(reg.honesty_particle_uplink_death_pack_ok());
}

#[test]
fn spectre_gattling_weapon_bonus_rof_application_residual_honesty() {
    assert!(honesty_gattling_weapon_bonus_rof());
    assert_eq!(SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE, 1);
    assert_eq!(SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO, 2);
    assert!((SPECTRE_GATTLING_ROF_MEAN - 2.0).abs() < 0.01);
    assert!((SPECTRE_GATTLING_ROF_FAST - 3.0).abs() < 0.01);
    assert_eq!(SpectreGattlingFireStage::Mean.tick_interval_frames(), 1);
    assert_eq!(SpectreGattlingFireStage::Fast.tick_interval_frames(), 1);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(!reg.honesty_gattling_weapon_bonus_rof_ok());
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    let field_id = reg.orbit_fields()[0].id;
    let spawn = reg.orbit_fields()[0].spawn_frame;

    // Base → MEAN → FAST WeaponBonus ROF residual applications.
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn);
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 3);
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 4);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.gattling_rof_mean_applications, 1);
        assert_eq!(f.gattling_rof_fast_applications, 1);
        assert_eq!(f.gattling_fire_level, 2);
        assert!(f.gattling_ticks >= 3);
    }
    assert!(reg.honesty_gattling_weapon_bonus_rof_ok());
}

#[test]
fn carpet_bomb_residual_pack_wave56_honesty() {
    assert!(honesty_carpet_bomb_residual_pack());
    assert_eq!(CARPET_BOMB_DROP_DELAY_MS, 300);
    assert_eq!(CARPET_BOMB_DROP_DELAY_FRAMES, 9);
    assert_eq!(CARPET_BOMB_DROP_DELAY_AIRF_MS, 130);
    assert_eq!(CARPET_BOMB_DROP_DELAY_AIRF_FRAMES, 4);
    assert_eq!(duration_ms_to_logic_frames(130), 4);
    assert!((CARPET_BOMB_PREFERRED_HEIGHT - 100.0).abs() < 0.01);
    assert!((CARPET_BOMB_DELIVERY_DISTANCE - 400.0).abs() < 0.01);
    assert_eq!(CARPET_BOMB_FIRE_FX, "FX_CarpetBomb");
    assert_eq!(CARPET_BOMB_TRANSPORT, "AmericaJetB52");
    assert_eq!(CarpetBombFactionTier::America.bomb_count(), 15);
    assert_eq!(CarpetBombFactionTier::AirForce.bomb_count(), 12);
    assert_eq!(CarpetBombFactionTier::China.bomb_count(), 10);
    assert_eq!(
        carpet_bomb_points_for_tier(Vec3::ZERO, CarpetBombFactionTier::AirForce).len(),
        12
    );
    assert_eq!(
        carpet_bomb_points_for_tier(Vec3::ZERO, CarpetBombFactionTier::China).len(),
        10
    );
    // AirF DropDelay stagger residual: bomb 1 at approach + 4.
    assert_eq!(
        carpet_bomb_impact_frame_for_tier(0, 1, CarpetBombFactionTier::AirForce),
        CARPET_BOMB_IMPACT_DELAY_FRAMES + CARPET_BOMB_DROP_DELAY_AIRF_FRAMES
    );
    // America line length residual: (15-1)*25 = 350.
    assert!((CarpetBombFactionTier::America.line_length() - 350.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_carpet_bomb_residual_pack_ok());
    let id = reg.queue(
        HostSuperweaponKind::CarpetBomb,
        ObjectId(1),
        Team::USA,
        Vec3::new(100.0, 0.0, 50.0),
        0,
    );
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.carpet_residual_pack_armed, 1);
        assert_eq!(s.carpet_preferred_height_applications, 1);
        assert_eq!(s.carpet_drop_delay_applications, 1);
        assert_eq!(s.carpet_drop_variance_applications, 1);
        assert_eq!(s.carpet_bomb_count_applications, 1);
        assert_eq!(s.carpet_delivery_distance_applications, 1);
        assert_eq!(s.ocl_points.len() as u32, CARPET_BOMB_COUNT);
    }
    assert!(reg.honesty_carpet_bomb_residual_pack_ok());
    // First bomb wave arms FireFX residual.
    let _ = reg.plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES, &[]);
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[]);
    {
        let s = reg.get(id).unwrap();
        assert!(s.carpet_fire_fx_applications >= 1);
    }
}

#[test]
fn cruise_missile_residual_pack_wave56_honesty() {
    assert!(honesty_cruise_missile_residual_pack());
    assert_eq!(CRUISE_MISSILE_PROJECTILE_OBJECT, "CruiseMissile");
    assert_eq!(CRUISE_MISSILE_DEATH_WEAPON, "MOABDetonationWeapon");
    assert_eq!(CRUISE_MISSILE_MOAB_FIRE_FX, "WeaponFX_MOAB_Blast");
    assert_eq!(CRUISE_MISSILE_SPECIAL_SPEED_TIME_FRAMES, 45);
    assert_eq!(CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES, 30);
    assert_eq!(CRUISE_MISSILE_LOFT_COMPOSITE_FRAMES, 75);
    assert!((CRUISE_MISSILE_DISTANCE_BEFORE_TURNING - 200.0).abs() < 0.01);
    assert!((MOAB_SHOCKWAVE_AMOUNT - 250.0).abs() < 0.1);
    assert!((MOAB_SHOCKWAVE_RADIUS - 200.0).abs() < 0.1);
    assert!((MOAB_SHOCKWAVE_TAPER_OFF - 0.33).abs() < 0.01);
    assert_eq!(duration_ms_to_logic_frames(1500), 45);
    assert_eq!(duration_ms_to_logic_frames(1000), 30);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_cruise_missile_residual_pack_ok());
    let id = reg.queue(
        HostSuperweaponKind::CruiseMissile,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.cruise_residual_pack_armed, 1);
        assert_eq!(s.cruise_loft_applications, 1);
        assert_eq!(s.cruise_height_die_applications, 1);
        assert_eq!(s.cruise_projectile_applications, 1);
        assert_eq!(s.cruise_moab_weapon_applications, 1);
    }
    assert!(reg.honesty_cruise_missile_residual_pack_ok());
    reg.record_impact_complete(id, 2000.0, 1, 0);
    {
        let s = reg.get(id).unwrap();
        assert!(s.cruise_moab_flame_applications >= 1);
        assert!(s.cruise_moab_fire_fx_applications >= 1);
        assert!(s.cruise_loft_applications >= 2);
    }
}

#[test]
fn artillery_barrage_residual_pack_wave56_honesty() {
    assert!(honesty_artillery_barrage_residual_pack());
    assert_eq!(ARTILLERY_BARRAGE_TRANSPORT, "ChinaArtilleryCannon");
    assert_eq!(ARTILLERY_BARRAGE_SHELL_OBJECT, "ChinaArtilleryBarrageShell");
    assert_eq!(ARTILLERY_BARRAGE_DELAY_DELIVERY_MIN_FRAMES, 0);
    assert_eq!(ARTILLERY_BARRAGE_DELAY_DELIVERY_MAX_MS, 3000);
    assert_eq!(
        duration_ms_to_logic_frames(ARTILLERY_BARRAGE_DELAY_DELIVERY_MAX_MS),
        ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
    );
    assert!((ARTILLERY_BARRAGE_PREFERRED_HEIGHT - 500.0).abs() < 0.1);
    assert!((ARTILLERY_BARRAGE_DELIVERY_DISTANCE - 250.0).abs() < 0.1);
    assert!((ARTILLERY_BARRAGE_ERROR_RADIUS - 100.0).abs() < 0.1);
    assert_eq!(ArtilleryBarrageScienceTier::Level3.formation_size(), 36);
    assert_eq!(ARTILLERY_BARRAGE_FIRE_FX, "FX_ArtilleryBarrage");
    assert!(ARTILLERY_BARRAGE_CANNON_KIND_OF.contains("EMP_HARDENED"));

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_artillery_barrage_residual_pack_ok());
    let id = reg.queue_with_artillery_tier(
        HostSuperweaponKind::ArtilleryBarrage,
        ObjectId(1),
        Team::China,
        Vec3::new(50.0, 0.0, 50.0),
        0,
        ArtilleryBarrageScienceTier::Level2,
    );
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.artillery_residual_pack_armed, 1);
        assert_eq!(s.artillery_cannon_transport_applications, 1);
        assert_eq!(s.artillery_formation_size_applications, 1);
        assert_eq!(s.artillery_delay_delivery_applications, 1);
        assert_eq!(s.artillery_weapon_error_radius_applications, 1);
        assert_eq!(s.artillery_preferred_height_applications, 1);
        assert_eq!(s.artillery_tier, ArtilleryBarrageScienceTier::Level2);
        assert_eq!(s.ocl_points.len() as u32, ARTILLERY_BARRAGE_SHELL_COUNT_L2);
    }
    assert!(reg.honesty_artillery_barrage_residual_pack_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[]);
    {
        let s = reg.get(id).unwrap();
        assert!(s.artillery_fire_fx_applications >= 1);
    }
}

#[test]
fn nuke_radiation_residual_pack_wave56_honesty() {
    assert!(honesty_nuke_radiation_residual_pack());
    assert_eq!(NUKE_RADIATION_FIRE_FX, "WeaponFX_LargeRadiationFieldWeapon");
    assert_eq!(NUKE_RADIATION_DAMAGE_TYPE, "RADIATION");
    assert_eq!(NUKE_RADIATION_SUSPEND_FX_DELAY_MS, 10000);
    assert_eq!(NUKE_RADIATION_SUSPEND_FX_DELAY_FRAMES, 300);
    assert_eq!(duration_ms_to_logic_frames(10000), 300);
    assert_eq!(NUKE_RADIATION_OCL, "OCL_NukeRadiationField");
    assert_eq!(NUKE_RADIATION_OBJECT_NAME, "NukeRadiationFieldWeapon");

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_nuke_radiation_residual_pack_ok());
    let id = reg.queue(
        HostSuperweaponKind::NuclearMissile,
        ObjectId(1),
        Team::China,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 1000.0, 1, 0);
    assert!(!reg.radiation_fields().is_empty());
    {
        let f = &reg.radiation_fields()[0];
        assert_eq!(f.radiation_residual_pack_armed, 1);
        assert_eq!(f.radiation_suspend_fx_applications, 1);
        assert_eq!(f.radiation_fire_fx_applications, 1);
    }
    {
        let s = reg.get(id).unwrap();
        assert!(s.nuke_radiation_residual_pack_applications >= 1);
    }
    assert!(reg.honesty_nuke_radiation_residual_pack_ok());
}

#[test]
fn anthrax_toxin_residual_pack_wave56_honesty() {
    assert!(honesty_anthrax_toxin_residual_pack());
    assert_eq!(
        ANTHRAX_TOXIN_FIRE_FX,
        "WeaponFX_LargePoisonFieldWeaponUpgraded"
    );
    assert_eq!(ANTHRAX_TOXIN_DEATH_TYPE, "POISONED_BETA");
    assert!((ANTHRAX_TOXIN_WEAPON_SPEED - 600.0).abs() < 0.1);
    assert_eq!(ANTHRAX_TOXIN_OCL, "OCL_PoisonFieldAnthraxBomb");
    assert_eq!(ANTHRAX_BOMB_WEAPON_NAME, "AnthraxBombWeapon");
    assert_eq!(duration_ms_to_logic_frames(500), 15);
    assert_eq!(duration_ms_to_logic_frames(60000), 1800);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    assert!(reg.honesty_anthrax_toxin_residual_pack_ok());
    let id = reg.queue(
        HostSuperweaponKind::AnthraxBomb,
        ObjectId(1),
        Team::GLA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 200.0, 1, 0);
    assert!(!reg.toxin_fields().is_empty());
    {
        let f = &reg.toxin_fields()[0];
        assert_eq!(f.toxin_residual_pack_armed, 1);
        assert_eq!(f.toxin_fire_fx_applications, 1);
        assert_eq!(f.toxin_damage_type_applications, 1);
        assert!((f.damage_per_tick - ANTHRAX_TOXIN_DAMAGE_PER_TICK).abs() < 0.01);
        assert!((f.radius - ANTHRAX_TOXIN_RADIUS).abs() < 0.1);
    }
    {
        let s = reg.get(id).unwrap();
        assert!(s.anthrax_toxin_residual_pack_applications >= 1);
    }
    assert!(reg.honesty_anthrax_toxin_residual_pack_ok());
}

#[test]
fn scud_storm_missile_thing_factory_pack_wave65_honesty() {
    assert!(honesty_scud_storm_missile_thing_factory_pack());
    assert_eq!(SCUD_STORM_MISSILE_OBJECT, "ScudStormMissile");
    assert!((SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_MISSILE_TRANSPORT_SLOT_COUNT, 10);
    assert!((SCUD_STORM_MISSILE_SHROUD_CLEARING_RANGE - 0.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_MISSILE_ARMOR, "ProjectileArmor");
    assert_eq!(SCUD_STORM_MISSILE_SPECIAL_POWER, "SuperweaponScudStorm");
    assert!(SCUD_STORM_MISSILE_HEIGHT_DIE_INCLUDES_STRUCTURES);
    assert_eq!(SCUD_STORM_MISSILE_DAMAGED_MODEL, "NONE");
    assert_eq!(SCUD_STORM_MISSILE_MODEL, "UBScudStrm_M");
    assert_eq!(SCUD_STORM_MISSILE_GEOMETRY, "Cylinder");
    assert!((SCUD_STORM_MISSILE_GEOMETRY_RADIUS - 7.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_GEOMETRY_HEIGHT - 30.0).abs() < 0.01);
    // Application residual still arms via existing Scud strike path.
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(50.0, 0.0, 50.0),
        0,
    );
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(50.0, 0.0, 50.0)]);
    {
        let s = reg.get(id).unwrap();
        assert!(s.scud_object_params_applications >= 1);
        assert!(s.scud_geometry_applications >= 1);
        assert!(s.scud_special_power_completion_applications >= 1);
        assert!(s.scud_body_draw_params_applications >= 1);
    }
    assert!(reg.honesty_scud_object_params_ok());
    assert!(reg.honesty_scud_geometry_ok());
    assert!(reg.honesty_scud_body_draw_params_ok());
}

#[test]
fn spectre_howitzer_shell_thing_factory_pack_wave65_honesty() {
    assert!(honesty_spectre_howitzer_shell_thing_factory_pack());
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_TYPES,
        "NONE +DETONATED"
    );
    assert_eq!(SPECTRE_HOWITZER_SHELL_DEATH_LASERED_TYPES, "NONE +LASERED");
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_TYPES,
        "ALL -LASERED -DETONATED"
    );
    assert_eq!(SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_FX, "FX_NukeGLA");
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_DEATH_LASERED_OCL,
        "OCL_GenericMissileDisintegrate"
    );
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX,
        "FX_GenericMissileDeath"
    );
    assert!((SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01);
    assert_eq!(SPECTRE_HOWITZER_SHELL_GEOMETRY, "Cylinder");
    assert_eq!(SPECTRE_HOWITZER_SHELL_SHADOW, "SHADOW_DECAL");
    assert!((SPECTRE_HOWITZER_SHELL_GEOMETRY_RADIUS - 4.0).abs() < 0.01);
    assert!((SPECTRE_HOWITZER_SHELL_GEOMETRY_HEIGHT - 4.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    assert!(!reg.orbit_fields().is_empty());
    let field_id = reg.orbit_fields()[0].id;
    let spawn = reg.orbit_fields()[0].spawn_frame;
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn);
    {
        let f = &reg.orbit_fields()[0];
        assert!(f.howitzer_shells_spawned >= 1);
        assert!(f.howitzer_shell_death_detonated_applications >= 1);
        assert!(f.howitzer_shell_death_lasered_applications >= 1);
        assert!(f.howitzer_shell_death_generic_applications >= 1);
        assert!(f.howitzer_shell_scale_applications >= 1);
        assert!(f.howitzer_shell_shadow_applications >= 1);
        assert!(f.howitzer_shell_geometry_applications >= 1);
    }
    assert!(reg.honesty_howitzer_shell_ok());
    assert!(reg.honesty_howitzer_shell_dumb_projectile_ok());
    assert!(reg.honesty_howitzer_shell_death_generic_ok());
    assert!(reg.honesty_howitzer_shell_model_draw_ok());
}

#[test]
fn trail_remnant_thing_factory_pack_wave65_honesty() {
    assert!(honesty_trail_remnant_thing_factory_pack());
    assert_eq!(PARTICLE_REMNANT_KIND_OF, "NO_COLLIDE UNATTACKABLE IMMOBILE");
    assert!(PARTICLE_REMNANT_KIND_OF_NO_COLLIDE);
    assert!(PARTICLE_REMNANT_KIND_OF_UNATTACKABLE);
    assert!(PARTICLE_REMNANT_KIND_OF_IMMOBILE);
    assert!((PARTICLE_REMNANT_MAX_HEALTH - 50.0).abs() < 0.01);
    assert_eq!(PARTICLE_REMNANT_BODY, "ImmortalBody");
    assert_eq!(
        PARTICLE_REMNANT_OBJECT_NAME,
        "ParticleUplinkCannonTrailRemnant"
    );
    assert!(PARTICLE_REMNANT_FIRE_WEAPON_UPDATE);
    assert!(PARTICLE_REMNANT_DELETION_UPDATE);
    assert_eq!(PARTICLE_REMNANT_DURATION_FRAMES, 120);
    assert!(honesty_immortal_body_health_floor(50.0, -100.0, 1.0));
    assert!(honesty_deletion_update_sleep_delay());
}

/// Wave 74: ThingFactory residual spawn bookkeeping on impact / shell / remnant.
#[test]
fn thing_factory_spawn_bookkeeping_wave74_honesty() {
    assert!(honesty_thing_factory_spawn_bookkeeping_wave74());

    // ScudStormMissile impact residual spawn bookkeeping.
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let scud_id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(50.0, 0.0, 50.0),
        0,
    );
    reg.record_impact_wave(scud_id, 0.0, 0, 0, 1, false, &[Vec3::new(50.0, 0.0, 50.0)]);
    {
        let s = reg.get(scud_id).unwrap();
        assert!(s.scud_thing_factory_spawn_applications >= 1);
        let spawn = scud_storm_missile_spawn_residual(s.impact_frame, s.target_position);
        assert!(honesty_thing_factory_spawn_residual(&spawn));
        assert_eq!(spawn.object_name, "ScudStormMissile");
        assert!((spawn.mass - 500.0).abs() < 0.01);
    }
    assert!(reg.honesty_scud_thing_factory_spawn_ok());

    // SpectreHowitzerShell residual spawn bookkeeping.
    let spectre_id = reg.queue(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(2),
        Team::USA,
        Vec3::new(10.0, 0.0, 10.0),
        0,
    );
    reg.record_impact_complete(spectre_id, 0.0, 0, 0);
    assert!(!reg.orbit_fields().is_empty());
    let field_id = reg.orbit_fields()[0].id;
    let spawn_frame = reg.orbit_fields()[0].spawn_frame;
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_frame);
    {
        let f = &reg.orbit_fields()[0];
        assert!(f.howitzer_shell_thing_factory_spawn_applications >= 1);
        let shell = spectre_howitzer_shell_spawn_residual(
            spawn_frame,
            f.position + Vec3::new(0.0, 80.0, 0.0),
        );
        assert!(honesty_thing_factory_spawn_residual(&shell));
        assert_eq!(shell.object_name, "SpectreHowitzerShell");
        assert!((shell.max_health - 100.0).abs() < 0.01);
    }
    assert!(reg.honesty_howitzer_shell_thing_factory_spawn_ok());

    // TrailRemnant residual spawn bookkeeping (ImmortalBody/DeletionUpdate closed).
    let remnant_id =
        reg.spawn_remnant_field(ObjectId(3), Team::USA, Vec3::new(7.0, 0.0, 8.0), 40, 0, 0);
    assert!(remnant_id >= 1);
    {
        let f = reg
            .remnant_fields()
            .iter()
            .find(|f| f.id == remnant_id)
            .expect("remnant field");
        assert_eq!(f.remnant_thing_factory_spawn_applications, 1);
        assert!(f.remnant_immortal_body_applications >= 1);
        assert!(f.remnant_fire_deletion_applications >= 1);
        let rem = trail_remnant_spawn_residual(f.spawn_frame, f.position);
        assert!(honesty_thing_factory_spawn_residual(&rem));
        assert!(rem.immortal_body);
        assert!(rem.deletion_update);
        assert_eq!(rem.body_module, "ImmortalBody");
    }
    assert!(reg.honesty_remnant_thing_factory_spawn_ok());
}

/// Wave 72 residual pack honesty: DaisyCutter / A10 deepen + combined pack.
#[test]
fn special_power_residual_pack_honesty_wave72() {
    assert!(honesty_daisy_cutter_residual_pack());
    assert!(honesty_a10_strike_residual_pack());
    assert!(honesty_special_power_residual_pack_ok());
    assert_eq!(DAISY_CUTTER_RELOAD_FRAMES, 10_800);
    assert_eq!(A10_STRIKE_RELOAD_FRAMES, 7_200);
    assert!((DAISY_CUTTER_RADIUS_CURSOR - 170.0).abs() < 0.01);
    assert!((A10_STRIKE_RADIUS_CURSOR - 50.0).abs() < 0.01);
    assert_eq!(HostSuperweaponKind::DaisyCutter.impact_delay_frames(), 90);
    assert_eq!(HostSuperweaponKind::A10Strike.impact_delay_frames(), 60);
    assert!((A10_MISSILE_PRIMARY_DAMAGE - 200.0).abs() < 0.1);
    assert!((DAISY_CUTTER_FLAME_DAMAGE - 5.0).abs() < 0.01);
}

#[test]
fn spectre_orbit_residual_pack_wave73_honesty() {
    assert!(honesty_spectre_orbit_residual_pack_wave73());
    assert_eq!(SPECTRE_HOWITZER_FIRING_RATE_MS, 300);
    assert_eq!(SPECTRE_ORBIT_TICK_INTERVAL_FRAMES, 9);
    assert_eq!(duration_ms_to_logic_frames(300), 9);
    assert_eq!(SPECTRE_HOWITZER_FOLLOW_LAG_MS, 400);
    assert_eq!(SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES, 12);
    assert!((SPECTRE_GUNSHIP_ORBIT_RADIUS - 250.0).abs() < 0.01);
    assert!((SPECTRE_ORBIT_RADIUS - 200.0).abs() < 0.01);
    assert!(SPECTRE_GUNSHIP_ORBIT_RADIUS > SPECTRE_ORBIT_RADIUS);
    assert_eq!(SPECTRE_ATTACK_AREA_DECAL_TEXTURE, "SCCSpecTarg");
    assert_eq!(SPECTRE_TARGETING_RETICLE_DECAL_TEXTURE, "SCCSpecRet");
    assert_eq!(SPECTRE_DECAL_COLOR, (127, 177, 222, 255));
    assert_eq!(SPECTRE_RELOAD_MS, 240_000);
    assert_eq!(SPECTRE_AIRF_RELOAD_MS, 180_000);
    // Science tiers: OrbitTime scales, AttackAreaRadius fixed.
    for tier in [
        SpectreGunshipScienceTier::Level1,
        SpectreGunshipScienceTier::Level2,
        SpectreGunshipScienceTier::Level3,
    ] {
        assert!((tier.attack_area_radius() - 200.0).abs() < 0.01);
    }
    assert_eq!(
        SpectreGunshipScienceTier::Level1.orbit_duration_frames(),
        300
    );
    assert_eq!(
        SpectreGunshipScienceTier::Level2.orbit_duration_frames(),
        450
    );
    assert_eq!(
        SpectreGunshipScienceTier::Level3.orbit_duration_frames(),
        600
    );
    // Dual-weapon ROF residual schedule.
    assert_eq!(spectre_howitzer_interval_frames(0), 9);
    assert_eq!(spectre_howitzer_interval_frames(2), 6);
    assert_eq!(spectre_howitzer_interval_frames(3), 4);
    assert_eq!(spectre_gattling_interval_frames(0), 3);
    assert_eq!(spectre_gattling_interval_frames(2), 1);
    assert_eq!(spectre_gattling_interval_frames(3), 1);

    // Application path: orbit spawn + dual-weapon tick still host-testable.
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue_with_tiers(
        HostSuperweaponKind::SpectreGunship,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
        ArtilleryBarrageScienceTier::Level1,
        SpectreGunshipScienceTier::Level3,
    );
    reg.record_impact_complete(id, 0.0, 0, 0);
    assert!(!reg.orbit_fields().is_empty());
    let field_id = reg.orbit_fields()[0].id;
    let spawn = reg.orbit_fields()[0].spawn_frame;
    assert_eq!(
        reg.orbit_fields()[0].expires_frame,
        spawn + SpectreGunshipScienceTier::Level3.orbit_duration_frames()
    );
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn);
    {
        let f = &reg.orbit_fields()[0];
        assert!(f.howitzer_ticks >= 1);
    }
    assert!(reg.honesty_orbit_ok());
}

#[test]
fn nuke_radiation_residual_pack_wave73_honesty() {
    assert!(honesty_nuke_radiation_residual_pack_wave73());
    assert!((NUKE_RADIATION_ATTACK_RANGE - 15.0).abs() < 0.01);
    assert!((NUKE_RADIATION_MINIMUM_ATTACK_RANGE - 10.0).abs() < 0.01);
    assert_eq!(NUKE_RADIATION_ARMOR, "HazardousMaterialArmor");
    assert_eq!(NUKE_RADIATION_GEOMETRY, "CYLINDER");
    assert!((NUKE_RADIATION_GEOMETRY_HEIGHT - 1.0).abs() < 0.01);
    assert!(!NUKE_RADIATION_GEOMETRY_IS_SMALL);
    assert_eq!(NUKE_RADIATION_DEATH_FX, "FX_RadiationPoolDie");
    assert_eq!(
        NUKE_RADIATION_HAZARD_FIELD_CORE_WEAPON,
        "HazardFieldCoreWeapon"
    );
    assert_eq!(NUCLEAR_MISSILE_RELOAD_MS, 360_000);
    assert_eq!(NUCLEAR_MISSILE_RELOAD_FRAMES, 10_800);
    assert_eq!(duration_ms_to_logic_frames(360_000), 10_800);
    assert!((NUCLEAR_MISSILE_RADIUS_CURSOR - 210.0).abs() < 0.01);
    assert_eq!(NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_FRAMES, 1_200);
    assert!(NUKE_RADIATION_KIND_OF.contains("CLEANUP_HAZARD"));

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::NuclearMissile,
        ObjectId(1),
        Team::China,
        Vec3::ZERO,
        0,
    );
    reg.record_impact_complete(id, 1000.0, 1, 0);
    assert!(!reg.radiation_fields().is_empty());
    {
        let f = &reg.radiation_fields()[0];
        assert_eq!(f.radiation_residual_pack_armed, 1);
        assert_eq!(f.radiation_suspend_fx_applications, 1);
        assert_eq!(f.radiation_fire_fx_applications, 1);
    }
    assert!(reg.honesty_nuke_radiation_residual_pack_ok());
    assert!(reg.honesty_radiation_ok());
}

#[test]
fn supw_variants_residual_pack_wave73_honesty() {
    assert!(honesty_supw_variants_residual_pack_wave73());
    assert_eq!(SUPW_NEUTRON_MISSILE_RELOAD_MS, 240_000);
    assert_eq!(SUPW_NEUTRON_MISSILE_RELOAD_FRAMES, 7_200);
    assert_eq!(
        SUPW_NEUTRON_MISSILE_SPECIAL_POWER,
        "SupW_SuperweaponNeutronMissile"
    );
    assert!((SUPW_NEUTRON_MISSILE_RADIUS_CURSOR - 210.0).abs() < 0.01);
    assert_eq!(SUPW_PUC_RELOAD_MS, 180_000);
    assert_eq!(SUPW_PUC_RELOAD_FRAMES, 5_400);
    assert_eq!(
        SUPW_PUC_SPECIAL_POWER,
        "SupW_SuperweaponParticleUplinkCannon"
    );
    assert_eq!(NUKE_GENERAL_NEUTRON_RELOAD_MS, 300_000);
    assert_eq!(NUKE_GENERAL_NEUTRON_RELOAD_FRAMES, 9_000);
    assert_eq!(
        NUKE_GENERAL_NEUTRON_SPECIAL_POWER,
        "Nuke_SuperweaponNeutronMissile"
    );
    // Ordering: SupW 240s < Nuke_ 300s < standard China 360s.
    assert!(SUPW_NEUTRON_MISSILE_RELOAD_MS < NUKE_GENERAL_NEUTRON_RELOAD_MS);
    assert!(NUKE_GENERAL_NEUTRON_RELOAD_MS < NUCLEAR_MISSILE_RELOAD_MS);
    // AirF Spectre faster than USA Spectre.
    assert!(SPECTRE_AIRF_RELOAD_MS < SPECTRE_RELOAD_MS);
    // SupW Cruise already residual.
    assert_eq!(CRUISE_MISSILE_RELOAD_MS, 120_000);
    assert!(honesty_special_power_residual_pack_wave73_ok());
}

/// Wave 76 residual: A10 science-tier FormationSize 1/2/3 + OCL deliver pack.
#[test]
fn a10_science_tier_residual_pack_wave76_honesty() {
    assert!(honesty_a10_science_tier_residual_pack_wave76());
    assert!(honesty_special_power_residual_pack_wave76_ok());
    assert_eq!(A10StrikeScienceTier::Level1.formation_size(), 1);
    assert_eq!(A10StrikeScienceTier::Level2.formation_size(), 2);
    assert_eq!(A10StrikeScienceTier::Level3.formation_size(), 3);
    assert_eq!(A10_DROP_DELAY_FRAMES, 15);
    assert_eq!(duration_ms_to_logic_frames(500), 15);
    assert!((A10_FORMATIONION_SPACING - 35.0).abs() < 0.01);
    assert!((A10_DELIVERY_DISTANCE - 450.0).abs() < 0.01);
    assert_eq!(A10_VISIBLE_NUM_BONES, 6);
    assert_eq!(A10_DELIVERY_DECAL_TEXTURE, "SCCA10Strike_USA");
    assert_eq!(A10_DELIVERY_DECAL_COLOR, (255, 156, 0, 255));
    assert_eq!(
        A10StrikeScienceTier::from_science_name("SCIENCE_A10ThunderboltMissileStrike3"),
        Some(A10StrikeScienceTier::Level3)
    );
    assert_eq!(
        A10StrikeScienceTier::highest_from_sciences([A10_SCIENCE_TIER1, A10_SCIENCE_TIER2,]),
        A10StrikeScienceTier::Level2
    );
    // FormationSize scales with science tier only; damage/reload shared.
    assert_eq!(A10_STRIKE_RELOAD_MS, 240_000);
    assert!((A10_MISSILE_PRIMARY_DAMAGE - 200.0).abs() < 0.1);
}

/// Wave 77 residual: SpecialPower.ini InitiateSound / InitiateAtLocationSound name tables.
#[test]
fn special_power_audio_name_table_wave77_honesty() {
    assert!(honesty_special_power_audio_name_table_wave77());
    assert!(honesty_special_power_residual_pack_wave77_ok());
    assert_eq!(SCUD_STORM_INITIATE_SOUND, "ScudStormInitiated");
    assert_eq!(ARTILLERY_BARRAGE_INITIATE_SOUND, "FireArtilleryCannonSound");
    assert_eq!(CRUISE_MISSILE_INITIATE_SOUND, "AirRaidSiren");
    assert_eq!(CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND, "AirRaidSiren");
    assert_eq!(NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND, "AirRaidSiren");
    // Neutron InitiateSound commented out in retail — empty residual honesty.
    assert!(NUCLEAR_MISSILE_INITIATE_SOUND.is_empty());
    assert!(
        HostSuperweaponKind::DaisyCutter
            .retail_initiate_sound()
            .is_empty()
    );
    // Host residual queue labels stay special-power template names.
    assert_eq!(
        HostSuperweaponKind::ScudStorm.activate_audio(),
        "SuperweaponScudStorm"
    );
    assert_eq!(
        HostSuperweaponKind::ScudStorm.retail_initiate_sound(),
        "ScudStormInitiated"
    );
}

/// Wave 78 residual: HostSuperweaponKind reload table + CarpetBomb/Artillery science tiers.
#[test]
fn host_superweapon_reload_table_wave78_honesty() {
    assert!(honesty_host_superweapon_reload_table_wave78());
    assert_eq!(HostSuperweaponKind::ScudStorm.reload_ms(), 300_000);
    assert_eq!(HostSuperweaponKind::ParticleCannon.reload_ms(), 240_000);
    assert_eq!(HostSuperweaponKind::AnthraxBomb.reload_ms(), 360_000);
    assert_eq!(HostSuperweaponKind::CruiseMissile.reload_frames(), 3_600);
    assert_eq!(HostSuperweaponKind::CarpetBomb.reload_frames(), 4_500);
    assert_eq!(HostSuperweaponKind::ArtilleryBarrage.reload_frames(), 9_000);
    assert_eq!(duration_ms_to_logic_frames(300_000), 9_000);
}

#[test]
fn carpet_bomb_science_tier_residual_pack_wave78_honesty() {
    assert!(honesty_carpet_bomb_science_tier_residual_pack_wave78());
    assert_eq!(CarpetBombFactionTier::AirForce.reload_ms(), 240_000);
    assert_eq!(CARPET_BOMB_RELOAD_NUKE_MS, 180_000);
    assert!((CarpetBombFactionTier::China.radius_cursor() - 180.0).abs() < 0.01);
    assert_eq!(
        CarpetBombFactionTier::America.delivery_decal_texture(),
        "SCCA10Strike_USA"
    );
    assert_eq!(
        CarpetBombFactionTier::AirForce.delivery_decal_color(),
        (255, 0, 0, 255)
    );
    assert_eq!(
        CarpetBombFactionTier::China.ocl_name(),
        "SUPERWEAPON_ChinaCarpetBomb"
    );
}

#[test]
fn artillery_science_tier_residual_pack_wave78_honesty() {
    assert!(honesty_artillery_science_tier_residual_pack_wave78());
    assert!(honesty_special_power_residual_pack_wave78_ok());
    assert_eq!(
        ArtilleryBarrageScienceTier::Level3.science_name(),
        "SCIENCE_ArtilleryBarrage3"
    );
    assert_eq!(
        ArtilleryBarrageScienceTier::Level2.ocl_name(),
        "SUPERWEAPON_ArtilleryBarrage2"
    );
    assert_eq!(
        ARTILLERY_DELIVERY_DECAL_TEXTURE,
        "SCCArtilleryBarrage_China"
    );
    assert_eq!(ARTILLERY_DELIVERY_DECAL_COLOR, (255, 156, 0, 255));
    assert_eq!(ARTILLERY_SCIENCE_POINT_COST, 1);
    assert_eq!(
        ArtilleryBarrageScienceTier::highest_from_sciences([
            ARTILLERY_SCIENCE_TIER1,
            ARTILLERY_SCIENCE_TIER3,
        ]),
        ArtilleryBarrageScienceTier::Level3
    );
}
