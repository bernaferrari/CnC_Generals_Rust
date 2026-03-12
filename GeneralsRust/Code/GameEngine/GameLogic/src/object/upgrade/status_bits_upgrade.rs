use once_cell::sync::Lazy;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::{
    AsciiString, LegacyModuleData, ObjectID, ObjectStatusMaskType, UpgradeMaskType,
};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::object::INVALID_ID;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the status bits to set/clear when an upgrade is applied.
#[derive(Debug, Clone)]
pub struct StatusBitsUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    status_to_set: ObjectStatusMaskType,
    status_to_clear: ObjectStatusMaskType,
}

impl Default for StatusBitsUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            status_to_set: ObjectStatusMaskType::none(),
            status_to_clear: ObjectStatusMaskType::none(),
        }
    }
}

impl StatusBitsUpgradeModuleData {
    pub fn status_to_set(&self) -> ObjectStatusMaskType {
        self.status_to_set
    }

    pub fn status_to_clear(&self) -> ObjectStatusMaskType {
        self.status_to_clear
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STATUS_BITS_UPGRADE_FIELDS)
    }

    pub fn set_status_to_set_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.status_to_set = parse_status_tokens(tokens)?;
        Ok(())
    }

    pub fn set_status_to_clear_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.status_to_clear = parse_status_tokens(tokens)?;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(StatusBitsUpgradeModuleData, module_tag_name_key);

impl Snapshotable for StatusBitsUpgradeModuleData {
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

/// Upgrade module that sets/clears status bits on the owning object.
pub struct StatusBitsUpgrade {
    inner: Arc<Mutex<StatusBitsUpgradeInner>>,
    module_name_key: NameKeyType,
    data: Arc<StatusBitsUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

#[derive(Debug)]
struct StatusBitsUpgradeInner {
    module_name_key: NameKeyType,
    data: Arc<StatusBitsUpgradeModuleData>,
    object_id: ObjectID,
}

type StatusUpgradeRegistry = HashMap<ObjectID, Vec<StatusUpgradeEntry>>;

static STATUS_UPGRADE_REGISTRY: Lazy<RwLock<StatusUpgradeRegistry>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

type StatusUpgradeModuleHandles = HashMap<ObjectID, Vec<Weak<Mutex<StatusBitsUpgradeInner>>>>;

static STATUS_UPGRADE_MODULES: Lazy<RwLock<StatusUpgradeModuleHandles>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Handle exposed to object runtime for applying or removing status bits upgrades.
pub(crate) struct StatusBitsUpgradeHandle {
    inner: Arc<Mutex<StatusBitsUpgradeInner>>,
}

impl StatusBitsUpgradeHandle {
    fn new(inner: Arc<Mutex<StatusBitsUpgradeInner>>) -> Self {
        Self { inner }
    }

    pub fn apply(&self, mask: UpgradeMaskType) -> bool {
        let guard = self.inner.lock().expect("StatusBitsUpgrade inner poisoned");
        mark_status_bits_applied(guard.object_id, &guard.data, mask);
        true
    }

    pub fn remove(&self, mask: UpgradeMaskType) {
        let _ = mask;
        // C++ does not revert status bits when the upgrade is removed; keep parity.
    }

    pub(crate) fn for_object(object_id: ObjectID) -> Vec<Self> {
        if object_id == INVALID_ID {
            return Vec::new();
        }
        let mut registry = STATUS_UPGRADE_MODULES
            .write()
            .expect("status bits upgrade module registry poisoned");
        if let Some(entries) = registry.get_mut(&object_id) {
            let mut handles = Vec::new();
            entries.retain(|weak| {
                if let Some(upgrade) = weak.upgrade() {
                    handles.push(StatusBitsUpgradeHandle::new(upgrade));
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

#[derive(Debug)]
struct StatusUpgradeEntry {
    data: Weak<StatusBitsUpgradeModuleData>,
    applied: bool,
    upgrade_mask: Option<UpgradeMaskType>,
    pending_clear: ObjectStatusMaskType,
    pending_restore: ObjectStatusMaskType,
}

impl StatusUpgradeEntry {
    fn new(data: &Arc<StatusBitsUpgradeModuleData>) -> Self {
        Self {
            data: Arc::downgrade(data),
            applied: false,
            upgrade_mask: None,
            pending_clear: ObjectStatusMaskType::none(),
            pending_restore: ObjectStatusMaskType::none(),
        }
    }

    fn upgrade(&self) -> Option<Arc<StatusBitsUpgradeModuleData>> {
        self.data.upgrade()
    }

    fn applied(&self) -> bool {
        self.applied
    }

    fn set_applied(&mut self, mask: UpgradeMaskType) {
        self.applied = true;
        self.upgrade_mask = Some(mask);
    }

    fn matches_data(&self, data: &Arc<StatusBitsUpgradeModuleData>) -> bool {
        if let Some(existing) = self.data.upgrade() {
            Arc::ptr_eq(&existing, data)
        } else {
            false
        }
    }

    fn is_active(&self, active_mask: UpgradeMaskType) -> bool {
        match self.upgrade_mask {
            Some(mask) => active_mask.contains(mask),
            None => true,
        }
    }

    fn clear_applied(&mut self) {
        self.applied = false;
        self.upgrade_mask = None;
    }
}

fn register_status_bits_upgrade(object_id: ObjectID, data: &Arc<StatusBitsUpgradeModuleData>) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = STATUS_UPGRADE_REGISTRY
        .write()
        .expect("status bits upgrade registry poisoned");
    registry
        .entry(object_id)
        .or_default()
        .push(StatusUpgradeEntry::new(data));
}

fn unregister_status_bits_upgrade(object_id: ObjectID, data: &Arc<StatusBitsUpgradeModuleData>) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = STATUS_UPGRADE_REGISTRY
        .write()
        .expect("status bits upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        entries.retain(|entry| !entry.matches_data(data));

        if entries.is_empty() {
            registry.remove(&object_id);
        }
    }
}

fn mark_status_bits_applied(
    object_id: ObjectID,
    data: &Arc<StatusBitsUpgradeModuleData>,
    upgrade_mask: UpgradeMaskType,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = STATUS_UPGRADE_REGISTRY
        .write()
        .expect("status bits upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        for entry in entries.iter_mut() {
            if entry.matches_data(data) {
                entry.set_applied(upgrade_mask);
                entry.pending_clear = ObjectStatusMaskType::none();
                entry.pending_restore = ObjectStatusMaskType::none();
                break;
            }
        }
    }
}

fn mark_status_bits_removed(
    object_id: ObjectID,
    data: &Arc<StatusBitsUpgradeModuleData>,
    upgrade_mask: UpgradeMaskType,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = STATUS_UPGRADE_REGISTRY
        .write()
        .expect("status bits upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        for entry in entries.iter_mut() {
            if entry.matches_data(data) {
                if let Some(mask) = entry.upgrade_mask {
                    if mask == upgrade_mask {
                        if let Some(data) = entry.upgrade() {
                            entry.pending_clear |= data.status_to_set();
                            entry.pending_restore |= data.status_to_clear();
                        }
                        entry.clear_applied();
                        break;
                    }
                }
            }
        }
    }
}

fn register_status_bits_upgrade_module(
    object_id: ObjectID,
    handle: Weak<Mutex<StatusBitsUpgradeInner>>,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = STATUS_UPGRADE_MODULES
        .write()
        .expect("status bits upgrade module registry poisoned");
    registry.entry(object_id).or_default().push(handle);
}

fn unregister_status_bits_upgrade_module(
    object_id: ObjectID,
    handle: Weak<Mutex<StatusBitsUpgradeInner>>,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = STATUS_UPGRADE_MODULES
        .write()
        .expect("status bits upgrade module registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        let target_ptr = handle.as_ptr();
        entries.retain(|weak| weak.as_ptr() != target_ptr && weak.upgrade().is_some());
        if entries.is_empty() {
            registry.remove(&object_id);
        }
    }
}

struct AggregatedStatusMasks {
    active_set: ObjectStatusMaskType,
    active_clear: ObjectStatusMaskType,
    inactive_set: ObjectStatusMaskType,
    inactive_restore: ObjectStatusMaskType,
}

fn aggregate_registered_masks(object: &Object) -> AggregatedStatusMasks {
    let object_id = object.get_object_id();
    let active_mask = object.completed_upgrades();
    let mut registry = STATUS_UPGRADE_REGISTRY
        .write()
        .expect("status bits upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        let mut set_mask = ObjectStatusMaskType::none();
        let mut clear_mask = ObjectStatusMaskType::none();
        let mut inactive_mask = ObjectStatusMaskType::none();
        let mut restore_mask = ObjectStatusMaskType::none();

        entries.retain_mut(|entry| {
            if let Some(data) = entry.upgrade() {
                if !entry.pending_clear.is_empty() {
                    inactive_mask |= entry.pending_clear;
                    entry.pending_clear = ObjectStatusMaskType::none();
                }
                if !entry.pending_restore.is_empty() {
                    restore_mask |= entry.pending_restore;
                    entry.pending_restore = ObjectStatusMaskType::none();
                }
                if entry.applied() {
                    set_mask |= data.status_to_set();
                    clear_mask |= data.status_to_clear();
                }
                true
            } else {
                false
            }
        });

        if entries.is_empty() {
            registry.remove(&object_id);
        }

        let inactive_set = inactive_mask & !set_mask;
        let inactive_restore = restore_mask & !clear_mask;
        AggregatedStatusMasks {
            active_set: set_mask,
            active_clear: clear_mask,
            inactive_set,
            inactive_restore,
        }
    } else {
        AggregatedStatusMasks {
            active_set: ObjectStatusMaskType::none(),
            active_clear: ObjectStatusMaskType::none(),
            inactive_set: ObjectStatusMaskType::none(),
            inactive_restore: ObjectStatusMaskType::none(),
        }
    }
}

pub(crate) fn apply_registered_status_upgrades(object: &mut Object) {
    let masks = aggregate_registered_masks(object);

    if !masks.active_set.is_empty() {
        object.set_status(masks.active_set, true);
    }

    if !masks.active_clear.is_empty() {
        object.clear_status(masks.active_clear);
    }

    if !masks.inactive_restore.is_empty() {
        object.set_status(masks.inactive_restore, true);
    }
}

impl StatusBitsUpgradeInner {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<StatusBitsUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        register_status_bits_upgrade(object_id, &data);
        Self {
            module_name_key,
            data,
            object_id,
        }
    }

    fn apply_status_bits(&self) -> Result<(), String> {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(format!(
                "StatusBitsUpgrade could not find object {} in registry",
                self.object_id
            ));
        };

        let mut object = object
            .write()
            .map_err(|_| "StatusBitsUpgrade failed to lock object for writing".to_string())?;

        Self::apply_masks_to_object(self.data.as_ref(), &mut object);
        Ok(())
    }

    fn apply_masks_to_object(data: &StatusBitsUpgradeModuleData, object: &mut Object) {
        let set_mask = data.status_to_set();
        if !set_mask.is_empty() {
            object.set_status(set_mask, true);
        }

        let clear_mask = data.status_to_clear();
        if !clear_mask.is_empty() {
            object.clear_status(clear_mask);
        }
    }
}

impl StatusBitsUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StatusBitsUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let data_clone = Arc::clone(&data);
        let inner = Arc::new(Mutex::new(StatusBitsUpgradeInner::new(
            module_name_key,
            data,
            object_id,
        )));
        register_status_bits_upgrade_module(object_id, Arc::downgrade(&inner));
        Self {
            inner,
            module_name_key,
            data: data_clone,
            object_id,
            applied: false,
        }
    }

    fn with_inner<R>(&self, f: impl FnOnce(&mut StatusBitsUpgradeInner) -> R) -> R {
        let mut guard = self.inner.lock().expect("StatusBitsUpgrade inner poisoned");
        f(&mut guard)
    }
}

impl Module for StatusBitsUpgrade {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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

impl Snapshotable for StatusBitsUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        let _ = _xfer.xfer_version(&mut version, 1);
        let mut applied = self.applied;
        let _ = _xfer.xfer_bool(&mut applied);
        self.applied = applied;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for StatusBitsUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        let applied = self.with_inner(|inner| {
            if inner.apply_status_bits().is_ok() {
                mark_status_bits_applied(inner.object_id, &inner.data, upgrade_mask);
                true
            } else {
                false
            }
        });
        if applied {
            self.applied = true;
        }
        applied
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        let _ = upgrade_mask;
        // C++ does not revert status bits when the upgrade is removed; keep parity.
    }
}

impl Drop for StatusBitsUpgrade {
    fn drop(&mut self) {
        unregister_status_bits_upgrade(self.object_id, &self.data);
        unregister_status_bits_upgrade_module(self.object_id, Arc::downgrade(&self.inner));
    }
}

fn parse_status_tokens(tokens: &[&str]) -> Result<ObjectStatusMaskType, String> {
    if tokens.is_empty() {
        return Ok(ObjectStatusMaskType::none());
    }

    let normalized: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    ObjectStatusMaskType::parse_tokens(normalized)
}

fn parse_status_to_set_field(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_status_to_set_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_status_to_clear_field(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_status_to_clear_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

const STATUS_BITS_UPGRADE_FIELDS: &[FieldParse<StatusBitsUpgradeModuleData>] = &[
    FieldParse {
        token: "StatusToSet",
        parse: parse_status_to_set_field,
    },
    FieldParse {
        token: "StatusToClear",
        parse: parse_status_to_clear_field,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::upgrade::UpgradeTemplate;
    use game_engine::common::thing::module::NameKeyType;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use std::sync::{Arc, RwLock};

    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn clear_registry_for_test() {
        STATUS_UPGRADE_REGISTRY
            .write()
            .expect("status bits upgrade registry poisoned")
            .clear();
    }

    fn clear_module_registry_for_test() {
        STATUS_UPGRADE_MODULES
            .write()
            .expect("status bits upgrade module registry poisoned")
            .clear();
    }

    #[test]
    fn status_bits_upgrade_data_parses_tokens() {
        let _guard = TEST_LOCK
            .lock()
            .expect("status bits upgrade test lock poisoned");
        clear_module_registry_for_test();
        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED", "DETECTED"])
            .expect("parse set mask");
        data.set_status_to_clear_from_tokens(&["+MASKED", "-MASKED"])
            .expect("parse clear mask");

        assert!(data
            .status_to_set()
            .contains(ObjectStatusMaskType::STEALTHED));
        assert!(data
            .status_to_set()
            .contains(ObjectStatusMaskType::DETECTED));
        assert!(data.status_to_clear().is_empty());
    }

    #[test]
    fn status_bits_upgrade_applies_masks_to_object() {
        let _guard = TEST_LOCK
            .lock()
            .expect("status bits upgrade test lock poisoned");
        let object_id: ObjectID = 1337;
        clear_module_registry_for_test();
        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED"]).unwrap();
        data.set_status_to_clear_from_tokens(&["MASKED"]).unwrap();
        let data = Arc::new(data);

        let mut test_object = Object::new_test(object_id, 100.0);
        test_object.set_status(ObjectStatusMaskType::MASKED, true);
        let object_handle = Arc::new(RwLock::new(test_object));
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let mut module = StatusBitsUpgrade::new(NameKeyType::default(), data.clone(), object_id);
        assert!(module.apply_upgrade(UpgradeMaskType::from_bits_retain(1u128)));

        let object = OBJECT_REGISTRY
            .get_object(object_id)
            .expect("object should remain registered");
        let object = object.read().expect("lock object for inspection");
        let status = object.get_status_bits();
        assert!(status.contains(ObjectStatusMaskType::STEALTHED));
        assert!(!status.contains(ObjectStatusMaskType::MASKED));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn status_bits_upgrade_runtime_applies_on_object_upgrade() {
        let _guard = TEST_LOCK
            .lock()
            .expect("status bits upgrade test lock poisoned");
        clear_registry_for_test();
        clear_module_registry_for_test();

        let object_id: ObjectID = 9001;
        let object_handle = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        {
            let mut object = object_handle.write().expect("lock object");
            object.set_status(ObjectStatusMaskType::MASKED, true);
        }
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED"])
            .expect("set mask parsed");
        data.set_status_to_clear_from_tokens(&["MASKED"])
            .expect("clear mask parsed");
        let data_arc = Arc::new(data);

        let upgrade = UpgradeTemplate::new(AsciiString::from("TestStatusUpgrade"));
        let upgrade_mask = UpgradeMaskType::from_bits_retain(upgrade.mask().to_bits());

        let mut module = StatusBitsUpgrade::new(NameKeyType::default(), data_arc, object_id);
        assert!(module.apply_upgrade(upgrade_mask));

        {
            let mut object = object_handle.write().expect("lock object");
            object.clear_status(ObjectStatusMaskType::STEALTHED);
            object.set_status(ObjectStatusMaskType::MASKED, true);

            object.give_upgrade(&upgrade);

            let status = object.get_status_bits();
            assert!(status.contains(ObjectStatusMaskType::STEALTHED));
            assert!(!status.contains(ObjectStatusMaskType::MASKED));
        }

        OBJECT_REGISTRY.unregister_object(object_id);

        clear_registry_for_test();
        clear_module_registry_for_test();
    }

    #[test]
    fn status_bits_upgrade_flags_clear_when_upgrade_removed() {
        let _guard = TEST_LOCK
            .lock()
            .expect("status bits upgrade test lock poisoned");
        clear_registry_for_test();
        clear_module_registry_for_test();

        let object_id: ObjectID = 9002;
        let object_handle = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED"]).unwrap();
        data.set_status_to_clear_from_tokens(&["MASKED"]).unwrap();
        let data_arc = Arc::new(data);

        let upgrade = UpgradeTemplate::new(AsciiString::from("TestStatusUpgradeClear"));
        let upgrade_mask = UpgradeMaskType::from_bits_retain(upgrade.mask().to_bits());

        let mut module = StatusBitsUpgrade::new(NameKeyType::default(), data_arc, object_id);
        assert!(module.apply_upgrade(upgrade_mask));

        {
            let mut object = object_handle.write().expect("lock object");
            object.set_status(ObjectStatusMaskType::MASKED, true);
            object.give_upgrade(&upgrade);
            assert!(object
                .get_status_bits()
                .contains(ObjectStatusMaskType::STEALTHED));

            object.remove_upgrade(&upgrade);
            let status = object.get_status_bits();
            // C++ StatusBitsUpgrade does not revert status bits on remove.
            assert!(status.contains(ObjectStatusMaskType::STEALTHED));
            assert!(!status.contains(ObjectStatusMaskType::MASKED));
        }

        OBJECT_REGISTRY.unregister_object(object_id);
        clear_registry_for_test();
        clear_module_registry_for_test();
    }
}
