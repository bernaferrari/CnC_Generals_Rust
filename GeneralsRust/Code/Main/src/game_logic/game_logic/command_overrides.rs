//! Original GameLogic buildability and CommandSet slot overrides.
use super::*;

pub(crate) type ControlBarOverrides = std::collections::HashMap<String, Option<String>>;

pub(crate) fn control_bar_override_key(set: &str, slot: i32) -> Option<String> {
    if !(0..gamelogic::command_button::MAX_COMMANDS_PER_SET as i32).contains(&slot) {
        return None;
    }
    Some(format!(
        "{}{}",
        char::from_u32('0' as u32 + slot as u32)?,
        set
    ))
}

impl GameLogic {
    /// Extend the owning action catalog with source-authored fixture definitions.
    pub fn admit_fixture_command_action_source(&mut self, source: &str) -> Result<(), String> {
        if let Some(content) = self.receiving_module_content.as_mut() {
            return content.admit_action_overrides(&[source.to_string()]);
        }
        let mut definitions = self.command_action_definitions.clone().unwrap_or_default();
        definitions.apply_source(
            source,
            &game_engine::common::rts::science::ScienceNameFacts::default(),
        )?;
        self.command_action_definitions = Some(definitions);
        Ok(())
    }

    pub(crate) fn build_eligibility_action_definitions(
        &self,
    ) -> Option<&game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions>
    {
        self.receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions())
            .or(self.command_action_definitions.as_ref())
    }

    pub fn set_buildable_status_override(&mut self, template: &str, status: i32) {
        self.buildable_status_overrides
            .insert(template.to_string(), status);
    }
    pub fn find_buildable_status_override(&self, template: &str) -> Option<i32> {
        self.buildable_status_overrides.get(template).copied()
    }
    pub(crate) fn effective_buildable_status(&self, template: &str, initial: u32) -> u32 {
        self.find_buildable_status_override(template)
            .map_or(initial, |value| value.max(0) as u32)
    }
    pub(crate) fn human_control_bar_buildable_hidden(&self, template: &str) -> bool {
        self.templates.get(template).is_some_and(|definition| {
            !crate::game_logic::host_production_buildable_command_residual::buildable_status_allows_human_residual(
                self.effective_buildable_status(template, definition.buildable_status))
        })
    }
    pub fn set_control_bar_override(&mut self, set: &str, slot: i32, button: Option<&str>) {
        if let Some(key) = control_bar_override_key(set, slot) {
            self.control_bar_overrides
                .insert(key, button.map(str::to_string));
        }
    }
    pub fn find_control_bar_override(&self, set: &str, slot: i32) -> Option<Option<&str>> {
        self.control_bar_overrides
            .get(&control_bar_override_key(set, slot)?)
            .map(|value| value.as_deref())
    }
    pub(crate) fn control_bar_overrides(&self) -> &ControlBarOverrides {
        &self.control_bar_overrides
    }
    pub(crate) fn snapshot_buildable_status_overrides(&self) -> Vec<(String, i32)> {
        let mut entries: Vec<_> = self
            .buildable_status_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }
    pub(crate) fn snapshot_control_bar_overrides_raw(&self) -> Vec<(String, Option<String>)> {
        let mut entries: Vec<_> = self
            .control_bar_overrides
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }
    pub(crate) fn restore_buildable_status_overrides(&mut self, entries: Vec<(String, i32)>) {
        self.buildable_status_overrides = entries.into_iter().collect();
    }
    pub(crate) fn restore_control_bar_overrides_raw(
        &mut self,
        entries: Vec<(String, Option<String>)>,
    ) {
        self.control_bar_overrides = entries.into_iter().collect();
    }
    /// Effective immutable Object definition from the receiving content owner.
    pub(crate) fn command_set_for_template(&self, name: &str) -> Option<String> {
        if let Some(content) = &self.receiving_module_content {
            return content
                .admitted_template(name)
                .map(|plan| plan.command_set_name().to_string());
        }
        self.templates
            .get(name)
            .map(|template| template.command_set_name.clone())
    }
    pub(crate) fn script_set_buildable_override(&mut self, template: &str, status: i32) {
        let exists = self.receiving_module_content.as_ref().map_or_else(
            || self.templates.contains_key(template),
            |content| content.admitted_template(template).is_some(),
        );
        if exists {
            self.set_buildable_status_override(template, status);
        }
    }
    pub(crate) fn admitted_command_button(
        &self,
        name: &str,
    ) -> Option<game_engine::common::ini::ini_command_button::CommandButton> {
        if let Some(content) = self.receiving_module_content.as_ref() {
            return content
                .action_definitions()
                .buttons
                .find_command_button_resolved(name)
                .cloned();
        }
        game_engine::common::ini::ini_command_button::get_control_bar()?
            .find_command_button_resolved(name)
            .cloned()
    }
    fn admitted_command_set(
        &self,
        name: &str,
    ) -> Option<game_engine::common::ini::ini_command_set::CommandSet> {
        if let Some(content) = self.receiving_module_content.as_ref() {
            return content
                .action_definitions()
                .sets
                .find_command_set_resolved(name)
                .cloned();
        }
        game_engine::common::ini::ini_command_set::get_command_set_manager()?
            .find_command_set_resolved(name)
            .cloned()
    }
    pub(crate) fn effective_command_slots(&self, set_name: &str) -> Option<Vec<Option<String>>> {
        let set = self.admitted_command_set(set_name)?;
        Some(
            (0..gamelogic::command_button::MAX_COMMANDS_PER_SET)
                .map(|slot| {
                    self.find_control_bar_override(set_name, slot as i32)
                        .map(|value| value.map(str::to_string))
                        .unwrap_or_else(|| set.get_button_at_position(slot).cloned())
                })
                .collect(),
        )
    }
    pub(crate) fn build_command_targets(&self, producer: &Object) -> Option<Vec<(String, String)>> {
        let set = producer
            .command_set_override
            .clone()
            .filter(|name| !name.is_empty())
            .or_else(|| self.command_set_for_template(&producer.template_name))?;
        if set.is_empty() {
            return None;
        }
        let slots = self.effective_command_slots(&set)?;
        Some(
            slots
                .iter()
                .flatten()
                .filter_map(|name| {
                    let button = self.admitted_command_button(name)?;
                    matches!(button.command.as_str(), "UNIT_BUILD" | "DOZER_CONSTRUCT")
                        .then(|| (button.command.clone(), button.object.clone()))
                })
                .collect(),
        )
    }
    pub(crate) fn receiving_build_authorized(
        &self,
        producer: &Object,
        target: &str,
        command: &str,
    ) -> Option<bool> {
        self.build_command_targets(producer).map(|targets| {
            targets
                .iter()
                .any(|(operation, name)| operation == command && name == target)
        })
    }

    /// Resolve the exact receiving button used by C++
    /// `ProductionUpdate::setSpecialPowerConstructionCommandButton`. Ordinary
    /// constructs must be in the producer's effective CommandSet. A
    /// FROM_SHORTCUT construct instead must be the exact admitted UI button
    /// whose ready receiving shortcut selected this producer.
    pub(crate) fn receiving_special_power_construction_button(
        &self,
        producer: &Object,
        button_name: &str,
        requested_template_name: &str,
    ) -> Option<crate::game_logic::object::PendingSpecialPowerConstructionButton> {
        let button = self.admitted_command_button(button_name)?;
        let is_shortcut = button
            .command
            .eq_ignore_ascii_case("SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT");
        if !is_shortcut
            && !button
                .command
                .eq_ignore_ascii_case("SPECIAL_POWER_CONSTRUCT")
        {
            return None;
        }
        let special_power_name = button.special_power_template?;
        if special_power_name.is_empty() {
            return None;
        }
        if is_shortcut {
            let owner_id = producer.owner_player_id?;
            if self
                .shortcut_command_source(owner_id, button_name, &special_power_name)
                .ok()?
                != Some(producer.id)
            {
                return None;
            }
        } else {
            let set_name = producer
                .command_set_override
                .clone()
                .filter(|name| !name.is_empty())
                .or_else(|| self.command_set_for_template(&producer.template_name))?;
            let slots = self.effective_command_slots(&set_name)?;
            if !slots.iter().flatten().any(|name| name == button_name) {
                return None;
            }
        }
        let authored_target = self.templates.get(&button.object)?;
        let requested_target = self.templates.get(requested_template_name)?;
        // BuildAssistant.cpp:1291-1295 compares ThingTemplate pointers, not
        // `isEquivalentTo`: reskins/build variants cannot inherit this
        // transient placement authorization.
        if !std::ptr::eq(authored_target, requested_target) {
            return None;
        }
        Some(
            crate::game_logic::object::PendingSpecialPowerConstructionButton {
                command_button_name: button.name,
                template_name: button.object,
                special_power_name,
                options_bits: button.options_bits,
            },
        )
    }
    /// CPP ScriptActions6253/6281 searches effective slots, then writes one map entry.
    pub(crate) fn script_command_bar_override(
        &mut self,
        button: &str,
        template: &str,
        designer_slot: Option<i32>,
    ) {
        let Some(set_name) = self.command_set_for_template(template) else {
            return;
        };
        if let Some(slot) = designer_slot {
            let slot = slot - 1;
            if control_bar_override_key(&set_name, slot).is_none() {
                return;
            }
            let exists = self.admitted_command_button(button).is_some();
            if exists {
                self.set_control_bar_override(&set_name, slot, Some(button));
            }
            return;
        }
        let Some(set) = self.admitted_command_set(&set_name) else {
            return;
        };
        for slot in 0..gamelogic::command_button::MAX_COMMANDS_PER_SET {
            let effective = self
                .find_control_bar_override(&set_name, slot as i32)
                .unwrap_or_else(|| set.get_button_at_position(slot).map(String::as_str));
            if effective == Some(button) {
                self.set_control_bar_override(&set_name, slot as i32, None);
                break;
            }
        }
    }
}
