use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::{AsciiString, LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::INVALID_ID;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the max health increase to apply.
#[derive(Debug, Clone)]
pub struct MaxHealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    add_max_health: Real,
    change_type: MaxHealthChangeType,
}

impl Default for MaxHealthUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            add_max_health: 0.0,
            change_type: MaxHealthChangeType::SameCurrentHealth,
        }
    }
}

impl MaxHealthUpgradeModuleData {
    pub fn add_max_health(&self) -> Real {
        self.add_max_health
    }

    pub fn change_type(&self) -> MaxHealthChangeType {
        self.change_type
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MAX_HEALTH_UPGRADE_FIELDS)
    }

    pub fn set_add_max_health(&mut self, value: Real) {
        self.add_max_health = value;
    }

    pub fn set_change_type(&mut self, type_str: &str) -> Result<(), String> {
        self.change_type = match type_str.to_uppercase().as_str() {
            "SAME_CURRENTHEALTH" => MaxHealthChangeType::SameCurrentHealth,
            "PRESERVE_RATIO" => MaxHealthChangeType::PreserveRatio,
            "ADD_CURRENT_HEALTH_TOO" => MaxHealthChangeType::AddCurrentHealthToo,
            _ => return Err(format!("Unknown change type: {}", type_str)),
        };
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(MaxHealthUpgradeModuleData, module_tag_name_key);

impl Snapshotable for MaxHealthUpgradeModuleData {
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

/// Upgrade module that increases max health on the owning object.
pub struct MaxHealthUpgrade {
    inner: Arc<Mutex<MaxHealthUpgradeInner>>,
    module_name_key: NameKeyType,
    data: Arc<MaxHealthUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

#[derive(Debug)]
struct MaxHealthUpgradeInner {
    #[allow(dead_code)]
    module_name_key: NameKeyType,
    data: Arc<MaxHealthUpgradeModuleData>,
    object_id: ObjectID,
    original_max_health: Option<Real>,
}

type MaxHealthUpgradeRegistry = HashMap<ObjectID, Vec<MaxHealthUpgradeEntry>>;

static MAX_HEALTH_UPGRADE_REGISTRY: Lazy<RwLock<MaxHealthUpgradeRegistry>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Debug)]
struct MaxHealthUpgradeEntry {
    data: Weak<MaxHealthUpgradeModuleData>,
    applied: bool,
    upgrade_mask: Option<UpgradeMaskType>,
}

impl MaxHealthUpgradeEntry {
    fn new(data: &Arc<MaxHealthUpgradeModuleData>) -> Self {
        Self {
            data: Arc::downgrade(data),
            applied: false,
            upgrade_mask: None,
        }
    }

    #[allow(dead_code)]
    fn upgrade(&self) -> Option<Arc<MaxHealthUpgradeModuleData>> {
        self.data.upgrade()
    }

    #[allow(dead_code)]
    fn applied(&self) -> bool {
        self.applied
    }

    fn set_applied(&mut self, mask: UpgradeMaskType) {
        self.applied = true;
        self.upgrade_mask = Some(mask);
    }

    fn matches_data(&self, data: &Arc<MaxHealthUpgradeModuleData>) -> bool {
        if let Some(existing) = self.data.upgrade() {
            Arc::ptr_eq(&existing, data)
        } else {
            false
        }
    }

    fn clear_applied(&mut self) {
        self.applied = false;
        self.upgrade_mask = None;
    }
}

fn register_max_health_upgrade(object_id: ObjectID, data: &Arc<MaxHealthUpgradeModuleData>) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = MAX_HEALTH_UPGRADE_REGISTRY
        .write()
        .expect("max health upgrade registry poisoned");
    registry
        .entry(object_id)
        .or_default()
        .push(MaxHealthUpgradeEntry::new(data));
}

fn unregister_max_health_upgrade(object_id: ObjectID, data: &Arc<MaxHealthUpgradeModuleData>) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = MAX_HEALTH_UPGRADE_REGISTRY
        .write()
        .expect("max health upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        entries.retain(|entry| !entry.matches_data(data));

        if entries.is_empty() {
            registry.remove(&object_id);
        }
    }
}

fn mark_max_health_applied(
    object_id: ObjectID,
    data: &Arc<MaxHealthUpgradeModuleData>,
    upgrade_mask: UpgradeMaskType,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = MAX_HEALTH_UPGRADE_REGISTRY
        .write()
        .expect("max health upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        for entry in entries.iter_mut() {
            if entry.matches_data(data) {
                entry.set_applied(upgrade_mask);
                break;
            }
        }
    }
}

#[allow(dead_code)]
fn mark_max_health_removed(
    object_id: ObjectID,
    data: &Arc<MaxHealthUpgradeModuleData>,
    upgrade_mask: UpgradeMaskType,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = MAX_HEALTH_UPGRADE_REGISTRY
        .write()
        .expect("max health upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        for entry in entries.iter_mut() {
            if entry.matches_data(data) {
                if let Some(mask) = entry.upgrade_mask {
                    if mask == upgrade_mask {
                        entry.clear_applied();
                        break;
                    }
                }
            }
        }
    }
}

impl MaxHealthUpgradeInner {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<MaxHealthUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        register_max_health_upgrade(object_id, &data);
        Self {
            module_name_key,
            data,
            object_id,
            original_max_health: None,
        }
    }

    /// Apply max health upgrade to object
    /// Matches C++ MaxHealthUpgrade::upgradeImplementation from MaxHealthUpgrade.cpp lines 56-68
    fn apply_max_health(&mut self) -> Result<(), String> {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            // C++ doesn't explicitly check for null object in MaxHealthUpgrade,
            // but getObject() would return null if not found and we'd just skip the upgrade
            return Ok(());
        };

        let object = object
            .write()
            .map_err(|_| "MaxHealthUpgrade failed to lock object for writing".to_string())?;

        // C++ code (lines 56-68):
        // const MaxHealthUpgradeModuleData *data = getMaxHealthUpgradeModuleData();
        // Object *obj = getObject();
        // BodyModuleInterface *body = obj->getBodyModule();
        // if( body ) {
        //     body->setMaxHealth( body->getMaxHealth() + data->m_addMaxHealth, data->m_maxHealthChangeType );
        // }
        if let Some(body) = &object.get_body() {
            let mut body_guard = body
                .lock()
                .map_err(|_| "MaxHealthUpgrade failed to lock body".to_string())?;

            // Store original max health for later restoration (Rust enhancement for remove_upgrade)
            // C++ doesn't have upgrade removal, so this is a Rust-specific feature
            if self.original_max_health.is_none() {
                self.original_max_health = Some(body_guard.get_max_health());
            }

            // Match C++ exactly: body->setMaxHealth( body->getMaxHealth() + data->m_addMaxHealth, data->m_maxHealthChangeType )
            let current_max = body_guard.get_max_health();
            let new_max = current_max + self.data.add_max_health();

            body_guard
                .set_max_health(new_max, self.data.change_type())
                .map_err(|e| format!("MaxHealthUpgrade failed to set max health: {:?}", e))?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn remove_max_health(&mut self) -> Result<(), String> {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(format!(
                "MaxHealthUpgrade could not find object {} in registry",
                self.object_id
            ));
        };

        let object = object
            .write()
            .map_err(|_| "MaxHealthUpgrade failed to lock object for writing".to_string())?;

        if let Some(body) = &object.get_body() {
            let mut body_guard = body
                .lock()
                .map_err(|_| "MaxHealthUpgrade failed to lock body".to_string())?;

            // Restore original max health if we have it stored
            if let Some(original) = self.original_max_health {
                body_guard
                    .set_max_health(original, self.data.change_type())
                    .map_err(|e| {
                        format!("MaxHealthUpgrade failed to restore max health: {:?}", e)
                    })?;
                self.original_max_health = None;
            } else {
                // Otherwise just subtract what we added
                let current_max = body_guard.get_max_health();
                let new_max = current_max - self.data.add_max_health();
                body_guard
                    .set_max_health(new_max, self.data.change_type())
                    .map_err(|e| {
                        format!("MaxHealthUpgrade failed to reduce max health: {:?}", e)
                    })?;
            }
        }

        Ok(())
    }
}

impl MaxHealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<MaxHealthUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let data_clone = Arc::clone(&data);
        let inner = Arc::new(Mutex::new(MaxHealthUpgradeInner::new(
            module_name_key,
            data,
            object_id,
        )));
        Self {
            inner,
            module_name_key,
            data: data_clone,
            object_id,
            applied: false,
        }
    }

    fn with_inner<R>(&self, f: impl FnOnce(&mut MaxHealthUpgradeInner) -> R) -> R {
        let mut guard = self.inner.lock().expect("MaxHealthUpgrade inner poisoned");
        f(&mut guard)
    }
}

impl Module for MaxHealthUpgrade {
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

impl Snapshotable for MaxHealthUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_state(xfer, &mut self.applied)?;
        if version >= 2 {
            let mut has_original: bool = false;
            let mut original_val: f32 = 0.0;
            if let Ok(guard) = self.inner.lock() {
                if let Some(val) = guard.original_max_health {
                    has_original = true;
                    original_val = val;
                }
            }
            xfer.xfer_bool(&mut has_original)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut original_val)
                .map_err(|e| e.to_string())?;
            if xfer.is_reading() {
                if let Ok(mut guard) = self.inner.lock() {
                    guard.original_max_health = if has_original {
                        Some(original_val)
                    } else {
                        None
                    };
                }
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for MaxHealthUpgrade {
    /// Check if upgrade can be applied
    /// Matches C++ UpgradeMux::wouldUpgrade logic from UpgradeModule.cpp lines 105-137
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        // If no upgrade mask provided, cannot upgrade
        // Note: UpgradeMaskType is a bitflags type, use is_empty() to check
        if upgrade_mask.is_empty() {
            return false;
        }

        // For MaxHealthUpgrade, like ArmorUpgrade, the C++ implementation doesn't define
        // custom ModuleData fields. It relies on the base UpgradeModuleData's UpgradeMuxData
        // for validation logic (activation masks, conflicting masks, requires_all_triggers).
        //
        // C++ MaxHealthUpgrade.cpp (lines 44-68) shows:
        // - Extends UpgradeModule (which contains UpgradeMux)
        // - Has custom ModuleData (MaxHealthUpgradeModuleData) with m_addMaxHealth, m_maxHealthChangeType
        // - The upgradeImplementation just adds health, no condition checking
        //
        // The validation is handled by the parent UpgradeMux::wouldUpgrade which checks:
        // 1. activation_mask.any() && upgrade_mask matches activation
        // 2. !upgrade_mask.testForAny(conflicting_mask)
        // 3. requires_all_triggers ? testForAll : testForAny
        // 4. !m_upgradeExecuted (not already upgraded)
        //
        // In this simplified Rust implementation, we accept any valid upgrade mask.
        // The full validation would be implemented in a containing UpgradeMux.
        true
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        let applied = self.with_inner(|inner| {
            if inner.apply_max_health().is_ok() {
                mark_max_health_applied(inner.object_id, &inner.data, upgrade_mask);
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
        // C++ does not remove max health bonuses once applied; keep parity.
    }
}

impl Drop for MaxHealthUpgrade {
    fn drop(&mut self) {
        unregister_max_health_upgrade(self.object_id, &self.data);
    }
}

fn parse_add_max_health_field(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.add_max_health = tokens[0]
        .parse::<Real>()
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_change_type_field(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.set_change_type(tokens[0])
        .map_err(|_| INIError::InvalidData)
}

const MAX_HEALTH_UPGRADE_FIELDS: &[FieldParse<MaxHealthUpgradeModuleData>] = &[
    FieldParse {
        token: "AddMaxHealth",
        parse: parse_add_max_health_field,
    },
    FieldParse {
        token: "ChangeType",
        parse: parse_change_type_field,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::UpgradeMaskType;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use crate::upgrade::UpgradeTemplate;
    use game_engine::common::thing::module::NameKeyType;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use std::sync::{Arc, RwLock};

    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn clear_registry_for_test() {
        MAX_HEALTH_UPGRADE_REGISTRY
            .write()
            .expect("max health upgrade registry poisoned")
            .clear();
    }

    #[test]
    fn max_health_upgrade_adds_health() {
        let _guard = TEST_LOCK
            .lock()
            .expect("max health upgrade test lock poisoned");
        clear_registry_for_test();

        let object_id: ObjectID = 2000;
        let object_handle = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let mut data = MaxHealthUpgradeModuleData::default();
        data.add_max_health = 50.0;
        data.change_type = MaxHealthChangeType::SameCurrentHealth;
        let data_arc = Arc::new(data);

        let upgrade = UpgradeTemplate::new(AsciiString::from("TestMaxHealthUpgrade"));
        let upgrade_mask = UpgradeMaskType::from_bits_retain(upgrade.mask().to_bits());

        let mut module = MaxHealthUpgrade::new(NameKeyType::default(), data_arc, object_id);
        assert!(module.apply_upgrade(upgrade_mask));

        {
            let object = object_handle.read().expect("lock object");
            if let Some(body) = object.get_body() {
                let body_guard = body.lock().expect("lock body");
                assert_eq!(body_guard.get_max_health(), 150.0);
            } else {
                panic!("Object should have a body");
            }
        }

        OBJECT_REGISTRY.unregister_object(object_id);
        clear_registry_for_test();
    }

    #[test]
    fn max_health_upgrade_removes_correctly() {
        let _guard = TEST_LOCK
            .lock()
            .expect("max health upgrade test lock poisoned");
        clear_registry_for_test();

        let object_id: ObjectID = 2001;
        let object_handle = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let mut data = MaxHealthUpgradeModuleData::default();
        data.add_max_health = 75.0;
        data.change_type = MaxHealthChangeType::PreserveRatio;
        let data_arc = Arc::new(data);

        let upgrade = UpgradeTemplate::new(AsciiString::from("TestMaxHealthRemove"));
        let upgrade_mask = UpgradeMaskType::from_bits_retain(upgrade.mask().to_bits());

        let mut module = MaxHealthUpgrade::new(NameKeyType::default(), data_arc, object_id);
        assert!(module.apply_upgrade(upgrade_mask));

        module.remove_upgrade(upgrade_mask);

        {
            let object = object_handle.read().expect("lock object");
            if let Some(body) = object.get_body() {
                let body_guard = body.lock().expect("lock body");
                // C++ MaxHealthUpgrade only applies on gain and does not roll back.
                assert_eq!(body_guard.get_max_health(), 175.0);
            }
        }

        OBJECT_REGISTRY.unregister_object(object_id);
        clear_registry_for_test();
    }
}
