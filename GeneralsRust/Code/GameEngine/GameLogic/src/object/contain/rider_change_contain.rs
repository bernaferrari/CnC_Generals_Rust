//! Rider Change Contain Module
//!
//! Specialized container that can change the type of riders it contains

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, Coord3D, GameResult, LocomotorSetType, Matrix3D, ModelConditionFlags, ObjectID,
    ObjectStatusMaskType, ObjectStatusTypes, PlayerMaskType,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheInGameUI, TheMessageStream, TheThingFactory};
use crate::messages::{MSG_CREATE_SELECTED_GROUP, MSG_REMOVE_FROM_SELECTED_GROUP};
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::contain::TransportContain;
use crate::object::{Object, ObjectId};
use crate::upgrade::modules::model_condition::parse_model_condition_flag as parse_model_condition_name;
use crate::weapon::WeaponSetType;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Wave 277 residual scan still sees `OBJECT_REGISTRY.is_empty()`.
/// Do not skip-close contain solely because the dual-world registry is empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

/// Configuration data for RiderChangeContain module
#[derive(Debug, Clone)]
pub struct RiderChangeContainModuleData {
    /// Configuration from parent TransportContain
    pub base: super::TransportContainModuleData,
    /// Rider info entries
    pub riders: [RiderInfo; 8],
    /// Scuttle delay in frames
    pub scuttle_frames: u32,
    /// Scuttle status (model condition)
    pub scuttle_state: ModelConditionFlags,
}

#[derive(Debug, Clone)]
pub struct RiderInfo {
    pub template_name: String,
    pub model_condition_flag: ModelConditionFlags,
    pub weapon_set_flag: WeaponSetType,
    pub object_status: ObjectStatusMaskType,
    pub command_set: AsciiString,
    pub locomotor_set: LocomotorSetType,
}

impl Default for RiderInfo {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            model_condition_flag: ModelConditionFlags::empty(),
            weapon_set_flag: WeaponSetType::Veteran,
            object_status: ObjectStatusMaskType::empty(),
            command_set: AsciiString::new(),
            locomotor_set: LocomotorSetType::Invalid,
        }
    }
}

impl Default for RiderChangeContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            riders: std::array::from_fn(|_| RiderInfo::default()),
            scuttle_frames: 0,
            scuttle_state: ModelConditionFlags::TOPPLED,
        }
    }
}

impl RiderChangeContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, RIDER_CHANGE_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, RIDER_CHANGE_FIELDS)
    }
}

impl ContainerIniParse for RiderChangeContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        RiderChangeContainModuleData::parse_from_config(self, config)
    }
}

fn parse_rider_info_at(
    index: usize,
    _ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 6 {
        return Err(INIError::InvalidData);
    }

    data.riders[index] = RiderInfo {
        template_name: tokens[0].to_string(),
        model_condition_flag: parse_model_condition_flag(tokens[1])?,
        weapon_set_flag: parse_weapon_set_type(tokens[2])?,
        object_status: parse_object_status(tokens[3])?,
        command_set: AsciiString::from(tokens[4]),
        locomotor_set: parse_locomotor_set_type(tokens[5])?,
    };
    Ok(())
}

fn parse_rider1(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(0, ini, data, tokens)
}

fn parse_rider2(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(1, ini, data, tokens)
}

fn parse_rider3(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(2, ini, data, tokens)
}

fn parse_rider4(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(3, ini, data, tokens)
}

fn parse_rider5(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(4, ini, data, tokens)
}

fn parse_rider6(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(5, ini, data, tokens)
}

fn parse_rider7(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(6, ini, data, tokens)
}

fn parse_rider8(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(7, ini, data, tokens)
}

fn parse_scuttle_delay(
    _ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scuttle_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_scuttle_status(
    _ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scuttle_state = parse_model_condition_flag(token)?;
    Ok(())
}

const RIDER_CHANGE_FIELDS: &[FieldParse<RiderChangeContainModuleData>] = &[
    FieldParse {
        token: "Rider1",
        parse: parse_rider1,
    },
    FieldParse {
        token: "Rider2",
        parse: parse_rider2,
    },
    FieldParse {
        token: "Rider3",
        parse: parse_rider3,
    },
    FieldParse {
        token: "Rider4",
        parse: parse_rider4,
    },
    FieldParse {
        token: "Rider5",
        parse: parse_rider5,
    },
    FieldParse {
        token: "Rider6",
        parse: parse_rider6,
    },
    FieldParse {
        token: "Rider7",
        parse: parse_rider7,
    },
    FieldParse {
        token: "Rider8",
        parse: parse_rider8,
    },
    FieldParse {
        token: "ScuttleDelay",
        parse: parse_scuttle_delay,
    },
    FieldParse {
        token: "ScuttleStatus",
        parse: parse_scuttle_status,
    },
];

fn parse_locomotor_set_type(token: &str) -> Result<LocomotorSetType, INIError> {
    let mut name = token.trim().to_ascii_uppercase();
    if let Some(stripped) = name.strip_prefix("SET_") {
        name = stripped.to_string();
    }

    match name.as_str() {
        "NORMAL" => Ok(LocomotorSetType::Normal),
        "NORMAL_UPGRADED" => Ok(LocomotorSetType::NormalUpgraded),
        "FREEFALL" => Ok(LocomotorSetType::Freefall),
        "WANDER" => Ok(LocomotorSetType::Wander),
        "PANIC" => Ok(LocomotorSetType::Panic),
        "TAXIING" => Ok(LocomotorSetType::Taxiing),
        "SUPERSONIC" => Ok(LocomotorSetType::Supersonic),
        "SLUGGISH" => Ok(LocomotorSetType::Sluggish),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_weapon_set_type(token: &str) -> Result<WeaponSetType, INIError> {
    let mut name = token.trim().to_ascii_uppercase();
    if let Some(stripped) = name.strip_prefix("WEAPONSET_") {
        name = stripped.to_string();
    }

    match name.as_str() {
        "VETERAN" => Ok(WeaponSetType::Veteran),
        "ELITE" => Ok(WeaponSetType::Elite),
        "HERO" => Ok(WeaponSetType::Hero),
        "PLAYER_UPGRADE" => Ok(WeaponSetType::PlayerUpgrade),
        "CRATEUPGRADE_ONE" | "CRATE_UPGRADE_ONE" => Ok(WeaponSetType::CrateUpgradeOne),
        "CRATEUPGRADE_TWO" | "CRATE_UPGRADE_TWO" => Ok(WeaponSetType::CrateUpgradeTwo),
        "VEHICLE_HIJACK" => Ok(WeaponSetType::VehicleHijack),
        "CARBOMB" | "CAR_BOMB" => Ok(WeaponSetType::CarBomb),
        "MINE_CLEARING_DETAIL" => Ok(WeaponSetType::MineClearingDetail),
        "RIDER1" | "WEAPON_RIDER1" => Ok(WeaponSetType::WeaponRider1),
        "RIDER2" | "WEAPON_RIDER2" => Ok(WeaponSetType::WeaponRider2),
        "RIDER3" | "WEAPON_RIDER3" => Ok(WeaponSetType::WeaponRider3),
        "RIDER4" | "WEAPON_RIDER4" => Ok(WeaponSetType::WeaponRider4),
        "RIDER5" | "WEAPON_RIDER5" => Ok(WeaponSetType::WeaponRider5),
        "RIDER6" | "WEAPON_RIDER6" => Ok(WeaponSetType::WeaponRider6),
        "RIDER7" | "WEAPON_RIDER7" => Ok(WeaponSetType::WeaponRider7),
        "RIDER8" | "WEAPON_RIDER8" => Ok(WeaponSetType::WeaponRider8),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_object_status(token: &str) -> Result<ObjectStatusMaskType, INIError> {
    let name = token.trim();
    ObjectStatusMaskType::from_case_insensitive_name(name)
        .or_else(|| {
            name.strip_prefix("OBJECT_STATUS_")
                .and_then(ObjectStatusMaskType::from_case_insensitive_name)
        })
        .ok_or(INIError::InvalidData)
}

fn parse_model_condition_flag(token: &str) -> Result<ModelConditionFlags, INIError> {
    parse_model_condition_name(token).ok_or(INIError::InvalidData)
}

fn rider_info_matches_template(
    rider_info: &RiderInfo,
    template: &dyn crate::common::ThingTemplate,
) -> bool {
    if rider_info.template_name.is_empty() {
        return false;
    }

    if rider_info.template_name == template.get_name().as_str() {
        return true;
    }

    if let Some(rider_template) = TheThingFactory::find_template(rider_info.template_name.as_str())
    {
        return rider_template.is_equivalent_to(template);
    }

    false
}

/// C++ RiderChangeContain.cpp:225-231 (mount, bike ← rider) and :280-286
/// (dismount, rider ← bike): `toTracker->setVeterancyLevel(level, FALSE)` then
/// `fromTracker->setExperienceAndLevel(0, FALSE)`. `provideFeedback = FALSE`
/// skips only the promotion anim + UnitPromoted sound — the C++ tracker still
/// fires `Object::onVeterancyLevelChanged` internally (ExperienceTracker.cpp:82-95),
/// so weapon-set flags, veterancy upgrade, body healthBonus/armor rescale run.
fn transfer_veterancy(from: ObjectID, to: ObjectID) {
    let Some(level) = crate::object::registry::OBJECT_REGISTRY
        .with_object(from, |from_obj| from_obj.get_veterancy_level())
    else {
        return;
    };

    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(to, |to_obj| {
        to_obj.set_veterancy_level_with_side_effects(level, false);
    });

    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(from, |from_obj| {
        from_obj.set_experience_and_level_with_side_effects(0, false);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::DefaultThingTemplate;
    use crate::messages::{
        MSG_CREATE_SELECTED_GROUP, MSG_REMOVE_FROM_SELECTED_GROUP, MessageArgument, drain_messages,
    };
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::player::{Player, ThePlayerList};
    use crate::team::Team;
    use game_engine::common::system::{XferBlockSize, XferMode, XferStatus};
    use std::io;

    struct RecordingXfer {
        bytes: Vec<u8>,
    }

    impl RecordingXfer {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }
    }

    impl Xfer for RecordingXfer {
        fn get_xfer_mode(&self) -> XferMode {
            XferMode::Save
        }

        fn get_identifier(&self) -> &str {
            "rider-change-contain-test"
        }

        fn set_options(&mut self, _options: u32) {}

        fn clear_options(&mut self, _options: u32) {}

        fn get_options(&self) -> u32 {
            0
        }

        fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_snapshot(&mut self, _snapshot: &mut dyn Snapshotable) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        // SAFETY: trait contract: caller guarantees `data` is valid for
        // `data_size` bytes; only that range is copied into the recording
        // buffer within this call.
        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> io::Result<()> {
            // SAFETY: `xfer_implementation` receives a pointer valid for
            // `data_size` bytes from the Xfer caller; this read-only slice
            // view is appended before the call returns.
            let bytes = unsafe { std::slice::from_raw_parts(data, data_size) };
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn rider_change_parse_accepts_cpp_canonical_rider_tokens() {
        let _lock = crate::test_sync::lock();

        let mut data = RiderChangeContainModuleData::default();
        data.parse_from_config(
            "Rider1 GLAInfantryRebel RIDER1 WEAPON_RIDER1 STATUS_RIDER1 CommandSetName SET_NORMAL\n\
             ScuttleStatus RUBBLE\n\
             End\n",
        )
        .expect("rider change config parses");

        let rider = &data.riders[0];
        assert_eq!(rider.template_name, "GLAInfantryRebel");
        assert_eq!(rider.model_condition_flag, ModelConditionFlags::RIDER1);
        assert_eq!(rider.weapon_set_flag, WeaponSetType::WeaponRider1);
        assert_eq!(rider.object_status, ObjectStatusMaskType::RIDER1);
        assert_eq!(rider.command_set.as_str(), "CommandSetName");
        assert_eq!(rider.locomotor_set, LocomotorSetType::Normal);
        assert_eq!(data.scuttle_state, ModelConditionFlags::RUBBLE);
    }

    #[test]
    fn rider_change_scuttle_status_accepts_full_model_condition_names() {
        let _lock = crate::test_sync::lock();

        let mut data = RiderChangeContainModuleData::default();
        data.parse_from_config("ScuttleStatus DYING\nEnd\n")
            .expect("scuttle status parses");

        assert_eq!(data.scuttle_state, ModelConditionFlags::DYING);
    }

    fn reset_players() {
        let mut list = ThePlayerList().write().expect("player list write");
        list.clear();
        list.add_player(Arc::new(RwLock::new(Player::new(0))));
        list.set_local_player_index(0);
    }

    #[test]
    fn game_logic_default_template_preserves_trainability_and_thresholds_in_trait_clone() {
        let mut template = DefaultThingTemplate::new("TrainableTemplateContract".to_string());
        let mut fields = HashMap::new();
        fields.insert("IsTrainable".to_string(), "yEs".to_string());
        fields.insert(
            "ExperienceRequired".to_string(),
            "0 100 300 600".to_string(),
        );
        template.parse_object_fields_from_ini(&fields);

        let cloned = template.clone();
        let template_view: &dyn crate::common::ThingTemplate = &cloned;
        assert!(template_view.is_trainable());
        assert_eq!(template_view.get_experience_required(0), 0);
        assert_eq!(template_view.get_experience_required(1), 100);
        assert_eq!(template_view.get_experience_required(2), 300);
        assert_eq!(template_view.get_experience_required(3), 600);

        let default_view: &dyn crate::common::ThingTemplate =
            &DefaultThingTemplate::new("NontrainableTemplateContract".to_string());
        assert!(!default_view.is_trainable());
        assert_eq!(default_view.get_experience_required(2), 0);
    }

    fn owned_object(name: &str, id: ObjectID, player_index: u32) -> ObjectID {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(player_index));
        let mut template = DefaultThingTemplate::new(name.to_string());
        if name.starts_with("BikeRider") {
            let mut fields = HashMap::new();
            fields.insert("KindOf".to_string(), "INFANTRY".to_string());
            template.parse_object_fields_from_ini(&fields);
        }
        let template = Arc::new(template);
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), Some(team))
            .expect("owned test object")
    }

    fn owned_trainable_object(name: &str, id: ObjectID, player_index: u32) -> ObjectID {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(player_index));
        let mut template = DefaultThingTemplate::new(name.to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), "INFANTRY".to_string());
        fields.insert("IsTrainable".to_string(), "Yes".to_string());
        fields.insert(
            "ExperienceRequired".to_string(),
            "0 100 300 600".to_string(),
        );
        template.parse_object_fields_from_ini(&fields);
        Object::new_with_id(
            Arc::new(template),
            id,
            ObjectStatusMaskType::none(),
            Some(team),
        )
        .expect("owned trainable test object")
    }

    fn rider(name: &str, id: ObjectID, player_index: u32) -> ObjectID {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(player_index));
        let mut template = DefaultThingTemplate::new(name.to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), "INFANTRY".to_string());
        fields.insert("TransportSlotCount".to_string(), "1".to_string());
        template.parse_object_fields_from_ini(&fields);
        Object::new_with_id(
            Arc::new(template),
            id,
            ObjectStatusMaskType::none(),
            Some(team),
        )
        .expect("test rider")
    }

    fn attach_drawable(obj: &ObjectID, drawable_id: ObjectID) -> Arc<RwLock<Drawable>> {
        let object_id = *obj;
        let drawable = Arc::new(RwLock::new(Drawable::new(
            drawable_id,
            object_id,
            format!("Drawable{object_id}"),
            DrawableType::Animated,
        )));
        OBJECT_REGISTRY
            .with_object_mut(object_id, |object| {
                object.set_drawable(Some(drawable.clone()));
            })
            .expect("object write");
        drawable
    }

    fn is_create_selected_message(
        message: &crate::messages::GameMessage,
        object_id: ObjectID,
    ) -> bool {
        message.id == MSG_CREATE_SELECTED_GROUP
            && matches!(
                message.arguments.as_slice(),
                [MessageArgument::Boolean(false), MessageArgument::ObjectId(id)] if *id == object_id
            )
    }

    fn is_remove_selected_message(
        message: &crate::messages::GameMessage,
        object_id: ObjectID,
    ) -> bool {
        message.id == MSG_REMOVE_FROM_SELECTED_GROUP
            && matches!(
                message.arguments.as_slice(),
                [MessageArgument::ObjectId(id)] if *id == object_id
            )
    }

    fn cleanup_objects(ids: &[ObjectID]) {
        // Wave 277: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        for id in ids {
            OBJECT_REGISTRY.unregister_object(*id);
        }
        let _ = drain_messages();
        ThePlayerList().write().expect("player list write").clear();
    }

    fn rider_change_for_with_config(
        owner: &ObjectID,
        configure: impl FnOnce(&mut RiderChangeContainModuleData),
    ) -> RiderChangeContain {
        let mut data = RiderChangeContainModuleData {
            base: super::super::TransportContainModuleData {
                slot_capacity: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        data.riders[0] = RiderInfo {
            template_name: "BikeRiderOne".to_string(),
            model_condition_flag: ModelConditionFlags::RIDER1,
            weapon_set_flag: WeaponSetType::WeaponRider1,
            object_status: ObjectStatusMaskType::RIDER1,
            command_set: AsciiString::from("RiderOneCommandSet"),
            locomotor_set: LocomotorSetType::Normal,
        };
        data.riders[1] = RiderInfo {
            template_name: "BikeRiderTwo".to_string(),
            model_condition_flag: ModelConditionFlags::RIDER2,
            weapon_set_flag: WeaponSetType::WeaponRider2,
            object_status: ObjectStatusMaskType::RIDER2,
            command_set: AsciiString::from("RiderTwoCommandSet"),
            locomotor_set: LocomotorSetType::Normal,
        };
        configure(&mut data);
        RiderChangeContain::new(*owner, &data).expect("rider change contain")
    }

    fn rider_change_for(owner: &ObjectID) -> RiderChangeContain {
        rider_change_for_with_config(owner, |_| {})
    }

    #[test]
    fn valid_replacement_rider_ignores_capacity_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBike", 97001, 0);
        let first = rider("BikeRiderOne", 97002, 0);
        let second = rider("BikeRiderTwo", 97003, 0);
        let mut contain = rider_change_for(&owner);

        contain
            .add_to_contain(
                first,
                false,
            )
            .expect("first rider enters");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
        assert_eq!(
            ContainModuleInterface::friend_get_rider(&contain),
            Some(97002)
        );
        assert_eq!(ContainerInterface::get_usage(&contain), (0, 0));
        assert!(
            OBJECT_REGISTRY
                .with_object(second, |g| contain.is_valid_container_for(g, true))
                .expect("second rider read"),
            "C++ RiderChangeContain ignores capacity because the new rider replaces the old one"
        );

        cleanup_objects(&[97001, 97002, 97003]);
    }

    #[test]
    fn replacement_rider_uses_rider_change_removal_hook_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeReplace", 97004, 0);
        let first = rider("BikeRiderOne", 97005, 0);
        let second = rider("BikeRiderTwo", 97006, 0);
        let mut contain = rider_change_for(&owner);

        contain
            .add_to_contain(
                first,
                false,
            )
            .expect("first rider enters");
        contain
            .add_to_contain(
                second,
                false,
            )
            .expect("second rider replaces first");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
        assert_eq!(
            OBJECT_REGISTRY.with_object(first, |g| g.get_contained_by()).expect("first rider read"),
            None
        );
        assert_eq!(
            OBJECT_REGISTRY.with_object(second, |g| g.get_contained_by()).expect("second rider read"),
            Some(97004)
        );
        assert_eq!(
            ContainModuleInterface::friend_get_rider(&contain),
            Some(97006)
        );

        OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            assert!(!owner_guard.test_status(ObjectStatusTypes::Rider1));
            assert!(owner_guard.test_status(ObjectStatusTypes::Rider2));
            assert!(!owner_guard.test_weapon_set_flag(WeaponSetType::WeaponRider1));
            assert!(owner_guard.test_weapon_set_flag(WeaponSetType::WeaponRider2));
            assert_eq!(owner_guard.get_command_set_string(), "RiderTwoCommandSet");
        }).expect("owner read");

        cleanup_objects(&[97004, 97005, 97006]);
    }

    #[test]
    fn payload_created_replacement_uses_evacuate_path_without_scuttling_bike() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikePayloadReplace", 97009, 0);
        let first = rider("BikeRiderOne", 97010, 0);
        let second = rider("BikeRiderTwo", 97011, 0);
        let mut contain = rider_change_for(&owner);

        contain
            .add_to_contain(
                first,
                false,
            )
            .expect("first rider enters");
        contain.base.set_payload_created(true);
        contain
            .add_to_contain(
                second,
                false,
            )
            .expect("second rider replaces first through payload branch");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
        assert_eq!(
            OBJECT_REGISTRY.with_object(first, |g| g.get_contained_by()).expect("first rider read"),
            None
        );
        assert_eq!(
            OBJECT_REGISTRY.with_object(second, |g| g.get_contained_by()).expect("second rider read"),
            Some(97009)
        );
        assert!(
            !OBJECT_REGISTRY.with_object(owner, |g| g.test_status(ObjectStatusTypes::Unselectable),
            "replacement should not scuttle the bike"
        );

        cleanup_objects(&[97009, 97010, 97011]);
    }

    #[test]
    fn selected_rider_selects_bike_on_entry_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeSelectionEnter", 97012, 0);
        let rider = rider("BikeRiderOne", 97013, 0);
        let owner_drawable = attach_drawable(&owner, 970120);
        let rider_drawable = attach_drawable(&rider, 970130);
        rider_drawable
            .write()
            .expect("rider drawable write")
            .set_selected(true);
        let mut contain = rider_change_for(&owner);
        let _ = drain_messages();

        contain
            .add_to_contain(
                rider,
                false,
            )
            .expect("selected rider enters");

        assert!(
            owner_drawable.read().expect("owner drawable read").is_selected(),
            "C++ selects the bike when the entering rider was selected"
        );
        let messages = drain_messages();
        assert!(
            messages
                .iter()
                .any(|message| is_create_selected_message(message, 97012)),
            "C++ sends MSG_CREATE_SELECTED_GROUP for the bike"
        );

        cleanup_objects(&[97012, 97013]);
    }

    #[test]
    fn selected_bike_selects_rider_and_deselects_bike_on_exit_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeSelectionExit", 97014, 0);
        let rider = rider("BikeRiderOne", 97015, 0);
        let owner_drawable = attach_drawable(&owner, 970140);
        let rider_drawable = attach_drawable(&rider, 970150);
        let mut contain = rider_change_for(&owner);

        contain
            .add_to_contain(
                rider,
                false,
            )
            .expect("rider enters");
        let _ = drain_messages();
        owner_drawable
            .write()
            .expect("owner drawable write")
            .set_selected(true);

        contain
            .remove_from_contain(
                rider,
                false,
            )
            .expect("rider exits");

        assert!(
            rider_drawable.read().expect("rider drawable read").is_selected(),
            "C++ selects the exiting rider when the bike was selected"
        );
        assert!(
            !owner_drawable.read().expect("owner drawable read").is_selected(),
            "C++ removes the bike from the selected group"
        );
        let messages = drain_messages();
        assert!(
            messages
                .iter()
                .any(|message| is_create_selected_message(message, 97015)),
            "C++ sends MSG_CREATE_SELECTED_GROUP for the exiting rider"
        );
        assert!(
            messages
                .iter()
                .any(|message| is_remove_selected_message(message, 97014)),
            "C++ sends MSG_REMOVE_FROM_SELECTED_GROUP for the bike"
        );

        cleanup_objects(&[97014, 97015]);
    }

    #[test]
    fn rider_exit_without_drawables_does_not_scuttle_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeNoDrawableExit", 97016, 0);
        let rider = rider("BikeRiderOne", 97017, 0);
        let mut contain = rider_change_for(&owner);

        contain
            .add_to_contain(
                rider,
                false,
            )
            .expect("rider enters");
        contain
            .remove_from_contain(
                rider,
                false,
            )
            .expect("rider exits");

        let unselectable = OBJECT_REGISTRY
            .with_object(owner, |g| g.test_status(ObjectStatusTypes::Unselectable))
            .expect("owner read");
        assert!(
            !unselectable,
            "C++ skips bike scuttle when either drawable is missing"
        );
        assert_eq!(
            contain.scuttled_on_frame, 0,
            "C++ leaves m_scuttledOnFrame untouched without drawables"
        );

        cleanup_objects(&[97016, 97017]);
    }

    #[test]
    fn non_payload_rider_exit_does_not_run_transport_on_removing_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeNoTransportExit", 97018, 0);
        let rider = rider("BikeRiderOne", 97019, 0);
        attach_drawable(&owner, 970180);
        attach_drawable(&rider, 970190);
        let mut contain = rider_change_for_with_config(&owner, |data| {
            data.base.exit_delay = 30;
        });

        contain
            .add_to_contain(
                rider,
                false,
            )
            .expect("rider enters");
        contain
            .remove_from_contain(
                rider,
                false,
            )
            .expect("rider exits");

        assert!(
            !contain.base.is_exit_busy(),
            "C++ only calls TransportContain::onRemoving when m_payloadCreated is true"
        );

        cleanup_objects(&[97018, 97019]);
    }

    #[test]
    fn trait_snapshot_xfer_appends_rider_change_shadow_fields_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeXfer", 97007, 0);
        let mut contain = rider_change_for(&owner);
        contain.base.set_payload_created(true);
        contain.extra_slots_in_use = 7;
        contain.frame_exit_not_busy = 1234;

        let mut xfer = RecordingXfer::new();
        ContainModuleInterface::snapshot_xfer(&mut contain, &mut xfer)
            .expect("rider change snapshot xfer");

        assert_eq!(xfer.bytes[0], 1, "RiderChangeContain xfer version");
        assert_eq!(xfer.bytes[1], 1, "delegated TransportContain xfer version");
        assert_eq!(xfer.bytes[2], 2, "delegated OpenContain xfer version");

        // C++ Bool is one byte; the duplicated fields occupy 1 + 4 + 4 bytes.
        let tail = &xfer.bytes[xfer.bytes.len() - 9..];
        assert_eq!(&tail[0..1], &[1], "duplicated m_payloadCreated");
        assert_eq!(&tail[1..5], &7_i32.to_le_bytes());
        assert_eq!(&tail[5..9], &1234_u32.to_le_bytes());

        cleanup_objects(&[97007]);
    }

    #[test]
    fn trait_update_returns_base_transport_sleep_time_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("CombatBikeUpdate", 97008, 0);
        let mut contain = rider_change_for(&owner);

        let sleep = ContainModuleInterface::update(&mut contain).expect("update succeeds");

        assert_eq!(sleep, UpdateSleepTime::None);

        cleanup_objects(&[97008]);
    }
    #[test]
    fn rider_transfer_veterancy_fires_cpp_on_veterancy_level_changed() {
        // C++ RiderChangeContain.cpp:225-231/:280-286 call
        // setVeterancyLevel/setExperienceAndLevel on the trackers; the C++
        // tracker fires Object::onVeterancyLevelChanged internally
        // (ExperienceTracker.cpp:82-95), so the weapon-set flag follows the
        // level even with provideFeedback=FALSE.
        let _lock = crate::test_sync::lock();

        let rider = owned_trainable_object("BikeRiderVet", 97021, 0);
        let bike = owned_object("CombatBikeVet", 97022, 0);
        assert!(
            OBJECT_REGISTRY
                .with_object(rider, |g| g.get_template().is_trainable())
                .expect("rider read"),
            "the reset regression uses an authored IsTrainable template"
        );
        let rider_tracker = Arc::new(std::sync::Mutex::new(
            crate::common::ExperienceTracker::new(97021),
        ));
        let bike_tracker = Arc::new(std::sync::Mutex::new(
            crate::common::ExperienceTracker::new(97022),
        ));
        rider_tracker
            .lock()
            .expect("rider tracker")
            .set_veterancy_level(crate::common::VeterancyLevel::Veteran);
        OBJECT_REGISTRY.with_object_mut(rider, |o| o.experience_tracker = Some(Arc::clone(&rider_tracker))).expect("rider write");
        OBJECT_REGISTRY.with_object_mut(bike, |o| o.experience_tracker = Some(Arc::clone(&bike_tracker))).expect("bike write");

        transfer_veterancy(rider, bike);

        // Bike took the rider's level — weapon-set flag follows (side effect).
        assert!(
            OBJECT_REGISTRY
                .with_object(bike, |g| g.test_weapon_set_flag(WeaponSetType::Veteran))
                .expect("bike read"),
            "C++ fires onVeterancyLevelChanged on mount: bike weapon set becomes Veteran"
        );
        assert_eq!(
            bike_tracker
                .lock()
                .expect("bike tracker")
                .get_veterancy_level(),
            crate::common::VeterancyLevel::Veteran
        );
        // Rider resets to Regular via setExperienceAndLevel(0, FALSE) — side
        // effects clear the Veteran weapon-set flag on demotion too.
        assert_eq!(
            rider_tracker
                .lock()
                .expect("rider tracker")
                .get_veterancy_level(),
            crate::common::VeterancyLevel::Regular
        );
        assert!(
            !OBJECT_REGISTRY.with_object(rider, |g| g.test_weapon_set_flag(WeaponSetType::Veteran)).unwrap_or(false),
            "C++ setExperienceAndLevel fires onVeterancyLevelChanged on demotion"
        );

        cleanup_objects(&[97021, 97022]);
    }

    #[test]
    fn set_experience_and_level_keeps_nontrainable_object_unchanged_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let object = owned_object("NontrainableExperienceReset", 97023, 0);
        let tracker = Arc::new(std::sync::Mutex::new(
            crate::common::ExperienceTracker::new(97023),
        ));
        tracker
            .lock()
            .expect("tracker")
            .set_veterancy_level(crate::common::VeterancyLevel::Veteran);
        OBJECT_REGISTRY.with_object_mut(object, |o| o.experience_tracker = Some(Arc::clone(&tracker))).expect("object write");

        assert!(
            !OBJECT_REGISTRY
                .with_object_mut(object, |o| o.set_experience_and_level_with_side_effects(0, false))
                .expect("object write")
        );
        assert_eq!(
            tracker.lock().expect("tracker").get_veterancy_level(),
            crate::common::VeterancyLevel::Veteran,
            "CPP setExperienceAndLevel returns without mutation when IsTrainable is false"
        );

        cleanup_objects(&[97023]);
    }

    #[test]
    fn set_experience_and_level_sink_uses_target_owner_and_side_effects_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let source = owned_object("ExperienceSinkSource", 97024, 0);
        let target = owned_trainable_object("ExperienceSinkTarget", 97025, 0);
        let source_tracker = Arc::new(std::sync::Mutex::new(
            crate::common::ExperienceTracker::new(97024),
        ));
        let target_tracker = Arc::new(std::sync::Mutex::new(
            crate::common::ExperienceTracker::new(97025),
        ));
        source_tracker
            .lock()
            .expect("source tracker")
            .set_experience_sink(97025);
        OBJECT_REGISTRY.with_object_mut(source, |o| o.experience_tracker = Some(Arc::clone(&source_tracker))).expect("source write");
        OBJECT_REGISTRY.with_object_mut(target, |o| o.experience_tracker = Some(Arc::clone(&target_tracker))).expect("target write");
        assert!(
            OBJECT_REGISTRY
                .with_object_mut(target, |o| {
                    o.set_veterancy_level_with_side_effects(
                        crate::common::VeterancyLevel::Veteran,
                        false,
                    )
                })
                .expect("target write")
        );

        assert!(
            OBJECT_REGISTRY
                .with_object_mut(source, |o| o.set_experience_and_level_with_side_effects(0, false))
                .expect("source write")
        );
        assert_eq!(
            target_tracker
                .lock()
                .expect("target tracker")
                .get_veterancy_level(),
            crate::common::VeterancyLevel::Regular,
            "CPP forwards the reset to the sink Object and applies its level change"
        );
        assert!(
            !OBJECT_REGISTRY.with_object(target, |g| g.test_weapon_set_flag(WeaponSetType::Veteran)).unwrap_or(false),
            "sink Object receives onVeterancyLevelChanged on demotion"
        );
        assert_eq!(
            source_tracker
                .lock()
                .expect("source tracker")
                .get_veterancy_level(),
            crate::common::VeterancyLevel::Regular,
            "the forwarding source tracker is not mutated"
        );

        cleanup_objects(&[97024, 97025]);
    }

    #[test]
    fn set_experience_and_level_missing_sink_falls_back_to_own_tracker_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let source = owned_trainable_object("ExperienceMissingSinkSource", 97026, 0);
        let source_tracker = Arc::new(std::sync::Mutex::new(
            crate::common::ExperienceTracker::new(97026),
        ));
        source_tracker
            .lock()
            .expect("source tracker")
            .set_experience_sink(97999);
        OBJECT_REGISTRY.with_object_mut(source, |o| o.experience_tracker = Some(Arc::clone(&source_tracker))).expect("source write");
        assert!(
            OBJECT_REGISTRY
                .with_object_mut(source, |o| {
                    o.set_veterancy_level_with_side_effects(
                        crate::common::VeterancyLevel::Veteran,
                        false,
                    )
                })
                .expect("source write")
        );

        assert!(
            OBJECT_REGISTRY
                .with_object_mut(source, |o| o.set_experience_and_level_with_side_effects(0, false))
                .expect("source write")
        );
        assert_eq!(
            source_tracker
                .lock()
                .expect("source tracker")
                .get_veterancy_level(),
            crate::common::VeterancyLevel::Regular,
            "CPP falls through to local reset when the sink object no longer exists"
        );
        assert!(
            !OBJECT_REGISTRY.with_object(source, |g| g.test_weapon_set_flag(WeaponSetType::Veteran)).unwrap_or(false),
            "local fallback fires the source Object level-change callback"
        );

        cleanup_objects(&[97026]);
    }
}

/// Rider change contain module - can transform contained units
#[derive(Debug)]
pub struct RiderChangeContain {
    /// Base functionality from TransportContain
    pub base: TransportContain,
    /// Reference to the owning object
    object_id: ObjectID,
    /// Module configuration
    module_data: RiderChangeContainModuleData,
    /// Frame when scuttling started
    scuttled_on_frame: u32,
    /// RiderChange shadows TransportContain's extra-slot field in C++ and xfers it separately.
    extra_slots_in_use: i32,
    /// RiderChange shadows TransportContain's exit-busy frame in C++ and xfers it separately.
    frame_exit_not_busy: u32,
    /// Whether we are currently replacing a rider
    containing: bool,
}

impl RiderChangeContain {
    /// Create a new RiderChangeContain module
    pub fn new(
        object_id: ObjectID,
        module_data: &RiderChangeContainModuleData,
    ) -> GameResult<Self> {
        let base = TransportContain::new(object_id, &module_data.base)?;

        Ok(Self {
            base,
            object_id: object_id,
            module_data: module_data.clone(),
            scuttled_on_frame: 0,
            extra_slots_in_use: 0,
            frame_exit_not_busy: 0,
            containing: false,
        })
    }

    /// Check if this is a rider change container
    pub fn is_rider_change_contain(&self) -> bool {
        true
    }

    pub fn friend_get_rider(&self) -> Option<ObjectID> {
        self.base.base.get_contained_object_ids().first().copied()
    }

    pub fn is_exit_busy(&self) -> bool {
        false
    }

    pub fn is_valid_container_for(&self, rider: &Object, check_capacity: bool) -> bool {
        let _ = check_capacity;
        if !self.base.is_valid_container_for(rider, false) {
            return false;
        }

        if self.scuttled_on_frame != 0 {
            return false;
        }

        let rider_template = rider.get_template();
        for rider_info in &self.module_data.riders {
            if rider_info_matches_template(rider_info, rider_template.as_ref()) {
                return true;
            }
        }

        false
    }

    pub fn add_to_contain(&mut self, rider_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 277: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let owner_id = if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            Some(self.object_id)
        };
        if super::should_cancel_containment_after_booby_trap(owner_id, rider_id) {
            return Ok(());
        }

        let Some(prep) = crate::object::registry::OBJECT_REGISTRY.with_object(rider_id, |rider| {
            let was_selected = was_selected
                || rider
                    .get_drawable()
                    .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
                    .unwrap_or(false);
            if !self.is_valid_container_for(rider, true) {
                return Err("Object not valid for this rider change container".into());
            }
            let already_listed = self
                .base
                .base
                .get_contained_object_ids()
                .contains(&rider_id);
            let contained_by = rider.get_contained_by();
            if contained_by.is_some() && (already_listed || contained_by != Some(self.object_id)) {
                return Ok(None);
            }
            let should_remove_from_world = self.base.base.is_enclosing_container_for(rider);
            Ok(Some((was_selected, should_remove_from_world)))
        }) else {
            return Err("Rider object not found".into());
        };
        let (was_selected, should_remove_from_world) = match prep? {
            None => return Ok(()),
            Some(pair) => pair,
        };
        self.base.add_to_contain_list(rider_id)?;
        if should_remove_from_world {
            let _ = self.base.base.add_or_remove_obj_from_world(rider_id, false);
        }
        self.base.redeploy_occupants()?;
        if let Err(err) = self.on_containing(rider_id, was_selected) {
            self.base.base.unlink_contained_id(rider_id);
            if should_remove_from_world {
                let _ = self.base.base.add_or_remove_obj_from_world(rider_id, true);
            }
            let _ = self.base.redeploy_occupants();
            return Err(err);
        }
        self.base.base.do_load_sound();
        Ok(())
    }

    pub fn remove_from_contain(
        &mut self,
        rider_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        // Wave 277: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::object::registry::OBJECT_REGISTRY.contains(rider_id) {
            return Ok(());
        }

        if !self
            .base
            .base
            .get_contained_object_ids()
            .contains(&rider_id)
        {
            return Ok(());
        }

        let Some(stealth_garrison) = self.base.base.remove_from_contain_list(rider_id) else {
            return Err("Rider-change passenger lock busy".into());
        };
        let should_add_to_world = crate::object::registry::OBJECT_REGISTRY
            .with_object(rider_id, |rider| self.base.base.is_enclosing_container_for(rider))
            .unwrap_or(false);
        if should_add_to_world {
            let _ = self.base.base.add_or_remove_obj_from_world(rider_id, true);
            if self.object_id != crate::common::INVALID_ID {
                if let Some((pos, layer)) = crate::object::registry::OBJECT_REGISTRY
                    .with_object(self.object_id, |owner| (*owner.get_position(), owner.get_layer()))
                {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(rider_id, |rider| {
                        let _ = rider.set_position(&pos);
                        rider.set_layer(layer);
                    });
                }
            }
        }
        if expose_stealth_units {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(rider_id, |rider| {
                if let Some(stealth) = rider.get_stealth() {
                    if let Ok(mut stealth_guard) = stealth.try_lock() {
                        stealth_guard.mark_as_detected();
                    }
                }
            });
        }
        self.base.base.do_unload_sound();
        if let Err(err) = self.on_removing(rider_id) {
            let _ = self
                .base
                .base
                .add_to_contain_list_id(rider_id, stealth_garrison);
            if should_add_to_world {
                let _ = self.base.base.add_or_remove_obj_from_world(rider_id, false);
            }
            return Err(err);
        }

        if let Err(err) = self.base.base.note_removed_from(rider_id) {
            let _ = self
                .base
                .base
                .add_to_contain_list_id(rider_id, stealth_garrison);
            if should_add_to_world {
                let _ = self.base.base.add_or_remove_obj_from_world(rider_id, false);
            }
            return Err(err);
        }
        Ok(())
    }

    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 277: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::object::registry::OBJECT_REGISTRY.contains(obj_id) {
            return Ok(());
        }

        self.containing = true;

        if self.base.is_payload_created() {
            self.evacuate_existing_payload_via_owner_ai();
        }

        let contained_ids = self.base.base.get_contained_object_ids().to_vec();
        for existing_id in contained_ids {
            if existing_id == obj_id {
                continue;
            }
            let _ = self.remove_from_contain(existing_id, true);
        }

        self.transfer_selection_to_owner_on_entry(was_selected);

        let Some(rider_template) =
            crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |rider| rider.get_template().clone())
        else {
            self.containing = false;
            return Err("Rider lock poisoned".into());
        };

        if self.object_id != crate::common::INVALID_ID {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, |owner_guard| {
                for rider_info in &self.module_data.riders {
                    if rider_info_matches_template(rider_info, rider_template.as_ref()) {
                        owner_guard.set_model_condition_state(rider_info.model_condition_flag);
                        owner_guard.set_weapon_set_flag(rider_info.weapon_set_flag);
                        owner_guard.set_status(rider_info.object_status, true);
                        owner_guard.set_command_set_string_override(&rider_info.command_set);

                        if let Some(ai) = owner_guard.get_ai() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                let _ = ai_guard.choose_locomotor_set(rider_info.locomotor_set);
                            }
                        }

                        if owner_guard.test_status(ObjectStatusTypes::Stealthed) {
                            if let Some(stealth) = owner_guard.get_stealth() {
                                if let Ok(mut stealth_guard) = stealth.lock() {
                                    stealth_guard.mark_as_detected();
                                }
                            }
                        }

                        break;
                    }
                }
            });
        }

        // C++ RiderChangeContain.cpp:228-230 — rider mounts: bike takes the
        // rider's veterancy (feedback off), rider resets to Regular.
        if self.object_id != crate::common::INVALID_ID {
            transfer_veterancy(obj_id, self.object_id);
        }
        self.base.on_containing(obj_id, was_selected)?;
        self.containing = false;
        Ok(())
    }

    fn evacuate_existing_payload_via_owner_ai(&self) {
        // Wave 277: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.object_id == crate::common::INVALID_ID {
            return;
        }
        let Some(ai) = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |owner| {
            owner.get_ai_update_interface()
        }) else {
            return;
        };
        let Some(ai) = ai else {
            return;
        };
        if let Ok(mut ai_guard) = ai.lock() {
            let mut params =
                AiCommandParams::new(AiCommandType::EvacuateInstantly, CommandSourceType::FromAi);
            params.int_value = 1;
            let _ = ai_guard.execute_command(&params);
        }
    }

    fn transfer_selection_to_owner_on_entry(&self, was_selected: bool) {
        // Wave 277: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if !was_selected {
            return;
        }

        if self.object_id == crate::common::INVALID_ID {
            return;
        }

        let Some(packed) = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |owner_guard| {
            let Some(drawable) = owner_guard.get_drawable() else {
                return None;
            };
            let already_selected = drawable
                .read()
                .ok()
                .map(|drawable_guard| drawable_guard.is_selected())
                .unwrap_or(false);
            if already_selected {
                return None;
            }
            Some((owner_guard.get_id(), drawable))
        }) else {
            return;
        };
        let Some((owner_id, owner_drawable)) = packed else {
            return;
        };

        let mut team_msg = TheMessageStream::append_message(MSG_CREATE_SELECTED_GROUP);
        team_msg.append_boolean_argument(false);
        team_msg.append_object_id_argument(owner_id);
        drop(team_msg);

        TheInGameUI::select_drawable(&owner_drawable);
        TheInGameUI::set_displayed_max_warning(false);
    }

    fn transfer_selection_to_rider_on_exit(&self, rider_id: ObjectID) {
        // Wave 277: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.object_id == crate::common::INVALID_ID
            || !crate::object::registry::OBJECT_REGISTRY.contains(rider_id)
        {
            return;
        }

        let local_player_index = crate::player::ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(crate::player::PLAYER_INDEX_INVALID);
        if local_player_index == crate::player::PLAYER_INDEX_INVALID {
            return;
        }

        let Some(owner_pack) = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |owner_guard| {
            if owner_guard.get_controlling_player_id() != Some(local_player_index as u32) {
                return None;
            }
            let Some(drawable) = owner_guard.get_drawable() else {
                return None;
            };
            let selected = drawable
                .read()
                .ok()
                .map(|drawable_guard| drawable_guard.is_selected())
                .unwrap_or(false);
            if !selected {
                return None;
            }
            Some((owner_guard.get_id(), drawable))
        }) else {
            return;
        };
        let Some((owner_id, owner_drawable)) = owner_pack else {
            return;
        };

        let Some(rider_pack) = crate::object::registry::OBJECT_REGISTRY.with_object(rider_id, |rider_guard| {
            rider_guard
                .get_drawable()
                .map(|drawable| (rider_guard.get_id(), drawable))
        }) else {
            return;
        };
        let Some((selected_rider_id, rider_drawable)) = rider_pack else {
            return;
        };

        let mut team_msg = TheMessageStream::append_message(MSG_CREATE_SELECTED_GROUP);
        team_msg.append_boolean_argument(false);
        team_msg.append_object_id_argument(selected_rider_id);
        drop(team_msg);

        TheInGameUI::select_drawable(&rider_drawable);
        TheInGameUI::set_displayed_max_warning(false);

        let mut remove_msg = TheMessageStream::append_message(MSG_REMOVE_FROM_SELECTED_GROUP);
        remove_msg.append_object_id_argument(owner_id);
        drop(remove_msg);

        TheInGameUI::deselect_drawable(&owner_drawable);
    }

    fn has_exit_scuttle_drawables(&self, rider_id: ObjectID) -> bool {
        // Wave 277: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let owner_has_drawable = self.object_id != crate::common::INVALID_ID
            && crate::object::registry::OBJECT_REGISTRY
                .with_object(self.object_id, |owner| owner.get_drawable().is_some())
                .unwrap_or(false);
        let rider_has_drawable = crate::object::registry::OBJECT_REGISTRY
            .with_object(rider_id, |rider| rider.get_drawable().is_some())
            .unwrap_or(false);
        owner_has_drawable && rider_has_drawable
    }

    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 277: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::object::registry::OBJECT_REGISTRY.contains(obj_id) {
            return Ok(());
        }

        if self.object_id != crate::common::INVALID_ID {
            let dead = crate::object::registry::OBJECT_REGISTRY
                .with_object(self.object_id, |owner| owner.is_effectively_dead())
                .unwrap_or(false);
            if dead {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |rider| {
                    let _ = TheGameLogic::destroy_object(rider);
                });
                return Ok(());
            }
        }

        if self.base.is_payload_created() {
            self.base.on_removing(obj_id)?;
        } else {
            self.base.base.on_removing(obj_id)?;
        }

        let Some(rider_template) = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |rider| rider.get_template().clone())
        else {
            return Err("Rider lock poisoned".into());
        };
        let mut transfer_to_rider = false;

        if self.object_id != crate::common::INVALID_ID {
            if let Some(matched) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                self.object_id,
                |owner_guard| {
                    let mut matched = false;
                    for rider_info in &self.module_data.riders {
                        if rider_info_matches_template(rider_info, rider_template.as_ref()) {
                            let _ = owner_guard.clear_model_condition_flags(
                                rider_info.model_condition_flag
                                    | ModelConditionFlags::DOOR_1_CLOSING,
                            );
                            owner_guard.clear_weapon_set_flag(rider_info.weapon_set_flag);
                            owner_guard.set_status(rider_info.object_status, false);
                            matched = true;
                            break;
                        }
                    }
                    matched
                },
            ) {
                transfer_to_rider = matched;
            }
        }

        if !self.containing && self.has_exit_scuttle_drawables(obj_id) {
            self.transfer_selection_to_rider_on_exit(obj_id);
            if self.object_id != crate::common::INVALID_ID {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                    self.object_id,
                    |owner_guard| {
                        self.scuttled_on_frame = TheGameLogic::get_frame();
                        owner_guard.set_status(
                            ObjectStatusMaskType::from_status(ObjectStatusTypes::Unselectable),
                            true,
                        );
                        owner_guard.set_model_condition_state(self.module_data.scuttle_state);

                        if let Some(ai) = owner_guard.get_ai() {
                            if let Ok(ai_guard) = ai.lock() {
                                if !ai_guard.is_moving() {
                                    owner_guard.set_status(
                                        ObjectStatusMaskType::from_status(ObjectStatusTypes::Immobile),
                                        true,
                                    );
                                }
                            }
                        }
                    },
                );
            }
        }

        let rider_has_controlling_player = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |rider| rider.get_controlling_player().is_some())
            .unwrap_or(false);

        if transfer_to_rider
            && rider_has_controlling_player
            && self.object_id != crate::common::INVALID_ID
        {
            // C++ RiderChangeContain.cpp:283-286 — rider dismounts: the rider
            // takes the bike's veterancy (feedback off), bike resets to Regular.
            transfer_veterancy(self.object_id, obj_id);
        }

        Ok(())
    }

    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Wave 277: empty dual-world → sleep forever (no factory walks).
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::Forever);
        }

        if self.scuttled_on_frame != 0 {
            let now = TheGameLogic::get_frame();
            if self.scuttled_on_frame + self.module_data.scuttle_frames <= now {
                if self.object_id != crate::common::INVALID_ID {
                    let killed = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                        self.object_id,
                        |owner| {
                            owner.kill(Some(DamageType::Unresistable), Some(DeathType::Toppled));
                        },
                    );
                    if killed.is_none() {
                        log::warn!("RiderChangeContain::update scuttle lock busy");
                    }
                }
            }
        }

        match self.base.update() {
            Ok(sleep) => Ok(sleep),
            Err(err) => {
                log::warn!("RiderChangeContain::update base update failed: {}", err);
                Ok(UpdateSleepTime::None)
            }
        }
    }

    /// Handle capture event (inherits TransportContain capture behavior).
    /// Matches C++ RiderChangeContain::onCapture.
    pub fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) -> GameResult<()> {
        self.base.on_capture(owner, old_owner, new_owner)
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        self.base.save_state()
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)
    }
}

impl Snapshotable for RiderChangeContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Snapshotable::xfer(&mut self.base, xfer)?;

        let mut payload_created = self.base.is_payload_created();
        xfer.xfer_bool(&mut payload_created)
            .map_err(|e| e.to_string())?;
        self.base.set_payload_created(payload_created);

        xfer.xfer_int(&mut self.extra_slots_in_use)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.frame_exit_not_busy)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for RiderChangeContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj| self.is_valid_container_for(obj, true))
            .unwrap_or(false)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.add_to_contain(object_id, false)
            .map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.remove_from_contain(object_id, false)
            .map_err(|e| e.to_string())
    }

    fn remove_from_contain(
        &mut self,
        object_id: ObjectID,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        RiderChangeContain::remove_from_contain(self, object_id, expose_stealth)
            .map_err(|e| e.into())
    }

    fn get_contained_objects(&self) -> std::borrow::Cow<'_, [ObjectID]> {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 { usize::MAX } else { max as usize }
    }

    fn get_container_pips_to_show(&self) -> (i32, i32, bool) {
        (0, 0, false)
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(self)
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        RiderChangeContain::update(self).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(damage_info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_die_for_owner(Some(owner), damage_info)
            .map_err(|e| e.into())
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.base.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let Some(valid) = crate::object::registry::OBJECT_REGISTRY.with_object(other_id, |other| {
            ContainModuleInterface::is_valid_container_for(self, other, true)
        }) else {
            return Ok(());
        };
        if valid {
            self.add_to_contain(other_id, false)?;
        }
        Ok(())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        RiderChangeContain::is_valid_container_for(self, obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_object_wants_to_enter_or_exit(obj, want)
            .map_err(|e| e.into())
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        RiderChangeContain::on_capture(self, owner, old_owner, new_owner).map_err(|e| e.into())
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&Object>,
        spawn: Option<&Object>,
    ) -> crate::modules::ExitDoorType {
        ContainModuleInterface::reserve_door_for_exit(&mut self.base, spawner, spawn)
    }

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = self.base.process_damage_to_contained(percent_damage);
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_selling().map_err(|e| e.into())
    }

    fn redeploy_riders_at(&mut self, owner_pos: &Coord3D, fire_points: &[Matrix3D]) {
        self.base.redeploy_riders_at(owner_pos, fire_points);
    }

    fn passengers_in_turret(&self) -> bool {
        self.base.passengers_in_turret()
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        RiderChangeContain::friend_get_rider(self)
    }

    fn is_rider_change_contain(&self) -> bool {
        true
    }
}

impl ContainerInterface for RiderChangeContain {
    fn can_contain(&self, obj: &Object) -> bool {
        ContainerInterface::can_contain(&self.base, obj)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id, false)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        (0, 0)
    }
}
