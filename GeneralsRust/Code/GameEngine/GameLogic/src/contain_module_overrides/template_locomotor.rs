//! Write top-level Object.ini `Locomotor = SET_* Name` lines into the already-
//! parsed AIUpdate module data (C++ `AIUpdateModuleData::parseLocomotorSet`).

use super::*;
use crate::common::{AsciiString, LocomotorSetType};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::thing::module::ModuleData;
use game_engine::common::thing::thing_template_locomotor::set_template_locomotor_applier;
use std::sync::Arc;

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
    data: Arc<dyn ModuleData>,
    set_name: &str,
    names: &[String],
    overrides_allowed: bool,
) -> Result<Arc<dyn ModuleData>, String> {
    let set = parse_locomotor_set_name(set_name)?;

    if let Some(ai) = data.as_any().downcast_ref::<AIUpdateModuleData>() {
        let mut cloned = ai.clone();
        write_set(&mut cloned, set, names, overrides_allowed)?;
        return Ok(Arc::new(cloned));
    }

    macro_rules! apply_base {
        ($ty:ty) => {
            if let Some(typed) = data.as_any().downcast_ref::<$ty>() {
                let mut cloned = typed.clone();
                write_set(&mut cloned.base, set, names, overrides_allowed)?;
                return Ok(Arc::new(cloned));
            }
        };
    }

    apply_base!(AssaultTransportAIUpdateModuleData);
    apply_base!(ChinookAIUpdateModuleData);
    apply_base!(DeliverPayloadAIUpdateModuleData);
    apply_base!(DeployStyleAIUpdateModuleData);
    apply_base!(DozerAIUpdateModuleData);
    apply_base!(HackInternetAIUpdateModuleData);
    apply_base!(JetAIUpdateModuleData);
    apply_base!(RailedTransportAIUpdateModuleData);
    apply_base!(SupplyTruckAIUpdateModuleData);
    apply_base!(WorkerAIUpdateModuleData);
    apply_base!(TransportAIUpdateModuleData);
    apply_base!(WanderAIUpdateModuleData);

    Ok(data)
}

#[cfg(test)]
mod policy_owner_tests {
    use super::*;
    #[test]
    fn actual_ai_data_applier_does_not_inherit_a_peers_override_policy() {
        let owner: Arc<dyn ModuleData> = Arc::new(AIUpdateModuleData::default());
        let peer: Arc<dyn ModuleData> = Arc::new(AIUpdateModuleData::default());
        let owner =
            apply_template_locomotor(owner, "SET_NORMAL", &["OwnerGround".into()], false).unwrap();
        let peer =
            apply_template_locomotor(peer, "SET_NORMAL", &["PeerGround".into()], false).unwrap();
        let changed = apply_template_locomotor(
            Arc::clone(&owner),
            "SET_NORMAL",
            &["OwnerOverride".into()],
            true,
        )
        .unwrap();
        assert!(
            apply_template_locomotor(
                Arc::clone(&peer),
                "SET_NORMAL",
                &["RejectedPeer".into()],
                false
            )
            .is_err()
        );
        assert!(
            apply_template_locomotor(
                Arc::clone(&changed),
                "SET_NORMAL",
                &["RejectedOwner".into()],
                false
            )
            .is_err()
        );
        let names = |data: &Arc<dyn ModuleData>| {
            data.as_any()
                .downcast_ref::<AIUpdateModuleData>()
                .unwrap()
                .locomotor_sets()[&LocomotorSetType::Normal]
                .clone()
        };
        assert_eq!(names(&owner), vec!["OwnerGround".to_owned()]);
        assert_eq!(names(&changed), vec!["OwnerOverride".to_owned()]);
        assert_eq!(names(&peer), vec!["PeerGround".to_owned()]);
    }
}
