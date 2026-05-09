use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::upgrade::modules::upgrade_mux::{UpgradeMux, UpgradeMuxData};
use crate::upgrade::UpgradeMask;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data for SubObjectsUpgrade.
#[derive(Debug, Clone)]
pub struct SubObjectsUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    show_sub_object_names: Vec<AsciiString>,
    hide_sub_object_names: Vec<AsciiString>,
}

impl Default for SubObjectsUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            show_sub_object_names: Vec::new(),
            hide_sub_object_names: Vec::new(),
        }
    }
}

impl SubObjectsUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUBOBJECTS_UPGRADE_FIELDS)
    }

    pub fn show_sub_object_names(&self) -> &[AsciiString] {
        &self.show_sub_object_names
    }

    pub fn hide_sub_object_names(&self) -> &[AsciiString] {
        &self.hide_sub_object_names
    }
}

impl ModuleData for SubObjectsUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for SubObjectsUpgradeModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Upgrade module that toggles sub-object visibility on the drawable.
pub struct SubObjectsUpgrade {
    inner: Arc<Mutex<SubObjectsUpgradeInner>>,
    module_name_key: NameKeyType,
    data: Arc<SubObjectsUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

#[derive(Debug)]
struct SubObjectsUpgradeInner {
    data: Arc<SubObjectsUpgradeModuleData>,
    mux: UpgradeMux,
    object_id: ObjectID,
}

type SubObjectsUpgradeHandles = HashMap<ObjectID, Vec<Weak<Mutex<SubObjectsUpgradeInner>>>>;

static SUBOBJECTS_UPGRADE_MODULES: Lazy<RwLock<SubObjectsUpgradeHandles>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Handle exposed to object runtime for applying/removing the upgrade.
pub(crate) struct SubObjectsUpgradeHandle {
    inner: Arc<Mutex<SubObjectsUpgradeInner>>,
}

impl SubObjectsUpgradeHandle {
    fn new(inner: Arc<Mutex<SubObjectsUpgradeInner>>) -> Self {
        Self { inner }
    }

    pub fn apply(&self, upgrade_mask: UpgradeMaskType) -> bool {
        let mut guard = self.inner.lock().expect("SubObjectsUpgrade inner poisoned");

        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if !guard.mux.would_upgrade(key_mask) {
            return false;
        }

        let Some(object) = OBJECT_REGISTRY.get_object(guard.object_id) else {
            log::warn!("SubObjectsUpgrade: Object {} not found", guard.object_id);
            return false;
        };

        let mut object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "SubObjectsUpgrade: Failed to lock object {}",
                    guard.object_id
                );
                return false;
            }
        };

        let (activation, conflicting) = guard.mux.data.clone().get_upgrade_activation_masks();
        let _ = activation;

        let conflicting_bits = UpgradeMaskType::from_bits_retain(conflicting.to_bits());
        if object_guard
            .completed_upgrades()
            .intersects(conflicting_bits)
        {
            return false;
        }

        if let Some(player) = object_guard.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if player_guard
                    .get_completed_upgrade_mask()
                    .intersects(conflicting_bits)
                {
                    return false;
                }
            }
        }

        guard.mux.data.perform_upgrade_fx(&mut object_guard);
        guard.mux.data.process_upgrade_removal(&mut object_guard);
        apply_subobject_visibility(&mut object_guard, guard.data.as_ref());
        guard.mux.set_upgrade_executed(true);
        true
    }

    pub fn remove(&self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert sub-object visibility for this upgrade.
    }

    pub(crate) fn for_object(object_id: ObjectID) -> Vec<Self> {
        let mut registry = SUBOBJECTS_UPGRADE_MODULES
            .write()
            .expect("subobjects upgrade registry poisoned");
        if let Some(entries) = registry.get_mut(&object_id) {
            let mut handles = Vec::new();
            entries.retain(|weak| {
                if let Some(upgrade) = weak.upgrade() {
                    handles.push(SubObjectsUpgradeHandle::new(upgrade));
                    true
                } else {
                    false
                }
            });
            if entries.is_empty() {
                registry.remove(&object_id);
            }
            handles
        } else {
            Vec::new()
        }
    }
}

fn register_subobjects_upgrade(object_id: ObjectID, inner: &Arc<Mutex<SubObjectsUpgradeInner>>) {
    let mut registry = SUBOBJECTS_UPGRADE_MODULES
        .write()
        .expect("subobjects upgrade registry poisoned");
    registry
        .entry(object_id)
        .or_default()
        .push(Arc::downgrade(inner));
}

fn unregister_subobjects_upgrade(object_id: ObjectID, inner: &Arc<Mutex<SubObjectsUpgradeInner>>) {
    let mut registry = SUBOBJECTS_UPGRADE_MODULES
        .write()
        .expect("subobjects upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        entries.retain(|entry| {
            entry
                .upgrade()
                .map(|strong| !Arc::ptr_eq(&strong, inner))
                .unwrap_or(false)
        });
        if entries.is_empty() {
            registry.remove(&object_id);
        }
    }
}

fn apply_subobject_visibility(
    object: &mut crate::object::Object,
    data: &SubObjectsUpgradeModuleData,
) {
    let Some(drawable) = object.get_drawable() else {
        return;
    };

    let mut update_sub_objects = false;
    let Ok(mut drawable_guard) = drawable.write() else {
        return;
    };
    for name in &data.show_sub_object_names {
        drawable_guard.show_sub_object(name.as_str(), true);
        update_sub_objects = true;
    }
    for name in &data.hide_sub_object_names {
        drawable_guard.show_sub_object(name.as_str(), false);
        update_sub_objects = true;
    }
    if update_sub_objects {
        drawable_guard.update_sub_objects();
    }
}

impl SubObjectsUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<SubObjectsUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        let inner = Arc::new(Mutex::new(SubObjectsUpgradeInner {
            data: Arc::clone(&data),
            mux,
            object_id,
        }));
        register_subobjects_upgrade(object_id, &inner);
        Self {
            inner,
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }
}

impl Drop for SubObjectsUpgrade {
    fn drop(&mut self) {
        unregister_subobjects_upgrade(self.object_id, &self.inner);
    }
}

impl Module for SubObjectsUpgrade {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for SubObjectsUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_state(xfer, &mut self.applied)?;
        if let Ok(mut guard) = self.inner.lock() {
            guard.mux.set_upgrade_executed(self.applied);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for SubObjectsUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        let mut guard = self.inner.lock().expect("SubObjectsUpgrade inner poisoned");

        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if !guard.mux.would_upgrade(key_mask) {
            return false;
        }

        let Some(object) = OBJECT_REGISTRY.get_object(guard.object_id) else {
            log::warn!("SubObjectsUpgrade: Object {} not found", guard.object_id);
            return false;
        };

        let mut object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "SubObjectsUpgrade: Failed to lock object {}",
                    guard.object_id
                );
                return false;
            }
        };

        let (activation, conflicting) = guard.mux.data.clone().get_upgrade_activation_masks();
        let _ = activation;

        let conflicting_bits = UpgradeMaskType::from_bits_retain(conflicting.to_bits());
        if object_guard
            .completed_upgrades()
            .intersects(conflicting_bits)
        {
            return false;
        }

        if let Some(player) = object_guard.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if player_guard
                    .get_completed_upgrade_mask()
                    .intersects(conflicting_bits)
                {
                    return false;
                }
            }
        }

        guard.mux.data.perform_upgrade_fx(&mut object_guard);
        guard.mux.data.process_upgrade_removal(&mut object_guard);
        apply_subobject_visibility(&mut object_guard, guard.data.as_ref());
        guard.mux.set_upgrade_executed(true);
        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert sub-object visibility for this upgrade.
    }
}

#[allow(dead_code)]
fn apply_subobject_visibility_for_object(
    object_id: ObjectID,
    data: &SubObjectsUpgradeModuleData,
) -> bool {
    let Some(object) = OBJECT_REGISTRY.get_object(object_id) else {
        return false;
    };
    let mut object_guard = match object.write() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    apply_subobject_visibility(&mut object_guard, data);
    true
}

fn parse_show_sub_objects(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.show_sub_object_names.push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_hide_sub_objects(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.hide_sub_object_names.push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .activation_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .conflicting_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .removal_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.upgrade_mux_data.requires_all_triggers = INI::parse_bool(value)?;
    Ok(())
}

const SUBOBJECTS_UPGRADE_FIELDS: &[FieldParse<SubObjectsUpgradeModuleData>] = &[
    FieldParse {
        token: "TriggeredBy",
        parse: parse_triggered_by,
    },
    FieldParse {
        token: "ConflictsWith",
        parse: parse_conflicts_with,
    },
    FieldParse {
        token: "RemovesUpgrades",
        parse: parse_removes_upgrades,
    },
    FieldParse {
        token: "RequiresAllTriggers",
        parse: parse_requires_all_triggers,
    },
    FieldParse {
        token: "ShowSubObjects",
        parse: parse_show_sub_objects,
    },
    FieldParse {
        token: "HideSubObjects",
        parse: parse_hide_sub_objects,
    },
];
