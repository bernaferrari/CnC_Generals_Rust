//! Scripts.cpp chunk decoding and writing for map and SCB script lists.
use super::program::*;
use crate::kind_of::classic::{ALL_KIND_OF, KindOf, parse_kind_of};
use crate::object::ObjectStatusMaskType;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};
use generals_map::{BinaryReader, DecodeError, DecodeResult, parse_chunk_sequence, read_chunk_header};
use glam::Vec3 as Coord3D;
use std::collections::HashMap;
use std::rc::Rc;

pub struct ScriptListReadInfo {
    pub lists: Vec<Box<ScriptList>>,
    templates: Result<Rc<ScriptTemplateLookup>, fn() -> ScriptTemplateLookup>,
    new_group: Box<dyn Fn() -> ScriptGroup>,
}

impl ScriptListReadInfo {
    /// Explicit definitions for a load operation. Construction has no side effects.
    pub fn with_templates(
        templates: ScriptTemplateLookup,
        new_group: impl Fn() -> ScriptGroup + 'static,
    ) -> Self {
        Self {
            lists: Vec::new(),
            templates: Ok(Rc::new(templates)),
            new_group: Box::new(new_group),
        }
    }

    /// Legacy entry points may provide definitions when the first list is decoded.
    pub fn with_template_source(
        source: fn() -> ScriptTemplateLookup,
        new_group: impl Fn() -> ScriptGroup + 'static,
    ) -> Self {
        Self {
            lists: Vec::new(),
            templates: Err(source),
            new_group: Box::new(new_group),
        }
    }

    fn templates(&mut self) -> Rc<ScriptTemplateLookup> {
        match &self.templates {
            Ok(templates) => Rc::clone(templates),
            Err(source) => {
                let templates = Rc::new(source());
                self.templates = Ok(Rc::clone(&templates));
                templates
            }
        }
    }

    /// Decode the contents of one PlayerScriptsList from a map or SCB file.
    pub fn decode(
        &mut self,
        data: &[u8],
        toc: &HashMap<u32, String>,
        intern_name: &mut dyn FnMut(&str) -> u32,
    ) -> DecodeResult<()> {
        let mut lists = Vec::new();
        parse_chunk_sequence(data, toc, |label, _, payload| {
            if label == "ScriptList" {
                let templates = self.templates();
                let mut decoder = ScriptDecoder {
                    toc,
                    intern_name,
                    templates: &templates,
                    new_group: &self.new_group,
                };
                lists.push(Box::new(decoder.read_list(payload)?));
            }
            Ok(())
        })?;
        self.lists.extend(lists);
        Ok(())
    }
}

pub fn parse_player_scripts_list_chunk(
    input: &mut DataChunkInput,
    _info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(read_info) = user_data.downcast_mut::<ScriptListReadInfo>() else {
        return false;
    };
    match input.read_chunk_body(|data, toc, names| read_info.decode(data, toc, names)) {
        Some(Ok(())) => true,
        Some(Err(error)) => {
            log::warn!("Failed to decode PlayerScriptsList: {error}");
            false
        }
        None => false,
    }
}

struct ScriptDecoder<'a> {
    toc: &'a HashMap<u32, String>,
    intern_name: &'a mut dyn FnMut(&str) -> u32,
    templates: &'a ScriptTemplateLookup,
    new_group: &'a dyn Fn() -> ScriptGroup,
}

impl ScriptDecoder<'_> {
    fn read_list(&mut self, data: &[u8]) -> DecodeResult<ScriptList> {
        let mut list = ScriptList::new();
        let mut reader = BinaryReader::new(data);
        let mut scripts = &mut list.first_script;
        let mut groups = &mut list.first_group;
        while let Some(chunk) = read_chunk_header(&mut reader, self.toc)? {
            let payload = reader.read_bytes(chunk.size)?;
            match chunk.label.as_str() {
                "Script" => {
                    scripts = &mut scripts
                        .insert(Box::new(self.read_script(payload, chunk.version)?))
                        .next_script;
                }
                "ScriptGroup" => {
                    groups = &mut groups
                        .insert(Box::new(self.read_group(payload, chunk.version)?))
                        .next_group;
                }
                _ => {}
            }
        }
        Ok(list)
    }

    fn read_group(&mut self, data: &[u8], version: u16) -> DecodeResult<ScriptGroup> {
        let mut reader = BinaryReader::new(data);
        let mut group = (self.new_group)();
        group.group_name = reader.read_ascii_string()?;
        group.is_group_active = reader.read_u8()? != 0;
        group.is_group_subroutine = version >= 2 && reader.read_u8()? != 0;
        let mut scripts = &mut group.first_script;
        while let Some(chunk) = read_chunk_header(&mut reader, self.toc)? {
            let payload = reader.read_bytes(chunk.size)?;
            if chunk.label == "Script" {
                scripts = &mut scripts
                    .insert(Box::new(self.read_script(payload, chunk.version)?))
                    .next_script;
            }
        }
        Ok(group)
    }

    fn read_script(&mut self, data: &[u8], version: u16) -> DecodeResult<Script> {
        let mut reader = BinaryReader::new(data);
        let mut script = Script::new();
        script.script_name = reader.read_ascii_string()?;
        script.comment = reader.read_ascii_string()?;
        script.condition_comment = reader.read_ascii_string()?;
        script.action_comment = reader.read_ascii_string()?;
        script.is_active = reader.read_u8()? != 0;
        script.is_one_shot = reader.read_u8()? != 0;
        script.easy = reader.read_u8()? != 0;
        script.normal = reader.read_u8()? != 0;
        script.hard = reader.read_u8()? != 0;
        script.is_subroutine = reader.read_u8()? != 0;
        if version >= 2 {
            script.delay_evaluation_seconds = reader.read_i32()?;
        }

        let mut conditions = &mut script.condition;
        let mut actions = &mut script.action;
        let mut false_actions = &mut script.action_false;
        while let Some(chunk) = read_chunk_header(&mut reader, self.toc)? {
            let payload = reader.read_bytes(chunk.size)?;
            match chunk.label.as_str() {
                "OrCondition" => {
                    conditions = &mut conditions
                        .insert(Box::new(self.read_or_condition(payload)?))
                        .next_or;
                }
                "ScriptAction" => {
                    actions = &mut actions
                        .insert(Box::new(self.read_action(payload, chunk.version)?))
                        .next_action;
                }
                "ScriptActionFalse" => {
                    false_actions = &mut false_actions
                        .insert(Box::new(self.read_action(payload, chunk.version)?))
                        .next_action;
                }
                _ => {}
            }
        }
        Ok(script)
    }

    fn read_or_condition(&mut self, data: &[u8]) -> DecodeResult<OrCondition> {
        let mut or_condition = OrCondition::new();
        let mut reader = BinaryReader::new(data);
        let mut conditions = &mut or_condition.first_and;
        while let Some(chunk) = read_chunk_header(&mut reader, self.toc)? {
            let payload = reader.read_bytes(chunk.size)?;
            if chunk.label == "Condition" {
                conditions = &mut conditions
                    .insert(Box::new(self.read_condition(payload, chunk.version)?))
                    .next_and_condition;
            }
        }
        Ok(or_condition)
    }

    fn read_condition(&mut self, data: &[u8], version: u16) -> DecodeResult<Condition> {
        let mut reader = BinaryReader::new(data);
        let mut kind =
            ConditionType::from_u32(reader.read_u32()?).unwrap_or(ConditionType::ConditionFalse);
        if version >= 4 {
            let key = self.read_name_key(&mut reader)?;
            kind = self.templates.resolve_condition(kind, key);
        }

        let mut condition = Condition::new(kind);
        condition.num_parms = read_parameters(&mut reader, &mut condition.parameters)?;
        if kind == ConditionType::SkirmishSpecialPowerReady && condition.num_parms == 1 {
            condition.num_parms = 2;
            condition.parameters[1] = condition.parameters[0].take();
            condition.parameters[0] = Some(Parameter::with_string(
                ParameterType::Side,
                THIS_PLAYER.to_string(),
            ));
        }
        Ok(condition)
    }

    fn read_action(&mut self, data: &[u8], version: u16) -> DecodeResult<ScriptAction> {
        let mut reader = BinaryReader::new(data);
        let mut kind = ScriptActionType::from_u32(reader.read_u32()?).unwrap_or(ScriptActionType::NoOp);
        if version >= 2 {
            let key = self.read_name_key(&mut reader)?;
            kind = self.templates.resolve_action(kind, key);
        }

        let mut action = ScriptAction::new(kind);
        action.num_parms = read_parameters(&mut reader, &mut action.parameters)?;
        upgrade_action_parameters(&mut action);
        Ok(action)
    }

    fn read_name_key(&mut self, reader: &mut BinaryReader<'_>) -> DecodeResult<u32> {
        // C++ DataChunkInput::readNameKey shifts a signed file id before the TOC lookup.
        let id = (reader.read_i32()? >> 8) as u32;
        let name = self.toc.get(&id).map(String::as_str).unwrap_or("");
        Ok((self.intern_name)(name))
    }
}

fn read_parameters(
    reader: &mut BinaryReader<'_>,
    slots: &mut [Option<Parameter>],
) -> DecodeResult<usize> {
    let count = reader.read_i32()? as usize;
    let used = slots
        .get_mut(..count)
        .ok_or_else(|| DecodeError::new("Script parameter count exceeded maximum capacity"))?;
    for slot in used {
        *slot = Some(read_parameter(reader)?);
    }
    Ok(count)
}

fn upgrade_action_parameters(action: &mut ScriptAction) {
    if action.action_type == ScriptActionType::SkirmishFireSpecialPowerAtMostCost
        && action.num_parms == 1
    {
        action.num_parms = 2;
        action.parameters[1] = action.parameters[0].take();
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

pub fn write_parameter(parameter: &mut Parameter, writer: &mut DataChunkOutput) {
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

fn read_parameter(reader: &mut BinaryReader<'_>) -> DecodeResult<Parameter> {
    let ordinal = reader.read_u32()?;
    let param_type = ParameterType::from_u32(ordinal)
        .ok_or_else(|| DecodeError::new(format!("Unknown ParameterType value {ordinal}")))?;
    let mut param = Parameter::new(param_type);
    param.initialized = true;
    if param_type == ParameterType::Coord3D {
        let x = reader.read_f32()?;
        let y = reader.read_f32()?;
        let z = reader.read_f32()?;
        param.coord_value = Coord3D::new(x, y, z);
    } else {
        param.int_value = reader.read_i32()?;
        param.real_value = reader.read_f32()?;
        param.string_value = reader.read_ascii_string()?;
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

    Ok(param)
}

pub fn write_condition_data_chunk(
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

pub fn write_action_data_chunk(
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

pub fn write_scripts_data_chunk(
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
