//! CommandButton readiness observations borrowed from the world executing the script.

use super::{GameLogic, ObjectId, ScriptCommandButton, ScriptOwnerQuery};
use game_engine::common::rts::special_power::SpecialPowerStore;

impl GameLogic {
    pub(super) fn script_command_button_definition(
        &self,
        name: &str,
    ) -> ScriptOwnerQuery<ScriptCommandButton> {
        self.command_definitions().with_buttons(|buttons| {
            let Some(button) = buttons.find_command_button_resolved(name) else {
                return ScriptOwnerQuery::Missing;
            };
            let special_power = button.special_power_template.as_ref().filter(|name| {
                self.with_script_button_powers(|powers| powers.find_template(name).is_some())
            });
            ScriptOwnerQuery::Present(ScriptCommandButton {
                special_power: special_power.cloned(),
                upgrade: self
                    .upgrade_template(&button.upgrade)
                    .map(|_| button.upgrade.clone()),
            })
        })
    }

    // Object::hasSpecialPower checks the type bit. CommandButton::isReady then
    // asks the first module for the exact template for its percent charged.
    pub(super) fn script_command_button_power(&self, id: ObjectId, name: &str) -> Option<bool> {
        let object = self.host_object(id)?;
        let modules = &object.thing().template.special_power_modules;
        self.with_script_button_powers(|powers| {
            let target = powers.find_template(name)?;
            let exact = modules.iter().find(|module| {
                module.special_power_template_id == target.id
                    && module
                        .special_power_template
                        .eq_ignore_ascii_case(&target.name)
            });
            let Some(module) = exact else {
                return modules
                    .iter()
                    .any(|module| {
                        powers
                            .find_template(&module.special_power_template)
                            .is_some_and(|power| power.power_type == target.power_type)
                    })
                    .then_some(false);
            };

            let (local_seconds, pauses) = object
                .authored_special_power_timers
                .get(&module.source_index)
                .copied()
                .unwrap_or_default();
            let shared_seconds = module
                .shared_n_sync
                .then(|| {
                    let owner = self.player_owner_for_host_object(object)?;
                    let player = self.get_player(owner)?;
                    Some(
                        player
                            .authored_special_power_timers
                            .get(&module.special_power_template.to_ascii_lowercase())
                            .copied()
                            .unwrap_or_default(),
                    )
                })
                .flatten();
            Some(pauses == 0 && shared_seconds.unwrap_or(local_seconds) <= 0.0)
        })
    }

    pub(super) fn script_command_button_upgrade(&self, id: ObjectId, name: &str) -> bool {
        self.host_object(id).is_some_and(|object| {
            !object.has_object_upgrade_complete(name)
                && self.authored_object_would_upgrade(object, name)
        })
    }

    fn with_script_button_powers<R>(&self, observe: impl FnOnce(&SpecialPowerStore) -> R) -> R {
        match &self.receiving_module_content {
            Some(content) => observe(&content.action_definitions().powers),
            None => observe(&self.world_services.common().ini.rts_special_power.read()),
        }
    }
}

#[cfg(test)]
#[path = "script_command_button_readiness_tests.rs"]
mod tests;
