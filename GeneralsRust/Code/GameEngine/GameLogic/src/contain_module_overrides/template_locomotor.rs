//! Locomotor admission through the legacy module factory.

#[cfg(test)]
mod policy_owner_tests {
    use game_engine::common::thing::ai_update_data::{AIUpdateModuleData, LocomotorSetType};
    use game_engine::common::thing::module::ModuleDataHandle;

    #[test]
    fn inherited_locomotor_admission_rolls_back_or_commits_to_the_original_handle() {
        use game_engine::common::ini::INILoadType;
        use game_engine::common::name_key_generator::NameKeyNamespace;
        use game_engine::common::thing::module_factory::ModuleFactory;
        use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;

        let mut factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
        crate::contain_module_overrides::register_native_module_overrides(&mut factory).unwrap();
        let mut content = ReceivingModuleContent::new(factory);
        content.load_ini_text(
            "Object Parent\nBehavior = AIUpdateInterface ParentAI\nEnd\nEnd",
            INILoadType::Overwrite,
        ).unwrap();
        let parent = content.admitted_template("Parent").unwrap();
        let original = parent.template().get_behavior_module_info().get_nth_data(0).unwrap().clone();
        let names = |handle: &ModuleDataHandle| {
            handle.read().as_any().downcast_ref::<AIUpdateModuleData>().unwrap()
                .locomotor_sets().get(&LocomotorSetType::Normal).cloned().unwrap_or_default()
        };
        assert!(names(&original).is_empty());

        let rejected = content.load_ini_text(
            "ChildObject Child Parent\nLocomotor = SET_NORMAL RejectedGround\nEnd\n\
             Object Invalid\nUnknownField = Yes\nEnd",
            INILoadType::Overwrite,
        );
        assert!(rejected.is_err());
        assert!(content.admitted_template("Child").is_none());
        let after_failure = content.admitted_template("Parent").unwrap();
        let failed_data = after_failure.template().get_behavior_module_info().get_nth_data(0).unwrap();
        assert!(ModuleDataHandle::ptr_eq(&original, failed_data));
        assert!(names(&original).is_empty());

        content.load_ini_text(
            "ChildObject Child Parent\nLocomotor = SET_NORMAL AcceptedGround\nEnd",
            INILoadType::Overwrite,
        ).unwrap();
        let after_success = content.admitted_template("Parent").unwrap();
        let child = content.admitted_template("Child").unwrap();
        let parent_data = after_success.template().get_behavior_module_info().get_nth_data(0).unwrap();
        let child_data = child.template().get_behavior_module_info().get_nth_data(0).unwrap();
        assert!(ModuleDataHandle::ptr_eq(&original, parent_data));
        assert!(ModuleDataHandle::ptr_eq(&original, child_data));
        assert_eq!(names(&original), vec!["AcceptedGround".to_owned()]);
    }
}
