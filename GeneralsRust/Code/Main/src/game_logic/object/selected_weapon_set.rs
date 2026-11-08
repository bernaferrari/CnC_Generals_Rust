//! CPP WeaponSet::updateWeaponSet: immutable receiving rows, exactly three live slots.
use super::Object;
use crate::game_logic::{ThingTemplate, Weapon, WeaponLockType};

#[derive(Debug, Clone)]
pub(crate) struct WeaponDefinition {
    prototype: Weapon,
    suspend_delay: u32,
    projectile_name: Option<String>,
    native: Option<std::sync::Arc<gamelogic::weapon::WeaponTemplate>>,
}
impl WeaponDefinition {
    fn instantiate(&self, frame: u32) -> Weapon {
        let mut weapon = self.prototype.clone();
        weapon.last_fire_time = 0.0;
        weapon.last_fire_frame = 0;
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
    condition_alternatives: Vec<u32>,
}
#[derive(Debug, Clone)]
pub(crate) struct BoundWeaponSets {
    base: WeaponRow,
    mine: Option<WeaponRow>,
    conditional: Vec<WeaponRow>,
    receiving: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeaponRowIdentity {
    Base,
    Mine,
    Conditional(usize),
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
                    native: Some(std::sync::Arc::new(rule.clone())),
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
                condition_alternatives: Vec::new(),
            },
            mine,
            conditional: Vec::new(),
            receiving: true,
        });
    }
    /// Resolve both active three-slot modes from this match's effective content.
    pub(crate) fn bind_receiving_weapon_sets(
        &mut self,
        catalog: &std::collections::HashMap<
            String,
            std::sync::Arc<gamelogic::weapon::WeaponTemplate>,
        >,
    ) {
        fn row(
            names: [Option<String>; 3],
            shared: bool,
            condition_alternatives: Vec<u32>,
            catalog: &std::collections::HashMap<
                String,
                std::sync::Arc<gamelogic::weapon::WeaponTemplate>,
            >,
        ) -> WeaponRow {
            let slots = std::array::from_fn(|i| {
                let native = catalog
                    .get(&names[i].as_ref()?.to_ascii_lowercase())?
                    .clone();
                Some(WeaponDefinition {
                    prototype: ThingTemplate::weapon_from_template_at(&native, 0)?,
                    suspend_delay: native.suspend_fx_delay,
                    projectile_name: Some(native.projectile_name.clone()),
                    native: Some(native),
                })
            });
            WeaponRow {
                slots,
                names,
                shared,
                condition_alternatives,
            }
        }
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
            .map(|names| row(names, self.mine_weapon_set_shared, Vec::new(), catalog));
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
                        row(
                            [
                                set.primary_weapon.clone(),
                                set.secondary_weapon.clone(),
                                set.tertiary_weapon.clone(),
                            ],
                            weapon_lock_shared(&set.attributes),
                            alternatives,
                            catalog,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.selected_weapon_sets = Some(BoundWeaponSets {
            base: row(
                names,
                self.weapon_lock_shared_across_sets,
                Vec::new(),
                catalog,
            ),
            mine,
            conditional,
            receiving: true,
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
                    native: Some(native.clone()),
                })
            } else {
                prototype.map(|prototype| WeaponDefinition {
                    prototype,
                    suspend_delay: 0,
                    projectile_name: None,
                    native: None,
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
            condition_alternatives: Vec::new(),
        });
        self.selected_weapon_sets = Some(BoundWeaponSets {
            base: WeaponRow {
                slots,
                names: base_names,
                shared: self.weapon_lock_shared_across_sets,
                condition_alternatives: Vec::new(),
            },
            mine,
            conditional: Vec::new(),
            receiving: false,
        });
    }
    pub(crate) fn has_receiving_weapon_sets(&self) -> bool {
        self.selected_weapon_sets
            .as_ref()
            .is_some_and(|rows| rows.receiving)
    }
}

fn weapon_condition_mask(condition: &str) -> Option<u32> {
    let condition = condition.trim().trim_matches(',').to_ascii_uppercase();
    let condition = condition.strip_prefix("WEAPONSET_").unwrap_or(&condition);
    let bit = match condition {
        "VETERAN" => 0,
        "ELITE" => 1,
        "HERO" => 2,
        "PLAYER_UPGRADE" => 3,
        "CRATEUPGRADE_ONE" | "CRATE_UPGRADE_ONE" => 4,
        "CRATEUPGRADE_TWO" | "CRATE_UPGRADE_TWO" => 5,
        "VEHICLE_HIJACK" => 6,
        "CARBOMB" | "CAR_BOMB" => 7,
        "MINE_CLEARING_DETAIL" => 8,
        "WEAPON_RIDER1" | "RIDER1" => 9,
        "WEAPON_RIDER2" | "RIDER2" => 10,
        "WEAPON_RIDER3" | "RIDER3" => 11,
        "WEAPON_RIDER4" | "RIDER4" => 12,
        "WEAPON_RIDER5" | "RIDER5" => 13,
        "WEAPON_RIDER6" | "RIDER6" => 14,
        "WEAPON_RIDER7" | "RIDER7" => 15,
        "WEAPON_RIDER8" | "RIDER8" => 16,
        "NONE" | "" => return None,
        _ => return None,
    };
    Some(1u32 << bit)
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
fn select_conditional_weapon_row(rows: &[WeaponRow], flags: u32) -> Option<(usize, &WeaponRow)> {
    let mut best: Option<(usize, &WeaponRow)> = None;
    let mut best_yes = 0;
    let mut best_extra = 999;
    for (index, row) in rows.iter().enumerate() {
        // WeaponTemplateSet currently exposes one mask. Keep C++'s reverse
        // alternative iteration so multiple authored masks keep source order.
        for &condition in row.condition_alternatives.iter().rev() {
            let yes = (condition & flags).count_ones();
            let extraneous = (condition & !flags).count_ones();
            if yes > best_yes || (yes >= best_yes && extraneous < best_extra) {
                best = Some((index, row));
                best_yes = yes;
                best_extra = extraneous;
            }
        }
    }
    best
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

    pub(crate) fn restore_weapon_condition_flags(
        &mut self, flags: u32, saved_model_bits: u128, rider_model_mask: u128,
    ) {
        self.weapon_condition_flags = flags;
        let mut mask = rider_model_mask;
        for bit in 0..17 {
            if let Some(model_bit) = Self::weapon_condition_model_bit(bit) {
                mask |= 1u128 << model_bit;
            }
        }
        self.model_condition_bits = (self.model_condition_bits & !mask) | (saved_model_bits & mask);
    }


    fn active_weapon_condition_mask(&self) -> u32 {
        self.weapon_condition_flags
    }

    fn selected_weapon_row_binding(&self) -> Option<(WeaponRowIdentity, &WeaponRow)> {
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        if let Some((index, row)) =
            select_conditional_weapon_row(&rows.conditional, self.active_weapon_condition_mask())
        {
            return Some((WeaponRowIdentity::Conditional(index), row));
        }
        if self.test_weapon_condition(8) {
            if let Some(row) = rows.mine.as_ref() {
                return Some((WeaponRowIdentity::Mine, row));
            }
        }
        Some((WeaponRowIdentity::Base, &rows.base))
    }

    fn selected_weapon_row(&self) -> Option<&WeaponRow> {
        self.selected_weapon_row_binding().map(|(_, row)| row)
    }

    pub(crate) fn selected_weapon_row_identity(&self) -> Option<WeaponRowIdentity> {
        self.selected_weapon_row_binding()
            .map(|(identity, _)| identity)
    }

    pub(crate) fn selected_weapon_row_is_veterancy_specific(&self) -> bool {
        self.selected_weapon_row_binding()
            .is_some_and(|(identity, row)| {
                matches!(identity, WeaponRowIdentity::Conditional(_))
                    && row
                        .condition_alternatives
                        .iter()
                        .any(|conditions| conditions & 0b111 != 0)
            })
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
        let slots: [Option<Weapon>; 3] = std::array::from_fn(|i| {
            row.slots[i]
                .as_ref()
                .map(|definition| definition.instantiate(frame))
        });
        if !row.shared {
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
    pub(in crate::game_logic) fn selected_native_weapon_rule(
        &self,
        slot: u8,
    ) -> Option<std::sync::Arc<gamelogic::weapon::WeaponTemplate>> {
        let row = self.selected_weapon_row()?;
        row.slots.get(usize::from(slot))?.as_ref()?.native.clone()
    }
    pub(in crate::game_logic) fn selected_projectile_name(&self, slot: u8) -> Option<&str> {
        let row = self.selected_weapon_row()?;
        row.slots
            .get(usize::from(slot))?
            .as_ref()?
            .projectile_name
            .as_deref()
    }
    /// Authored WeaponSet row whose Conditions include PLAYER_UPGRADE (C++
    /// WeaponSetUpgrade → WEAPONSET_PLAYER_UPGRADE → updateWeaponSet); slot name.
    pub(in crate::game_logic) fn authored_player_upgrade_weapon_name(
        &self,
        slot: u8,
    ) -> Option<&str> {
        const PLAYER_UPGRADE: u32 = 1 << 3;
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        let (_, row) = select_conditional_weapon_row(
            &rows.conditional,
            self.active_weapon_condition_mask() | PLAYER_UPGRADE,
        )?;
        if !row
            .condition_alternatives
            .iter()
            .any(|mask| mask & PLAYER_UPGRADE != 0)
        {
            return None;
        }
        row.names.get(slot as usize)?.as_deref()
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
        let row = self.selected_weapon_row()?;
        row.names.get(slot as usize)?.as_deref()
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
        WeaponRow {
            slots: std::array::from_fn(|_| None),
            names: [Some(name.into()), None, None],
            shared: false,
            condition_alternatives: conditions,
        }
    }

    #[test]
    fn matching_keeps_base_at_rookie_and_first_exact_tie_then_prefers_more_yes() {
        let base = conditional_row("base", vec![0]);
        let veteran = conditional_row("veteran", vec![1]);
        let rows = [base, veteran];
        let selected = select_conditional_weapon_row(&rows, 0);
        assert_eq!(selected.unwrap().1.names[0].as_deref(), Some("base"));

        let first = conditional_row("first", vec![1 | 2]);
        let tied_later = conditional_row("tied-later", vec![1 | 2]);
        let rows = [first, tied_later];
        let selected = select_conditional_weapon_row(&rows, 1);
        assert_eq!(selected.unwrap().1.names[0].as_deref(), Some("first"));

        let one_yes = conditional_row("one-yes", vec![1]);
        let two_yes = conditional_row("two-yes", vec![1 | 2]);
        let rows = [one_yes, two_yes];
        let selected = select_conditional_weapon_row(&rows, 1 | 2);
        assert_eq!(selected.unwrap().1.names[0].as_deref(), Some("two-yes"));
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
        }
        object.set_weapon_condition_at_frame(8, false, 81);
        assert_eq!(object.active_weapon_slot, 0);
        assert!(!object.is_weapon_locked());
        assert_eq!(object.weapon_slot(0).unwrap().damage, 30.0);
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

        let (mut world, id) = world(90.0);
        world.add_player(Player::new(0, Team::USA, "Weapon owner", true));
        {
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
