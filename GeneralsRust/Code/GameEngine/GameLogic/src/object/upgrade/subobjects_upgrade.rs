use std::sync::Arc;

use crate::common::{AsciiString, ObjectID, UpgradeMaskType};
use crate::upgrade::modules::upgrade_mux::UpgradeMuxData;
use game_engine::common::ini::{FieldParse, INI, INIError};
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

/// Mutable execution state belongs to the installed module, not an object-ID registry.
pub struct SubObjectsUpgrade {
    module_name_key: NameKeyType,
    data: Arc<SubObjectsUpgradeModuleData>,
    applied: bool,
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
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            applied: false,
        }
    }

    /// Eligibility is captured under the module borrow. The caller releases it
    /// before FX/removals, which can synchronously reset this very module.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<SubObjectsUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| Arc::clone(&self.data))
    }

    /// C++ giveSelfUpgrade commits execution even if implementation skips the
    /// drawable because a live object/player conflict remains after removals.
    pub(crate) fn finish_upgrade(&mut self, object: &mut crate::object::Object) {
        self.refresh_visibility(object);
        self.applied = true;
    }

    pub(crate) fn refresh_for_object(&self, object: &mut crate::object::Object) {
        if self.applied {
            self.refresh_visibility(object);
        }
    }

    fn refresh_visibility(&self, object: &mut crate::object::Object) {
        let (_, conflicting) = self
            .data
            .upgrade_mux_data
            .clone()
            .get_upgrade_activation_masks();
        let conflicting = UpgradeMaskType::from_bits_retain(conflicting.to_bits());
        if object.completed_upgrades().intersects(conflicting)
            || object.with_controlling_player(|player| {
                player.get_completed_upgrade_mask().intersects(conflicting)
            }) == Some(true)
        {
            return;
        }
        apply_subobject_visibility(object, &self.data);
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ base CRCs are empty; UpgradeMux CRC writes its version and flag.
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        let mut executed = self.applied;
        xfer.xfer_bool(&mut executed).map_err(|e| e.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            "SubObjectsUpgrade",
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl SubObjectsUpgrade {
    pub(crate) fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        crate::object::upgrade::upgrade_module::mux_can_upgrade(
            &self.data.upgrade_mux_data,
            self.applied,
            upgrade_mask,
        )
    }

    pub(crate) fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ resetUpgrade clears execution; it never undoes visibility.
        crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
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
    data.upgrade_mux_data
        .parse_requires_all_triggers_tokens(tokens)
}

fn parse_fx_list_upgrade(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.upgrade_mux_data.parse_fx_list_upgrade_tokens(tokens)
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
        token: "FXListUpgrade",
        parse: parse_fx_list_upgrade,
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

#[cfg(test)]
mod tests;
