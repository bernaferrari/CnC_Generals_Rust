//! The behavior roster used by the Object's capture and delete callbacks.
use super::{AuthoredCostModifierUpgrade, ModuleDataHandle, Object};
use crate::assets::ObjectDefinition;
use crate::game_logic::thing::ReceivingClientTemplate;
use std::sync::Arc;

pub(in crate::game_logic) struct BehaviorCallbackOrder {
    admitted: Option<ReceivingClientTemplate>,
    projected: Option<Arc<ObjectDefinition>>,
}

pub(in crate::game_logic) struct BehaviorCallbackEntry<'a> {
    class: &'a str,
    tag: Option<&'a str>,
    projection_index: Option<usize>,
    data: Option<&'a ModuleDataHandle>,
}

impl BehaviorCallbackEntry<'_> {
    pub(in crate::game_logic) fn is(&self, class: &str) -> bool {
        self.class.eq_ignore_ascii_case(class)
    }

    pub(in crate::game_logic) fn cost_modifier(
        &self,
        state_index: usize,
    ) -> Option<AuthoredCostModifierUpgrade> {
        AuthoredCostModifierUpgrade::from_data(state_index, self.data?)
    }
}

impl BehaviorCallbackOrder {
    pub(in crate::game_logic) fn is_declared(&self) -> bool {
        self.admitted.is_some() || self.projected.is_some()
    }

    pub(in crate::game_logic) fn iter(&self) -> impl Iterator<Item = BehaviorCallbackEntry<'_>> {
        let admitted = self.admitted.iter().flat_map(|plan| {
            plan.behavior_modules()
                .iter()
                .map(|entry| BehaviorCallbackEntry {
                    class: entry.name.as_str(),
                    tag: Some(entry.module_tag.as_str()),
                    projection_index: None,
                    data: Some(entry.data),
                })
        });
        let projected = self
            .projected
            .iter()
            .filter(move |_| self.admitted.is_none())
            .flat_map(|plan| {
                plan.behavior_modules
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| BehaviorCallbackEntry {
                        class: &entry.class_name,
                        tag: entry.module_tag.as_deref(),
                        projection_index: Some(index),
                        data: None,
                    })
            });
        admitted.chain(projected)
    }

    /// Plant bonuses still identify their source in the host projection.
    pub(in crate::game_logic) fn projection_index(
        &self,
        slot: &BehaviorCallbackEntry<'_>,
    ) -> Option<usize> {
        slot.projection_index.or_else(|| {
            let tag = slot.tag?;
            self.projected
                .as_ref()?
                .behavior_modules
                .iter()
                .position(|entry| {
                    entry.module_tag.as_deref() == Some(tag) && slot.is(&entry.class_name)
                })
        })
    }
}

impl Object {
    pub(in crate::game_logic) fn behavior_callback_order(&self) -> BehaviorCallbackOrder {
        let template = self.get_template();
        BehaviorCallbackOrder {
            admitted: template.receiving_client_template.clone(),
            projected: template.receiving_object_definition.clone(),
        }
    }
}
