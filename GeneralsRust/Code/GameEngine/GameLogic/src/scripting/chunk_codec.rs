//! Scripts.cpp chunk decoding. Mutable script nodes and immutable template keys
//! travel together; nested callbacks never select an ambient ScriptEngine.
use super::core::*;
use super::engine::with_script_engine_ref;
use crate::common::{ALL_KIND_OF, KindOf, ObjectStatusMaskType};
use crate::object::behavior::auto_heal_behavior::parse_kind_of;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};
use std::rc::Rc;

pub use generals_sim::scripting::program::ScriptTemplateLookup;

// TEMPORARY root adapter while canonical world ownership is migrated (hq-r0o8i).
// The frozen definitions, not the engine, are retained by the decoder.
fn current_template_lookup() -> ScriptTemplateLookup {
    with_script_engine_ref(|engine| engine.script_template_lookup()).unwrap_or_default()
}

struct DecodeState<T> {
    node: T,
    // DataChunkInput callback user data is Any ('static), so nested builders
    // share owned immutable definitions rather than retaining an engine borrow.
    templates: Rc<ScriptTemplateLookup>,
}
impl<T> DecodeState<T> {
    fn new(node: T, templates: Rc<ScriptTemplateLookup>) -> Self {
        Self { node, templates }
    }
}
#[derive(Default)]
pub struct ScriptListReadInfo {
    pub lists: Vec<Box<ScriptList>>,
    templates: Option<Rc<ScriptTemplateLookup>>,
}

impl ScriptListReadInfo {
    /// Explicit definitions for a load operation. Construction has no side effects.
    pub fn with_templates(templates: ScriptTemplateLookup) -> Self {
        Self {
            lists: Vec::new(),
            templates: Some(Rc::new(templates)),
        }
    }
    fn templates(&mut self) -> Rc<ScriptTemplateLookup> {
        Rc::clone(
            self.templates
                .get_or_insert_with(|| Rc::new(current_template_lookup())),
        )
    }
}

fn user_data_mut<T: 'static>(user_data: &mut dyn std::any::Any) -> Option<&mut T> {
    user_data.downcast_mut::<T>()
}

pub(crate) fn parse_script_list_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(read_info) = user_data_mut::<ScriptListReadInfo>(user_data) else {
        return false;
    };

    let mut state = DecodeState::new(ScriptList::new(), read_info.templates());
    input.register_parser("Script", &info.label, parse_script_from_list_data_chunk);
    input.register_parser("ScriptGroup", &info.label, parse_group_data_chunk);
    let _ = input.parse(&mut state);
    read_info.lists.push(Box::new(state.node));
    true
}

fn parse_script_from_list_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(list) = user_data_mut::<DecodeState<ScriptList>>(user_data) else {
        return false;
    };
    if let Some(script) = parse_script(input, info, Rc::clone(&list.templates)) {
        list.node.append_script(Box::new(script));
    }
    input.at_end_of_chunk()
}

fn parse_script_from_group_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(group) = user_data_mut::<DecodeState<ScriptGroup>>(user_data) else {
        return false;
    };
    if let Some(script) = parse_script(input, info, Rc::clone(&group.templates)) {
        group.node.append_script(Box::new(script));
    }
    input.at_end_of_chunk()
}

fn parse_group_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(list) = user_data_mut::<DecodeState<ScriptList>>(user_data) else {
        return false;
    };
    let mut group = DecodeState::new(new_script_group(), Rc::clone(&list.templates));
    group.node.group_name = input.read_ascii_string();
    group.node.is_group_active = input.read_byte() != 0;
    if info.version >= 2 {
        group.node.is_group_subroutine = input.read_byte() != 0;
    }
    input.register_parser("Script", &info.label, parse_script_from_group_data_chunk);
    let _ = input.parse(&mut group);
    list.node.append_group(Box::new(group.node));
    true
}

fn parse_script(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    templates: Rc<ScriptTemplateLookup>,
) -> Option<Script> {
    let mut script = Script::new();
    script.script_name = input.read_ascii_string();
    script.comment = input.read_ascii_string();
    script.condition_comment = input.read_ascii_string();
    script.action_comment = input.read_ascii_string();
    script.is_active = input.read_byte() != 0;
    script.is_one_shot = input.read_byte() != 0;
    script.easy = input.read_byte() != 0;
    script.normal = input.read_byte() != 0;
    script.hard = input.read_byte() != 0;
    script.is_subroutine = input.read_byte() != 0;
    if info.version >= 2 {
        script.delay_evaluation_seconds = input.read_int();
    }

    input.register_parser("OrCondition", "Script", parse_or_condition_data_chunk);
    input.register_parser("ScriptAction", "Script", parse_action_data_chunk);
    input.register_parser("ScriptActionFalse", "Script", parse_action_false_data_chunk);
    let mut state = DecodeState::new(script, templates);
    let _ = input.parse(&mut state);
    Some(state.node)
}

fn parse_or_condition_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(script) = user_data_mut::<DecodeState<Script>>(user_data) else {
        return false;
    };
    let mut or_cond = DecodeState::new(OrCondition::new(), Rc::clone(&script.templates));
    input.register_parser("Condition", &info.label, parse_condition_data_chunk);
    let _ = input.parse(&mut or_cond);
    script.node.append_or_condition(Box::new(or_cond.node));
    true
}

fn parse_condition_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(or_cond) = user_data_mut::<DecodeState<OrCondition>>(user_data) else {
        return false;
    };

    let mut condition_type =
        ConditionType::from_u32(input.read_int() as u32).unwrap_or(ConditionType::ConditionFalse);

    if info.version >= 4 {
        let name_key = input.read_name_key();
        condition_type = or_cond
            .templates
            .resolve_condition(condition_type, name_key);
    }

    let num_parms = input.read_int().max(0) as usize;
    let mut condition = Box::new(Condition::new(condition_type));
    condition.num_parms = num_parms;
    for idx in 0..num_parms.min(MAX_PARMS) {
        condition.parameters[idx] = read_parameter(input);
    }

    if condition.condition_type == ConditionType::SkirmishSpecialPowerReady
        && condition.num_parms == 1
    {
        condition.num_parms = 2;
        if let Some(first) = condition.parameters[0].clone() {
            condition.parameters[1] = Some(first);
        }
        condition.parameters[0] = Some(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.to_string(),
        ));
    }

    let mut tail = &mut or_cond.node.first_and;
    while let Some(node) = tail {
        tail = &mut node.next_and_condition;
    }
    *tail = Some(condition);

    input.at_end_of_chunk()
}

fn parse_action(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    templates: &ScriptTemplateLookup,
) -> Box<ScriptAction> {
    let mut action_type =
        ScriptActionType::from_u32(input.read_int() as u32).unwrap_or(ScriptActionType::NoOp);

    if info.version >= 2 {
        let name_key = input.read_name_key();
        action_type = templates.resolve_action(action_type, name_key);
    }

    let mut action = Box::new(ScriptAction::new(action_type));
    let num_parms = input.read_int().max(0) as usize;
    action.num_parms = num_parms;
    for idx in 0..num_parms.min(MAX_PARMS) {
        action.parameters[idx] = read_parameter(input);
    }

    if action.action_type == ScriptActionType::SkirmishFireSpecialPowerAtMostCost
        && action.num_parms == 1
    {
        action.num_parms = 2;
        if let Some(first) = action.parameters[0].clone() {
            action.parameters[1] = Some(first);
        }
        action.parameters[0] = Some(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.to_string(),
        ));
    }

    if action.action_type == ScriptActionType::TeamFollowWaypoints && action.num_parms == 2 {
        action.num_parms = 3;
        action.parameters[2] = Some(Parameter::with_int(ParameterType::Boolean, 1));
    }

    if action.action_type == ScriptActionType::SkirmishBuildBaseDefenseFront
        && action.num_parms == 1
    {
        let flank = action.parameters[0]
            .as_ref()
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        action.parameters[0] = None;
        action.num_parms = 0;
        if flank {
            action.action_type = ScriptActionType::SkirmishBuildBaseDefenseFlank;
        }
    }

    if matches!(
        action.action_type,
        ScriptActionType::NamedSetAttitude | ScriptActionType::TeamSetAttitude
    ) && action.num_parms >= 2
    {
        if let Some(param) = action.parameters[1].clone() {
            if param.param_type == ParameterType::Int {
                action.parameters[1] =
                    Some(Parameter::with_int(ParameterType::AiMood, param.int_value));
            }
        }
    }

    if matches!(
        action.action_type,
        ScriptActionType::MapRevealAtWaypoint | ScriptActionType::MapShroudAtWaypoint
    ) && action.num_parms == 2
    {
        action.num_parms = 3;
        action.parameters[2] = Some(Parameter::new(ParameterType::Side));
    }

    if matches!(
        action.action_type,
        ScriptActionType::MapRevealAll
            | ScriptActionType::MapRevealAllPerm
            | ScriptActionType::MapRevealAllUndoPerm
            | ScriptActionType::MapShroudAll
    ) && action.num_parms == 0
    {
        action.num_parms = 1;
        action.parameters[0] = Some(Parameter::new(ParameterType::Side));
    }

    if action.action_type == ScriptActionType::SpeechPlay && action.num_parms == 1 {
        action.num_parms = 2;
        action.parameters[1] = Some(Parameter::with_int(ParameterType::Boolean, 1));
    }

    if matches!(
        action.action_type,
        ScriptActionType::CameraModSetFinalZoom | ScriptActionType::CameraModSetFinalPitch
    ) && action.num_parms == 1
    {
        action.num_parms = 3;
        action.parameters[1] = Some(Parameter::with_real(ParameterType::Percent, 0.0));
        action.parameters[2] = Some(Parameter::with_real(ParameterType::Percent, 0.0));
    }

    if matches!(
        action.action_type,
        ScriptActionType::MoveCameraTo
            | ScriptActionType::MoveCameraAlongWaypointPath
            | ScriptActionType::CameraLookTowardObject
    ) && action.num_parms == 3
    {
        action.num_parms = 5;
        action.parameters[3] = Some(Parameter::with_real(ParameterType::Real, 0.0));
        action.parameters[4] = Some(Parameter::with_real(ParameterType::Real, 0.0));
    }

    if matches!(
        action.action_type,
        ScriptActionType::ResetCamera
            | ScriptActionType::ZoomCamera
            | ScriptActionType::PitchCamera
            | ScriptActionType::RotateCamera
    ) && action.num_parms == 2
    {
        action.num_parms = 4;
        action.parameters[2] = Some(Parameter::with_real(ParameterType::Real, 0.0));
        action.parameters[3] = Some(Parameter::with_real(ParameterType::Real, 0.0));
    }

    if action.action_type == ScriptActionType::CameraLookTowardWaypoint {
        if action.num_parms == 2 {
            action.num_parms = 5;
            action.parameters[2] = Some(Parameter::with_real(ParameterType::Real, 0.0));
            action.parameters[3] = Some(Parameter::with_real(ParameterType::Real, 0.0));
            action.parameters[4] = Some(Parameter::with_int(ParameterType::Boolean, 0));
        } else if action.num_parms == 4 {
            action.num_parms = 5;
            action.parameters[4] = Some(Parameter::with_int(ParameterType::Boolean, 0));
        }
    }

    action
}

fn parse_action_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(script) = user_data_mut::<DecodeState<Script>>(user_data) else {
        return false;
    };
    let action = parse_action(input, info, &script.templates);
    script.node.append_action(action);
    input.at_end_of_chunk()
}

fn parse_action_false_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(script) = user_data_mut::<DecodeState<Script>>(user_data) else {
        return false;
    };
    let action = parse_action(input, info, &script.templates);
    script.node.append_action_false(action);
    input.at_end_of_chunk()
}

pub fn parse_player_scripts_list_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(read_info) = user_data_mut::<ScriptListReadInfo>(user_data) else {
        return false;
    };
    input.register_parser("ScriptList", &info.label, parse_script_list_data_chunk);
    input.parse(read_info)
}

const K_SCRIPTS_DATA_VERSION_1: u16 = 1;
const K_SCRIPT_LIST_DATA_VERSION_1: u16 = 1;
const K_SCRIPT_GROUP_DATA_VERSION_2: u16 = 2;
const K_SCRIPT_DATA_VERSION_2: u16 = 2;
const K_SCRIPT_ACTION_VERSION_2: u16 = 2;
const K_SCRIPT_CONDITION_VERSION_4: u16 = 4;
const K_SCRIPT_OR_CONDITION_DATA_VERSION_1: u16 = 1;

fn kind_of_index_to_name(index: i32) -> &'static str {
    let idx = index.max(0) as usize;
    if idx >= ALL_KIND_OF.len() {
        return "";
    }
    match ALL_KIND_OF[idx] {
        KindOf::Selectable => "SELECTABLE",
        KindOf::Unit => "UNIT",
        KindOf::Building => "BUILDING",
        KindOf::Vehicle => "VEHICLE",
        KindOf::Infantry => "INFANTRY",
        KindOf::Aircraft => "AIRCRAFT",
        KindOf::Drone => "DRONE",
        KindOf::CliffJumper => "CLIFFJUMPER",
        KindOf::Structure => "STRUCTURE",
        KindOf::Weapon => "WEAPON",
        KindOf::Projectile => "PROJECTILE",
        KindOf::CanSeeThrough => "CAN_SEE_THROUGH",
        KindOf::AlwaysSelectable => "ALWAYS_SELECTABLE",
        KindOf::Crate => "CRATE",
        KindOf::ResourceNode => "RESOURCE_NODE",
        KindOf::SupplySourceOnPreview => "SUPPLY_SOURCE_ON_PREVIEW",
        KindOf::SupplySource => "SUPPLY_SOURCE",
        KindOf::Disguiser => "DISGUISER",
        KindOf::PortableStructure => "PORTABLE_STRUCTURE",
        KindOf::TechBuilding => "TECH_BUILDING",
        KindOf::Bridge => "BRIDGE",
        KindOf::Barrier => "BARRIER",
        KindOf::Civilian => "CIVILIAN",
        KindOf::Destructible => "DESTRUCTIBLE",
        KindOf::CanCrossBridges => "CAN_CROSS_BRIDGES",
        KindOf::Amphibious => "AMPHIBIOUS",
        KindOf::AmphibiousTransport => "AMPHIBIOUS_TRANSPORT",
        KindOf::CanCapture => "CAN_CAPTURE",
        KindOf::Saboteur => "SABOTEUR",
        KindOf::Hacker => "HACKER",
        KindOf::Hero => "HERO",
        KindOf::KeyStructure => "KEY_STRUCTURE",
        KindOf::CommandCenter => "COMMAND_CENTER",
        KindOf::Prison => "PRISON",
        KindOf::CollectsPrisonBounty => "COLLECTS_PRISON_BOUNTY",
        KindOf::PowTruck => "POW_TRUCK",
        KindOf::PowerPlant => "POWER_PLANT",
        KindOf::Refinery => "REFINERY",
        KindOf::Factory => "FACTORY",
        KindOf::Defense => "DEFENSE",
        KindOf::Shrubbery => "SHRUBBERY",
        KindOf::Dozer => "DOZER",
        KindOf::Harvester => "HARVESTER",
        KindOf::Hulk => "HULK",
        KindOf::Salvager => "SALVAGER",
        KindOf::WeaponSalvager => "WEAPON_SALVAGER",
        KindOf::ArmorSalvager => "ARMOR_SALVAGER",
        KindOf::AircraftCarrier => "AIRCRAFT_CARRIER",
        KindOf::FSBarracks => "FS_BARRACKS",
        KindOf::FSWarfactory => "FS_WARFACTORY",
        KindOf::FSAirfield => "FS_AIRFIELD",
        KindOf::FSInternetCenter => "FS_INTERNET_CENTER",
        KindOf::FSPower => "FS_POWER",
        KindOf::FSBaseDefense => "FS_BASE_DEFENSE",
        KindOf::FSSupplyDropzone => "FS_SUPPLY_DROPZONE",
        KindOf::FSSupplyCenter => "FS_SUPPLY_CENTER",
        KindOf::FSSuperweapon => "FS_SUPERWEAPON",
        KindOf::FSStrategyCenter => "FS_STRATEGY_CENTER",
        KindOf::CountsForVictory => "COUNTS_FOR_VICTORY",
        KindOf::Mine => "MINE",
        KindOf::CleanupHazard => "CLEANUP_HAZARD",
        KindOf::HealPad => "HEAL_PAD",
        KindOf::WaveGuide => "WAVE_GUIDE",
        KindOf::BridgeTower => "BRIDGE_TOWER",
        KindOf::Immobile => "IMMOBILE",
        KindOf::BoobyTrap => "BOOBY_TRAP",
        KindOf::CanBeRepulsed => "CAN_BE_REPULSED",
        KindOf::EmpHardened => "EMP_HARDENED",
        KindOf::SpawnsAreTheWeapons => "SPAWNS_ARE_THE_WEAPONS",
        KindOf::IgnoreDockingBones => "IGNORE_DOCKING_BONES",
        KindOf::CanSurrender => "CAN_SURRENDER",
        KindOf::RepairPad => "REPAIR_PAD",
        KindOf::RejectUnmanned => "REJECT_UNMANNED",
        KindOf::IgnoredInGui => "IGNORED_IN_GUI",
        KindOf::MobNexus => "MOB_NEXUS",
        KindOf::Capturable => "CAPTURABLE",
        KindOf::ImmuneToCapture => "IMMUNE_TO_CAPTURE",
        KindOf::CashGenerator => "CASH_GENERATOR",
        KindOf::RebuildHole => "REBUILD_HOLE",
        KindOf::FSTechnology => "FS_TECHNOLOGY",
        KindOf::GarrisonableUntilDestroyed => "GARRISONABLE_UNTIL_DESTROYED",
        KindOf::NoGarrison => "NO_GARRISON",
        _ => "",
    }
}

pub(super) fn write_parameter(parameter: &mut Parameter, writer: &mut DataChunkOutput) {
    writer.write_int(parameter.param_type as i32);
    if parameter.param_type == ParameterType::KindOfParam {
        parameter.string_value = kind_of_index_to_name(parameter.int_value).to_string();
    }
    if parameter.param_type == ParameterType::Coord3D {
        writer.write_real(parameter.coord_value.x);
        writer.write_real(parameter.coord_value.y);
        writer.write_real(parameter.coord_value.z);
    } else {
        writer.write_int(parameter.int_value);
        writer.write_real(parameter.real_value);
        writer.write_ascii_string(&parameter.string_value);
    }
}

fn read_parameter(input: &mut DataChunkInput) -> Option<Parameter> {
    let param_type = ParameterType::from_u32(input.read_int() as u32)?;
    let mut param = Parameter::new(param_type);
    param.initialized = true;
    if param_type == ParameterType::Coord3D {
        let x = input.read_real();
        let y = input.read_real();
        let z = input.read_real();
        param.coord_value = Coord3D::new(x, y, z);
    } else {
        param.int_value = input.read_int();
        param.real_value = input.read_real();
        param.string_value = input.read_ascii_string();
    }

    if param_type == ParameterType::ObjectType && param.string_value.starts_with("Fundamentalist") {
        let old = param.string_value.clone();
        let suffix = old.trim_start_matches("Fundamentalist");
        param.string_value = format!("GLA{suffix}");
    }

    if param_type == ParameterType::Upgrade
        && (param.string_value == "Upgrade_AmericaRangerCaptureBuilding"
            || param.string_value == "Upgrade_ChinaRedguardCaptureBuilding"
            || param.string_value == "Upgrade_GLARebelCaptureBuilding")
    {
        param.string_value = "Upgrade_InfantryCaptureBuilding".to_string();
    }

    if param_type == ParameterType::ObjectStatus {
        if let Some(mask) = ObjectStatusMaskType::from_case_insensitive_name(&param.string_value) {
            param.object_status = mask;
        }
    }

    if param_type == ParameterType::KindOfParam {
        if !param.string_value.is_empty() {
            if let Some(kind) = parse_kind_of(&param.string_value) {
                if let Some(index) = ALL_KIND_OF.iter().position(|k| *k == kind) {
                    param.int_value = index as i32;
                }
            }
        } else {
            param.string_value = kind_of_index_to_name(param.int_value).to_string();
        }
    }

    Some(param)
}

pub(crate) fn write_condition_data_chunk(
    mut condition: Option<&Condition>,
    output: &mut DataChunkOutput,
    templates: &ScriptTemplateLookup,
) {
    while let Some(cur) = condition {
        output.open_data_chunk("Condition", K_SCRIPT_CONDITION_VERSION_4);
        output.write_int(cur.condition_type as i32);
        let key = templates
            .condition_key(cur.condition_type)
            .unwrap_or_else(|| NameKeyGenerator::name_to_key("Bogus"));
        output.write_name_key(key);
        output.write_int(cur.num_parms as i32);
        for index in 0..cur.num_parms {
            if let Some(param) = cur.parameters.get(index).and_then(|p| p.as_ref()) {
                let mut param = param.clone();
                write_parameter(&mut param, output);
            }
        }
        output.close_data_chunk();
        condition = cur.next_and_condition.as_deref();
    }
}

fn write_or_condition_data_chunk(
    mut condition: Option<&OrCondition>,
    output: &mut DataChunkOutput,
    templates: &ScriptTemplateLookup,
) {
    while let Some(cur) = condition {
        output.open_data_chunk("OrCondition", K_SCRIPT_OR_CONDITION_DATA_VERSION_1);
        if let Some(first_and) = cur.first_and.as_deref() {
            write_condition_data_chunk(Some(first_and), output, templates);
        }
        output.close_data_chunk();
        condition = cur.next_or.as_deref();
    }
}

pub(crate) fn write_action_data_chunk(
    mut action: Option<&ScriptAction>,
    output: &mut DataChunkOutput,
    label: &str,
    templates: &ScriptTemplateLookup,
) {
    while let Some(cur) = action {
        output.open_data_chunk(label, K_SCRIPT_ACTION_VERSION_2);
        output.write_int(cur.action_type as i32);
        let key = templates
            .action_key(cur.action_type)
            .unwrap_or_else(|| NameKeyGenerator::name_to_key("Bogus"));
        output.write_name_key(key);
        output.write_int(cur.num_parms as i32);
        for index in 0..cur.num_parms {
            if let Some(param) = cur.parameters.get(index).and_then(|p| p.as_ref()) {
                let mut param = param.clone();
                write_parameter(&mut param, output);
            }
        }
        output.close_data_chunk();
        action = cur.next_action.as_deref();
    }
}

fn write_script_data_chunk(
    mut script: Option<&Script>,
    output: &mut DataChunkOutput,
    templates: &ScriptTemplateLookup,
) {
    while let Some(cur) = script {
        output.open_data_chunk("Script", K_SCRIPT_DATA_VERSION_2);
        output.write_ascii_string(&cur.script_name);
        output.write_ascii_string(&cur.comment);
        output.write_ascii_string(&cur.condition_comment);
        output.write_ascii_string(&cur.action_comment);
        output.write_byte(cur.is_active as u8);
        output.write_byte(cur.is_one_shot as u8);
        output.write_byte(cur.easy as u8);
        output.write_byte(cur.normal as u8);
        output.write_byte(cur.hard as u8);
        output.write_byte(cur.is_subroutine as u8);
        output.write_int(cur.delay_evaluation_seconds);
        if let Some(condition) = cur.condition.as_deref() {
            write_or_condition_data_chunk(Some(condition), output, templates);
        }
        if let Some(action) = cur.action.as_deref() {
            write_action_data_chunk(Some(action), output, "ScriptAction", templates);
        }
        if let Some(action_false) = cur.action_false.as_deref() {
            write_action_data_chunk(Some(action_false), output, "ScriptActionFalse", templates);
        }
        output.close_data_chunk();
        script = cur.next_script.as_deref();
    }
}

fn write_group_data_chunk(
    mut group: Option<&ScriptGroup>,
    output: &mut DataChunkOutput,
    templates: &ScriptTemplateLookup,
) {
    while let Some(cur) = group {
        output.open_data_chunk("ScriptGroup", K_SCRIPT_GROUP_DATA_VERSION_2);
        output.write_ascii_string(&cur.group_name);
        output.write_byte(cur.is_group_active as u8);
        output.write_byte(cur.is_group_subroutine as u8);
        if let Some(script) = cur.first_script.as_deref() {
            write_script_data_chunk(Some(script), output, templates);
        }
        output.close_data_chunk();
        group = cur.next_group.as_deref();
    }
}

pub(crate) fn write_scripts_data_chunk(
    output: &mut DataChunkOutput,
    script_lists: &[Option<&ScriptList>],
    templates: &ScriptTemplateLookup,
) {
    output.open_data_chunk("PlayerScriptsList", K_SCRIPTS_DATA_VERSION_1);
    for list in script_lists {
        output.open_data_chunk("ScriptList", K_SCRIPT_LIST_DATA_VERSION_1);
        if let Some(list) = list {
            write_script_list_data_chunk(list, output, templates);
        }
        output.close_data_chunk();
    }
    output.close_data_chunk();
}

fn write_script_list_data_chunk(
    list: &ScriptList,
    output: &mut DataChunkOutput,
    templates: &ScriptTemplateLookup,
) {
    if let Some(script) = list.first_script.as_deref() {
        write_script_data_chunk(Some(script), output, templates);
    }
    if let Some(group) = list.first_group.as_deref() {
        write_group_data_chunk(Some(group), output, templates);
    }
}
