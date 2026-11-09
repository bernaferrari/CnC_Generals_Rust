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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<ProneUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        return missing_owner_module("ProneUpdate", module_data);
    };
    let behavior = match ProneUpdate::new(object, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(err) => {
            warn!("ProneUpdate init failed: {err}; installing no-op module");
            return missing_owner_module("ProneUpdate", module_data);
        }
    };
    let module_name = AsciiString::from("ProneUpdate");
    Box::new(ProneUpdateModule::new(behavior, &module_name, module_data))
}
