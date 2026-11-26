//! Executable script definitions, parameter caches, and ordered program trees.

use crate::object::ObjectStatusMaskType;
use crate::{GameLogicError, GameLogicResult};
use serde::{Deserialize, Serialize};

pub use generals_game_domain::script_opcodes::{ConditionType, ParameterType, ScriptActionType};
pub use glam::Vec3 as Coord3D;

pub const MAX_PARMS: usize = 12;
pub const THIS_TEAM: &str = "<This Team>";
pub const ANY_TEAM: &str = "<Any Team>";
pub const THIS_OBJECT: &str = "<This Object>";
pub const ANY_OBJECT: &str = "<Any Object>";
pub const THIS_PLAYER: &str = "<This Player>";
pub const LOCAL_PLAYER: &str = "<Local Player>";
pub const THE_PLAYER: &str = "ThePlayer";
pub const TEAM_THE_PLAYER: &str = "teamThePlayer";
pub const THIS_PLAYER_ENEMY: &str = "<This Player's Enemy>";

/// Comparison types matching C++ Parameter comparison types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ComparisonType {
    LessThan = 0,
    LessEqual = 1,
    Equal = 2,
    GreaterEqual = 3,
    Greater = 4,
    NotEqual = 5,
}

impl ComparisonType {
    /// Decode the comparison ordinal authored in a script parameter.
    pub fn from_i32(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::LessThan,
            1 => Self::LessEqual,
            2 => Self::Equal,
            3 => Self::GreaterEqual,
            4 => Self::Greater,
            5 => Self::NotEqual,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod comparison_type_tests {
    use super::ComparisonType;

    #[test]
    fn comparison_ordinals_round_trip_without_accepting_unknown_values() {
        for comparison in [
            ComparisonType::LessThan,
            ComparisonType::LessEqual,
            ComparisonType::Equal,
            ComparisonType::GreaterEqual,
            ComparisonType::Greater,
            ComparisonType::NotEqual,
        ] {
            assert_eq!(
                ComparisonType::from_i32(comparison as i32),
                Some(comparison)
            );
        }
        assert_eq!(ComparisonType::from_i32(-1), None);
        assert_eq!(ComparisonType::from_i32(6), None);
    }
}

/// Relation types matching C++ Parameter relation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum RelationType {
    Enemy = 0,   // ENEMIES
    Neutral = 1, // NEUTRAL
    Friend = 2,  // ALLIES
}

/// Parameter class matching C++ Parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub param_type: ParameterType,
    pub initialized: bool,
    pub int_value: i32,
    pub real_value: f32,
    pub string_value: String,
    pub coord_value: Coord3D,
    pub object_status: ObjectStatusMaskType,
}

impl Parameter {
    pub fn new(param_type: ParameterType) -> Self {
        Self {
            param_type,
            initialized: false,
            int_value: 0,
            real_value: 0.0,
            string_value: String::new(),
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_int(param_type: ParameterType, value: i32) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: value,
            real_value: 0.0,
            string_value: String::new(),
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_real(param_type: ParameterType, value: f32) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: 0,
            real_value: value,
            string_value: String::new(),
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_string(param_type: ParameterType, value: String) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: 0,
            real_value: 0.0,
            string_value: value,
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_coord(param_type: ParameterType, value: Coord3D) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: 0,
            real_value: 0.0,
            string_value: String::new(),
            coord_value: value,
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn get_int(&self) -> i32 {
        self.int_value
    }

    pub fn get_real(&self) -> f32 {
        self.real_value
    }

    pub fn get_string(&self) -> &str {
        &self.string_value
    }

    pub fn get_coord(&self) -> Coord3D {
        self.coord_value
    }

    pub fn get_object_status(&self) -> ObjectStatusMaskType {
        self.object_status
    }

    pub fn get_parameter_type(&self) -> ParameterType {
        self.param_type
    }

    /// C++ `ScriptConditions::playerFromParam` stores a single player mask
    /// or the failed lookup marker in the original SIDE parameter.
    pub fn cache_condition_player_mask(&mut self, resolved_mask: Option<u32>) {
        const MISSING_PLAYER_MASK: u32 = 0xFFFF_0000;

        if self.int_value == 0 {
            self.int_value = match resolved_mask {
                None => MISSING_PLAYER_MASK,
                Some(_) if self.string_value == THIS_PLAYER_ENEMY => 0,
                Some(mask) => mask,
            } as i32;
        }
    }

    pub fn qualify(&mut self, qualifier: &str, player_template_name: &str, new_player_name: &str) {
        match self.param_type {
            ParameterType::Side => {
                let mut tmp = self.string_value.clone();
                tmp.push_str(qualifier);
                if tmp == player_template_name {
                    self.string_value = new_player_name.to_string();
                }
            }
            ParameterType::Team => {
                if self.string_value != THIS_TEAM {
                    self.string_value.push_str(qualifier);
                }
            }
            ParameterType::Script
            | ParameterType::Counter
            | ParameterType::Flag
            | ParameterType::ScriptSubroutine => {
                self.string_value.push_str(qualifier);
            }
            _ => {}
        }
    }
}

/// Condition class matching C++ Condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub condition_type: ConditionType,
    pub num_parms: usize,
    pub parameters: Vec<Option<Parameter>>,
    pub next_and_condition: Option<Box<Condition>>,
    pub has_warnings: bool,
    pub custom_data: i32,
    pub custom_frame: u32,
}

impl Condition {
    pub fn new(condition_type: ConditionType) -> Self {
        Self {
            condition_type,
            num_parms: 0,
            parameters: vec![None; MAX_PARMS],
            next_and_condition: None,
            has_warnings: false,
            custom_data: 0,
            custom_frame: 0,
        }
    }

    pub fn get_condition_type(&self) -> ConditionType {
        self.condition_type
    }

    pub fn get_parameter(&self, index: usize) -> Option<&Parameter> {
        if index < self.parameters.len() {
            self.parameters[index].as_ref()
        } else {
            None
        }
    }

    pub fn get_num_parameters(&self) -> usize {
        self.num_parms
    }

    pub fn get_next(&self) -> Option<&Condition> {
        self.next_and_condition.as_ref().map(|c| c.as_ref())
    }

    pub fn set_next_condition(&mut self, condition: Option<Box<Condition>>) {
        self.next_and_condition = condition;
    }

    pub fn add_parameter(&mut self, parameter: Parameter) -> GameLogicResult<()> {
        if self.num_parms >= MAX_PARMS {
            return Err(GameLogicError::Configuration(
                "Maximum number of parameters exceeded".to_string(),
            ));
        }

        self.parameters[self.num_parms] = Some(parameter);
        self.num_parms += 1;
        Ok(())
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<Condition> {
        let mut new_condition = Condition {
            condition_type: self.condition_type,
            num_parms: self.num_parms,
            parameters: self.parameters.clone(),
            next_and_condition: None,
            has_warnings: self.has_warnings,
            custom_data: self.custom_data,
            custom_frame: self.custom_frame,
        };

        for index in 0..new_condition.num_parms {
            if let Some(param) = new_condition
                .parameters
                .get_mut(index)
                .and_then(|p| p.as_mut())
            {
                param.qualify(qualifier, player_template_name, new_player_name);
            }
        }

        if let Some(next) = &self.next_and_condition {
            new_condition.next_and_condition =
                Some(next.duplicate_and_qualify(qualifier, player_template_name, new_player_name));
        }

        Box::new(new_condition)
    }
}

/// OR Condition class matching C++ OrCondition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrCondition {
    pub next_or: Option<Box<OrCondition>>,
    pub first_and: Option<Box<Condition>>,
}

impl OrCondition {
    pub fn new() -> Self {
        Self {
            next_or: None,
            first_and: None,
        }
    }

    pub fn get_next_or_condition(&self) -> Option<&OrCondition> {
        self.next_or.as_ref().map(|c| c.as_ref())
    }

    pub fn get_first_and_condition(&self) -> Option<&Condition> {
        self.first_and.as_ref().map(|c| c.as_ref())
    }

    pub fn set_next_or_condition(&mut self, condition: Option<Box<OrCondition>>) {
        self.next_or = condition;
    }

    pub fn set_first_and_condition(&mut self, condition: Option<Box<Condition>>) {
        self.first_and = condition;
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<OrCondition> {
        let mut new_or = OrCondition::new();
        if let Some(first_and) = &self.first_and {
            new_or.first_and = Some(first_and.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }

        let mut current_src = &self.next_or;
        let mut current_dst = &mut new_or;
        while let Some(src) = current_src {
            let mut next_or = OrCondition::new();
            if let Some(first_and) = &src.first_and {
                next_or.first_and = Some(first_and.duplicate_and_qualify(
                    qualifier,
                    player_template_name,
                    new_player_name,
                ));
            }
            current_dst.next_or = Some(Box::new(next_or));
            current_dst = current_dst.next_or.as_mut().unwrap();
            current_src = &src.next_or;
        }

        Box::new(new_or)
    }
}

/// Script Action class matching C++ ScriptAction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAction {
    pub action_type: ScriptActionType,
    pub num_parms: usize,
    pub parameters: Vec<Option<Parameter>>,
    pub next_action: Option<Box<ScriptAction>>,
    pub has_warnings: bool,
}

impl ScriptAction {
    pub fn new(action_type: ScriptActionType) -> Self {
        Self {
            action_type,
            num_parms: 0,
            parameters: vec![None; MAX_PARMS],
            next_action: None,
            has_warnings: false,
        }
    }

    pub fn get_action_type(&self) -> ScriptActionType {
        self.action_type
    }

    pub fn get_parameter(&self, index: usize) -> Option<&Parameter> {
        if index < self.parameters.len() {
            self.parameters[index].as_ref()
        } else {
            None
        }
    }

    pub fn get_num_parameters(&self) -> usize {
        self.num_parms
    }

    pub fn get_next(&self) -> Option<&ScriptAction> {
        self.next_action.as_ref().map(|a| a.as_ref())
    }

    pub fn set_next_action(&mut self, action: Option<Box<ScriptAction>>) {
        self.next_action = action;
    }

    pub fn add_parameter(&mut self, parameter: Parameter) -> GameLogicResult<()> {
        if self.num_parms >= MAX_PARMS {
            return Err(GameLogicError::Configuration(
                "Maximum number of parameters exceeded".to_string(),
            ));
        }

        self.parameters[self.num_parms] = Some(parameter);
        self.num_parms += 1;
        Ok(())
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<ScriptAction> {
        let mut new_action = ScriptAction {
            action_type: self.action_type,
            num_parms: self.num_parms,
            parameters: self.parameters.clone(),
            next_action: None,
            has_warnings: self.has_warnings,
        };

        for index in 0..new_action.num_parms {
            if let Some(param) = new_action
                .parameters
                .get_mut(index)
                .and_then(|p| p.as_mut())
            {
                param.qualify(qualifier, player_template_name, new_player_name);
            }
        }

        if let Some(next) = &self.next_action {
            new_action.next_action =
                Some(next.duplicate_and_qualify(qualifier, player_template_name, new_player_name));
        }

        Box::new(new_action)
    }
}

impl Script {
    pub fn append_action(&mut self, action: Box<ScriptAction>) {
        let mut current = self.action.as_mut();
        while let Some(node) = current {
            if node.next_action.is_none() {
                node.next_action = Some(action);
                return;
            }
            current = node.next_action.as_mut();
        }
        self.action = Some(action);
    }

    pub fn append_action_false(&mut self, action: Box<ScriptAction>) {
        let mut current = self.action_false.as_mut();
        while let Some(node) = current {
            if node.next_action.is_none() {
                node.next_action = Some(action);
                return;
            }
            current = node.next_action.as_mut();
        }
        self.action_false = Some(action);
    }

    pub fn append_or_condition(&mut self, condition: Box<OrCondition>) {
        let mut current = self.condition.as_mut();
        while let Some(node) = current {
            if node.next_or.is_none() {
                node.next_or = Some(condition);
                return;
            }
            current = node.next_or.as_mut();
        }
        self.condition = Some(condition);
    }
}

/// Script class matching C++ Script
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub script_name: String,
    pub comment: String,
    pub condition_comment: String,
    pub action_comment: String,
    pub delay_evaluation_seconds: i32,
    pub is_active: bool,
    pub is_one_shot: bool,
    pub is_subroutine: bool,
    pub easy: bool,
    pub normal: bool,
    pub hard: bool,
    pub condition: Option<Box<OrCondition>>,
    pub action: Option<Box<ScriptAction>>,
    pub action_false: Option<Box<ScriptAction>>,
    pub next_script: Option<Box<Script>>,
    // Runtime fields
    pub frame_to_evaluate_at: u32,
    pub has_warnings: bool,
    pub condition_team_name: String,
    pub condition_time: f32,
    pub cur_time: f32,
    pub condition_executed_count: i32,
}

include!("script_duplicate.rs");

impl Script {
    pub fn new() -> Self {
        Self {
            script_name: String::new(),
            comment: String::new(),
            condition_comment: String::new(),
            action_comment: String::new(),
            delay_evaluation_seconds: 0,
            is_active: true,
            is_one_shot: true,
            is_subroutine: false,
            easy: true,
            normal: true,
            hard: true,
            condition: None,
            action: None,
            action_false: None,
            next_script: None,
            frame_to_evaluate_at: 0,
            has_warnings: false,
            condition_team_name: String::new(),
            condition_time: 0.0,
            cur_time: 0.0,
            condition_executed_count: 0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.script_name
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn is_one_shot(&self) -> bool {
        self.is_one_shot
    }

    pub fn is_subroutine(&self) -> bool {
        self.is_subroutine
    }

    pub fn get_or_condition(&self) -> Option<&OrCondition> {
        self.condition.as_ref().map(|c| c.as_ref())
    }

    pub fn get_action(&self) -> Option<&ScriptAction> {
        self.action.as_ref().map(|a| a.as_ref())
    }

    pub fn get_false_action(&self) -> Option<&ScriptAction> {
        self.action_false.as_ref().map(|a| a.as_ref())
    }

    pub fn get_next(&self) -> Option<&Script> {
        self.next_script.as_ref().map(|s| s.as_ref())
    }

    pub fn set_name(&mut self, name: String) {
        self.script_name = name;
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub fn set_one_shot(&mut self, one_shot: bool) {
        self.is_one_shot = one_shot;
    }

    pub fn set_subroutine(&mut self, subroutine: bool) {
        self.is_subroutine = subroutine;
    }

    pub fn set_or_condition(&mut self, condition: Option<Box<OrCondition>>) {
        self.condition = condition;
    }

    pub fn set_action(&mut self, action: Option<Box<ScriptAction>>) {
        self.action = action;
    }

    pub fn set_false_action(&mut self, action: Option<Box<ScriptAction>>) {
        self.action_false = action;
    }

    pub fn set_next_script(&mut self, script: Option<Box<Script>>) {
        self.next_script = script;
    }

    pub fn increment_condition_count(&mut self) {
        self.condition_executed_count += 1;
    }

    pub fn add_to_condition_time(&mut self, time: f32) {
        self.condition_time += time;
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<Script> {
        let mut new_script = Script::new();
        new_script.script_name = format!("{}{}", self.script_name, qualifier);
        new_script.comment = self.comment.clone();
        new_script.condition_comment = self.condition_comment.clone();
        new_script.action_comment = self.action_comment.clone();
        new_script.delay_evaluation_seconds = self.delay_evaluation_seconds;
        new_script.is_active = self.is_active;
        new_script.is_one_shot = self.is_one_shot;
        new_script.is_subroutine = self.is_subroutine;
        new_script.easy = self.easy;
        new_script.normal = self.normal;
        new_script.hard = self.hard;

        if let Some(condition) = &self.condition {
            new_script.condition = Some(condition.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }
        if let Some(action) = &self.action {
            new_script.action = Some(action.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }
        if let Some(action_false) = &self.action_false {
            new_script.action_false = Some(action_false.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }

        Box::new(new_script)
    }
}

/// Script Group class matching C++ ScriptGroup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptGroup {
    pub first_script: Option<Box<Script>>,
    pub group_name: String,
    pub is_group_active: bool,
    pub is_group_subroutine: bool,
    pub next_group: Option<Box<ScriptGroup>>,
    pub has_warnings: bool,
}

impl ScriptGroup {
    pub fn new(id: u32) -> Self {
        Self {
            first_script: None,
            group_name: format!("Script Group {}", id),
            is_group_active: true,
            is_group_subroutine: false,
            next_group: None,
            has_warnings: false,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.group_name
    }

    pub fn is_active(&self) -> bool {
        self.is_group_active
    }

    pub fn is_subroutine(&self) -> bool {
        self.is_group_subroutine
    }

    pub fn get_script(&self) -> Option<&Script> {
        self.first_script.as_ref().map(|s| s.as_ref())
    }

    pub fn get_next(&self) -> Option<&ScriptGroup> {
        self.next_group.as_ref().map(|g| g.as_ref())
    }

    pub fn set_name(&mut self, name: String) {
        self.group_name = name;
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_group_active = active;
    }

    pub fn set_subroutine(&mut self, subroutine: bool) {
        self.is_group_subroutine = subroutine;
    }

    pub fn append_script(&mut self, script: Box<Script>) {
        let mut current = self.first_script.as_mut();
        while let Some(node) = current {
            if node.next_script.is_none() {
                node.next_script = Some(script);
                return;
            }
            current = node.next_script.as_mut();
        }
        self.first_script = Some(script);
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<ScriptGroup> {
        let mut new_group = ScriptGroup {
            first_script: None,
            group_name: format!("{}{}", self.group_name, qualifier),
            is_group_active: self.is_group_active,
            is_group_subroutine: self.is_group_subroutine,
            next_group: None,
            has_warnings: self.has_warnings,
        };

        let mut current_src = &self.first_script;
        let mut tail: Option<&mut Box<Script>> = None;
        while let Some(src) = current_src {
            let dup = src.duplicate_and_qualify(qualifier, player_template_name, new_player_name);
            if let Some(tail_ref) = tail {
                tail_ref.next_script = Some(dup);
                tail = tail_ref.next_script.as_mut();
            } else {
                new_group.first_script = Some(dup);
                tail = new_group.first_script.as_mut();
            }
            current_src = &src.next_script;
        }

        Box::new(new_group)
    }
}

/// Script List class matching C++ ScriptList
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptList {
    pub first_group: Option<Box<ScriptGroup>>,
    pub first_script: Option<Box<Script>>,
}

impl ScriptList {
    pub fn new() -> Self {
        Self {
            first_group: None,
            first_script: None,
        }
    }

    pub fn get_script_group(&self) -> Option<&ScriptGroup> {
        self.first_group.as_ref().map(|g| g.as_ref())
    }

    pub fn get_script(&self) -> Option<&Script> {
        self.first_script.as_ref().map(|s| s.as_ref())
    }

    pub fn append_script(&mut self, script: Box<Script>) {
        let mut current = self.first_script.as_mut();
        while let Some(node) = current {
            if node.next_script.is_none() {
                node.next_script = Some(script);
                return;
            }
            current = node.next_script.as_mut();
        }
        self.first_script = Some(script);
    }

    /// C++ `ScriptList::addScript(pScr, ndx)` — insert at `ndx` (0 prepends).
    pub fn add_script(&mut self, mut script: Box<Script>, mut ndx: i32) {
        script.next_script = None;
        if ndx <= 0 || self.first_script.is_none() {
            script.next_script = self.first_script.take();
            self.first_script = Some(script);
            return;
        }
        let mut cur = self.first_script.as_mut().expect("first_script checked");
        ndx -= 1;
        while ndx > 0 && cur.next_script.is_some() {
            cur = cur.next_script.as_mut().expect("next_script checked");
            ndx -= 1;
        }
        script.next_script = cur.next_script.take();
        cur.next_script = Some(script);
    }

    pub fn append_group(&mut self, group: Box<ScriptGroup>) {
        let mut current = self.first_group.as_mut();
        while let Some(node) = current {
            if node.next_group.is_none() {
                node.next_group = Some(group);
                return;
            }
            current = node.next_group.as_mut();
        }
        self.first_group = Some(group);
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<ScriptList> {
        let mut new_list = Box::new(ScriptList::new());

        let mut current_src_group = &self.first_group;
        let mut group_tail: Option<&mut Box<ScriptGroup>> = None;
        while let Some(src) = current_src_group {
            let dup = src.duplicate_and_qualify(qualifier, player_template_name, new_player_name);
            if let Some(tail_ref) = group_tail {
                tail_ref.next_group = Some(dup);
                group_tail = tail_ref.next_group.as_mut();
            } else {
                new_list.first_group = Some(dup);
                group_tail = new_list.first_group.as_mut();
            }
            current_src_group = &src.next_group;
        }

        let mut current_src_script = &self.first_script;
        let mut script_tail: Option<&mut Box<Script>> = None;
        while let Some(src) = current_src_script {
            let dup = src.duplicate_and_qualify(qualifier, player_template_name, new_player_name);
            if let Some(tail_ref) = script_tail {
                tail_ref.next_script = Some(dup);
                script_tail = tail_ref.next_script.as_mut();
            } else {
                new_list.first_script = Some(dup);
                script_tail = new_list.first_script.as_mut();
            }
            current_src_script = &src.next_script;
        }

        new_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::with_int(ParameterType::Int, 42);
        assert_eq!(param.get_int(), 42);
        assert_eq!(param.get_parameter_type(), ParameterType::Int);
        assert!(param.initialized);
    }

    #[test]
    fn test_script_action_creation() {
        let mut action = ScriptAction::new(ScriptActionType::Victory);
        assert_eq!(action.get_action_type(), ScriptActionType::Victory);
        assert_eq!(action.get_num_parameters(), 0);

        let param = Parameter::with_string(ParameterType::TextString, "Victory!".to_string());
        action.add_parameter(param).unwrap();
        assert_eq!(action.get_num_parameters(), 1);
    }

    #[test]
    fn test_condition_creation() {
        let mut condition = Condition::new(ConditionType::PlayerAllDestroyed);
        assert_eq!(
            condition.get_condition_type(),
            ConditionType::PlayerAllDestroyed
        );
        assert_eq!(condition.get_num_parameters(), 0);

        let param = Parameter::with_string(ParameterType::Side, "Player_1".to_string());
        condition.add_parameter(param).unwrap();
        assert_eq!(condition.get_num_parameters(), 1);
    }

    #[test]
    fn test_script_creation() {
        let script = Script::new();
        assert!(script.is_active());
        assert!(script.is_one_shot());
        assert!(!script.is_subroutine());
        assert_eq!(script.get_name(), "");
    }

    #[test]
    fn test_coord3d() {
        let coord = Coord3D::new(1.0, 2.0, 3.0);
        assert_eq!(coord.x, 1.0);
        assert_eq!(coord.y, 2.0);
        assert_eq!(coord.z, 3.0);
    }

    #[test]
    fn add_script_zero_prepends_like_cpp() {
        let mut list = ScriptList::new();
        let mut first = Script::new();
        first.set_name("MapScript".into());
        list.append_script(Box::new(first));
        let mut mp = Script::new();
        mp.set_name("MultiplayerVictory".into());
        list.add_script(Box::new(mp), 0);
        assert_eq!(list.get_script().unwrap().get_name(), "MultiplayerVictory");
        assert_eq!(
            list.get_script().unwrap().get_next().unwrap().get_name(),
            "MapScript"
        );
    }
}

/// Ordered definition keys for one script read/write operation.
///
/// Keys retain the receiving engine's namespace. Decoders must pass that same
/// namespace to `DataChunkInput::with_namespace`; the numeric snapshot alone
/// cannot be moved to another namespace.
#[derive(Clone, Default)]
pub struct ScriptTemplateLookup {
    condition_keys: Vec<u32>,
    action_keys: Vec<u32>,
}
impl ScriptTemplateLookup {
    pub fn from_keys(condition_keys: Vec<u32>, action_keys: Vec<u32>) -> Self {
        Self {
            condition_keys,
            action_keys,
        }
    }
    pub fn condition_key(&self, kind: ConditionType) -> Option<u32> {
        self.condition_keys.get(kind as usize).copied()
    }

    pub fn action_key(&self, kind: ScriptActionType) -> Option<u32> {
        self.action_keys.get(kind as usize).copied()
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

#[cfg(test)]
mod template_lookup_tests {
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
