//! Produce ordered module-state payloads from a live Main `Object`.

use super::Object;
use super::entity_lifecycle_projectiles::ProjectileFlightResiduals;
use super::entity_lifecycle_residuals::{
    ActiveBodyCrushResidual, CreateObjectDieTransferResidual, EmoticonSurrenderResidual,
    FireWeaponWhenDeadResidual, PhysicsBehaviorResidual, RailroadBehaviorResidual,
    SpecialPowerCooldownResidual, WeaponLockResidual,
};
use super::entity_lifecycle_tags::*;
use crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData;
use crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponRuntimeBundle;
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FireWeaponWhenDamagedBundle {
    pub data: Option<HostFireWeaponWhenDamagedData>,
    pub runtime: TemporaryWeaponRuntimeBundle,
    pub pending_weapon: Option<String>,
}

pub(crate) fn encode_payload<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, EntityLifecycleCodecError> {
    bincode::serialize(value).map_err(|_| EntityLifecycleCodecError::UnexpectedEof {
        context: "payload_encode",
    })
}

pub(crate) fn decode_payload<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<T, EntityLifecycleCodecError> {
    bincode::deserialize(bytes).map_err(|_| EntityLifecycleCodecError::UnexpectedEof {
        context: "payload_decode",
    })
}

fn push_opt<T: Serialize>(
    out: &mut Vec<EntityModuleState>,
    tag: &'static str,
    value: &Option<T>,
) -> Result<(), EntityLifecycleCodecError> {
    if let Some(inner) = value {
        out.push(EntityModuleState {
            tag: tag.to_string(),
            payload: encode_payload(inner)?,
        });
    }
    Ok(())
}
/// C++ `SpawnBehavior::xfer` live hive residual (spawn_ids / replacement_times).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpawnBehaviorHiveResidual {
    pub slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave; 3],
    pub count: u8,
    pub hp: f32,
    pub respawn_frame: u32,
}

impl SpawnBehaviorHiveResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.hive_slave_count > 0
            || object.hive_slave_respawn_frame != 0
            || object
                .hive_slaves
                .iter()
                .any(|slave| slave.alive || slave.hp > 0.0)
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            slaves: object.hive_slaves,
            count: object.hive_slave_count,
            hp: object.hive_slave_hp,
            respawn_frame: object.hive_slave_respawn_frame,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.hive_slaves = self.slaves;
        object.hive_slave_count = self.count;
        object.hive_slave_hp = self.hp;
        object.hive_slave_respawn_frame = self.respawn_frame;
    }
}

/// C++ `FiringTracker::xfer` live consecutive-shot / coast residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FiringTrackerResidual {
    pub consecutive_shots: u32,
    pub consecutive_target: Option<crate::game_logic::ObjectId>,
    pub continuous_fire_level: u8,
    pub coast_until_frame: u32,
    pub force_reload_frame: u32,
}

impl FiringTrackerResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.consecutive_shots_at_target > 0
            || object.consecutive_shot_target.is_some()
            || object.continuous_fire_level > 0
            || object.continuous_fire_coast_until_frame != 0
            || object.frame_to_force_reload != 0
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            consecutive_shots: object.consecutive_shots_at_target,
            consecutive_target: object.consecutive_shot_target,
            continuous_fire_level: object.continuous_fire_level,
            coast_until_frame: object.continuous_fire_coast_until_frame,
            force_reload_frame: object.frame_to_force_reload,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.consecutive_shots_at_target = self.consecutive_shots;
        object.consecutive_shot_target = self.consecutive_target;
        object.continuous_fire_level = self.continuous_fire_level;
        object.continuous_fire_coast_until_frame = self.coast_until_frame;
        object.frame_to_force_reload = self.force_reload_frame;
    }
}

pub(crate) fn collect_module_states(
    object: &Object,
) -> Result<Vec<EntityModuleState>, EntityLifecycleCodecError> {
    let mut out = Vec::new();
    push_opt(&mut out, TAG_UPGRADE_DIE, &object.upgrade_die)?;
    push_opt(
        &mut out,
        TAG_SPECIAL_POWER_COMPLETION,
        &object.special_power_completion,
    )?;
    push_opt(&mut out, TAG_BATTLE_BUS_BODY, &object.battle_bus_body)?;
    push_opt(&mut out, TAG_CAPTURE_CHANNEL, &object.capture_channel)?;
    push_opt(
        &mut out,
        TAG_HACKER_DISABLE_CHANNEL,
        &object.hacker_disable_channel,
    )?;
    push_opt(&mut out, TAG_TOPPLE, &object.topple_data)?;
    push_opt(
        &mut out,
        TAG_STRUCTURE_TOPPLE,
        &object.structure_topple_data,
    )?;
    push_opt(
        &mut out,
        TAG_STRUCTURE_COLLAPSE,
        &object.structure_collapse_data,
    )?;
    push_opt(&mut out, TAG_KEEP_OBJECT_DIE, &object.keep_object_die)?;
    push_opt(&mut out, TAG_WAVE_GUIDE, &object.wave_guide_data)?;
    push_opt(&mut out, TAG_BONE_FX_DAMAGE, &object.bone_fx_damage)?;
    push_opt(&mut out, TAG_POISONED, &object.poisoned_behavior)?;
    push_opt(&mut out, TAG_DEFECTION, &object.defection_helper)?;
    push_opt(&mut out, TAG_FIRE_WEAPON_POWER, &object.fire_weapon_power)?;
    if object.fire_weapon_when_damaged.is_some()
        || object.pending_fire_when_damaged_weapon.is_some()
    {
        out.push(EntityModuleState {
            tag: TAG_FIRE_WEAPON_WHEN_DAMAGED.to_string(),
            payload: encode_payload(&FireWeaponWhenDamagedBundle {
                data: object.fire_weapon_when_damaged.clone(),
                runtime: object.temporary_weapon_runtime.clone(),
                pending_weapon: object.pending_fire_when_damaged_weapon.clone(),
            })?,
        });
    }
    push_opt(
        &mut out,
        TAG_TRANSITION_DAMAGE_FX,
        &object.transition_damage_fx,
    )?;
    push_opt(&mut out, TAG_FX_LIST_DIE, &object.fx_list_die)?;
    push_opt(&mut out, TAG_CREATE_OBJECT_DIE, &object.create_object_die)?;
    push_opt(&mut out, TAG_LIFETIME, &object.lifetime_update)?;
    push_opt(&mut out, TAG_SLOW_DEATH, &object.slow_death)?;
    push_opt(&mut out, TAG_HEIGHT_DIE, &object.height_die)?;
    push_opt(
        &mut out,
        TAG_FUEL_AIR_GAS_SLOW_DEATH,
        &object.fuel_air_gas_slow_death,
    )?;
    push_opt(
        &mut out,
        TAG_NEUTRON_MISSILE,
        &object.neutron_missile_update,
    )?;
    push_opt(
        &mut out,
        TAG_MISSILE_LAUNCHER_BUILDING,
        &object.missile_launcher_building,
    )?;
    push_opt(
        &mut out,
        TAG_SCUD_STORM_FLIGHT,
        &object.scud_storm_missile_flight,
    )?;
    push_opt(
        &mut out,
        TAG_CARPET_BOMB_TRANSPORT,
        &object.carpet_bomb_transport,
    )?;
    push_opt(
        &mut out,
        TAG_ARTILLERY_BARRAGE_TRANSPORT,
        &object.artillery_barrage_transport,
    )?;
    push_opt(
        &mut out,
        TAG_A10_STRIKE_TRANSPORT,
        &object.a10_strike_transport,
    )?;
    push_opt(
        &mut out,
        TAG_DAISY_CUTTER_TRANSPORT,
        &object.daisy_cutter_transport,
    )?;
    push_opt(
        &mut out,
        TAG_ANTHRAX_BOMB_TRANSPORT,
        &object.anthrax_bomb_transport,
    )?;
    push_opt(
        &mut out,
        TAG_CLUSTER_MINES_TRANSPORT,
        &object.cluster_mines_transport,
    )?;
    push_opt(
        &mut out,
        TAG_EMP_PULSE_TRANSPORT,
        &object.emp_pulse_transport,
    )?;
    push_opt(&mut out, TAG_TENSILE_FORMATION, &object.tensile_formation)?;
    push_opt(&mut out, TAG_FIRE_SPREAD, &object.fire_spread)?;
    push_opt(&mut out, TAG_BASE_REGENERATE, &object.base_regenerate)?;
    push_opt(&mut out, TAG_DEFAULT_AUTO_HEAL, &object.default_auto_heal)?;
    push_opt(&mut out, TAG_ENEMY_NEAR, &object.enemy_near)?;

    push_opt(&mut out, TAG_ANIMATION_STEERING, &object.animation_steering)?;
    push_opt(&mut out, TAG_FLOAT_UPDATE, &object.float_update)?;
    push_opt(&mut out, TAG_PRONE_UPDATE, &object.prone_update)?;
    push_opt(&mut out, TAG_RADIUS_DECAL, &object.radius_decal_update)?;
    push_opt(&mut out, TAG_CHECKPOINT, &object.checkpoint_update)?;
    push_opt(
        &mut out,
        TAG_SPECTRE_GUNSHIP_DEPLOYMENT,
        &object.spectre_gunship_deployment,
    )?;
    push_opt(
        &mut out,
        TAG_SPECTRE_GUNSHIP_UPDATE,
        &object.spectre_gunship_update,
    )?;
    push_opt(
        &mut out,
        TAG_SMART_BOMB_HOMING,
        &object.smart_bomb_target_homing,
    )?;
    push_opt(
        &mut out,
        TAG_HELICOPTER_SLOW_DEATH,
        &object.helicopter_slow_death,
    )?;
    push_opt(&mut out, TAG_JET_SLOW_DEATH, &object.jet_slow_death)?;
    push_opt(&mut out, TAG_MINE, &object.mine_data)?;
    push_present(
        &mut out,
        TAG_SPAWN_BEHAVIOR,
        SpawnBehaviorHiveResidual::present(object),
        &SpawnBehaviorHiveResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_FIRING_TRACKER,
        FiringTrackerResidual::present(object),
        &FiringTrackerResidual::from_object(object),
    )?;
    push_opt(
        &mut out,
        TAG_FIRE_OCL_AFTER_COOLDOWN,
        &object.fire_ocl_after_cooldown,
    )?;
    push_opt(&mut out, TAG_ASSAULT_TRANSPORT, &object.assault_transport)?;
    push_opt(&mut out, TAG_DEPLOY_STYLE, &object.deploy_style)?;
    push_opt(
        &mut out,
        TAG_COMMAND_BUTTON_HUNT,
        &object.command_button_hunt,
    )?;
    push_present(
        &mut out,
        TAG_FIRE_WEAPON_WHEN_DEAD,
        FireWeaponWhenDeadResidual::present(object),
        &FireWeaponWhenDeadResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_CREATE_OBJECT_DIE_TRANSFER,
        CreateObjectDieTransferResidual::present(object),
        &CreateObjectDieTransferResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_SPECIAL_POWER_COOLDOWNS,
        SpecialPowerCooldownResidual::present(object),
        &SpecialPowerCooldownResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_WEAPON_LOCK,
        WeaponLockResidual::present(object),
        &WeaponLockResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_EMOTICON_SURRENDER,
        EmoticonSurrenderResidual::present(object),
        &EmoticonSurrenderResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_PROJECTILE_FLIGHT,
        ProjectileFlightResiduals::present(object),
        &ProjectileFlightResiduals::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_ACTIVE_BODY,
        ActiveBodyCrushResidual::present(object),
        &ActiveBodyCrushResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_PHYSICS_BEHAVIOR,
        PhysicsBehaviorResidual::present(object),
        &PhysicsBehaviorResidual::from_object(object),
    )?;
    push_present(
        &mut out,
        TAG_RAILROAD,
        RailroadBehaviorResidual::present(object),
        &RailroadBehaviorResidual::from_object(object),
    )?;
    Ok(out)
}

fn push_present<T: Serialize>(
    out: &mut Vec<EntityModuleState>,
    tag: &'static str,
    present: bool,
    value: &T,
) -> Result<(), EntityLifecycleCodecError> {
    if present {
        out.push(EntityModuleState {
            tag: tag.to_string(),
            payload: encode_payload(value)?,
        });
    }
    Ok(())
}
