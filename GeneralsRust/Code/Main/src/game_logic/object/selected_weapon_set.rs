//! CPP WeaponSet::updateWeaponSet: immutable receiving rows, exactly three live slots.
use super::Object;
use crate::game_logic::{ThingTemplate, Weapon, WeaponLockType};

#[derive(Debug, Clone)]
pub(crate) struct WeaponDefinition {
    prototype: Weapon,
    suspend_delay: u32,
    projectile_name: Option<String>,
}
impl WeaponDefinition {
    fn instantiate(&self, frame: u32) -> Weapon {
        let mut weapon = self.prototype.clone();
        weapon.last_fire_time = 0.0;
        weapon.reloading_clip = false;
        weapon.last_bonus_rof = 0.0;
        weapon.ammo = (weapon.clip_size > 0).then_some(weapon.clip_size);
        weapon.suspend_fx_frame = frame.wrapping_add(self.suspend_delay);
        weapon
    }
}
#[derive(Debug, Clone)]
pub(crate) struct WeaponRow {
    slots: [Option<WeaponDefinition>; 3],
    pub(crate) names: [Option<String>; 3],
    shared: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct BoundWeaponSets {
    base: WeaponRow,
    mine: Option<WeaponRow>,
}
impl ThingTemplate {
    /// Factory installs the already admitted immutable base row at its frame.
    /// Some([None;3]) is explicit no-weapon content, not missing admission.
    pub(in crate::game_logic) fn instantiate_base_weapon_row(
        &self,
        frame: u32,
    ) -> Option<[Option<Weapon>; 3]> {
        let row = &self.selected_weapon_sets.as_ref()?.base;
        Some(std::array::from_fn(|i| {
            row.slots[i].as_ref().map(|rule| rule.instantiate(frame))
        }))
    }
    /// Admit receiving immutable native rules without a second catalog lookup.
    pub(in crate::game_logic) fn bind_native_weapon_row(
        &mut self,
        native: [Option<&gamelogic::weapon::WeaponTemplate>; 3],
    ) {
        let names = std::array::from_fn(|i| native[i].map(|rule| rule.name.clone()));
        let slots = std::array::from_fn(|i| {
            native[i].and_then(|rule| {
                Some(WeaponDefinition {
                    prototype: ThingTemplate::weapon_from_template_at(rule, 0)?,
                    suspend_delay: rule.suspend_fx_delay,
                    projectile_name: Some(rule.projectile_name.clone()),
                })
            })
        });
        let mine = self
            .selected_weapon_sets
            .as_ref()
            .and_then(|rows| rows.mine.clone());
        self.selected_weapon_sets = Some(BoundWeaponSets {
            base: WeaponRow {
                slots,
                names,
                shared: self.weapon_lock_shared_across_sets,
            },
            mine,
        });
    }
    pub(crate) fn bind_selected_weapon_sets(&mut self, base: [Option<Weapon>; 3]) {
        fn bind(name: Option<&str>, prototype: Option<Weapon>) -> Option<WeaponDefinition> {
            let native = name.and_then(|name| {
                crate::game_logic::weapon_bootstrap::with_host_weapon_store(|store| {
                    store.find_weapon_template(name).cloned()
                })
                .ok()
                .flatten()
            });
            if let Some(native) = native {
                let prototype = ThingTemplate::weapon_from_template_at(&native, 0)?;
                Some(WeaponDefinition {
                    prototype,
                    suspend_delay: native.suspend_fx_delay,
                    projectile_name: Some(native.projectile_name.clone()),
                })
            } else {
                prototype.map(|prototype| WeaponDefinition {
                    prototype,
                    suspend_delay: 0,
                    projectile_name: None,
                })
            }
        }
        let base_names = [
            self.primary_weapon_name.clone(),
            self.secondary_weapon_name.clone(),
            self.tertiary_weapon_name.clone(),
        ];
        let slots = std::array::from_fn(|i| bind(base_names[i].as_deref(), base[i].clone()));
        let mine_names = self.mine_weapon_set_names.clone().or_else(|| {
            (self.mine_clearing_primary_weapon_name.is_some()
                || self.mine_clearing_primary_weapon.is_some())
            .then(|| [self.mine_clearing_primary_weapon_name.clone(), None, None])
        });
        let mine = mine_names.map(|names| WeaponRow {
            slots: std::array::from_fn(|i| {
                bind(
                    names[i].as_deref(),
                    (i == 0)
                        .then(|| self.mine_clearing_primary_weapon.clone())
                        .flatten(),
                )
            }),
            names,
            shared: self.mine_weapon_set_shared,
        });
        self.selected_weapon_sets = Some(BoundWeaponSets {
            base: WeaponRow {
                slots,
                names: base_names,
                shared: self.weapon_lock_shared_across_sets,
            },
            mine,
        });
    }
}
impl Object {
    pub(in crate::game_logic) fn selected_projectile_name(&self, slot: u8) -> Option<&str> {
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        let row = if self.weapon_set_mine_clearing_detail {
            rows.mine.as_ref().unwrap_or(&rows.base)
        } else {
            &rows.base
        };
        row.slots
            .get(usize::from(slot))?
            .as_ref()?
            .projectile_name
            .as_deref()
    }
    pub(crate) fn selected_weapon_name(&self, slot: u8) -> Option<&str> {
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        let row = if self.weapon_set_mine_clearing_detail {
            rows.mine.as_ref().unwrap_or(&rows.base)
        } else {
            &rows.base
        };
        row.names.get(slot as usize)?.as_deref()
    }
    pub fn set_weapon_set_mine_clearing_detail_at_frame(&mut self, enabled: bool, frame: u32) {
        if enabled == self.weapon_set_mine_clearing_detail {
            return;
        }
        let Some(rows) = self.thing.template.selected_weapon_sets.as_ref() else {
            // A plain inert object has no conditional row to replace.
            self.weapon_set_mine_clearing_detail = enabled;
            return;
        };
        let previous_mine = self.weapon_set_mine_clearing_detail && rows.mine.is_some();
        let next_mine = enabled && rows.mine.is_some();
        self.weapon_set_mine_clearing_detail = enabled;
        if previous_mine == next_mine {
            return;
        }
        let row = if next_mine {
            rows.mine.as_ref().unwrap()
        } else {
            &rows.base
        };
        let slots: [Option<Weapon>; 3] = std::array::from_fn(|i| {
            row.slots[i]
                .as_ref()
                .map(|definition| definition.instantiate(frame))
        });
        let shared = row.shared;
        if !shared {
            self.release_weapon_lock(WeaponLockType::LockedPermanently);
            self.set_active_weapon_slot(0);
        }
        // Replace the owned slots in the original TERTIARY → PRIMARY order.
        let [primary, secondary, tertiary] = slots;
        self.tertiary_weapon = tertiary;
        self.secondary_weapon = secondary;
        self.weapon = primary;
        self.reset_weapon_barrel_states();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, ObjectId, Team};
    fn world(mine_damage: f32) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        let mut definition = ThingTemplate::new("ReceivingMineSet");
        definition.primary_weapon = Some(Weapon {
            damage: 10.0,
            clip_size: 5,
            ammo: Some(5),
            ..Weapon::default()
        });
        definition.secondary_weapon = Some(Weapon::default());
        definition.tertiary_weapon = Some(Weapon::default());
        definition.mine_clearing_primary_weapon = Some(Weapon {
            damage: mine_damage,
            clip_size: 2,
            ammo: Some(2),
            ..Weapon::default()
        });
        world.templates.insert(definition.name.clone(), definition);
        let id = world
            .create_object("ReceivingMineSet", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        (world, id)
    }
    #[test]
    fn parsed_receiving_mine_row_replaces_all_three_and_preserves_shared_lock() {
        let mut world = GameLogic::new();
        let mut ini = String::new();
        for (name, damage) in [("OwnedMineP", 30), ("OwnedMineS", 40), ("OwnedMineT", 50)] {
            ini.push_str(&format!("Weapon {name}\nPrimaryDamage = {damage}\nAttackRange = 100\nClipSize = 3\nSuspendFXDelay = 1000\nEnd\n"));
        }
        assert_eq!(
            crate::assets::ini_template_loader::register_weapons_from_ini_text(&ini),
            3
        );
        let mut parser = crate::assets::IniParser::new();
        parser.parse_ini_content("Object ParsedMineRow\nKindOf = VEHICLE\nWeaponSet\nConditions = None\nWeapon = PRIMARY OwnedMineP\nWeapon = SECONDARY OwnedMineS\nWeapon = TERTIARY OwnedMineT\nEnd\nWeaponSet\nConditions = MINE_CLEARING_DETAIL\nWeapon = PRIMARY OwnedMineT\nWeapon = SECONDARY OwnedMineP\nWeapon = TERTIARY OwnedMineS\nShareWeaponLock = Yes\nEnd\nEnd\n", "owned-mine-row.ini").unwrap();
        let template = GameLogic::build_template_from_object_definition(
            "ParsedMineRow",
            parser.get_definition("ParsedMineRow").unwrap(),
            None,
        );
        world.templates.insert("ParsedMineRow".into(), template);
        let id = world
            .create_object("ParsedMineRow", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = world.host_object_mut(id).unwrap();
        object.set_active_weapon_slot(2);
        assert!(object.set_weapon_lock(2, WeaponLockType::LockedPermanently));
        object.set_weapon_set_mine_clearing_detail_at_frame(true, 80);
        assert_eq!(object.active_weapon_slot, 2);
        assert!(object.is_weapon_locked());
        for (slot, damage) in [(0, 50.0), (1, 30.0), (2, 40.0)] {
            let weapon = object.weapon_slot(slot).unwrap();
            assert_eq!(weapon.damage, damage);
            assert_eq!(weapon.ammo, Some(3));
            assert_eq!(weapon.suspend_fx_frame, 110);
        }
        object.set_weapon_set_mine_clearing_detail_at_frame(false, 81);
        assert_eq!(object.active_weapon_slot, 0);
        assert!(!object.is_weapon_locked());
        assert_eq!(object.weapon_slot(0).unwrap().damage, 30.0);
    }
    #[test]
    fn receiving_selected_set_recreates_three_slots_once_without_peer_cache() {
        let (mut owner, id) = world(90.0);
        let (mut peer, peer_id) = world(180.0);
        assert_eq!(id, peer_id);
        let object = owner.host_object_mut(id).unwrap();
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.weapon.as_mut().unwrap().last_fire_time = 25.0;
        object.set_active_weapon_slot(2);
        object.set_weapon_lock(2, WeaponLockType::LockedPermanently);
        object.set_weapon_set_mine_clearing_detail_at_frame(true, 30);
        assert_eq!(object.weapon.as_ref().unwrap().damage, 90.0);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(2));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_time, 0.0);
        assert_eq!(object.active_weapon_slot, 0);
        assert!(!object.is_weapon_locked());
        assert!(object.secondary_weapon.is_none() && object.tertiary_weapon.is_none());
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.weapon.as_mut().unwrap().last_fire_time = 1.0;
        object.set_weapon_set_mine_clearing_detail_at_frame(true, 31);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_time, 1.0);
        peer.host_object_mut(peer_id)
            .unwrap()
            .set_weapon_set_mine_clearing_detail_at_frame(true, 700);
        assert_eq!(
            peer.host_object(peer_id)
                .unwrap()
                .weapon
                .as_ref()
                .unwrap()
                .damage,
            180.0
        );
        owner
            .host_object_mut(id)
            .unwrap()
            .set_weapon_set_mine_clearing_detail_at_frame(false, 32);
        let object = owner.host_object(id).unwrap();
        assert_eq!(object.weapon.as_ref().unwrap().damage, 10.0);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(5));
        assert!(object.secondary_weapon.is_some() && object.tertiary_weapon.is_some());
    }
    #[test]
    fn normal_encoded_save_continues_selected_active_three_without_reallocating() {
        let (mut original, id) = world(90.0);
        original.frame = 37;
        let object = original.host_object_mut(id).unwrap();
        object.set_weapon_set_mine_clearing_detail_at_frame(true, 37);
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.weapon.as_mut().unwrap().last_fire_time = 1.2;
        object.weapon.as_mut().unwrap().suspend_fx_frame = 49;
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&original).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _) = world(90.0);
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        let object = restored.host_object_mut(id).unwrap();
        assert!(object.weapon_set_mine_clearing_detail);
        assert!(object.secondary_weapon.is_none() && object.tertiary_weapon.is_none());
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_time, 1.2);
        assert_eq!(object.weapon.as_ref().unwrap().suspend_fx_frame, 49);
        object.set_weapon_set_mine_clearing_detail_at_frame(true, 38);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        object.set_weapon_set_mine_clearing_detail_at_frame(false, 39);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(5));
    }
}

#[cfg(test)]
mod native_row_admission_tests {
    use super::*;
    #[test]
    fn explicit_projectileless_base_preserves_existing_mine_row() {
        let mut template = ThingTemplate::new("ReceivingRows");
        template.mine_clearing_primary_weapon = Some(Weapon {
            damage: 88.0,
            range: 200.0,
            ..Default::default()
        });
        template.bind_selected_weapon_sets([None, None, None]);
        let mut native = gamelogic::weapon::WeaponTemplate::new("ReceivingInstant".into());
        native.primary_damage = 17.0;
        native.attack_range = 100.0;
        native.projectile_name = String::new();
        template.bind_native_weapon_row([Some(&native), None, None]);
        let rows = template.selected_weapon_sets.as_ref().unwrap();
        assert_eq!(
            rows.base.slots[0]
                .as_ref()
                .unwrap()
                .projectile_name
                .as_deref(),
            Some("")
        );
        assert_eq!(
            rows.mine.as_ref().unwrap().slots[0]
                .as_ref()
                .unwrap()
                .prototype
                .damage,
            88.0
        );
        let owner = Object::new(
            template,
            crate::game_logic::ObjectId(1),
            crate::game_logic::Team::USA,
        );
        assert_eq!(owner.selected_projectile_name(0), Some(""));
    }
}
