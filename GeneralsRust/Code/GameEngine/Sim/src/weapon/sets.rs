use std::sync::Arc;

use super::{Weapon, WeaponCatalog, WeaponDefinition, WeaponSetType, WeaponSlotType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponChoicePolicy {
    auto_choose_masks: [u32; 3],
    preferred_against_masks: [u128; 3],
}

impl Default for WeaponChoicePolicy {
    fn default() -> Self {
        // WeaponTemplateSet::clear (WeaponSet.cpp:78-87).
        Self {
            auto_choose_masks: [u32::MAX; 3],
            preferred_against_masks: [0; 3],
        }
    }
}

impl WeaponChoicePolicy {
    pub fn allows_automatic_choice(&self, slot: u8, command_source: u32) -> bool {
        // WeaponSet.cpp:815-823. DEFAULT_SWITCH_WEAPON admits every source.
        use game_engine::common::game_common::CommandSourceType;

        let source = 1u32.checked_shl(command_source).unwrap_or(0);
        let default_switch = 1 << CommandSourceType::DefaultSwitchWeapon as u32;
        self.auto_choose_masks
            .get(usize::from(slot))
            .is_some_and(|mask| mask & (source | default_switch) != 0)
    }

    pub fn prefers(&self, slot: u8, victim_kinds: u128) -> bool {
        // WeaponSet.cpp:870 and BitFlags.h:223-237 require every bit.
        self.preferred_against_masks
            .get(usize::from(slot))
            .is_some_and(|&required| required != 0 && victim_kinds & required == required)
    }

    /// Apply a declaration in source order, retaining the original +/- rules.
    pub fn apply_ini_flags(&mut self, slot: WeaponSlotType, flags: &str, auto_choose: bool) {
        // Each declaration updates this slot in source order, including +/-.
        if auto_choose {
            const SOURCE_NAMES: &[&str] = &[
                "FROM_PLAYER",
                "FROM_SCRIPT",
                "FROM_AI",
                "FROM_DOZER",
                "DEFAULT_SWITCH_WEAPON",
            ];
            let mask = &mut self.auto_choose_masks[slot as usize];
            *mask = generals_game_domain::ini::parse_bit_string_32_with_initial(
                flags,
                SOURCE_NAMES,
                *mask,
            )
            .unwrap_or(0);
        } else {
            use game_engine::common::system::kind_of::KindOfMask;
            let mask = &mut self.preferred_against_masks[slot as usize];
            *mask = KindOfMask::parse_ini(KindOfMask::from_bits_truncate(*mask), flags)
                .map(|kind| kind.bits())
                .unwrap_or(0);
        }
    }
}

/// The immutable source for one of the three live weapon slots.
#[derive(Debug, Clone)]
pub enum WeaponSlotDefinition {
    Authored(Arc<WeaponDefinition>),
    Prototype(Weapon),
}

impl WeaponSlotDefinition {
    fn instantiate(&self, frame: u32) -> Weapon {
        match self {
            Self::Authored(definition) => {
                let mut weapon = Weapon::from_definition_at(definition, frame);
                weapon.frame_clock_active = true;
                weapon.next_fire_frame = frame;
                weapon.reload_started_frame = frame;
                weapon
            }
            Self::Prototype(prototype) => {
                let mut weapon = prototype.clone();
                weapon.last_fire_time = 0.0;
                weapon.last_fire_frame = 0;
                weapon.frame_clock_active = false;
                weapon.next_fire_frame = 0;
                weapon.reload_started_frame = 0;
                weapon.frame_clock_status = Default::default();
                weapon.reloading_clip = false;
                weapon.last_bonus_rof = 0.0;
                weapon.ammo = (weapon.clip_size > 0).then_some(weapon.clip_size);
                weapon.suspend_fx_frame = frame;
                weapon
            }
        }
    }
}

/// An admitted C++ WeaponTemplateSet row. Names are retained for missing rules.
#[derive(Debug, Clone)]
pub struct WeaponRow {
    slots: [Option<WeaponSlotDefinition>; 3],
    names: [Option<String>; 3],
    shared: bool,
    condition_alternatives: Vec<u32>,
    choice: Option<WeaponChoicePolicy>,
}

impl WeaponRow {
    pub fn new(
        names: [Option<String>; 3],
        shared: bool,
        condition_alternatives: Vec<u32>,
        choice: Option<WeaponChoicePolicy>,
        slots: [Option<WeaponSlotDefinition>; 3],
    ) -> Self {
        Self {
            names,
            shared,
            condition_alternatives,
            choice,
            slots,
        }
    }

    pub fn from_catalog(
        names: [Option<String>; 3],
        shared: bool,
        condition_alternatives: Vec<u32>,
        choice: Option<WeaponChoicePolicy>,
        catalog: &WeaponCatalog,
    ) -> Self {
        let slots = std::array::from_fn(|i| {
            let name = names[i].as_ref()?.to_ascii_lowercase();
            let definition = catalog.get(&name)?;
            Some(WeaponSlotDefinition::Authored(Arc::clone(definition)))
        });
        Self::new(names, shared, condition_alternatives, choice, slots)
    }

    pub fn instantiate(&self, frame: u32) -> [Option<Weapon>; 3] {
        self.slots
            .each_ref()
            .map(|slot| slot.as_ref().map(|rule| rule.instantiate(frame)))
    }

    pub fn native_rule(&self, slot: u8) -> Option<&WeaponDefinition> {
        self.native_rule_handle(slot).map(Arc::as_ref)
    }

    pub fn native_rule_handle(&self, slot: u8) -> Option<&Arc<WeaponDefinition>> {
        match self.slots.get(usize::from(slot))?.as_ref()? {
            WeaponSlotDefinition::Authored(rule) => Some(rule),
            WeaponSlotDefinition::Prototype(_) => None,
        }
    }

    pub fn name(&self, slot: u8) -> Option<&str> {
        self.names.get(usize::from(slot))?.as_deref()
    }

    pub fn shares_lock(&self) -> bool {
        self.shared
    }

    pub fn choice(&self) -> Option<&WeaponChoicePolicy> {
        self.choice.as_ref()
    }
}

/// All weapon rows admitted for one object template.
#[derive(Debug, Clone)]
pub struct BoundWeaponSets {
    base: WeaponRow,
    mine: Option<WeaponRow>,
    conditional: Vec<WeaponRow>,
    receiving: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponRowIdentity {
    Base,
    Mine,
    Conditional(usize),
}

impl BoundWeaponSets {
    pub fn receiving(
        base: WeaponRow,
        mine: Option<WeaponRow>,
        conditional: Vec<WeaponRow>,
    ) -> Self {
        Self {
            base,
            mine,
            conditional,
            receiving: true,
        }
    }

    pub fn compatibility(base: WeaponRow, mine: Option<WeaponRow>) -> Self {
        Self {
            base,
            mine,
            conditional: Vec::new(),
            receiving: false,
        }
    }

    pub fn base(&self) -> &WeaponRow {
        &self.base
    }

    pub fn mine(&self) -> Option<&WeaponRow> {
        self.mine.as_ref()
    }

    pub fn is_receiving(&self) -> bool {
        self.receiving
    }

    pub fn select(&self, flags: u32) -> (WeaponRowIdentity, &WeaponRow) {
        if let Some((index, row)) = select_conditional_weapon_row(&self.conditional, flags) {
            return (WeaponRowIdentity::Conditional(index), row);
        }
        if flags & (1 << WeaponSetType::MineClearingDetail as u8) != 0 {
            if let Some(row) = self.mine() {
                return (WeaponRowIdentity::Mine, row);
            }
        }
        (WeaponRowIdentity::Base, &self.base)
    }

    pub fn selects_veterancy_specific(&self, flags: u32) -> bool {
        let (identity, row) = self.select(flags);
        matches!(identity, WeaponRowIdentity::Conditional(_))
            && row
                .condition_alternatives
                .iter()
                .any(|conditions| conditions & 0b111 != 0)
    }

    pub fn player_upgrade_row(&self, flags: u32) -> Option<&WeaponRow> {
        let upgrade = 1 << WeaponSetType::PlayerUpgrade as u8;
        let (_, row) = select_conditional_weapon_row(&self.conditional, flags | upgrade)?;
        row.condition_alternatives
            .iter()
            .any(|mask| mask & upgrade != 0)
            .then_some(row)
    }
}

pub fn weapon_condition_mask(condition: &str) -> Option<u32> {
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
