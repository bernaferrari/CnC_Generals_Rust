use std::any::Any;

use game_engine::common::thing::module::Module;

pub(crate) enum ProjectileLaunchKindMut<'a> {
    MissileAIUpdateBehavior(
        &'a mut crate::object::update::missile_ai_update::MissileAIUpdateBehavior,
    ),
    NeutronMissileUpdate(
        &'a mut crate::object::update::neutron_missile_update::NeutronMissileUpdate,
    ),
    DumbProjectileBehavior(
        &'a mut crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehavior,
    ),
}

pub(crate) fn module_projectile_launch_kind(
    module: &mut dyn Module,
) -> Option<ProjectileLaunchKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::update::missile_ai_update::MissileAIUpdateBehavior>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::missile_ai_update::MissileAIUpdateBehavior>()
            .expect("type check and downcast must match");
        return Some(ProjectileLaunchKindMut::MissileAIUpdateBehavior(module));
    }
    if module
        .as_any()
        .is::<crate::object::update::neutron_missile_update::NeutronMissileUpdate>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::neutron_missile_update::NeutronMissileUpdate>()
            .expect("type check and downcast must match");
        return Some(ProjectileLaunchKindMut::NeutronMissileUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehavior>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehavior>()
            .expect("type check and downcast must match");
        return Some(ProjectileLaunchKindMut::DumbProjectileBehavior(module));
    }

    None
}
