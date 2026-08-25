//! Main-side read-inventory audit (hq-79qs slice C).
//!
//! Compares present residual groups vs envelope tags vs Entity attach for a
//! representative object set, and locks `KNOWN_GAPS` against object/mod.rs.

use super::Object;
use super::entity_lifecycle_projectiles::ProjectileFlightResiduals;
use super::entity_lifecycle_residuals::{
    ActiveBodyCrushResidual, CreateObjectDieTransferResidual, EmoticonSurrenderResidual,
    FireWeaponWhenDeadResidual, PhysicsBehaviorResidual, RailroadBehaviorResidual,
    SpecialPowerCooldownResidual, WeaponLockResidual,
};
use super::entity_lifecycle_tags::*;
use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;
use crate::game_logic::host_upgrade_die::HostUpgradeDieData;
use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate, Weapon};
use gamelogic::world::entities::{EntityStore, TemplateRef, Transform};
use gamelogic::world::{ENTITY_ONLY_GROUPS, KNOWN_GAPS};

const INVENTORIED_IN_WINDOW: &[&str] = &[
    "fire_weapon_when_dead_fired",
    "bone_fx_damage",
    "poisoned_behavior",
    "defection_helper",
    "fire_weapon_power",
    "fire_weapon_when_damaged",
    "temporary_weapon_runtime",
    "pending_fire_when_damaged_weapon",
    "transition_damage_fx",
    "fx_list_die",
    "create_object_die",
    "create_object_die_transfer_damage",
    "lifetime_update",
    "slow_death",
    "height_die",
    "fuel_air_gas_slow_death",
    "neutron_missile_update",
    "missile_launcher_building",
    "scud_storm_missile_flight",
    "carpet_bomb_payload",
    "carpet_bomb_transport",
    "artillery_barrage_shell",
    "artillery_barrage_transport",
    "a10_strike_missile",
    "a10_strike_transport",
    "leaflet_transport_target",
    "leaflet_container",
    "paradrop_transport_target",
    "paradrop_parachute",
    "daisy_cutter_transport",
    "daisy_cutter_bomb",
    "anthrax_bomb_transport",
    "anthrax_bomb_payload",
    "sneak_tunnel_start",
    "cluster_mines_transport",
    "cluster_mines_bomb",
    "emp_pulse_transport",
    "emp_pulse_bomb",
    "emp_pulse_spheroid",
    "emp_pulse_spheroid_expires_frame",
    "particle_trail_remnant",
    "particle_trail_remnant_expires_frame",
    "nuke_radiation_field",
    "nuke_radiation_field_expires_frame",
    "anthrax_toxin_field",
    "anthrax_toxin_field_expires_frame",
    "spectre_howitzer_shell",
    "spectre_howitzer_shell_expires_frame",
    "particle_orbital_laser",
    "particle_orbital_laser_expires_frame",
    "particle_connector_laser",
    "particle_connector_laser_expires_frame",
    "point_defense_laser_beam",
    "point_defense_laser_beam_expires_frame",
    "missile_defender_laser_beam",
    "missile_defender_laser_beam_expires_frame",
    "booby_trap_special",
    "booby_trap_attached_to",
    "countermeasure_flare",
    "countermeasure_flare_expires_frame",
    "angry_mob_member",
    "angry_mob_nexus_id",
    "weapon_laser_beam",
    "weapon_laser_beam_expires_frame",
];

fn present_groups(object: &Object) -> Vec<&'static str> {
    let mut out = Vec::new();
    let pairs: [(&'static str, bool); 63] = [
        (TAG_UPGRADE_DIE, object.upgrade_die.is_some()),
        (
            TAG_SPECIAL_POWER_COMPLETION,
            object.special_power_completion.is_some(),
        ),
        (TAG_BATTLE_BUS_BODY, object.battle_bus_body.is_some()),
        (TAG_CAPTURE_CHANNEL, object.capture_channel.is_some()),
        (
            TAG_HACKER_DISABLE_CHANNEL,
            object.hacker_disable_channel.is_some(),
        ),
        (TAG_TOPPLE, object.topple_data.is_some()),
        (TAG_STRUCTURE_TOPPLE, object.structure_topple_data.is_some()),
        (
            TAG_STRUCTURE_COLLAPSE,
            object.structure_collapse_data.is_some(),
        ),
        (TAG_KEEP_OBJECT_DIE, object.keep_object_die.is_some()),
        (TAG_WAVE_GUIDE, object.wave_guide_data.is_some()),
        (TAG_BONE_FX_DAMAGE, object.bone_fx_damage.is_some()),
        (TAG_POISONED, object.poisoned_behavior.is_some()),
        (TAG_DEFECTION, object.defection_helper.is_some()),
        (TAG_FIRE_WEAPON_POWER, object.fire_weapon_power.is_some()),
        (
            TAG_FIRE_WEAPON_WHEN_DAMAGED,
            object.fire_weapon_when_damaged.is_some()
                || object.pending_fire_when_damaged_weapon.is_some(),
        ),
        (
            TAG_TRANSITION_DAMAGE_FX,
            object.transition_damage_fx.is_some(),
        ),
        (TAG_FX_LIST_DIE, object.fx_list_die.is_some()),
        (TAG_CREATE_OBJECT_DIE, object.create_object_die.is_some()),
        (TAG_LIFETIME, object.lifetime_update.is_some()),
        (TAG_SLOW_DEATH, object.slow_death.is_some()),
        (TAG_HEIGHT_DIE, object.height_die.is_some()),
        (
            TAG_FUEL_AIR_GAS_SLOW_DEATH,
            object.fuel_air_gas_slow_death.is_some(),
        ),
        (TAG_NEUTRON_MISSILE, object.neutron_missile_update.is_some()),
        (
            TAG_MISSILE_LAUNCHER_BUILDING,
            object.missile_launcher_building.is_some(),
        ),
        (
            TAG_SCUD_STORM_FLIGHT,
            object.scud_storm_missile_flight.is_some(),
        ),
        (
            TAG_CARPET_BOMB_TRANSPORT,
            object.carpet_bomb_transport.is_some(),
        ),
        (
            TAG_ARTILLERY_BARRAGE_TRANSPORT,
            object.artillery_barrage_transport.is_some(),
        ),
        (
            TAG_A10_STRIKE_TRANSPORT,
            object.a10_strike_transport.is_some(),
        ),
        (
            TAG_DAISY_CUTTER_TRANSPORT,
            object.daisy_cutter_transport.is_some(),
        ),
        (
            TAG_ANTHRAX_BOMB_TRANSPORT,
            object.anthrax_bomb_transport.is_some(),
        ),
        (
            TAG_CLUSTER_MINES_TRANSPORT,
            object.cluster_mines_transport.is_some(),
        ),
        (
            TAG_EMP_PULSE_TRANSPORT,
            object.emp_pulse_transport.is_some(),
        ),
        (TAG_TENSILE_FORMATION, object.tensile_formation.is_some()),
        (TAG_FIRE_SPREAD, object.fire_spread.is_some()),
        (TAG_BASE_REGENERATE, object.base_regenerate.is_some()),
        (TAG_DEFAULT_AUTO_HEAL, object.default_auto_heal.is_some()),
        (TAG_ENEMY_NEAR, object.enemy_near.is_some()),
        (TAG_ANIMATION_STEERING, object.animation_steering.is_some()),
        (TAG_FLOAT_UPDATE, object.float_update.is_some()),
        (TAG_PRONE_UPDATE, object.prone_update.is_some()),
        (TAG_RADIUS_DECAL, object.radius_decal_update.is_some()),
        (TAG_CHECKPOINT, object.checkpoint_update.is_some()),
        (
            TAG_SPECTRE_GUNSHIP_DEPLOYMENT,
            object.spectre_gunship_deployment.is_some(),
        ),
        (
            TAG_SPECTRE_GUNSHIP_UPDATE,
            object.spectre_gunship_update.is_some(),
        ),
        (
            TAG_SMART_BOMB_HOMING,
            object.smart_bomb_target_homing.is_some(),
        ),
        (
            TAG_HELICOPTER_SLOW_DEATH,
            object.helicopter_slow_death.is_some(),
        ),
        (TAG_JET_SLOW_DEATH, object.jet_slow_death.is_some()),
        (TAG_MINE, object.mine_data.is_some()),
        (
            TAG_SPAWN_BEHAVIOR,
            super::entity_lifecycle_inventory::SpawnBehaviorHiveResidual::present(object),
        ),
        (
            TAG_FIRING_TRACKER,
            super::entity_lifecycle_inventory::FiringTrackerResidual::present(object),
        ),
        (
            TAG_FIRE_OCL_AFTER_COOLDOWN,
            object.fire_ocl_after_cooldown.is_some(),
        ),
        (TAG_ASSAULT_TRANSPORT, object.assault_transport.is_some()),
        (TAG_DEPLOY_STYLE, object.deploy_style.is_some()),
        (
            TAG_COMMAND_BUTTON_HUNT,
            object.command_button_hunt.is_some(),
        ),
        (
            TAG_FIRE_WEAPON_WHEN_DEAD,
            FireWeaponWhenDeadResidual::present(object),
        ),
        (
            TAG_CREATE_OBJECT_DIE_TRANSFER,
            CreateObjectDieTransferResidual::present(object),
        ),
        (
            TAG_SPECIAL_POWER_COOLDOWNS,
            SpecialPowerCooldownResidual::present(object),
        ),
        (TAG_WEAPON_LOCK, WeaponLockResidual::present(object)),
        (
            TAG_EMOTICON_SURRENDER,
            EmoticonSurrenderResidual::present(object),
        ),
        (
            TAG_PROJECTILE_FLIGHT,
            ProjectileFlightResiduals::present(object),
        ),
        (TAG_ACTIVE_BODY, ActiveBodyCrushResidual::present(object)),
        (
            TAG_PHYSICS_BEHAVIOR,
            PhysicsBehaviorResidual::present(object),
        ),
        (TAG_RAILROAD, RailroadBehaviorResidual::present(object)),
    ];
    for (tag, present) in pairs {
        if present {
            out.push(tag);
        }
    }
    out
}

fn attach_round_trip(object: &Object) {
    let envelope = object.entity_lifecycle_envelope();
    let present = present_groups(object);
    let emitted: Vec<&str> = envelope
        .module_states
        .iter()
        .map(|m| m.tag.as_str())
        .collect();
    assert_eq!(emitted, present, "envelope tags must match present groups");
    let mut prev = 0usize;
    for tag in &emitted {
        let idx = INVENTORY_TAGS
            .iter()
            .position(|t| t == tag)
            .expect("emitted tag must be inventoried");
        assert!(idx >= prev, "tag order must follow Object field order");
        prev = idx;
    }
    let mut store = EntityStore::new();
    let id = store.spawn(
        TemplateRef::new(object.template_name.clone()),
        None,
        Transform::default(),
        10.0,
    );
    let entity = store.get_mut(id).expect("spawned");
    entity.attach_envelope(envelope.clone());
    let taken = entity.take_envelope().expect("attached");
    assert_eq!(taken, envelope);
}

fn window_fields(src: &str) -> Vec<&str> {
    const START: &str = "pub fire_weapon_when_dead_fired";
    const END: &str = "pub weapon_laser_beam_expires_frame";
    let start = src.find(START).expect("gap window start");
    let end = src.find(END).expect("gap window end") + END.len();
    src[start..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("pub ")?;
            let name = rest.split(':').next()?.trim();
            if name.is_empty() || name.contains('(') {
                return None;
            }
            Some(name)
        })
        .collect()
}

#[test]
fn representative_objects_emit_present_groups_in_declaration_order() {
    let mut plain = Object::new(ThingTemplate::new("AuditRanger"), ObjectId(1), Team::USA);
    plain.upgrade_die = Some(HostUpgradeDieData::new("Upgrade_AmericaScoutDrone"));
    attach_round_trip(&plain);

    let mut vehicle_t = ThingTemplate::new("AuditCrusader");
    vehicle_t.add_kind_of(KindOf::Vehicle);
    let mut vehicle = Object::new(vehicle_t, ObjectId(2), Team::USA);
    vehicle.weapon = Some(Weapon::default());
    vehicle.lifetime_update = Some(HostLifetimeUpdateData {
        expire_at_frame: 30,
        active: true,
    });
    attach_round_trip(&vehicle);

    let mut bunker_t = ThingTemplate::new("AuditBunker");
    bunker_t.add_kind_of(KindOf::Structure);
    bunker_t.garrison_contain_max = Some(8);
    let mut bunker = Object::new(bunker_t, ObjectId(3), Team::USA);
    bunker.keep_object_die =
        Some(crate::game_logic::host_keep_object_die::HostKeepObjectDieData::default());
    attach_round_trip(&bunker);

    let mut factory_t = ThingTemplate::new("AuditBarracks");
    factory_t.add_kind_of(KindOf::Structure);
    factory_t.add_kind_of(KindOf::FSBarracks);
    let mut factory = Object::new(factory_t, ObjectId(4), Team::USA);
    factory.base_regenerate =
        Some(crate::game_logic::host_base_regenerate::HostBaseRegenerateData::default());
    attach_round_trip(&factory);
}

#[test]
fn known_gaps_match_object_mod_window_exactly() {
    let src = include_str!("mod.rs");
    let fields = window_fields(src);
    let mut gaps: Vec<&str> = fields
        .into_iter()
        .filter(|name| !INVENTORIED_IN_WINDOW.contains(name))
        .collect();
    assert_eq!(gaps, KNOWN_GAPS, "new untracked residual or stale gap");
    for gap in KNOWN_GAPS {
        assert!(
            !INVENTORY_TAGS.contains(gap),
            "stale KNOWN_GAPS {gap} is now inventoried; remove it from both sides"
        );
        assert!(!ENTITY_ONLY_GROUPS.contains(gap));
    }
}
