//! Scripts.cpp chunk decoding. Mutable script nodes and immutable template keys
//! travel together; nested callbacks never select an ambient ScriptEngine.
use super::core::*;
use super::engine::with_script_engine_ref;
use game_engine::common::system::{DataChunkInfo, DataChunkInput};
use std::rc::Rc;

/// Ordered definition keys from one engine, with no mutable simulation state.
#[derive(Clone, Default)]
pub struct ScriptTemplateLookup {
    condition_keys: Vec<u32>,
    action_keys: Vec<u32>,
}
impl ScriptTemplateLookup {
    /// Capture ordered definitions from the explicitly supplied engine.
    pub fn from_engine(engine: &super::engine::ScriptEngine) -> Self {
        engine.script_template_lookup()
    }

    pub(crate) fn from_keys(condition_keys: Vec<u32>, action_keys: Vec<u32>) -> Self {
        Self {
            condition_keys,
            action_keys,
        }
    }
    /// C++ Scripts.cpp:1663-1680: prefer the stored ordinal, then first key match.
    pub fn resolve_condition(&self, stored: ConditionType, key: u32) -> ConditionType {
        if self.condition_keys.get(stored as usize) == Some(&key) {
            return stored;
        }
        self.condition_keys
            .iter()
            .enumerate()
            .find_map(|(i, candidate)| {
                (*candidate == key)
                    .then(|| ConditionType::from_u32(i as u32))
                    .flatten()
            })
            .unwrap_or(ConditionType::ConditionFalse)
    }
    /// C++ Scripts.cpp:2423-2449: preserve ordered action rematching and NO_OP.
    pub fn resolve_action(&self, stored: ScriptActionType, key: u32) -> ScriptActionType {
        if self.action_keys.get(stored as usize) == Some(&key) {
            return stored;
        }
        self.action_keys
            .iter()
            .enumerate()
            .find_map(|(i, candidate)| {
                (*candidate == key)
                    .then(|| ScriptActionType::from_u32(i as u32))
                    .flatten()
            })
            .unwrap_or(ScriptActionType::NoOp)
    }
}

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
    let mut group = DecodeState::new(ScriptGroup::new(), Rc::clone(&list.templates));
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
        condition.parameters[idx] = Parameter::read_parameter(input);
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
        action.parameters[idx] = Parameter::read_parameter(input);
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_ordinal_wins_over_earlier_duplicate_key() {
        let lookup = ScriptTemplateLookup::from_keys(vec![7, 7], vec![9, 9]);
        assert_eq!(
            lookup.resolve_condition(ConditionType::Counter, 7),
            ConditionType::Counter
        );
        assert_eq!(
            lookup.resolve_action(ScriptActionType::SetFlag, 9),
            ScriptActionType::SetFlag
        );
    }
    #[test]
    fn rematching_uses_first_valid_enum_index() {
        let lookup = ScriptTemplateLookup::from_keys(vec![7, 7], vec![9, 9]);
        assert_eq!(
            lookup.resolve_condition(ConditionType::ConditionTrue, 7),
            ConditionType::from_u32(0).unwrap()
        );
        assert_eq!(
            lookup.resolve_action(ScriptActionType::Victory, 9),
            ScriptActionType::from_u32(0).unwrap()
        );
    }
    #[test]
    fn unknown_and_missing_keys_keep_cpp_false_noop_policy() {
        for lookup in [
            ScriptTemplateLookup::default(),
            ScriptTemplateLookup::from_keys(vec![7], vec![9]),
        ] {
            assert_eq!(
                lookup.resolve_condition(ConditionType::ConditionTrue, 88),
                ConditionType::ConditionFalse
            );
            assert_eq!(
                lookup.resolve_action(ScriptActionType::Victory, 88),
                ScriptActionType::NoOp
            );
        }
    }
}
