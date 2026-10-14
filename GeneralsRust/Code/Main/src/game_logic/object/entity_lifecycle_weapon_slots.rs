//! CPP WeaponSet active three-slot continuation in Main's normal envelope.
//! Weapon serde skips SuspendFX; this component explicitly owns all three deadlines.

use super::Object;
use super::entity_lifecycle_inventory::{decode_payload, encode_payload};
use super::entity_lifecycle_tags::TAG_WEAPON_SLOTS;
use crate::game_logic::Weapon;
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct WeaponSlotsResidual {
    pub primary: Option<Weapon>,
    pub secondary: Option<Weapon>,
    pub tertiary: Option<Weapon>,
    pub active_slot: u8,
    pub mine_detail: bool,
    pub suspend_frames: [u32; 3],
}

impl WeaponSlotsResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.weapon.is_some()
            || object.secondary_weapon.is_some()
            || object.tertiary_weapon.is_some()
            || object.active_weapon_slot != 0
            || object.weapon_set_mine_clearing_detail
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            primary: object.weapon.clone(),
            secondary: object.secondary_weapon.clone(),
            tertiary: object.tertiary_weapon.clone(),
            active_slot: object.active_weapon_slot,
            mine_detail: object.weapon_set_mine_clearing_detail,
            suspend_frames: std::array::from_fn(|i| {
                object
                    .weapon_slot(i as u8)
                    .map_or(0, |weapon| weapon.suspend_fx_frame)
            }),
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.weapon = self.primary;
        object.secondary_weapon = self.secondary;
        object.tertiary_weapon = self.tertiary;
        object.active_weapon_slot = self.active_slot;
        object.weapon_set_mine_clearing_detail = self.mine_detail;
        for slot in 0..3 {
            if let Some(weapon) = object.weapon_slot_mut(slot) {
                weapon.suspend_fx_frame = self.suspend_frames[slot as usize];
            }
        }
    }
}
pub(crate) fn collect(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    if WeaponSlotsResidual::present(object) {
        let mut payload = b"WS3\0".to_vec();
        payload.extend(encode_payload(&WeaponSlotsResidual::from_object(object))?);
        out.push(EntityModuleState {
            tag: TAG_WEAPON_SLOTS.to_owned(),
            payload,
        });
    }
    Ok(())
}
pub(crate) fn decode(payload: &[u8]) -> Result<WeaponSlotsResidual, EntityLifecycleCodecError> {
    let bytes =
        payload
            .strip_prefix(b"WS3\0")
            .ok_or(EntityLifecycleCodecError::InvalidOwnerComponent {
                component: "weapon slots version",
            })?;
    let slots = decode_payload::<WeaponSlotsResidual>(bytes)?;
    if slots.active_slot > 2 {
        return Err(EntityLifecycleCodecError::InvalidOwnerComponent {
            component: "weapon slots",
        });
    }
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::super::entity_lifecycle_inventory::PathFollowResidual;
    use super::super::entity_lifecycle_tags::TAG_PATH_EXTRA_DISTANCE;
    use super::*;
    use crate::game_logic::{ObjectId, Team, ThingTemplate};

    #[test]
    fn malformed_or_duplicate_selected_slots_reject_before_any_owner_mutation() {
        let mut owner = Object::new(
            ThingTemplate::new("InvalidWeaponSlots"),
            ObjectId(1),
            Team::USA,
        );
        owner.weapon = Some(Weapon {
            ammo: Some(1),
            clip_size: 3,
            last_fire_time: 0.25,
            ..Weapon::default()
        });
        owner.path_extra_distance = 7.0;
        for kind in 0..3 {
            let mut envelope = owner.entity_lifecycle_envelope().unwrap();
            let mut slots = WeaponSlotsResidual::from_object(&owner);
            slots.active_slot = if kind == 1 { 3 } else { 0 };
            let mut bytes = if kind == 0 {
                b"WS3\x01".to_vec()
            } else {
                b"WS3\0".to_vec()
            };
            bytes.extend(encode_payload(&slots).unwrap());
            envelope.module_states.retain(|m| m.tag != TAG_WEAPON_SLOTS);
            envelope.module_states.insert(
                0,
                EntityModuleState {
                    tag: TAG_PATH_EXTRA_DISTANCE.to_owned(),
                    payload: encode_payload(&PathFollowResidual {
                        path_extra_distance: 99.0,
                        is_exact_path: false,
                        is_attack_path: false,
                    })
                    .unwrap(),
                },
            );
            let weapon = EntityModuleState {
                tag: TAG_WEAPON_SLOTS.to_owned(),
                payload: bytes,
            };
            envelope.module_states.push(weapon.clone());
            if kind == 2 {
                envelope.module_states.push(weapon);
            }
            assert!(owner.entity_apply_lifecycle_envelope(&envelope).is_err());
            assert_eq!(owner.path_extra_distance, 7.0);
            assert_eq!(owner.weapon.as_ref().unwrap().ammo, Some(1));
            assert_eq!(owner.weapon.as_ref().unwrap().last_fire_time, 0.25);
            assert!(!owner.weapon_set_mine_clearing_detail);
        }
    }
}
