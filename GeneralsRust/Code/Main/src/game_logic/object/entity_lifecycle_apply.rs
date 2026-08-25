//! Apply known envelope tags onto a Main `Object`. Unknown tags are skipped.

use super::Object;
use super::entity_lifecycle_inventory::{
    FireWeaponWhenDamagedBundle, FiringTrackerResidual, SpawnBehaviorHiveResidual, decode_payload,
};
use super::entity_lifecycle_projectiles::ProjectileFlightResiduals;
use super::entity_lifecycle_residuals::{
    ActiveBodyCrushResidual, CreateObjectDieTransferResidual, EmoticonSurrenderResidual,
    FireWeaponWhenDeadResidual, PhysicsBehaviorResidual, RailroadBehaviorResidual,
    SpecialPowerCooldownResidual, WeaponLockResidual,
};
use super::entity_lifecycle_tags::*;
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};

pub(crate) fn apply_module_states(
    object: &mut Object,
    modules: &[EntityModuleState],
) -> Result<(), EntityLifecycleCodecError> {
    for module in modules {
        apply_one(object, module)?;
    }
    Ok(())
}

fn apply_one(
    object: &mut Object,
    module: &EntityModuleState,
) -> Result<(), EntityLifecycleCodecError> {
    let payload = module.payload.as_slice();
    match module.tag.as_str() {
        TAG_UPGRADE_DIE => object.upgrade_die = Some(decode_payload(payload)?),
        TAG_SPECIAL_POWER_COMPLETION => {
            object.special_power_completion = Some(decode_payload(payload)?);
        }
        TAG_BATTLE_BUS_BODY => object.battle_bus_body = Some(decode_payload(payload)?),
        TAG_CAPTURE_CHANNEL => object.capture_channel = Some(decode_payload(payload)?),
        TAG_HACKER_DISABLE_CHANNEL => {
            object.hacker_disable_channel = Some(decode_payload(payload)?);
        }
        TAG_TOPPLE => object.topple_data = Some(decode_payload(payload)?),
        TAG_STRUCTURE_TOPPLE => object.structure_topple_data = Some(decode_payload(payload)?),
        TAG_STRUCTURE_COLLAPSE => object.structure_collapse_data = Some(decode_payload(payload)?),
        TAG_KEEP_OBJECT_DIE => object.keep_object_die = Some(decode_payload(payload)?),
        TAG_WAVE_GUIDE => object.wave_guide_data = Some(decode_payload(payload)?),
        TAG_BONE_FX_DAMAGE => object.bone_fx_damage = Some(decode_payload(payload)?),
        TAG_POISONED => object.poisoned_behavior = Some(decode_payload(payload)?),
        TAG_DEFECTION => object.defection_helper = Some(decode_payload(payload)?),
        TAG_FIRE_WEAPON_POWER => object.fire_weapon_power = Some(decode_payload(payload)?),
        TAG_FIRE_WEAPON_WHEN_DAMAGED => {
            let bundle: FireWeaponWhenDamagedBundle = decode_payload(payload)?;
            object.fire_weapon_when_damaged = bundle.data;
            object.temporary_weapon_runtime = bundle.runtime;
            object.pending_fire_when_damaged_weapon = bundle.pending_weapon;
        }
        TAG_TRANSITION_DAMAGE_FX => object.transition_damage_fx = Some(decode_payload(payload)?),
        TAG_FX_LIST_DIE => object.fx_list_die = Some(decode_payload(payload)?),
        TAG_CREATE_OBJECT_DIE => object.create_object_die = Some(decode_payload(payload)?),
        TAG_LIFETIME => object.lifetime_update = Some(decode_payload(payload)?),
        TAG_SLOW_DEATH => object.slow_death = Some(decode_payload(payload)?),
        TAG_HEIGHT_DIE => object.height_die = Some(decode_payload(payload)?),
        TAG_FUEL_AIR_GAS_SLOW_DEATH => {
            object.fuel_air_gas_slow_death = Some(decode_payload(payload)?);
        }
        TAG_NEUTRON_MISSILE => object.neutron_missile_update = Some(decode_payload(payload)?),
        TAG_MISSILE_LAUNCHER_BUILDING => {
            object.missile_launcher_building = Some(decode_payload(payload)?);
        }
        TAG_SCUD_STORM_FLIGHT => object.scud_storm_missile_flight = Some(decode_payload(payload)?),
        TAG_CARPET_BOMB_TRANSPORT => object.carpet_bomb_transport = Some(decode_payload(payload)?),
        TAG_ARTILLERY_BARRAGE_TRANSPORT => {
            object.artillery_barrage_transport = Some(decode_payload(payload)?);
        }
        TAG_A10_STRIKE_TRANSPORT => object.a10_strike_transport = Some(decode_payload(payload)?),
        TAG_DAISY_CUTTER_TRANSPORT => {
            object.daisy_cutter_transport = Some(decode_payload(payload)?)
        }
        TAG_ANTHRAX_BOMB_TRANSPORT => {
            object.anthrax_bomb_transport = Some(decode_payload(payload)?)
        }
        TAG_CLUSTER_MINES_TRANSPORT => {
            object.cluster_mines_transport = Some(decode_payload(payload)?);
        }
        TAG_EMP_PULSE_TRANSPORT => object.emp_pulse_transport = Some(decode_payload(payload)?),
        TAG_TENSILE_FORMATION => object.tensile_formation = Some(decode_payload(payload)?),
        TAG_FIRE_SPREAD => object.fire_spread = Some(decode_payload(payload)?),
        TAG_BASE_REGENERATE => object.base_regenerate = Some(decode_payload(payload)?),
        TAG_DEFAULT_AUTO_HEAL => object.default_auto_heal = Some(decode_payload(payload)?),
        TAG_ENEMY_NEAR => object.enemy_near = Some(decode_payload(payload)?),

        TAG_ANIMATION_STEERING => object.animation_steering = Some(decode_payload(payload)?),
        TAG_FLOAT_UPDATE => object.float_update = Some(decode_payload(payload)?),
        TAG_PRONE_UPDATE => object.prone_update = Some(decode_payload(payload)?),
        TAG_RADIUS_DECAL => object.radius_decal_update = Some(decode_payload(payload)?),
        TAG_CHECKPOINT => object.checkpoint_update = Some(decode_payload(payload)?),
        TAG_SPECTRE_GUNSHIP_DEPLOYMENT => {
            object.spectre_gunship_deployment = Some(decode_payload(payload)?);
        }
        TAG_SPECTRE_GUNSHIP_UPDATE => {
            object.spectre_gunship_update = Some(decode_payload(payload)?);
        }
        TAG_SMART_BOMB_HOMING => object.smart_bomb_target_homing = Some(decode_payload(payload)?),
        TAG_HELICOPTER_SLOW_DEATH => object.helicopter_slow_death = Some(decode_payload(payload)?),
        TAG_JET_SLOW_DEATH => object.jet_slow_death = Some(decode_payload(payload)?),
        TAG_MINE => object.mine_data = Some(decode_payload(payload)?),
        TAG_SPAWN_BEHAVIOR => {
            let residual: SpawnBehaviorHiveResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_FIRING_TRACKER => {
            let residual: FiringTrackerResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_FIRE_OCL_AFTER_COOLDOWN => {
            object.fire_ocl_after_cooldown = Some(decode_payload(payload)?);
        }
        TAG_ASSAULT_TRANSPORT => object.assault_transport = Some(decode_payload(payload)?),
        TAG_DEPLOY_STYLE => object.deploy_style = Some(decode_payload(payload)?),
        TAG_COMMAND_BUTTON_HUNT => object.command_button_hunt = Some(decode_payload(payload)?),
        TAG_FIRE_WEAPON_WHEN_DEAD => {
            let residual: FireWeaponWhenDeadResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_CREATE_OBJECT_DIE_TRANSFER => {
            let residual: CreateObjectDieTransferResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_SPECIAL_POWER_COOLDOWNS => {
            let residual: SpecialPowerCooldownResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_WEAPON_LOCK => {
            let residual: WeaponLockResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_EMOTICON_SURRENDER => {
            let residual: EmoticonSurrenderResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_PROJECTILE_FLIGHT => {
            let residual: ProjectileFlightResiduals = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_ACTIVE_BODY => {
            let residual: ActiveBodyCrushResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_PHYSICS_BEHAVIOR => {
            let residual: PhysicsBehaviorResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        TAG_RAILROAD => {
            let residual: RailroadBehaviorResidual = decode_payload(payload)?;
            residual.apply(object);
        }
        _ => {}
    }
    Ok(())
}
