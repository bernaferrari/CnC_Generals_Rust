use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data for PassengersFireUpgrade (no custom fields in C++).
#[derive(Debug, Clone)]
pub struct PassengersFireUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for PassengersFireUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl PassengersFireUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PASSENGERS_FIRE_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(
    PassengersFireUpgradeModuleData,
    module_tag_name_key
);

impl Snapshotable for PassengersFireUpgradeModuleData {
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

/// Upgrade module that enables passengers to fire from a container.
pub struct PassengersFireUpgrade {
    inner: Arc<Mutex<PassengersFireUpgradeInner>>,
    module_name_key: NameKeyType,
    data: Arc<PassengersFireUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

#[derive(Debug)]
struct PassengersFireUpgradeInner {
    #[allow(dead_code)]
    data: Arc<PassengersFireUpgradeModuleData>,
    object_id: ObjectID,
}

type PassengersFireUpgradeHandles = HashMap<ObjectID, Vec<Weak<Mutex<PassengersFireUpgradeInner>>>>;

static PASSENGERS_FIRE_UPGRADE_MODULES: Lazy<RwLock<PassengersFireUpgradeHandles>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Handle exposed to object runtime for applying/removing the upgrade.
pub(crate) struct PassengersFireUpgradeHandle {
    inner: Arc<Mutex<PassengersFireUpgradeInner>>,
}

impl PassengersFireUpgradeHandle {
    fn new(inner: Arc<Mutex<PassengersFireUpgradeInner>>) -> Self {
        Self { inner }
    }

    pub fn apply(&self, _mask: UpgradeMaskType) -> bool {
        let guard = self
            .inner
            .lock()
            .expect("PassengersFireUpgrade inner poisoned");
        apply_passengers_fire(guard.object_id)
    }

    pub fn remove(&self, _mask: UpgradeMaskType) {
        // C++ does not revert this upgrade; keep parity by doing nothing.
    }

    pub(crate) fn for_object(object_id: ObjectID) -> Vec<Self> {
        let mut registry = PASSENGERS_FIRE_UPGRADE_MODULES
            .write()
            .expect("passengers fire upgrade registry poisoned");
        if let Some(entries) = registry.get_mut(&object_id) {
            let mut handles = Vec::new();
            entries.retain(|weak| {
                if let Some(upgrade) = weak.upgrade() {
                    handles.push(PassengersFireUpgradeHandle::new(upgrade));
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

fn register_passengers_fire_upgrade(
    object_id: ObjectID,
    inner: &Arc<Mutex<PassengersFireUpgradeInner>>,
) {
    let mut registry = PASSENGERS_FIRE_UPGRADE_MODULES
        .write()
        .expect("passengers fire upgrade registry poisoned");
    registry
        .entry(object_id)
        .or_default()
        .push(Arc::downgrade(inner));
}

fn unregister_passengers_fire_upgrade(
    object_id: ObjectID,
    inner: &Arc<Mutex<PassengersFireUpgradeInner>>,
) {
    let mut registry = PASSENGERS_FIRE_UPGRADE_MODULES
        .write()
        .expect("passengers fire upgrade registry poisoned");
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

fn apply_passengers_fire(object_id: ObjectID) -> bool {
    let Some(object) = OBJECT_REGISTRY.get_object(object_id) else {
        log::warn!("PassengersFireUpgrade: Object {} not found", object_id);
        return true;
    };

    let object_guard = match object.write() {
        Ok(guard) => guard,
        Err(_) => {
            log::error!("PassengersFireUpgrade: Failed to lock object {}", object_id);
            return true;
        }
    };

    let Some(contain) = object_guard.get_contain() else {
        return true;
    };

    if let Ok(mut contain_guard) = contain.lock() {
        contain_guard.set_passenger_allowed_to_fire(true);
    } else {
        log::warn!(
            "PassengersFireUpgrade: Failed to lock contain module for object {}",
            object_id
        );
    }

    true
}

impl PassengersFireUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<PassengersFireUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let inner = Arc::new(Mutex::new(PassengersFireUpgradeInner {
            data: Arc::clone(&data),
            object_id,
        }));
        register_passengers_fire_upgrade(object_id, &inner);
        Self {
            inner,
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }
}

impl Drop for PassengersFireUpgrade {
    fn drop(&mut self) {
        unregister_passengers_fire_upgrade(self.object_id, &self.inner);
    }
}

impl Module for PassengersFireUpgrade {
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
        LegacyModuleData::get_module_tag_name_key(self.data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for PassengersFireUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_state(xfer, &mut self.applied)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for PassengersFireUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        let _ = upgrade_mask;
        let applied = apply_passengers_fire(self.object_id);
        if applied {
            self.applied = true;
        }
        applied
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert this upgrade; keep parity by doing nothing.
    }
}

const PASSENGERS_FIRE_UPGRADE_FIELDS: &[FieldParse<PassengersFireUpgradeModuleData>] = &[];
