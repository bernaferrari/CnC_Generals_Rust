//! Preview module-graph attach for GameWorld entities.
//!
//! Helpers follow C++ `Object.cpp:299-384` / `install_ctor_helpers` order.
//! Template module tags append after helpers (`rebuild_behavior_list`).
//! `on_delete` walks that same list order (`Object::onDestroy` / `on_delete`).
//! Flattened Entity fields stay the write surface.

use super::entities::{EntityId, EntityInstalledModules, EntityModuleRecord, EntityModuleState};
use super::GameWorld;
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
}

impl GameWorldEntityModules {
    pub fn clear(&mut self) {
        self.graphs.clear();
        self.last_delete.clear();
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
        let mut tags = ctor_helper_tags(spec);
        tags.extend(spec.template_module_tags.iter().cloned());
        self.graphs.insert(
            id.get(),
            EntityModuleGraph {
                tags: tags.clone(),
                on_created: true,
                on_delete_order: Vec::new(),
            },
        );
        tags
    }

    pub fn on_delete(&mut self, id: EntityId) -> Vec<String> {
        let Some(mut graph) = self.graphs.remove(&id.get()) else {
            return self.last_delete.get(&id.get()).cloned().unwrap_or_default();
        };
        graph.on_delete_order = graph.tags.clone();
        self.last_delete
            .insert(id.get(), graph.on_delete_order.clone());
        graph.on_delete_order
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
    pub fn install_entity_modules(
        &mut self,
        id: EntityId,
        spec: &EntityModuleInstallSpec,
    ) -> Vec<String> {
        let tags = self.entity_modules.install(id, spec);
        if let Some(entity) = self.world_mut().entity_mut(id) {
            let mut envelope = entity.take_envelope().unwrap_or_default();
            for tag in &tags {
                if !envelope.module_states.iter().any(|m| m.tag == *tag) {
                    envelope.module_states.push(EntityModuleState {
                        tag: tag.clone(),
                        payload: Vec::new(),
                    });
                }
            }
            entity.attach_envelope(envelope);
            entity.entity_modules = Some(EntityInstalledModules {
                records: records_from_tags(&tags),
                on_created: true,
                on_delete_order: Vec::new(),
            });
        }
        tags
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
mod tests {
    use super::*;
    use crate::world::entities::{TemplateRef, Transform};

    #[test]
    fn default_helpers_then_template_and_on_delete_preserves_order() {
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("TestObject"),
            None,
            Transform::default(),
            10.0,
        );
        let spec = EntityModuleInstallSpec {
            template_module_tags: vec!["BodyModule".to_string(), "AIUpdate".to_string()],
            ..EntityModuleInstallSpec::default()
        };
        let tags = world.install_entity_modules(id, &spec);
        assert_eq!(
            tags,
            vec![
                HELPER_TAG_SMC,
                HELPER_TAG_STATUS,
                HELPER_TAG_SUBDUAL,
                HELPER_TAG_DEFECTION,
                "BodyModule",
                "AIUpdate",
            ]
        );
        assert_eq!(world.entity_module_tags(id), tags.as_slice());
        let installed = world
            .entity(id)
            .and_then(|e| e.entity_modules.clone())
            .expect("entity modules");
        assert!(installed.on_created);
        assert_eq!(installed.records.len(), tags.len());
        assert_eq!(installed.records[0].handle, "ObjectSMCHelper");
        let deleted = world.walk_entity_modules_on_delete(id);
        assert_eq!(deleted, tags);
        assert_eq!(world.last_entity_on_delete_order(id), tags.as_slice());
        assert!(world.entity_module_tags(id).is_empty());
    }

    #[test]
    fn tank_fixture_installs_helpers_then_template_in_cpp_order() {
        let spec = EntityModuleInstallSpec {
            template_module_tags: vec!["ActiveBody".to_string(), "AIUpdate".to_string()],
            has_weapons: true,
            ..EntityModuleInstallSpec::default()
        };
        let mut expected = ctor_helper_tags(&spec);
        expected.extend(spec.template_module_tags.iter().cloned());
        assert_eq!(
            expected,
            vec![
                HELPER_TAG_SMC,
                HELPER_TAG_STATUS,
                HELPER_TAG_SUBDUAL,
                HELPER_TAG_DEFECTION,
                HELPER_TAG_WEAPON_STATUS,
                HELPER_TAG_FIRING_TRACKER,
                HELPER_TAG_TEMP_WEAPON_BONUS,
                "ActiveBody",
                "AIUpdate",
            ]
        );
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("AmericaTankCrusader"),
            None,
            Transform::default(),
            100.0,
        );
        let installed = world.install_entity_modules(id, &spec);
        assert_eq!(installed, expected);
        world.mark_entity_destroyed(id);
        assert_eq!(world.process_destroy_list(), 1);
        assert_eq!(world.last_entity_on_delete_order(id), expected.as_slice());
    }

    #[test]
    fn shrubbery_omits_defection_inactive_body_omits_status() {
        let spec = EntityModuleInstallSpec {
            inactive_body: true,
            shrubbery: true,
            ..EntityModuleInstallSpec::default()
        };
        assert_eq!(ctor_helper_tags(&spec), vec![HELPER_TAG_SMC.to_string()]);
    }
}
