//! Connect authored template data and live objects to the simulation's WeaponSets.
use std::sync::Arc;

use super::Object;
use crate::game_logic::{ThingTemplate, Weapon, WeaponLockType};
pub(crate) use generals_sim::weapon::{BoundWeaponSets, WeaponRow, WeaponRowIdentity};
use generals_sim::weapon::{
    WeaponCatalog, WeaponChoicePolicy, WeaponDefinition, WeaponSlotDefinition,
    weapon_condition_mask,
};

impl ThingTemplate {
    /// Instantiate the admitted base row before onCreate.
    pub(in crate::game_logic) fn instantiate_base_weapon_row(
        &self,
        frame: u32,
    ) -> Option<[Option<Weapon>; 3]> {
        Some(
            self.selected_weapon_sets
                .as_ref()?
                .base()
                .instantiate(frame),
        )
    }

    /// Admit supplied immutable definitions and preserve the template's mine row.
    pub(in crate::game_logic) fn bind_native_weapon_row(
        &mut self,
        native: [Option<&WeaponDefinition>; 3],
    ) {
        let names = native.map(|rule| rule.map(|rule| rule.name.clone()));
        let slots = native
            .map(|rule| rule.map(|rule| WeaponSlotDefinition::Authored(Arc::new(rule.clone()))));
        let mine = self
            .selected_weapon_sets
            .as_ref()
            .and_then(BoundWeaponSets::mine)
            .cloned();
        let base = WeaponRow::new(
            names,
            self.weapon_lock_shared_across_sets,
            Vec::new(),
            None,
            slots,
        );
        self.selected_weapon_sets = Some(BoundWeaponSets::receiving(base, mine, Vec::new()));
    }

    /// Resolve every authored row from this match's effective content.
    pub(crate) fn bind_receiving_weapon_sets(&mut self, catalog: &WeaponCatalog) {
        let names = [
            self.primary_weapon_name.clone(),
            self.secondary_weapon_name.clone(),
            self.tertiary_weapon_name.clone(),
        ];
        let mine = self
            .mine_weapon_set_names
            .clone()
            .or_else(|| {
                self.mine_clearing_primary_weapon_name
                    .as_ref()
                    .map(|name| [Some(name.clone()), None, None])
            })
            .map(|names| {
                WeaponRow::from_catalog(
                    names,
                    self.mine_weapon_set_shared,
                    Vec::new(),
                    None,
                    catalog,
                )
            });
        let conditional = self
            .receiving_object_definition
            .as_ref()
            .map(|definition| {
                definition
                    .weapon_sets
                    .iter()
                    .map(|set| {
                        let tokens = set
                            .conditions
                            .iter()
                            .flat_map(|condition| {
                                condition.split(|ch: char| {
                                    ch.is_ascii_whitespace() || matches!(ch, ',' | '|')
                                })
                            })
                            .map(str::trim)
                            .filter(|token| {
                                !token.is_empty() && !token.eq_ignore_ascii_case("none")
                            })
                            .collect::<Vec<_>>();
                        let alternatives = if set.is_unconditional() {
                            vec![0]
                        } else {
                            tokens
                                .iter()
                                .map(|token| weapon_condition_mask(token))
                                .collect::<Option<Vec<_>>>()
                                .map(|bits| bits.into_iter().fold(0, |mask, bit| mask | bit))
                                .filter(|mask| *mask != 0)
                                .into_iter()
                                .collect()
                        };
                        WeaponRow::from_catalog(
                            [
                                set.primary_weapon.clone(),
                                set.secondary_weapon.clone(),
                                set.tertiary_weapon.clone(),
                            ],
                            weapon_lock_shared(&set.attributes),
                            alternatives,
                            Some(set.choice),
                            catalog,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let base = WeaponRow::from_catalog(
            names,
            self.weapon_lock_shared_across_sets,
            Vec::new(),
            None,
            catalog,
        );
        self.selected_weapon_sets = Some(BoundWeaponSets::receiving(base, mine, conditional));
    }

    pub(crate) fn bind_selected_weapon_sets(&mut self, base: [Option<Weapon>; 3]) {
        fn bind(name: Option<&str>, prototype: Option<Weapon>) -> Option<WeaponSlotDefinition> {
            let native =
                name.and_then(crate::game_logic::weapon_bootstrap::legacy_weapon_definition);
            native
                .map(WeaponSlotDefinition::Authored)
                .or_else(|| prototype.map(WeaponSlotDefinition::Prototype))
        }

        let base_names = [
            self.primary_weapon_name.clone(),
            self.secondary_weapon_name.clone(),
            self.tertiary_weapon_name.clone(),
        ];
        let slots = std::array::from_fn(|i| bind(base_names[i].as_deref(), base[i].clone()));
        let base = WeaponRow::new(
            base_names,
            self.weapon_lock_shared_across_sets,
            Vec::new(),
            None,
            slots,
        );
        let mine_names = self.mine_weapon_set_names.clone().or_else(|| {
            (self.mine_clearing_primary_weapon_name.is_some()
                || self.mine_clearing_primary_weapon.is_some())
            .then(|| [self.mine_clearing_primary_weapon_name.clone(), None, None])
        });
        let mine = mine_names.map(|names| {
            let mut prototypes = [self.mine_clearing_primary_weapon.clone(), None, None];
            let slots = std::array::from_fn(|i| bind(names[i].as_deref(), prototypes[i].take()));
            WeaponRow::new(names, self.mine_weapon_set_shared, Vec::new(), None, slots)
        });
        self.selected_weapon_sets = Some(BoundWeaponSets::compatibility(base, mine));
    }

    pub(crate) fn has_receiving_weapon_sets(&self) -> bool {
        self.selected_weapon_sets
            .as_ref()
            .is_some_and(BoundWeaponSets::is_receiving)
    }
}

fn weapon_lock_shared(attributes: &std::collections::HashMap<String, String>) -> bool {
    attributes.iter().any(|(key, value)| {
        (key.eq_ignore_ascii_case("WeaponLockSharedAcrossSets")
            || key.eq_ignore_ascii_case("ShareWeaponLock"))
            && matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "yes" | "true" | "1"
            )
    })
}
impl Object {
    /// Native WeaponSetType bits: veterancy 0–2, player upgrade 3, crate
    /// upgrades 4–5, hijack 6, car bomb 7, mine clearing 8, riders 9–16.
    pub fn weapon_condition_flags(&self) -> u32 {
        self.weapon_condition_flags
    }

    /// Test a native WeaponSetType index.
    pub fn test_weapon_condition(&self, bit: u8) -> bool {
        bit < 17 && self.weapon_condition_flags & (1u32 << bit) != 0
    }

    /// Highest active salvage tier for legacy tier consumers.
    pub fn weapon_crate_upgrade(&self) -> u8 {
        if self.test_weapon_condition(5) { 2 }
        else if self.test_weapon_condition(4) { 1 }
        else { 0 }
    }

    /// Apply one native WeaponSetType mutation, including its immediate rebind.
    pub fn set_weapon_condition_at_frame(&mut self, bit: u8, enabled: bool, frame: u32) {
        if bit >= 17 { return; }
        let legacy_change = !self.thing.template.has_receiving_weapon_sets()
            && self.test_weapon_condition(bit) != enabled;
        let previous_sources = (legacy_change && matches!(bit, 3 | 6 | 7))
            .then(|| self.active_weapon_barrel_source_identities());
        let previous = self.selected_weapon_row_identity();
        if enabled { self.weapon_condition_flags |= 1u32 << bit; }
        else { self.weapon_condition_flags &= !(1u32 << bit); }
        let _ = self.rebind_receiving_weapon_row_if_changed(previous, frame);
        if legacy_change {
            if let Some(condition) = match bit {
                3 => Some("PLAYER_UPGRADE"),
                6 => Some("VEHICLE_HIJACK"),
                7 => Some("CARBOMB"),
                _ => None,
            } {
                if enabled {
                    self.adopt_weapon_set_lock_share_for_condition(condition);
                }
                self.release_weapon_lock_on_set_change();
            } else if bit <= 2 {
                self.release_weapon_lock_on_set_change();
            }
        }
        if let Some(sources) = previous_sources {
            self.reset_weapon_barrel_states_if_sources_changed(sources);
        }
        self.set_weapon_condition_model_bit(bit, enabled);
    }

    fn weapon_condition_model_bit(bit: u8) -> Option<usize> {
        let model = match bit {
            0 => "WEAPONSET_VETERAN",
            1 => "WEAPONSET_ELITE",
            2 => "WEAPONSET_HERO",
            3 => "WEAPONSET_PLAYER_UPGRADE",
            4 => "WEAPONSET_CRATEUPGRADE_ONE",
            5 => "WEAPONSET_CRATEUPGRADE_TWO",
            9 => "RIDER1", 10 => "RIDER2",
            11 => "RIDER3", 12 => "RIDER4",
            13 => "RIDER5", 14 => "RIDER6",
            15 => "RIDER7", 16 => "RIDER8",
            _ => return None,
        };
        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(model)
            .filter(|bit| *bit < 128)
    }

    fn set_weapon_condition_model_bit(&mut self, bit: u8, enabled: bool) {
        if let Some(model_bit) = Self::weapon_condition_model_bit(bit) {
            if enabled { self.model_condition_bits |= 1u128 << model_bit; }
            else { self.model_condition_bits &= !(1u128 << model_bit); }
        }
    }

    pub(crate) fn restore_weapon_condition_flags(&mut self, flags: u32) {
        self.weapon_condition_flags = flags;
    }

    fn selected_weapon_row_binding(&self) -> Option<(WeaponRowIdentity, &WeaponRow)> {
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        Some(rows.select(self.weapon_condition_flags))
    }

    fn selected_weapon_row(&self) -> Option<&WeaponRow> {
        self.selected_weapon_row_binding().map(|(_, row)| row)
    }

    pub(in crate::game_logic) fn selected_authored_weapon_row(&self) -> Option<&WeaponRow> {
        self.selected_weapon_row()
            .filter(|row| row.choice().is_some())
    }

    pub(in crate::game_logic) fn selected_weapon_choice(&self) -> Option<&WeaponChoicePolicy> {
        self.selected_authored_weapon_row()?.choice()
    }

    pub(crate) fn selected_weapon_row_identity(&self) -> Option<WeaponRowIdentity> {
        self.selected_weapon_row_binding()
            .map(|(identity, _)| identity)
    }

    pub(crate) fn selected_weapon_row_is_veterancy_specific(&self) -> bool {
        self.thing
            .template
            .selected_weapon_sets
            .as_ref()
            .is_some_and(|rows| rows.selects_veterancy_specific(self.weapon_condition_flags))
    }

    pub(crate) fn rebind_receiving_weapon_row_if_changed(
        &mut self,
        previous: Option<WeaponRowIdentity>,
        frame: u32,
    ) -> bool {
        let Some((identity, row)) = self.selected_weapon_row_binding() else {
            return false;
        };
        if previous == Some(identity) {
            return false;
        }
        let slots = row.instantiate(frame);
        if !row.shares_lock() {
            self.release_weapon_lock(WeaponLockType::LockedPermanently);
            self.set_active_weapon_slot(0);
        }
        let [primary, secondary, tertiary] = slots;
        self.tertiary_weapon = tertiary;
        self.secondary_weapon = secondary;
        self.weapon = primary;
        self.reset_weapon_barrel_states();
        true
    }
    /// Borrow the current slot's immutable rule for read-only queries.
    pub(in crate::game_logic) fn borrow_selected_native_weapon_rule(
        &self,
        slot: u8,
    ) -> Option<&generals_sim::weapon::WeaponDefinition> {
        self.selected_weapon_row()?.native_rule(slot)
    }

    pub(in crate::game_logic) fn selected_native_weapon_rule(
        &self,
        slot: u8,
    ) -> Option<std::sync::Arc<generals_sim::weapon::WeaponDefinition>> {
        self.selected_weapon_row()?
            .native_rule_handle(slot)
            .cloned()
    }

    pub(in crate::game_logic) fn selected_projectile_name(&self, slot: u8) -> Option<&str> {
        self.borrow_selected_native_weapon_rule(slot)
            .map(|definition| definition.projectile_name.as_str())
    }

    /// Authored WeaponSet row whose Conditions include PLAYER_UPGRADE (C++
    /// WeaponSetUpgrade → WEAPONSET_PLAYER_UPGRADE → updateWeaponSet); slot name.
    pub(in crate::game_logic) fn authored_player_upgrade_weapon_name(
        &self,
        slot: u8,
    ) -> Option<&str> {
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        rows.player_upgrade_row(self.weapon_condition_flags)?
            .name(slot)
    }

    /// Authored PRIMARY identities (base template + PLAYER_UPGRADE row);
    /// C++ upgrade modules key on authored data, not the template name.
    pub(in crate::game_logic) fn authored_primary_weapon_names(&self) -> [Option<&str>; 2] {
        [
            self.thing.template.primary_weapon_name.as_deref(),
            self.authored_player_upgrade_weapon_name(0),
        ]
    }
    pub(crate) fn selected_weapon_name(&self, slot: u8) -> Option<&str> {
        self.selected_weapon_row()?.name(slot)
    }
    /// Set or clear the authored RiderChange weapon condition.
    pub(crate) fn set_rider_weapon_condition_at_frame(&mut self, condition: Option<&str>, frame: u32) {
        if let Some(previous) = self.rider_change_weapon_set.take() {
            if let Some(mask) = weapon_condition_mask(&previous) {
                self.set_weapon_condition_at_frame(mask.trailing_zeros() as u8, false, frame);
            }
        }
        if let Some(condition) = condition {
            if let Some(mask) = weapon_condition_mask(condition) {
                self.set_weapon_condition_at_frame(mask.trailing_zeros() as u8, true, frame);
            }
            self.rider_change_weapon_set = Some(condition.to_owned());
        }
    }

    /// RiderChangeContain sets and clears the individual RiderN flag; during
    /// initial payload creation several flags can coexist.
    pub(crate) fn mutate_rider_weapon_condition_at_frame(
        &mut self, condition: &str, enabled: bool, frame: u32,
    ) {
        if let Some(mask) = weapon_condition_mask(condition) {
            self.set_weapon_condition_at_frame(mask.trailing_zeros() as u8, enabled, frame);
        }
        if enabled {
            self.rider_change_weapon_set = Some(condition.to_owned());
        } else if self.rider_change_weapon_set.as_deref()
            .is_some_and(|current| current.eq_ignore_ascii_case(condition))
        {
            self.rider_change_weapon_set = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, ObjectId, Team};

    fn conditional_row(name: &str, conditions: Vec<u32>) -> WeaponRow {
        WeaponRow::new(
            [Some(name.into()), None, None],
            false,
            conditions,
            None,
            [None, None, None],
        )
    }

    #[test]
    fn matching_keeps_base_at_rookie_and_first_exact_tie_then_prefers_more_yes() {
        let fallback = conditional_row("fallback", vec![]);
        let base = conditional_row("base", vec![0]);
        let veteran = conditional_row("veteran", vec![1]);
        let rows = BoundWeaponSets::receiving(fallback.clone(), None, vec![base, veteran]);
        assert_eq!(rows.select(0).1.name(0), Some("base"));

        let first = conditional_row("first", vec![1 | 2]);
        let tied_later = conditional_row("tied-later", vec![1 | 2]);
        let rows = BoundWeaponSets::receiving(fallback.clone(), None, vec![first, tied_later]);
        assert_eq!(rows.select(1).1.name(0), Some("first"));

        let one_yes = conditional_row("one-yes", vec![1]);
        let two_yes = conditional_row("two-yes", vec![1 | 2]);
        let rows = BoundWeaponSets::receiving(fallback, None, vec![one_yes, two_yes]);
        assert_eq!(rows.select(1 | 2).1.name(0), Some("two-yes"));
    }

    #[test]
    fn condition_mask_preserves_overlapping_native_flags() {
        let (mut world, id) = world(30.0);
        let object = world.host_object_mut(id).unwrap();
        for bit in [0, 3, 5, 6, 7, 8, 11] {
            object.set_weapon_condition_at_frame(bit, true, 0);
        }
        assert_eq!(object.weapon_condition_flags(), 0x9e9);
        object.set_rider_weapon_condition_at_frame(Some("PLAYER_UPGRADE"), 0);
        object.set_rider_weapon_condition_at_frame(None, 0);
        assert!(!object.test_weapon_condition(3));
        assert!(object.test_weapon_condition(6));
        assert!(object.test_weapon_condition(11));
    }

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
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "owned-mine-row.ini".to_owned(),
                    "Object ParsedMineRow\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY OwnedMineP\nWeapon = SECONDARY OwnedMineS\nWeapon = TERTIARY OwnedMineT\nEnd\nWeaponSet\nConditions = MINE_CLEARING_DETAIL\nWeapon = PRIMARY OwnedMineT\nWeapon = SECONDARY OwnedMineP\nWeapon = TERTIARY OwnedMineS\nWeaponLockSharedAcrossSets = Yes\nEnd\nEnd\n".to_owned(),
                )])
                .expect("paired Object and WeaponSet content"),
            )
            .expect("receiving module owner");
        let mut definitions = std::collections::HashMap::new();
        assert_eq!(
            crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                &mut definitions,
                &ini,
                crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
            ),
            3
        );
        world
            .admit_weapon_content_snapshot(
                definitions
                    .into_values()
                    .map(|rule| (rule.name.clone(), rule))
                    .collect(),
                &[],
            )
            .expect("receiving three-weapon catalog");
        let id = world
            .create_object("ParsedMineRow", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = world.host_object_mut(id).unwrap();
        for slot in 0..3 {
            let weapon = object.weapon_slot_mut(slot).unwrap();
            assert!(weapon.frame_clock_active);
            assert_eq!(weapon.next_fire_frame, 0);
            weapon.next_fire_frame = 900;
            weapon.reload_started_frame = 700;
            weapon.reloading_clip = true;
            weapon.frame_clock_status = super::super::WeaponFireStatus::ReloadingClip;
        }
        object.set_active_weapon_slot(2);
        assert!(object.set_weapon_lock(2, WeaponLockType::LockedPermanently));
        object.set_weapon_condition_at_frame(8, true, 80);
        assert_eq!(object.active_weapon_slot, 2);
        assert!(object.is_weapon_locked());
        for (slot, damage) in [(0, 50.0), (1, 30.0), (2, 40.0)] {
            let weapon = object.weapon_slot(slot).unwrap();
            assert_eq!(weapon.damage, damage);
            assert_eq!(weapon.ammo, Some(3));
            assert_eq!(weapon.suspend_fx_frame, 110);
            assert!(weapon.frame_clock_active);
            assert_eq!(weapon.next_fire_frame, 80);
            assert_eq!(weapon.reload_started_frame, 80);
            assert!(!weapon.reloading_clip);
            assert_eq!(weapon.frame_clock_status, super::super::WeaponFireStatus::ReadyToFire);
        }
        object.set_weapon_condition_at_frame(8, false, 81);
        assert_eq!(object.active_weapon_slot, 0);
        assert!(!object.is_weapon_locked());
        assert_eq!(object.weapon_slot(0).unwrap().damage, 30.0);
        for slot in 0..3 {
            let weapon = object.weapon_slot(slot).unwrap();
            assert_eq!(weapon.next_fire_frame, 81);
            assert_eq!(weapon.reload_started_frame, 81);
        }
    }

    #[test]
    fn rider_clears_shared_mine_flag_and_rebinds_only_when_row_changes() {
        let (mut owner, id) = world(90.0);
        let object = owner.host_object_mut(id).unwrap();
        object.set_weapon_condition_at_frame(8, true, 30);
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.set_rider_weapon_condition_at_frame(Some("MINE_CLEARING_DETAIL"), 31);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        object.set_rider_weapon_condition_at_frame(None, 32);
        assert!(!object.test_weapon_condition(8));
        assert_eq!(object.weapon.as_ref().unwrap().damage, 10.0);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(5));
    }
    #[test]
    fn receiving_selected_set_recreates_three_slots_once_without_peer_cache() {
        let (mut owner, id) = world(90.0);
        let (mut peer, peer_id) = world(180.0);
        assert_eq!(id, peer_id);
        let object = owner.host_object_mut(id).unwrap();
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.weapon.as_mut().unwrap().last_fire_time = 25.0;
        object.weapon.as_mut().unwrap().last_fire_frame = 29;
        object.set_active_weapon_slot(2);
        object.set_weapon_lock(2, WeaponLockType::LockedPermanently);
        object.set_weapon_condition_at_frame(8, true, 30);
        assert_eq!(object.weapon.as_ref().unwrap().damage, 90.0);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(2));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_time, 0.0);
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_frame, 0);
        assert_eq!(object.active_weapon_slot, 0);
        assert!(!object.is_weapon_locked());
        assert!(object.secondary_weapon.is_none() && object.tertiary_weapon.is_none());
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.weapon.as_mut().unwrap().last_fire_time = 1.0;
        object.weapon.as_mut().unwrap().last_fire_frame = 30;
        object.set_weapon_condition_at_frame(8, true, 31);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_time, 1.0);
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_frame, 30);
        peer.host_object_mut(peer_id)
            .unwrap().set_weapon_condition_at_frame(8, true, 700);
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
            .unwrap().set_weapon_condition_at_frame(8, false, 32);
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
        object.set_weapon_condition_at_frame(8, true, 37);
        object.weapon.as_mut().unwrap().ammo = Some(1);
        object.weapon.as_mut().unwrap().last_fire_time = 1.2;
        object.weapon.as_mut().unwrap().suspend_fx_frame = 49;
        object.weapon.as_mut().unwrap().last_fire_frame = 36;
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&original).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _) = world(90.0);
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        let object = restored.host_object_mut(id).unwrap();
        assert!(object.test_weapon_condition(8));
        assert!(object.secondary_weapon.is_none() && object.tertiary_weapon.is_none());
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_time, 1.2);
        assert_eq!(object.weapon.as_ref().unwrap().suspend_fx_frame, 49);
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_frame, 36);
        object.set_weapon_condition_at_frame(8, true, 38);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(1));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_frame, 36);
        object.set_weapon_condition_at_frame(8, false, 39);
        assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(5));
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_frame, 0);
    }
    #[test]
    fn disk_save_restores_independent_weapon_shot_frames() {
        use crate::game_logic::Player;
        use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
        use std::time::{Duration, UNIX_EPOCH};

        let mut world = GameLogic::new();
        let content = crate::assets::ReceivingObjectContent::from_sources([(
            "weapon-frames.ini".into(),
            "Object ShotFrameOwner\nKindOf = VEHICLE CAN_ATTACK\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY ShotFramePrimary\nWeapon = SECONDARY ShotFrameSecondary\nEnd\nEnd\n".into(),
        )]).expect("ordered slot definitions");
        let mut definitions = std::collections::HashMap::new();
        assert_eq!(
            crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                &mut definitions,
                "Weapon ShotFramePrimary\nPrimaryDamage = 10\nAttackRange = 100\nClipSize = 5\nEnd\nWeapon ShotFrameSecondary\nPrimaryDamage = 20\nAttackRange = 100\nClipSize = 5\nEnd\n",
                crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
            ),
            2
        );
        world.admit_receiving_module_content_with_weapons(
            content,
            Some(generals_sim::weapon::WeaponCatalog::from_named_definitions(definitions).unwrap()),
        ).expect("receiving weapon slots");
        world.add_player(Player::new(0, Team::USA, "Weapon owner", true));
        let id = world.create_object("ShotFrameOwner", Team::USA, glam::Vec3::ZERO).unwrap();
        world.set_current_frame(37);
        {
            // Both slots are real receiving Weapons; only their mutable shot
            // history differs, and NewGame must restore it independently.
            let object = world.host_object_mut(id).unwrap();
            object.weapon.as_mut().unwrap().last_fire_frame = 35;
            object.secondary_weapon.as_mut().unwrap().last_fire_frame = 36;
        }
        let directory = tempfile::TempDir::new().unwrap();
        let mut manager = SaveFileManager::with_save_directory(directory.path());
        manager.init().unwrap();
        let info = SaveGameInfo {
            pristine_map_name: None,
            filename: "weapon_frames".into(),
            display_name: "Weapon frames".into(),
            description: "Per-slot shot timing".into(),
            map_name: "WeaponFramesMap".into(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH,
            game_version: env!("CARGO_PKG_VERSION").into(),
            play_time: Duration::ZERO,
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        manager.save_game("weapon_frames", &world, &info).unwrap();
        world.host_object_mut(id).unwrap().weapon.as_mut().unwrap().last_fire_frame = 99;
        manager.load_game("weapon_frames", &mut world).unwrap();
        let object = world.host_object(id).unwrap();
        assert_eq!(object.weapon.as_ref().unwrap().last_fire_frame, 35);
        assert_eq!(object.secondary_weapon.as_ref().unwrap().last_fire_frame, 36);
        assert!(object.weapon.as_ref().unwrap().frame_clock_active);
        assert!(object.secondary_weapon.as_ref().unwrap().frame_clock_active);
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
        let mut native = generals_sim::weapon::WeaponDefinition::new("ReceivingInstant".into());
        native.primary_damage = 17.0;
        native.attack_range = 100.0;
        native.projectile_name = String::new();
        template.bind_native_weapon_row([Some(&native), None, None]);
        let rows = template.selected_weapon_sets.as_ref().unwrap();
        assert_eq!(
            rows.base()
                .native_rule(0)
                .map(|rule| rule.projectile_name.as_str()),
            Some("")
        );
        assert_eq!(
            rows.mine().unwrap().instantiate(0)[0]
                .as_ref()
                .unwrap()
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
