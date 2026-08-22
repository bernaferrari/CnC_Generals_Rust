//! Produce ordered module-state payloads from a live Main `Object`.

use super::entity_lifecycle_projectiles::ProjectileFlightResiduals;
use super::entity_lifecycle_residuals::{
    CreateObjectDieTransferResidual, EmoticonSurrenderResidual, FireWeaponWhenDeadResidual,
    SpecialPowerCooldownResidual, WeaponLockResidual,
};
use super::entity_lifecycle_tags::*;
use super::Object;
use crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponRuntimeBundle;
use crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData;
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
    push_opt(&mut out, TAG_STRUCTURE_TOPPLE, &object.structure_topple_data)?;
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
    push_opt(&mut out, TAG_EMP_PULSE_TRANSPORT, &object.emp_pulse_transport)?;
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
    push_opt(
        &mut out,
        TAG_FIRE_OCL_AFTER_COOLDOWN,
        &object.fire_ocl_after_cooldown,
    )?;
    push_opt(&mut out, TAG_ASSAULT_TRANSPORT, &object.assault_transport)?;
    push_opt(&mut out, TAG_DEPLOY_STYLE, &object.deploy_style)?;
    push_opt(&mut out, TAG_COMMAND_BUTTON_HUNT, &object.command_button_hunt)?;
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
