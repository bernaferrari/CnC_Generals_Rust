//! CPP WeaponSet active three-slot continuation in Main's normal envelope.
//! Weapon serde skips FX/fire clocks; the versioned tail owns all three slots.

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
    pub suspend_frames: [u32; 3],
    pub frame_clocks: [(u32, bool, u32); 3],
    pub weapon_bonus_target_faerie_fire: bool,
    pub weapon_bonus_garrisoned: bool,
    pub frame_statuses: [super::WeaponFireStatus; 3],
}

impl WeaponSlotsResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.weapon.is_some()
            || object.secondary_weapon.is_some()
            || object.tertiary_weapon.is_some()
            || object.active_weapon_slot != 0
            || object.weapon_bonus_target_faerie_fire
            || object.weapon_bonus_garrisoned
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            primary: object.weapon.clone(),
            secondary: object.secondary_weapon.clone(),
            tertiary: object.tertiary_weapon.clone(),
            active_slot: object.active_weapon_slot,
            weapon_bonus_target_faerie_fire: object.weapon_bonus_target_faerie_fire,
            weapon_bonus_garrisoned: object.weapon_bonus_garrisoned,
            frame_statuses: std::array::from_fn(|slot| {
                object.weapon_slot(slot as u8).map_or(Default::default(), |weapon| weapon.frame_clock_status)
            }),
            suspend_frames: std::array::from_fn(|i| {
                object
                    .weapon_slot(i as u8)
                    .map_or(0, |weapon| weapon.suspend_fx_frame)
            }),
            frame_clocks: std::array::from_fn(|i| {
                object.weapon_slot(i as u8).map_or((0, false, 0), |weapon| {
                    (weapon.next_fire_frame, weapon.frame_clock_active, weapon.reload_started_frame)
                })
            }),
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.weapon = self.primary;
        object.secondary_weapon = self.secondary;
        object.tertiary_weapon = self.tertiary;
        object.active_weapon_slot = self.active_slot;
        object.weapon_bonus_target_faerie_fire = self.weapon_bonus_target_faerie_fire;
        object.weapon_bonus_garrisoned = self.weapon_bonus_garrisoned;
        for slot in 0..3 {
            if let Some(weapon) = object.weapon_slot_mut(slot) {
                weapon.suspend_fx_frame = self.suspend_frames[slot as usize];
                weapon.frame_clock_status = self.frame_statuses[slot as usize];
                let (deadline, active, reload_started) = self.frame_clocks[slot as usize];
                weapon.next_fire_frame = deadline;
                weapon.frame_clock_active = active;
                weapon.reload_started_frame = reload_started;
            }
        }
    }
}
pub(crate) fn collect(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    if WeaponSlotsResidual::present(object) {
        let mut payload = b"WS6\0".to_vec();
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
            .strip_prefix(b"WS6\0")
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
    fn encoded_slots_preserve_frame_clocks_and_reload_continuation() {
        let mut source = Object::new(
            ThingTemplate::new("WeaponClock"), ObjectId(2), Team::USA,
        );
        use crate::game_logic::object::WeaponFireStatus;
        source.weapon = Some(Weapon {
            next_fire_frame: 123,
            reload_started_frame: 100,
            frame_clock_active: true,
            frame_clock_status: WeaponFireStatus::ReloadingClip,
            reloading_clip: true,
            ammo: Some(0),
            clip_size: 3,
            ..Weapon::default()
        });
        source.secondary_weapon = Some(Weapon {
            frame_clock_active: true,
            frame_clock_status: WeaponFireStatus::BetweenFiringShots,
            ammo: Some(0),
            ..Weapon::default()
        });
        source.tertiary_weapon = Some(Weapon {
            next_fire_frame: u32::MAX,
            reload_started_frame: 456,
            frame_clock_active: true,
            frame_clock_status: WeaponFireStatus::OutOfAmmo,
            ammo: Some(0),
            ..Weapon::default()
        });
        source.active_weapon_slot = 2;
        source.weapon_bonus_target_faerie_fire = true;
        source.weapon_bonus_garrisoned = true;
        let mut records = Vec::new();
        collect(&source, &mut records).unwrap();
        assert!(records[0].payload.starts_with(b"WS6\0"));
        let decoded = decode(&records[0].payload).unwrap();
        let mut restored = Object::new(
            ThingTemplate::new("WeaponClock"), ObjectId(2), Team::USA,
        );
        decoded.apply(&mut restored);
        let primary = restored.weapon.as_ref().unwrap();
        assert_eq!(primary.next_fire_frame, 123);
        assert_eq!(primary.reload_started_frame, 100);
        assert!(primary.frame_clock_active);
        assert!(primary.reloading_clip);
        assert_eq!(primary.ammo, Some(0));
        assert_eq!(primary.frame_clock_status, WeaponFireStatus::ReloadingClip);
        let secondary = restored.secondary_weapon.as_ref().unwrap();
        assert_eq!(secondary.ammo, Some(0));
        assert_eq!(secondary.frame_clock_status, WeaponFireStatus::BetweenFiringShots);
        let tertiary = restored.tertiary_weapon.as_ref().unwrap();
        assert_eq!(tertiary.next_fire_frame, u32::MAX);
        assert_eq!(tertiary.reload_started_frame, 456);
        assert!(tertiary.frame_clock_active);
        assert_eq!(tertiary.ammo, Some(0));
        assert_eq!(tertiary.frame_clock_status, WeaponFireStatus::OutOfAmmo);
        assert_eq!(restored.active_weapon_slot, 2);
        assert!(restored.weapon_bonus_target_faerie_fire);
        assert!(restored.weapon_bonus_garrisoned);
        let mut old_version = records[0].payload.clone();
        old_version[..4].copy_from_slice(b"WS5\0");
        assert!(decode(&old_version).is_err());
        source.weapon = None;
        source.secondary_weapon = None;
        source.tertiary_weapon = None;
        source.active_weapon_slot = 0;
        records.clear();
        collect(&source, &mut records).unwrap();
        assert_eq!(records.len(), 1, "condition survives without a weapon");
        decode(&records[0].payload).unwrap().apply(&mut restored);
        assert!(restored.weapon_bonus_target_faerie_fire);
        assert!(restored.weapon_bonus_garrisoned);
        assert!(restored.weapon.is_none());
        assert!(restored.tertiary_weapon.is_none());
    }

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
                b"WS6\x01".to_vec()
            } else {
                b"WS6\0".to_vec()
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
            assert!(!owner.test_weapon_condition(8));
        }
    }
}
