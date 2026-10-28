//! Choose a shortcut's source from the receiving match, without changing selection.
use super::super::*;
use game_engine::common::rts::special_power::SpecialPowerType;

/// Facts needed by both the shortcut bar and the command admission boundary.
pub(crate) struct ShortcutPowerSource {
    pub id: ObjectId,
    pub disabled: bool,
    pub paused: bool,
    pub remaining: Option<f32>,
    pub ready_count: i32,
}

impl GameLogic {
    /// Identify an authored shortcut command and resolve its current source.
    /// Ordinary commands return `Ok(None)`; an authored shortcut without a
    /// usable source fails closed instead of falling back to the UI selection.
    pub fn shortcut_command_source(
        &self,
        player_id: u32,
        command_name: &str,
        power_name: &str,
    ) -> Result<Option<ObjectId>, &'static str> {
        let Some(actions) = self
            .receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions())
        else {
            return Ok(None);
        };
        let Some(button) = actions.buttons.find_command_button_resolved(command_name) else {
            return Ok(None);
        };
        if ![
            "SPECIAL_POWER_FROM_SHORTCUT",
            "SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT",
        ]
        .iter()
        .any(|command| button.command.eq_ignore_ascii_case(command))
        {
            return Ok(None);
        }
        let authored_name = button
            .special_power_template
            .as_deref()
            .ok_or("shortcut button has no authored special power")?;
        if !authored_name.eq_ignore_ascii_case(power_name) {
            return Err("shortcut power does not match its receiving command");
        }
        let power = actions
            .powers
            .find_template(authored_name)
            .ok_or("shortcut power is missing from the receiving definitions")?;
        let source = self
            .shortcut_power_source(player_id, power.power_type)
            .ok_or("shortcut power has no receiving source")?;
        if source.disabled
            || source.paused
            || !source.remaining.is_some_and(|remaining| remaining <= 0.0)
        {
            return Err("shortcut power source is not ready");
        }
        Ok(Some(source.id))
    }

    /// C++ `Player::findMostReadyShortcutSpecialPowerOfType`, using this
    /// world's player, objects and definitions. The existing host countdown
    /// is in seconds; exact original ready-frame and team-list ordering still
    /// need their own migration.
    pub(crate) fn shortcut_power_source(
        &self,
        player_id: u32,
        power_type: SpecialPowerType,
    ) -> Option<ShortcutPowerSource> {
        let player = self.get_player(player_id)?;
        let actions = self.receiving_module_content.as_ref()?.action_definitions();
        let mut best: Option<ShortcutPowerSource> = None;
        let mut ready_count = 0;
        for object in self.host_objects().registered_values() {
            if object.owner_player_id != Some(player_id)
                || object.status.under_construction
                || object.status.sold
                || object.status.effectively_dead
            {
                continue;
            }
            let Some(module) =
                object
                    .thing()
                    .template
                    .special_power_modules
                    .iter()
                    .find(|module| {
                        !module.scripted_special_power_only
                            && actions
                                .powers
                                .find_template(&module.special_power_template)
                                .is_some_and(|candidate| candidate.power_type == power_type)
                    })
            else {
                continue;
            };
            let remaining = module
                .command_power
                .as_ref()
                .map(|kind| {
                    if module.shared_n_sync {
                        player.shared_special_power_remaining(kind)
                    } else {
                        object.special_power_countdown_seconds(kind)
                    }
                })
                .filter(|seconds| seconds.is_finite());
            let paused = module
                .command_power
                .as_ref()
                .is_some_and(|kind| object.is_special_power_countdown_paused(kind));
            let disabled = object.is_disabled();
            if !disabled
                && !paused
                && remaining.is_some_and(|seconds| seconds <= 0.0)
                && (!module.shared_n_sync || ready_count == 0)
            {
                ready_count += 1;
            }
            let rank = |disabled: bool, remaining: Option<f32>| {
                if disabled {
                    f32::INFINITY
                } else {
                    remaining.unwrap_or(f32::INFINITY)
                }
            };
            if best
                .as_ref()
                .is_none_or(|old| rank(disabled, remaining) < rank(old.disabled, old.remaining))
            {
                best = Some(ShortcutPowerSource {
                    id: object.id,
                    disabled,
                    paused,
                    remaining,
                    ready_count: 0,
                });
            }
        }
        best.map(|mut source| {
            source.ready_count = ready_count.max(1);
            source
        })
    }
}
