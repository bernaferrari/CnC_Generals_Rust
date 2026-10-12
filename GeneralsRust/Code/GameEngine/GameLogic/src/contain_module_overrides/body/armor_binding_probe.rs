//! Test-only observation of the actual sealed installed body binding.
use super::*;

pub(crate) fn with_active<R>(
    module: &dyn Module,
    cached: &Arc<Mutex<dyn BodyModuleInterface>>,
    operation: impl FnOnce(&ActiveBody) -> R,
) -> R {
    macro_rules! read_body {
        ($data:ty, $body:ty $(, $active:ident)*) => {
            if let Some(binding) = module.as_any().downcast_ref::<BodyBindingModule<$data, $body>>() {
                let exact: Arc<Mutex<dyn BodyModuleInterface>> = binding.body.clone();
                assert!(Arc::ptr_eq(&exact, cached), "wrapper and cache alias one runtime");
                let runtime = binding.body.lock().unwrap();
                let active: &ActiveBody = (&*runtime)$(.$active())*;
                return operation(active);
            }
        };
    }
    read_body!(ActiveBodyModuleData, ActiveBody);
    read_body!(StructureBodyModuleData, StructureBody, active_body);
    read_body!(ActiveBodyModuleData, HighlanderBody, active_body);
    read_body!(ActiveBodyModuleData, ImmortalBody, active_body);
    read_body!(
        HiveStructureBodyModuleData,
        HiveStructureBody,
        structure_body,
        active_body
    );
    read_body!(UndeadBodyModuleData, UndeadBody, active_body);
    panic!("actual authored active-body binding");
}

pub(crate) fn with_active_mut<R>(
    module: &mut dyn Module,
    cached: &Arc<Mutex<dyn BodyModuleInterface>>,
    operation: impl FnOnce(&mut ActiveBody) -> R,
) -> R {
    macro_rules! write_body {
        ($data:ty, $body:ty $(, $active:ident)*) => {
            if let Some(binding) = module.as_any_mut().downcast_mut::<BodyBindingModule<$data, $body>>() {
                let exact: Arc<Mutex<dyn BodyModuleInterface>> = binding.body.clone();
                assert!(Arc::ptr_eq(&exact, cached), "wrapper and cache alias one runtime");
                let mut runtime = binding.body.lock().unwrap();
                let active: &mut ActiveBody = (&mut *runtime)$(.$active())*;
                return operation(active);
            }
        };
    }
    write_body!(ActiveBodyModuleData, ActiveBody);
    write_body!(StructureBodyModuleData, StructureBody, active_body_mut);
    write_body!(ActiveBodyModuleData, HighlanderBody, active_body_mut);
    write_body!(ActiveBodyModuleData, ImmortalBody, active_body_mut);
    write_body!(
        HiveStructureBodyModuleData,
        HiveStructureBody,
        structure_body_mut,
        active_body_mut
    );
    write_body!(UndeadBodyModuleData, UndeadBody, active_body_mut);
    panic!("actual authored active-body binding");
}
