//! C++ `ThingTemplate::friend_getAIModuleInfo` + `AIUpdateModuleData::parseLocomotorSet`
//! hook. Common cannot name GameLogic `AIUpdateModuleData`, so GameLogic registers
//! an applier that clones, writes locomotor names, and returns the updated Arc.

use crate::common::rts::AsciiString;
use crate::common::thing::module::ModuleData;
use std::cell::Cell;
use std::sync::{Arc, OnceLock};

/// Rewrite an AI module-data Arc with one top-level `Locomotor = SET_* …` line.
pub type ApplyTemplateLocomotorFn = fn(
    data: Arc<dyn ModuleData>,
    set_name: &str,
    names: &[AsciiString],
) -> Result<Arc<dyn ModuleData>, String>;

static APPLY_TEMPLATE_LOCOMOTOR: OnceLock<ApplyTemplateLocomotorFn> = OnceLock::new();

thread_local! {
    static LOCOMOTOR_OVERRIDES_ALLOWED: Cell<bool> = const { Cell::new(false) };
}

pub fn set_template_locomotor_applier(apply: ApplyTemplateLocomotorFn) {
    let _ = APPLY_TEMPLATE_LOCOMOTOR.set(apply);
}

pub fn template_locomotor_applier() -> Option<ApplyTemplateLocomotorFn> {
    APPLY_TEMPLATE_LOCOMOTOR.get().copied()
}

/// C++ `ini->getLoadType() == INI_LOAD_CREATE_OVERRIDES`.
pub fn set_locomotor_overrides_allowed(allowed: bool) {
    LOCOMOTOR_OVERRIDES_ALLOWED.with(|cell| cell.set(allowed));
}

pub fn locomotor_overrides_allowed() -> bool {
    LOCOMOTOR_OVERRIDES_ALLOWED.with(|cell| cell.get())
}

pub fn apply_locomotor_set_to_module_data(
    data: Arc<dyn ModuleData>,
    set_name: &str,
    names: &[AsciiString],
) -> Result<Arc<dyn ModuleData>, String> {
    match template_locomotor_applier() {
        Some(apply) => apply(data, set_name, names),
        None => Ok(data),
    }
}
