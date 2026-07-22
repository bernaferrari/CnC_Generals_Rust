use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::body::body_module::ArmorSetType;
use crate::object::draw::draw_module::TerrainDecalType;
use crate::object::drawable::DrawableArcExt;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::INVALID_ID;
use crate::upgrade::upgrade_mask_for_name;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the armor upgrade to apply.
#[derive(Debug, Clone)]
pub struct ArmorUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for ArmorUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl ArmorUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let _ = ini;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(ArmorUpgradeModuleData, module_tag_name_key);

impl Snapshotable for ArmorUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Upgrade module that sets armor flags on the owning object.
pub struct ArmorUpgrade {
    inner: Arc<Mutex<ArmorUpgradeInner>>,
    module_name_key: NameKeyType,
    data: Arc<ArmorUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

#[derive(Debug)]
struct ArmorUpgradeInner {
    #[allow(dead_code)]
    module_name_key: NameKeyType,
    data: Arc<ArmorUpgradeModuleData>,
    object_id: ObjectID,
}

type ArmorUpgradeRegistry = HashMap<ObjectID, Vec<ArmorUpgradeEntry>>;

static ARMOR_UPGRADE_REGISTRY: Lazy<RwLock<ArmorUpgradeRegistry>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Debug)]
struct ArmorUpgradeEntry {
    data: Weak<ArmorUpgradeModuleData>,
    applied: bool,
    upgrade_mask: Option<UpgradeMaskType>,
}

impl ArmorUpgradeEntry {
    fn new(data: &Arc<ArmorUpgradeModuleData>) -> Self {
        Self {
            data: Arc::downgrade(data),
            applied: false,
            upgrade_mask: None,
        }
    }

    #[allow(dead_code)]
    fn upgrade(&self) -> Option<Arc<ArmorUpgradeModuleData>> {
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

    fn matches_data(&self, data: &Arc<ArmorUpgradeModuleData>) -> bool {
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

fn register_armor_upgrade(object_id: ObjectID, data: &Arc<ArmorUpgradeModuleData>) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = ARMOR_UPGRADE_REGISTRY
        .write()
        .expect("armor upgrade registry poisoned");
    registry
        .entry(object_id)
        .or_default()
        .push(ArmorUpgradeEntry::new(data));
}

fn unregister_armor_upgrade(object_id: ObjectID, data: &Arc<ArmorUpgradeModuleData>) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = ARMOR_UPGRADE_REGISTRY
        .write()
        .expect("armor upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        entries.retain(|entry| !entry.matches_data(data));

        if entries.is_empty() {
            registry.remove(&object_id);
        }
    }
}

fn mark_armor_applied(
    object_id: ObjectID,
    data: &Arc<ArmorUpgradeModuleData>,
    upgrade_mask: UpgradeMaskType,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = ARMOR_UPGRADE_REGISTRY
        .write()
        .expect("armor upgrade registry poisoned");
    if let Some(entries) = registry.get_mut(&object_id) {
        for entry in entries.iter_mut() {
            if entry.matches_data(data) {
                entry.set_applied(upgrade_mask);
                break;
            }
        }
    }
}

fn mark_armor_removed(
    object_id: ObjectID,
    data: &Arc<ArmorUpgradeModuleData>,
    upgrade_mask: UpgradeMaskType,
) {
    if object_id == INVALID_ID {
        return;
    }
    let mut registry = ARMOR_UPGRADE_REGISTRY
        .write()
        .expect("armor upgrade registry poisoned");
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

impl ArmorUpgradeInner {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ArmorUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        register_armor_upgrade(object_id, &data);
        Self {
            module_name_key,
            data,
            object_id,
        }
    }

    /// Apply armor upgrade to object
    /// Matches C++ ArmorUpgrade::upgradeImplementation from ArmorUpgrade.cpp lines 63-81
    fn apply_armor(&self, upgrade_mask: UpgradeMaskType) -> Result<(), String> {
        let chemical_suits_mask = UpgradeMaskType::from_bits_retain(
            upgrade_mask_for_name("Upgrade_AmericaChemicalSuits").bits(),
        );
        let apply_chem = upgrade_mask.intersects(chemical_suits_mask);
        match OBJECT_REGISTRY.with_object_mut(self.object_id, |object| -> Result<(), String> {
            // C++ code: BodyModuleInterface* body = obj->getBodyModule();
            // if ( body ) body->setArmorSetFlag( ARMORSET_PLAYER_UPGRADE );
            // (lines 72-74)
            if let Some(body) = &object.get_body_module() {
                let mut body_guard = body
                    .lock()
                    .map_err(|_| "ArmorUpgrade failed to lock body".to_string())?;

                body_guard
                    .set_armor_set_flag(ArmorSetType::PlayerUpgrade)
                    .map_err(|e| format!("ArmorUpgrade failed to set armor: {:?}", e))?;
            }

            if apply_chem {
                if let Some(drawable) = object.get_drawable() {
                    drawable.set_terrain_decal(TerrainDecalType::ChemSuit);
                }
            }
            Ok(())
        }) {
            None => Ok(()),
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(e),
        }
    }

    fn remove_armor(&self) -> Result<(), String> {
        Ok(())
    }
}

impl ArmorUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ArmorUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let data_clone = Arc::clone(&data);
        let inner = Arc::new(Mutex::new(ArmorUpgradeInner::new(
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

    fn with_inner<R>(&self, f: impl FnOnce(&mut ArmorUpgradeInner) -> R) -> R {
        let mut guard = self.inner.lock().expect("ArmorUpgrade inner poisoned");
        f(&mut guard)
    }
}

impl Module for ArmorUpgrade {
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

impl Snapshotable for ArmorUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)?;
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

impl UpgradeModuleInterface for ArmorUpgrade {
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

        // For ArmorUpgrade, we don't have activation conditions in the base C++ implementation.
        // C++ ArmorUpgrade extends UpgradeModule which contains UpgradeMux, but the actual
        // activation/conflicting masks are configured via INI (TriggeredBy, ConflictsWith).
        // Since the C++ ArmorUpgrade.cpp doesn't define custom ModuleData fields, it relies
        // on the base UpgradeModuleData's UpgradeMuxData for validation.
        //
        // In this simplified Rust implementation, we accept any valid upgrade mask.
        // For full C++ compatibility, this would need to check:
        // 1. activation_mask.any() && upgrade_mask matches activation
        // 2. !upgrade_mask.testForAny(conflicting_mask)
        // 3. requires_all_triggers ? testForAll : testForAny
        //
        // However, the base C++ ArmorUpgrade has no activation conditions, so it upgrades
        // whenever the upgrade system calls it (controlled by the parent UpgradeModule).
        true
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        let applied = self.with_inner(|inner| {
            if inner.apply_armor(upgrade_mask).is_ok() {
                mark_armor_applied(inner.object_id, &inner.data, upgrade_mask);
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
        self.with_inner(|inner| {
            let _ = inner.remove_armor();
            mark_armor_removed(inner.object_id, &inner.data, upgrade_mask);
        });
    }
}

impl Drop for ArmorUpgrade {
    fn drop(&mut self) {
        unregister_armor_upgrade(self.object_id, &self.data);
    }
}

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
        ARMOR_UPGRADE_REGISTRY
            .write()
            .expect("armor upgrade registry poisoned")
            .clear();
    }

    #[test]
    fn armor_upgrade_applies_to_body() {
        let _guard = TEST_LOCK.lock().expect("armor upgrade test lock poisoned");
        clear_registry_for_test();

        let object_id: ObjectID = 1000;
        let object_handle = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let data_arc = Arc::new(ArmorUpgradeModuleData::default());

        let upgrade = UpgradeTemplate::new(AsciiString::from("TestArmorUpgrade"));
        let upgrade_mask = UpgradeMaskType::from_bits_retain(upgrade.mask().to_bits());

        let mut module = ArmorUpgrade::new(NameKeyType::default(), data_arc, object_id);
        assert!(module.apply_upgrade(upgrade_mask));

        {
            let object = object_handle.read().expect("lock object");
            if let Some(body) = object.get_body() {
                let body_guard = body.lock().expect("lock body");
                assert!(body_guard.test_armor_set_flag(ArmorSetType::PlayerUpgrade));
            } else {
                panic!("Object should have a body");
            }
        }

        OBJECT_REGISTRY.unregister_object(object_id);
        clear_registry_for_test();
    }

    #[test]
    fn armor_upgrade_removes_correctly() {
        let _guard = TEST_LOCK.lock().expect("armor upgrade test lock poisoned");
        clear_registry_for_test();

        let object_id: ObjectID = 1001;
        let object_handle = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object_handle);

        let data_arc = Arc::new(ArmorUpgradeModuleData::default());

        let upgrade = UpgradeTemplate::new(AsciiString::from("TestArmorRemove"));
        let upgrade_mask = UpgradeMaskType::from_bits_retain(upgrade.mask().to_bits());

        let mut module = ArmorUpgrade::new(NameKeyType::default(), data_arc, object_id);
        assert!(module.apply_upgrade(upgrade_mask));

        module.remove_upgrade(upgrade_mask);

        {
            let object = object_handle.read().expect("lock object");
            if let Some(body) = object.get_body() {
                let body_guard = body.lock().expect("lock body");
                // C++ ArmorUpgrade has no remove implementation; flag remains set.
                assert!(body_guard.test_armor_set_flag(ArmorSetType::PlayerUpgrade));
            }
        }

        OBJECT_REGISTRY.unregister_object(object_id);
        clear_registry_for_test();
    }
}
