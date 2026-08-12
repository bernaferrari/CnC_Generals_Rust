use super::*;

pub(super) const CMD_NEED_TARGET_ENEMY_OBJECT: u32 = 0x0000_0001;
pub(super) const CMD_NEED_TARGET_NEUTRAL_OBJECT: u32 = 0x0000_0002;
pub(super) const CMD_NEED_TARGET_ALLY_OBJECT: u32 = 0x0000_0004;
pub(super) const CMD_NEED_TARGET_PRISONER: u32 = 0x0000_0008;
pub(super) const CMD_ALLOW_SHRUBBERY_TARGET: u32 = 0x0000_0010;
pub(super) const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
pub(super) const CMD_CONTEXTMODE_COMMAND: u32 = 0x0000_0200;
pub(super) const CMD_ALLOW_MINE_TARGET: u32 = 0x0000_0800;
pub(super) const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
pub(super) const SPECIAL_POWER_INVALID: u32 = 0;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ContextPickProfile {
    pub(super) include_selectable: bool,
    pub(super) include_force_attackable: bool,
    pub(super) include_mines: bool,
    pub(super) include_shrubbery: bool,
}

impl Default for ContextPickProfile {
    fn default() -> Self {
        Self {
            include_selectable: true,
            include_force_attackable: false,
            include_mines: false,
            include_shrubbery: false,
        }
    }
}

pub(super) fn selection_has_flame_weapon(selection: &HashSet<ObjectID>) -> bool {
    for &id in selection {
        let Some(obj) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(guard) = obj.read() else {
            continue;
        };
        if guard.is_destroyed() {
            continue;
        }
        if guard
            .weapon_set
            .has_weapon_to_deal_damage_type(DamageType::Flame)
        {
            return true;
        }
    }
    false
}

pub(super) fn context_pick_profile(
    force_attack_mode: bool,
    selection: &HashSet<ObjectID>,
) -> ContextPickProfile {
    let mut profile = ContextPickProfile::default();
    if force_attack_mode {
        profile.include_force_attackable = true;
    }

    let pending_options = TheInGameUI::get_pending_command()
        .map(|pending| pending.options)
        .or_else(|| TheInGameUI::get_pending_special_power().map(|pending| pending.options));

    if let Some(options) = pending_options {
        if options & CMD_ALLOW_MINE_TARGET != 0 {
            profile.include_mines = true;
        }
        if options & CMD_ALLOW_SHRUBBERY_TARGET != 0 {
            profile.include_shrubbery = true;
        }
    } else if force_attack_mode && selection_has_flame_weapon(selection) {
        // Matches C++ getPickTypesForCurrentSelection(forceAttackMode): flame weapons can target shrubbery.
        profile.include_shrubbery = true;
    }

    profile
}
