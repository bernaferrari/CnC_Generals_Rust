//! Preview module-graph attach for GameWorld entities.
//!
//! Helpers follow C++ `Object.cpp:299-384` / `install_ctor_helpers` order.
//! Template module tags append after helpers (`rebuild_behavior_list`).
//! `on_delete` walks that same list order (`Object::onDestroy` / `on_delete`).
//! Flattened Entity fields stay the write surface.

use super::GameWorld;
use super::entities::{EntityId, EntityInstalledModules, EntityModuleRecord, EntityModuleState};
use super::entity_module_instances::{
    EntityLiveModule, live_modules_from_spec, live_modules_from_tags,
};
use crate::object::Object;
use std::collections::HashMap;

pub const HELPER_TAG_SMC: &str = "ModuleTag_SMCHelper";
pub const HELPER_TAG_STATUS: &str = "ModuleTag_StatusDamageHelper";
pub const HELPER_TAG_SUBDUAL: &str = "ModuleTag_SubdualDamageHelper";
pub const HELPER_TAG_REPULSOR: &str = "ModuleTag_RepulsorHelper";
pub const HELPER_TAG_DEFECTION: &str = "ModuleTag_DefectionHelper";
pub const HELPER_TAG_WEAPON_STATUS: &str = "ModuleTag_WeaponStatusHelper";
pub const HELPER_TAG_FIRING_TRACKER: &str = "ModuleTag_FiringTrackerHelper";
pub const HELPER_TAG_TEMP_WEAPON_BONUS: &str = "ModuleTag_TempWeaponBonusHelper";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityModuleInstallSpec {
    pub template_module_tags: Vec<String>,
    pub inactive_body: bool,
    pub shrubbery: bool,
    pub can_be_repulsed: bool,
    pub has_weapons: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EntityModuleGraph {
    pub tags: Vec<String>,
    pub on_created: bool,
    pub on_delete_order: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct GameWorldEntityModules {
    graphs: HashMap<u32, EntityModuleGraph>,
    last_delete: HashMap<u32, Vec<String>>,
    live: HashMap<u32, Vec<EntityLiveModule>>,
}

impl GameWorldEntityModules {
    pub fn clear(&mut self) {
        self.graphs.clear();
        self.last_delete.clear();
        self.live.clear();
    }

    pub fn get(&self, id: EntityId) -> Option<&EntityModuleGraph> {
        self.graphs.get(&id.get())
    }

    pub fn last_on_delete(&self, id: EntityId) -> &[String] {
        self.last_delete
            .get(&id.get())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn install(&mut self, id: EntityId, spec: &EntityModuleInstallSpec) -> Vec<String> {
        let live = live_modules_from_spec(spec);
        self.install_live(id, live)
    }

    pub fn install_from_crate_object(&mut self, id: EntityId, object: &Object) -> Vec<String> {
        let tags = object.installed_module_tags();
        let live = live_modules_from_tags(&tags);
        self.install_live(id, live)
    }

    fn install_live(&mut self, id: EntityId, live: Vec<EntityLiveModule>) -> Vec<String> {
        let tags: Vec<String> = live.iter().map(|m| m.tag().to_string()).collect();
        self.graphs.insert(
            id.get(),
            EntityModuleGraph {
                tags: tags.clone(),
                on_created: true,
                on_delete_order: Vec::new(),
            },
        );
        self.live.insert(id.get(), live);
        tags
    }

    pub fn live_count(&self, id: EntityId) -> usize {
        self.live.get(&id.get()).map(Vec::len).unwrap_or(0)
    }

    pub fn on_delete(&mut self, id: EntityId) -> Vec<String> {
        let live = self.live.remove(&id.get()).unwrap_or_default();
        let order: Vec<String> = if live.is_empty() {
            self.graphs
                .get(&id.get())
                .map(|g| g.tags.clone())
                .unwrap_or_default()
        } else {
            live.iter().map(|m| m.tag().to_string()).collect()
        };
        drop(live);
        if let Some(mut graph) = self.graphs.remove(&id.get()) {
            graph.on_delete_order = order.clone();
        }
        self.last_delete.insert(id.get(), order.clone());
        order
    }
}

pub fn ctor_helper_tags(spec: &EntityModuleInstallSpec) -> Vec<String> {
    let mut tags = vec![HELPER_TAG_SMC.to_string()];
    if !spec.inactive_body {
        tags.push(HELPER_TAG_STATUS.to_string());
        tags.push(HELPER_TAG_SUBDUAL.to_string());
    }
    if spec.can_be_repulsed {
        tags.push(HELPER_TAG_REPULSOR.to_string());
    }
    if !spec.shrubbery {
        tags.push(HELPER_TAG_DEFECTION.to_string());
    }
    if spec.has_weapons {
        tags.push(HELPER_TAG_WEAPON_STATUS.to_string());
        tags.push(HELPER_TAG_FIRING_TRACKER.to_string());
        tags.push(HELPER_TAG_TEMP_WEAPON_BONUS.to_string());
    }
    tags
}

pub fn helper_handle_name(tag: &str) -> &str {
    match tag {
        HELPER_TAG_SMC => "ObjectSMCHelper",
        HELPER_TAG_STATUS => "StatusDamageHelper",
        HELPER_TAG_SUBDUAL => "SubdualDamageHelper",
        HELPER_TAG_REPULSOR => "ObjectRepulsorHelper",
        HELPER_TAG_DEFECTION => "ObjectDefectionHelper",
        HELPER_TAG_WEAPON_STATUS => "ObjectWeaponStatusHelper",
        HELPER_TAG_FIRING_TRACKER => "FiringTracker",
        HELPER_TAG_TEMP_WEAPON_BONUS => "TempWeaponBonusHelper",
        other => other,
    }
}

fn records_from_tags(tags: &[String]) -> Vec<EntityModuleRecord> {
    tags.iter()
        .map(|tag| EntityModuleRecord {
            tag: tag.clone(),
            handle: helper_handle_name(tag).to_string(),
        })
        .collect()
}

impl GameWorld {
    pub fn install_entity_modules_from_crate(
        &mut self,
        id: EntityId,
        object: &Object,
    ) -> Vec<String> {
        let tags = self.entity_modules.install_from_crate_object(id, object);
        self.bind_entity_module_records(id, &tags);
        tags
    }

    pub fn install_entity_modules(
        &mut self,
        id: EntityId,
        spec: &EntityModuleInstallSpec,
    ) -> Vec<String> {
        let tags = self.entity_modules.install(id, spec);
        self.bind_entity_module_records(id, &tags);
        tags
    }

    fn bind_entity_module_records(&mut self, id: EntityId, tags: &[String]) {
        if let Some(entity) = self.world_mut().entity_mut(id) {
            let mut envelope = entity.take_envelope().unwrap_or_default();
            for tag in tags {
                if !envelope.module_states.iter().any(|m| m.tag == *tag) {
                    envelope.module_states.push(EntityModuleState {
                        tag: tag.clone(),
                        payload: Vec::new(),
                    });
                }
            }
            entity.attach_envelope(envelope);
            entity.entity_modules = Some(EntityInstalledModules {
                records: records_from_tags(tags),
                on_created: true,
                on_delete_order: Vec::new(),
                live_instances: tags.len(),
            });
        }
    }

    pub fn entity_live_module_count(&self, id: EntityId) -> usize {
        self.entity_modules.live_count(id)
    }

    pub fn entity_module_tags(&self, id: EntityId) -> &[String] {
        self.entity_modules
            .get(id)
            .map(|g| g.tags.as_slice())
            .unwrap_or(&[])
    }

    pub fn last_entity_on_delete_order(&self, id: EntityId) -> &[String] {
        self.entity_modules.last_on_delete(id)
    }

    pub fn walk_entity_modules_on_delete(&mut self, id: EntityId) -> Vec<String> {
        if let Some(entity) = self.world_mut().entity_mut(id) {
            if let Some(installed) = entity.entity_modules.as_mut() {
                installed.on_delete_order =
                    installed.records.iter().map(|r| r.tag.clone()).collect();
            }
        }
        self.entity_modules.on_delete(id)
    }
}

#[cfg(test)]
#[path = "entity_modules_tests.rs"]
mod tests;
