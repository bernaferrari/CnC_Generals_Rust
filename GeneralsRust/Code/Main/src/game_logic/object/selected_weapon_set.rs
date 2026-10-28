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
    fn active_weapon_condition_mask(&self) -> u32 {
        // Rank and mine rows are the only conditional transitions which
        // currently rebind these receiving Weapon instances. Leave unrelated
        // runtime flags out until their setters share that owner path.
        u32::from(self.weapon_set_veteran)
            | (u32::from(self.weapon_set_elite) << 1)
            | (u32::from(self.weapon_set_hero) << 2)
            | (u32::from(self.weapon_set_mine_clearing_detail) << 8)
    }

    fn selected_weapon_row_binding(&self) -> Option<(WeaponRowIdentity, &WeaponRow)> {
        let rows = self.thing.template.selected_weapon_sets.as_ref()?;
        if let Some((index, row)) =
            select_conditional_weapon_row(&rows.conditional, self.active_weapon_condition_mask())
        {
            return Some((WeaponRowIdentity::Conditional(index), row));
        }
        if self.weapon_set_mine_clearing_detail {
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
    pub(crate) fn selected_weapon_name(&self, slot: u8) -> Option<&str> {
        let row = self.selected_weapon_row()?;
        row.names.get(slot as usize)?.as_deref()
    }
    pub fn set_weapon_set_mine_clearing_detail_at_frame(&mut self, enabled: bool, frame: u32) {
        if enabled == self.weapon_set_mine_clearing_detail {
            return;
        }
        let Some(_) = self.thing.template.selected_weapon_sets.as_ref() else {
            // A plain inert object has no conditional row to replace.
            self.weapon_set_mine_clearing_detail = enabled;
            return;
        };
        let previous = self.selected_weapon_row_identity();
        self.weapon_set_mine_clearing_detail = enabled;
        let _ = self.rebind_receiving_weapon_row_if_changed(previous, frame);
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
        let selected = select_conditional_weapon_row(&[base, veteran], 0);
        assert_eq!(selected.unwrap().1.names[0].as_deref(), Some("base"));

        let first = conditional_row("first", vec![1 | 2]);
        let tied_later = conditional_row("tied-later", vec![1 | 2]);
        let selected = select_conditional_weapon_row(&[first, tied_later], 1);
        assert_eq!(selected.unwrap().1.names[0].as_deref(), Some("first"));

        let one_yes = conditional_row("one-yes", vec![1]);
        let two_yes = conditional_row("two-yes", vec![1 | 2]);
        let selected = select_conditional_weapon_row(&[one_yes, two_yes], 1 | 2);
        assert_eq!(selected.unwrap().1.names[0].as_deref(), Some("two-yes"));
    }

    #[test]
    fn active_mask_only_includes_flags_with_receiving_rebind_transitions() {
        let (mut world, id) = world(30.0);
        let object = world.host_object_mut(id).unwrap();
        object.weapon_set_veteran = true;
        object.weapon_set_mine_clearing_detail = true;
        object.weapon_crate_upgrade = 2;
        object.weapon_set_player_upgrade = true;
        object.weapon_set_carbomb = true;
        object.weapon_set_vehicle_hijack = true;
        object.rider_change_active_slot = Some(3);
        object.armed_riders_upgrade_weapon_set = true;
        let mask = object.active_weapon_condition_mask();
        assert_ne!(mask & 1, 0);
        assert_ne!(mask & (1 << 8), 0);
        for unsupported in [3, 4, 5, 6, 7, 11] {
            assert_eq!(mask & (1 << unsupported), 0);
        }
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
