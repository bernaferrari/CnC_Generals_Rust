// Live-host getCommandAvailability residual when OBJECT_REGISTRY is empty.
// C++ ControlBarCommand.cpp:1032-1514.

/// C++ Weapon::getStatus READY_TO_FIRE / RELOADING_CLIP ordinals.
const WEAPON_FIRE_READY: u8 = 0;
const WEAPON_FIRE_RELOADING_CLIP: u8 = 3;

/// Presentation-stamped facts leftover ControlBar uses instead of dual-world Object.
#[derive(Debug, Clone, Default)]
pub struct PresentationAvailabilityResidual {
    pub single_use_used: bool,
    pub moving: bool,
    pub completed_upgrades: Vec<String>,
    pub has_production: bool,
    pub occupant_count: usize,
    pub hacking_packing_or_unpacking: bool,
    pub dock_open: bool,
    pub battle_plan_bombardment: bool,
    pub battle_plan_hold_the_line: bool,
    pub battle_plan_search_and_destroy: bool,
    pub overcharge_active: bool,
    pub special_power_in_use: bool,
    pub weapon_reload_time: f32,
    pub weapon_fire_status: u8,
    pub has_primary_weapon: bool,
    pub has_secondary_weapon: bool,
    pub has_tertiary_weapon: bool,
    pub active_weapon_slot: u8,
    pub mine_clearing_weaponset: bool,
    /// C++ OBJECT_STATUS_SCRIPT_DISABLED — entire command set Hidden.
    pub script_disabled: bool,
    /// C++ OBJECT_STATUS_SCRIPT_UNPOWERED — entire command set Hidden.
    pub script_unpowered: bool,
    /// C++ DISABLED_UNMANNED — entire command set Hidden.
    pub unmanned: bool,
    /// C++ OBJECT_STATUS_SCRIPT_UNSELLABLE — hide Sell.
    pub script_unsellable: bool,
    /// C++ DISABLED_SUBDUED — restrict Sell / Evacuate / Exit.
    pub disabled_subdued: bool,
    /// Object-level applied upgrades (OBJECT_UPGRADE hasUpgrade).
    pub object_applied_upgrades: Vec<String>,
    /// Player-level completed upgrades (PLAYER_UPGRADE hasUpgradeComplete).
    pub player_completed_upgrades: Vec<String>,
    /// OBJECT_UPGRADE names this object is not affected by.
    pub object_unaffected_upgrades: Vec<String>,
}

impl ControlBar {
    pub fn apply_presentation_availability(&mut self, residual: PresentationAvailabilityResidual) {
        self.presentation_availability = residual;
    }

    pub fn presentation_availability(&self) -> &PresentationAvailabilityResidual {
        &self.presentation_availability
    }
}

fn leftover_presentation_has_upgrade(
    residual: &PresentationAvailabilityResidual,
    upgrade: &str,
) -> bool {
    residual
        .completed_upgrades
        .iter()
        .any(|owned| owned.eq_ignore_ascii_case(upgrade))
}

fn leftover_presentation_has_weapon_slot(
    residual: &PresentationAvailabilityResidual,
    slot: WeaponSlotType,
) -> bool {
    match slot {
        WeaponSlotType::Primary => residual.has_primary_weapon,
        WeaponSlotType::Secondary => residual.has_secondary_weapon,
        WeaponSlotType::Tertiary => residual.has_tertiary_weapon,
    }
}

fn leftover_presentation_slot_index(slot: WeaponSlotType) -> u8 {
    match slot {
        WeaponSlotType::Primary => 0,
        WeaponSlotType::Secondary => 1,
        WeaponSlotType::Tertiary => 2,
    }
}

/// C++ ControlBarCommand.cpp:1032-1106, 1361-1372, 1474-1486.
fn leftover_presentation_common_restricted(
    bar: &ControlBar,
    command: &CommandButton,
    entry: Option<&crate::presentation_translator_residual::TranslatorCatalogEntry>,
) -> bool {
    let residual = &bar.presentation_availability;
    if residual.single_use_used {
        return true;
    }
    if (command.options & CommandOption::MustBeStopped as u32) != 0 && residual.moving {
        return true;
    }
    if (command.options & CommandOption::NeedUpgrade as u32) != 0 && !command.upgrade.is_empty() {
        if !leftover_presentation_has_upgrade(residual, &command.upgrade) {
            return true;
        }
    }
    if (command.options & CommandOption::NotQueueable as u32) != 0 && residual.has_production {
        return true;
    }
    if command.command_type == CommandType::Evacuate {
        let count = entry
            .map(|e| e.occupant_count as usize)
            .unwrap_or(residual.occupant_count);
        if count == 0 {
            return true;
        }
    }
    if command.command_type == CommandType::InternetHack && residual.hacking_packing_or_unpacking {
        return true;
    }
    false
}

/// C++ ControlBarCommand.cpp:1375-1383, 1488-1503.
fn leftover_presentation_stop_or_rail(
    bar: &ControlBar,
    command: &CommandButton,
) -> Option<CommandAvailability> {
    let residual = &bar.presentation_availability;
    match command.command_type {
        CommandType::DoStop if (command.options & CommandOption::OptionOne as u32) != 0 => {
            let non_bombardment = residual.battle_plan_hold_the_line
                || residual.battle_plan_search_and_destroy;
            if non_bombardment && !residual.battle_plan_bombardment {
                Some(CommandAvailability::Restricted)
            } else {
                Some(CommandAvailability::Available)
            }
        }
        CommandType::ExecuteRailedTransport => {
            if residual.dock_open {
                Some(CommandAvailability::Available)
            } else {
                Some(CommandAvailability::Restricted)
            }
        }
        _ => None,
    }
}

/// C++ ControlBarCommand.cpp:1184-1196, 1361-1372 — SCRIPT_UNSELLABLE / SUBDUED.
fn leftover_presentation_sell_or_subdued(
    bar: &ControlBar,
    command: &CommandButton,
) -> Option<CommandAvailability> {
    let residual = &bar.presentation_availability;
    if command.command_type == CommandType::Sell && residual.script_unsellable {
        return Some(CommandAvailability::Hidden);
    }
    if residual.disabled_subdued
        && matches!(
            command.command_type,
            CommandType::Sell | CommandType::Evacuate | CommandType::Exit
        )
    {
        return Some(CommandAvailability::Restricted);
    }
    None
}

fn leftover_presentation_fire_weapon(
    residual: &PresentationAvailabilityResidual,
    command: &CommandButton,
) -> CommandAvailability {
    if leftover_presentation_has_weapon_slot(residual, command.weapon_slot) {
        if residual.weapon_reload_time == 0.0 {
            return CommandAvailability::Available;
        }
        if residual.weapon_fire_status == WEAPON_FIRE_RELOADING_CLIP
            || residual.weapon_fire_status != WEAPON_FIRE_READY
        {
            return CommandAvailability::NotReady;
        }
        return CommandAvailability::Available;
    }
    if (command.options & CommandOption::UsesMineClearingWeaponSet as u32) != 0
        && !residual.mine_clearing_weaponset
    {
        return CommandAvailability::Available;
    }
    CommandAvailability::Restricted
}

fn leftover_presentation_switch_weapon(
    residual: &PresentationAvailabilityResidual,
    command: &CommandButton,
) -> CommandAvailability {
    if !leftover_presentation_has_weapon_slot(residual, command.weapon_slot) {
        return CommandAvailability::Restricted;
    }
    if residual.active_weapon_slot == leftover_presentation_slot_index(command.weapon_slot) {
        CommandAvailability::Active
    } else {
        CommandAvailability::Available
    }
}

fn leftover_presentation_battle_plan_active(
    residual: &PresentationAvailabilityResidual,
    command: &CommandButton,
) -> bool {
    let opts = command.options;
    (residual.battle_plan_bombardment && (opts & CommandOption::OptionOne as u32) != 0)
        || (residual.battle_plan_hold_the_line && (opts & CommandOption::OptionTwo as u32) != 0)
        || (residual.battle_plan_search_and_destroy
            && (opts & CommandOption::OptionThree as u32) != 0)
}

fn leftover_presentation_clock_availability(
    bar: &ControlBar,
    command: &CommandButton,
    entry: Option<&crate::presentation_translator_residual::TranslatorCatalogEntry>,
) -> CommandAvailability {
    let residual = &bar.presentation_availability;
    match command.command_type {
        CommandType::DoSpecialPower => {
            if command.special_power.is_empty() {
                return CommandAvailability::Restricted;
            }
            let ready = entry
                .map(|e| e.special_power_ready)
                .unwrap_or(false)
                || bar.portrait_state.special_power_ready;
            if !ready {
                CommandAvailability::NotReady
            } else if residual.special_power_in_use {
                CommandAvailability::Restricted
            } else if leftover_presentation_battle_plan_active(residual, command) {
                CommandAvailability::Active
            } else {
                CommandAvailability::Available
            }
        }
        CommandType::ToggleOvercharge => {
            if residual.overcharge_active {
                CommandAvailability::Active
            } else {
                CommandAvailability::Available
            }
        }
        CommandType::FireWeapon => leftover_presentation_fire_weapon(residual, command),
        CommandType::SwitchWeapons => leftover_presentation_switch_weapon(residual, command),
        _ => CommandAvailability::Restricted,
    }
}

#[cfg(test)]
mod presentation_availability_tests {
    use super::*;

    fn button(command_type: CommandType) -> CommandButton {
        let mut button = CommandButton::default();
        button.command_type = command_type;
        button.command_name = "Command_Test".to_string();
        button
    }

    #[test]
    fn need_upgrade_must_be_stopped_evacuate_hack_and_single_use_restrict() {
        let mut bar = ControlBar::new();
        let mut need = button(CommandType::DoSpecialPower);
        need.options = CommandOption::NeedUpgrade as u32;
        need.upgrade = "Upgrade_AmericaRangerCaptureBuilding".to_string();
        assert!(leftover_presentation_common_restricted(&bar, &need, None));
        bar.presentation_availability
            .completed_upgrades
            .push("Upgrade_AmericaRangerCaptureBuilding".into());
        assert!(!leftover_presentation_common_restricted(&bar, &need, None));

        let mut stop = button(CommandType::DoSpecialPower);
        stop.options = CommandOption::MustBeStopped as u32;
        bar.presentation_availability.moving = true;
        assert!(leftover_presentation_common_restricted(&bar, &stop, None));

        let evac = button(CommandType::Evacuate);
        bar.presentation_availability.occupant_count = 0;
        assert!(leftover_presentation_common_restricted(&bar, &evac, None));
        bar.presentation_availability.occupant_count = 2;
        assert!(!leftover_presentation_common_restricted(&bar, &evac, None));

        let hack = button(CommandType::InternetHack);
        bar.presentation_availability.hacking_packing_or_unpacking = true;
        assert!(leftover_presentation_common_restricted(&bar, &hack, None));

        bar.presentation_availability.single_use_used = true;
        assert!(leftover_presentation_common_restricted(
            &bar,
            &button(CommandType::DoSpecialPower),
            None
        ));
    }

    #[test]
    fn strategy_center_stop_option_one_and_railed_transport_restrict() {
        let mut bar = ControlBar::new();
        let mut stop = button(CommandType::DoStop);
        stop.options = CommandOption::OptionOne as u32;
        assert_eq!(
            leftover_presentation_stop_or_rail(&bar, &stop),
            Some(CommandAvailability::Available)
        );
        bar.presentation_availability.battle_plan_hold_the_line = true;
        assert_eq!(
            leftover_presentation_stop_or_rail(&bar, &stop),
            Some(CommandAvailability::Restricted)
        );
        bar.presentation_availability.battle_plan_bombardment = true;
        assert_eq!(
            leftover_presentation_stop_or_rail(&bar, &stop),
            Some(CommandAvailability::Available)
        );

        let rail = button(CommandType::ExecuteRailedTransport);
        bar.presentation_availability.dock_open = false;
        assert_eq!(
            leftover_presentation_stop_or_rail(&bar, &rail),
            Some(CommandAvailability::Restricted)
        );
        bar.presentation_availability.dock_open = true;
        assert_eq!(
            leftover_presentation_stop_or_rail(&bar, &rail),
            Some(CommandAvailability::Available)
        );
    }

    #[test]
    fn fire_weapon_and_switch_weapons_use_clip_and_slot() {
        let residual = PresentationAvailabilityResidual {
            weapon_reload_time: 0.0,
            has_primary_weapon: true,
            ..PresentationAvailabilityResidual::default()
        };
        let fire = button(CommandType::FireWeapon);
        assert_eq!(
            leftover_presentation_fire_weapon(&residual, &fire),
            CommandAvailability::Available
        );

        let reloading = PresentationAvailabilityResidual {
            weapon_reload_time: 4.0,
            weapon_fire_status: WEAPON_FIRE_RELOADING_CLIP,
            has_primary_weapon: true,
            ..PresentationAvailabilityResidual::default()
        };
        assert_eq!(
            leftover_presentation_fire_weapon(&reloading, &fire),
            CommandAvailability::NotReady
        );

        let missing = PresentationAvailabilityResidual::default();
        assert_eq!(
            leftover_presentation_fire_weapon(&missing, &fire),
            CommandAvailability::Restricted
        );
        let mut mine = fire.clone();
        mine.options = CommandOption::UsesMineClearingWeaponSet as u32;
        assert_eq!(
            leftover_presentation_fire_weapon(&missing, &mine),
            CommandAvailability::Available
        );

        let mut switch = button(CommandType::SwitchWeapons);
        switch.weapon_slot = WeaponSlotType::Secondary;
        let empty = PresentationAvailabilityResidual::default();
        assert_eq!(
            leftover_presentation_switch_weapon(&empty, &switch),
            CommandAvailability::Restricted
        );
        let active = PresentationAvailabilityResidual {
            has_secondary_weapon: true,
            active_weapon_slot: 1,
            ..PresentationAvailabilityResidual::default()
        };
        assert_eq!(
            leftover_presentation_switch_weapon(&active, &switch),
            CommandAvailability::Active
        );
        let other = PresentationAvailabilityResidual {
            has_secondary_weapon: true,
            active_weapon_slot: 0,
            ..PresentationAvailabilityResidual::default()
        };
        assert_eq!(
            leftover_presentation_switch_weapon(&other, &switch),
            CommandAvailability::Available
        );
    }

    #[test]
    fn presentation_typed_availability_uses_leftover_fire_and_switch() {
        let mut bar = ControlBar::new();
        bar.apply_presentation_availability(PresentationAvailabilityResidual {
            weapon_reload_time: 0.0,
            has_primary_weapon: true,
            has_secondary_weapon: true,
            active_weapon_slot: 0,
            ..PresentationAvailabilityResidual::default()
        });
        let fire = button(CommandType::FireWeapon);
        assert_eq!(
            bar.presentation_typed_availability(&fire, None),
            CommandAvailability::Available
        );
        let mut switch = button(CommandType::SwitchWeapons);
        switch.weapon_slot = WeaponSlotType::Secondary;
        assert_eq!(
            bar.presentation_typed_availability(&switch, None),
            CommandAvailability::Available
        );
        bar.presentation_availability.active_weapon_slot = 1;
        assert_eq!(
            bar.presentation_typed_availability(&switch, None),
            CommandAvailability::Active
        );
        bar.apply_presentation_availability(PresentationAvailabilityResidual::default());
        assert_eq!(
            bar.presentation_typed_availability(&switch, None),
            CommandAvailability::Restricted
        );
    }

    #[test]
    fn special_power_in_use_battle_plan_and_overcharge_states() {
        let mut bar = ControlBar::new();
        bar.portrait_state.special_power_ready = true;
        let mut special = button(CommandType::DoSpecialPower);
        special.special_power = "SpecialPowerChangeBattlePlans".to_string();
        special.options = CommandOption::OptionOne as u32;
        assert_eq!(
            leftover_presentation_clock_availability(&bar, &special, None),
            CommandAvailability::Available
        );
        bar.presentation_availability.special_power_in_use = true;
        assert_eq!(
            leftover_presentation_clock_availability(&bar, &special, None),
            CommandAvailability::Restricted
        );
        bar.presentation_availability.special_power_in_use = false;
        bar.presentation_availability.battle_plan_bombardment = true;
        assert_eq!(
            leftover_presentation_clock_availability(&bar, &special, None),
            CommandAvailability::Active
        );

        let over = button(CommandType::ToggleOvercharge);
        assert_eq!(
            leftover_presentation_clock_availability(&bar, &over, None),
            CommandAvailability::Available
        );
        bar.presentation_availability.overcharge_active = true;
        assert_eq!(
            leftover_presentation_clock_availability(&bar, &over, None),
            CommandAvailability::Active
        );
    }
}
