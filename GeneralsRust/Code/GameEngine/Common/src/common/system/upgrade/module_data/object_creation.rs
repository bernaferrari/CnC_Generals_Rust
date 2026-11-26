//! Authored ObjectCreationUpgrade rules. The receiving world owns OCL resolution.

use super::{
    FieldParse, INI, INIError, ModuleData, NameKeyType, Snapshotable, UpgradeMuxData, Xfer,
};
use crate::common::ascii_string::AsciiString;
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct ObjectCreationUpgradeDefinition {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    ocl_name: Option<AsciiString>,
}

impl ObjectCreationUpgradeDefinition {
    pub fn ocl_name(&self) -> Option<&str> {
        self.ocl_name.as_ref().map(AsciiString::as_str)
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.parse_from_ini_with_ocl(ini, |_| {})
    }

    /// Notify the caller for every authored OCL reference at its original
    /// position in the block, including repeated fields before a later error.
    pub fn parse_from_ini_with_ocl(
        &mut self,
        ini: &mut INI,
        mut on_ocl: impl FnMut(&str),
    ) -> Result<(), INIError> {
        ini.init_from_ini_with_fields_intercept(self, FIELDS, |_, data, key, tokens| {
            if !key.eq_ignore_ascii_case("UpgradeObject") {
                return Ok(false);
            }
            let name = tokens
                .iter()
                .copied()
                .find(|token| *token != "=")
                .ok_or(INIError::InvalidData)?;
            data.ocl_name = Some(AsciiString::from(name));
            on_ocl(name);
            Ok(true)
        })
    }
}

impl_definition!(
    ObjectCreationUpgradeDefinition,
    fn get_object_creation_upgrade_definition(&self) -> Option<&ObjectCreationUpgradeDefinition> {
        Some(self)
    },
);

const FIELDS: &[FieldParse<ObjectCreationUpgradeDefinition>] =
    upgrade_fields!(ObjectCreationUpgradeDefinition,);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_creation_upgrade_reports_every_ocl_before_a_later_parse_error() {
        let mut definition = ObjectCreationUpgradeDefinition::default();
        let mut names = Vec::new();
        let result = INI::new().with_inline_source(
            "UpgradeObject = OCL_First\nTriggeredBy = Upgrade_Test\nUpgradeObject = OCL_Second\nUnknown = invalid\nEnd\n",
            |ini| definition.parse_from_ini_with_ocl(ini, |name| names.push(name.to_owned())),
        );
        assert_eq!(result, Err(INIError::UnknownToken));
        assert_eq!(names, ["OCL_First", "OCL_Second"]);
        assert_eq!(definition.ocl_name(), Some("OCL_Second"));
        assert!(
            definition
                .upgrade_mux_data
                .definition
                .is_triggered_by("Upgrade_Test")
        );
    }
}
