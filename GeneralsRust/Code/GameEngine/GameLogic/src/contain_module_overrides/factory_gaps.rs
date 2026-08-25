//! Live factories that were never wired into `contain_module_overrides`.
//! Split out so leftover.rs / behavior.rs stay at their current size.

use super::helpers::*;
use super::*;

pub(super) fn prone_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProneUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProneUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn prone_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<ProneUpdateModuleData>("ProneUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("ProneUpdate", data_for_missing);
    };
    let legacy: Arc<dyn crate::common::ModuleData> = data_arc.clone();
    let behavior = match ProneUpdate::new(object, legacy) {
        Ok(behavior) => behavior,
        Err(err) => {
            warn!("ProneUpdate init failed: {err}; installing no-op module");
            let data_for_missing: Arc<dyn ModuleData> = data_arc;
            return missing_owner_module("ProneUpdate", data_for_missing);
        }
    };
    let module_name = AsciiString::from("ProneUpdate");
    Box::new(ProneUpdateModule::new(behavior, &module_name, data_arc))
}
