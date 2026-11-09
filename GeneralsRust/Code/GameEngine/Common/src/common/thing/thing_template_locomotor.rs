//! C++ `ThingTemplate::friend_getAIModuleInfo` + `AIUpdateModuleData::parseLocomotorSet`
//! hook. Common cannot name GameLogic `AIUpdateModuleData`, so GameLogic registers
//! an applier that updates the admitted definition through its shared handle.

use crate::common::rts::AsciiString;
use crate::common::thing::module::ModuleDataHandle;
use std::sync::OnceLock;

/// Apply one top-level `Locomotor = SET_* …` line to the admitted data.
pub type ApplyTemplateLocomotorFn = fn(
    data: ModuleDataHandle,
    set_name: &str,
    names: &[AsciiString],
    overrides_allowed: bool,
) -> Result<ModuleDataHandle, String>;

static APPLY_TEMPLATE_LOCOMOTOR: OnceLock<ApplyTemplateLocomotorFn> = OnceLock::new();

pub fn set_template_locomotor_applier(apply: ApplyTemplateLocomotorFn) {
    let _ = APPLY_TEMPLATE_LOCOMOTOR.set(apply);
}

pub fn template_locomotor_applier() -> Option<ApplyTemplateLocomotorFn> {
    APPLY_TEMPLATE_LOCOMOTOR.get().copied()
}

pub fn apply_locomotor_set_to_module_data(
    data: ModuleDataHandle,
    set_name: &str,
    names: &[AsciiString],
    overrides_allowed: bool,
) -> Result<ModuleDataHandle, String> {
    match template_locomotor_applier() {
        Some(apply) => apply(data, set_name, names, overrides_allowed),
        None => Ok(data),
    }
}
