//! Main WeaponSet slot inventory. Not Entity Xfer authority.
//! Weapon::suspend_fx_frame is serde-skipped (ObjectSnapshot tail owns it).

use super::Object;
use super::entity_lifecycle_inventory::{decode_payload, push_present};
use super::entity_lifecycle_tags::TAG_WEAPON_SLOTS;
use crate::game_logic::Weapon;
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct WeaponSlotsResidual {
    pub primary: Option<Weapon>,
    pub secondary: Option<Weapon>,
    pub tertiary: Option<Weapon>,
    pub mine_clearing: Option<Weapon>,
    pub active_slot: u8,
}

impl WeaponSlotsResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.weapon.is_some()
            || object.secondary_weapon.is_some()
            || object.tertiary_weapon.is_some()
            || object.mine_clearing_primary_weapon.is_some()
            || object.active_weapon_slot != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            primary: object.weapon.clone(),
            secondary: object.secondary_weapon.clone(),
            tertiary: object.tertiary_weapon.clone(),
            mine_clearing: object.mine_clearing_primary_weapon.clone(),
            active_slot: object.active_weapon_slot,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.weapon = self.primary;
        object.secondary_weapon = self.secondary;
        object.tertiary_weapon = self.tertiary;
        object.mine_clearing_primary_weapon = self.mine_clearing;
        object.active_weapon_slot = self.active_slot;
    }
}

pub(crate) fn collect(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    push_present(
        out,
        TAG_WEAPON_SLOTS,
        WeaponSlotsResidual::present(object),
        &WeaponSlotsResidual::from_object(object),
    )
}

pub(crate) fn apply(
    object: &mut Object,
    module: &EntityModuleState,
) -> Result<bool, EntityLifecycleCodecError> {
    if module.tag != TAG_WEAPON_SLOTS {
        return Ok(false);
    }
    decode_payload::<WeaponSlotsResidual>(module.payload.as_slice())?.apply(object);
    Ok(true)
}
