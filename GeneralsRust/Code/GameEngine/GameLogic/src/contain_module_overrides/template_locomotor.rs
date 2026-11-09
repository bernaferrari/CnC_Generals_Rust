//! Write top-level Object.ini `Locomotor = SET_* Name` lines into the already-
//! parsed AIUpdate module data (C++ `AIUpdateModuleData::parseLocomotorSet`).

use super::*;
use crate::common::{AsciiString, LocomotorSetType};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::thing::thing_template_locomotor::set_template_locomotor_applier;

pub(super) fn install_template_locomotor_applier() {
    set_template_locomotor_applier(apply_template_locomotor);
}

fn parse_locomotor_set_name(set_name: &str) -> Result<LocomotorSetType, String> {
    match set_name {
        "SET_NORMAL" => Ok(LocomotorSetType::Normal),
        "SET_NORMAL_UPGRADED" => Ok(LocomotorSetType::NormalUpgraded),
        "SET_FREEFALL" => Ok(LocomotorSetType::Freefall),
        "SET_WANDER" => Ok(LocomotorSetType::Wander),
        "SET_PANIC" => Ok(LocomotorSetType::Panic),
        "SET_TAXIING" => Ok(LocomotorSetType::Taxiing),
        "SET_SUPERSONIC" => Ok(LocomotorSetType::Supersonic),
        "SET_SLUGGISH" => Ok(LocomotorSetType::Sluggish),
        _ => Err(format!("unknown Locomotor set {set_name}")),
    }
}

fn write_set(
    ai: &mut AIUpdateModuleData,
    set: LocomotorSetType,
    names: &[String],
    overrides_allowed: bool,
) -> Result<(), String> {
    if ai.has_locomotor_set(set) && !overrides_allowed {
        return Err("re-specifying a LocomotorSet is no longer allowed".to_string());
    }
    ai.set_locomotor_set_entries(
        set,
        names
            .iter()
            .map(|n| AsciiString::from(n.as_str()))
            .collect(),
    );
    Ok(())
}

fn apply_template_locomotor(
    data: ModuleDataHandle,
    set_name: &str,
    names: &[String],
    overrides_allowed: bool,
) -> Result<ModuleDataHandle, String> {
    let set = parse_locomotor_set_name(set_name)?;
    {
        let mut guard = data.write();
        if let Some(ai) = guard.as_any_mut().downcast_mut::<AIUpdateModuleData>() {
            write_set(ai, set, names, overrides_allowed)?;
        } else {
            macro_rules! apply_base {
                ($($ty:ty),* $(,)?) => {
                    $(if let Some(typed) = guard.as_any_mut().downcast_mut::<$ty>() {
                        write_set(&mut typed.base, set, names, overrides_allowed)?;
                    } else)* {}
                };
            }
            apply_base!(
                AssaultTransportAIUpdateModuleData,
                ChinookAIUpdateModuleData,
                DeliverPayloadAIUpdateModuleData,
                DeployStyleAIUpdateModuleData,
                DozerAIUpdateModuleData,
                HackInternetAIUpdateModuleData,
                JetAIUpdateModuleData,
                RailedTransportAIUpdateModuleData,
                SupplyTruckAIUpdateModuleData,
                WorkerAIUpdateModuleData,
                TransportAIUpdateModuleData,
                WanderAIUpdateModuleData,
            );
        }
    }
    Ok(data)
}

#[cfg(test)]
mod policy_owner_tests {
    use super::*;
    #[test]
    fn inherited_locomotor_admission_rolls_back_or_commits_to_the_original_handle() {
        use game_engine::common::ini::INILoadType;
        use game_engine::common::name_key_generator::NameKeyNamespace;
        use game_engine::common::thing::module_factory::ModuleFactory;
        use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;

        install_template_locomotor_applier();
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

    #[test]
    fn actual_ai_data_applier_does_not_inherit_a_peers_override_policy() {
        let owner = ModuleDataHandle::new(AIUpdateModuleData::default());
        let peer = ModuleDataHandle::new(AIUpdateModuleData::default());
        let owner =
            apply_template_locomotor(owner, "SET_NORMAL", &["OwnerGround".into()], false).unwrap();
        let peer =
            apply_template_locomotor(peer, "SET_NORMAL", &["PeerGround".into()], false).unwrap();
        let changed = apply_template_locomotor(
            owner.clone(),
            "SET_NORMAL",
            &["OwnerOverride".into()],
            true,
        )
        .unwrap();
        assert!(
            apply_template_locomotor(
                peer.clone(),
                "SET_NORMAL",
                &["RejectedPeer".into()],
                false
            )
            .is_err()
        );
        assert!(
            apply_template_locomotor(
                changed.clone(),
                "SET_NORMAL",
                &["RejectedOwner".into()],
                false
            )
            .is_err()
        );
        let names = |data: &ModuleDataHandle| {
            data.read().as_any()
                .downcast_ref::<AIUpdateModuleData>()
                .unwrap()
                .locomotor_sets()[&LocomotorSetType::Normal]
                .clone()
        };
        assert!(ModuleDataHandle::ptr_eq(&owner, &changed));
        assert!(!ModuleDataHandle::ptr_eq(&owner, &peer));
        assert_eq!(names(&owner), vec!["OwnerOverride".to_owned()]);
        assert_eq!(names(&changed), vec!["OwnerOverride".to_owned()]);
        assert_eq!(names(&peer), vec!["PeerGround".to_owned()]);
    }
}
