//! INI Script Parser
//!
//! This module provides a complete INI parser for loading mission scripts from .map files.
//! It handles the hierarchical structure of ScriptList -> ScriptGroup -> Script with
//! conditions and actions.

use super::core::*;
use crate::{GameLogicError, GameLogicResult};
use std::collections::HashMap;
use std::str::FromStr;

/// Line information for error reporting
#[derive(Debug, Clone)]
struct LineInfo {
    line_number: usize,
    content: String,
}

impl LineInfo {
    fn new(line_number: usize, content: String) -> Self {
        Self {
            line_number,
            content,
        }
    }
}

/// INI parsing context
struct ParseContext {
    lines: Vec<LineInfo>,
    current_index: usize,
}

impl ParseContext {
    fn new(content: &str) -> Self {
        let lines = content
            .lines()
            .enumerate()
            .map(|(i, line)| LineInfo::new(i + 1, line.to_string()))
            .collect();

        Self {
            lines,
            current_index: 0,
        }
    }

    fn current_line(&self) -> Option<&LineInfo> {
        self.lines.get(self.current_index)
    }

    fn advance(&mut self) {
        self.current_index += 1;
    }

    fn peek(&self) -> Option<&LineInfo> {
        self.lines.get(self.current_index)
    }

    fn is_at_end(&self) -> bool {
        self.current_index >= self.lines.len()
    }

    fn error(&self, message: &str) -> GameLogicError {
        if let Some(line) = self.current_line() {
            GameLogicError::Configuration(format!(
                "Parse error at line {}: {} ({})",
                line.line_number, message, line.content
            ))
        } else {
            GameLogicError::Configuration(format!("Parse error: {}", message))
        }
    }
}

/// INI Script Parser
pub struct IniScriptParser {
    /// Parsed script lists by name
    script_lists: HashMap<String, ScriptList>,
    /// Parse errors encountered
    errors: Vec<String>,
    /// Parse warnings
    warnings: Vec<String>,
}

impl IniScriptParser {
    /// Create a new INI script parser
    pub fn new() -> Self {
        Self {
            script_lists: HashMap::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Parse script content from a string
    pub fn parse(&mut self, content: &str) -> GameLogicResult<()> {
        let mut context = ParseContext::new(content);

        while !context.is_at_end() {
            let line = match context.current_line() {
                Some(line) => line,
                None => break,
            };

            let trimmed = line.content.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            // Check for ScriptList
            if trimmed.starts_with("ScriptList") {
                match self.parse_script_list(&mut context) {
                    Ok(script_list) => {
                        let name = script_list
                            .first_group
                            .as_ref()
                            .map(|g| g.group_name.clone())
                            .unwrap_or_else(|| "unnamed".to_string());
                        self.script_lists.insert(name, script_list);
                    }
                    Err(e) => {
                        self.errors.push(format!("{}", e));
                        // Try to recover by advancing to next major section
                        self.skip_to_next_section(&mut context);
                    }
                }
            } else {
                context.advance();
            }
        }

        if !self.errors.is_empty() {
            return Err(GameLogicError::Configuration(format!(
                "Script parsing failed with {} errors",
                self.errors.len()
            )));
        }

        Ok(())
    }

    /// Parse a ScriptList block
    #[allow(unused_assignments)]
    fn parse_script_list(&mut self, context: &mut ParseContext) -> GameLogicResult<ScriptList> {
        let line = context
            .current_line()
            .ok_or_else(|| context.error("Unexpected end of file"))?;

        let parts: Vec<&str> = line.content.trim().split_whitespace().collect();
        if parts.len() < 2 {
            return Err(context.error("ScriptList requires a name"));
        }

        let _list_name = parts[1];
        context.advance();

        let mut script_list = ScriptList::new();
        let mut last_group: Option<Box<ScriptGroup>> = None;
        let mut first_group: Option<Box<ScriptGroup>> = None;

        // Parse script groups until we hit EndScriptList
        while !context.is_at_end() {
            let line = context
                .current_line()
                .ok_or_else(|| context.error("Unexpected end of file in ScriptList"))?;

            let trimmed = line.content.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            if trimmed == "EndScriptList" || trimmed.starts_with("EndScriptList") {
                context.advance();
                break;
            }

            if trimmed.starts_with("ScriptGroup") {
                let group = self.parse_script_group(context)?;

                if first_group.is_none() {
                    first_group = Some(group.clone());
                }

                if let Some(mut prev) = last_group {
                    prev.next_group = Some(group.clone());
                    last_group = Some(prev);
                }

                last_group = Some(group);
            } else if trimmed.starts_with("Script") && !trimmed.starts_with("ScriptGroup") {
                // Orphan script without a group - create implicit group
                let mut group = ScriptGroup::new();
                group.group_name = "DefaultGroup".to_string();

                let script = self.parse_script(context)?;
                group.first_script = Some(script);

                if first_group.is_none() {
                    first_group = Some(Box::new(group.clone()));
                }

                last_group = Some(Box::new(group));
            } else {
                context.advance();
            }
        }

        script_list.first_group = first_group;

        Ok(script_list)
    }

    /// Parse a ScriptGroup block
    #[allow(unused_assignments)]
    fn parse_script_group(
        &mut self,
        context: &mut ParseContext,
    ) -> GameLogicResult<Box<ScriptGroup>> {
        let line = context
            .current_line()
            .ok_or_else(|| context.error("Unexpected end of file"))?;

        let parts: Vec<&str> = line.content.trim().split_whitespace().collect();
        if parts.len() < 2 {
            return Err(context.error("ScriptGroup requires a name"));
        }

        let group_name = parts[1].to_string();
        context.advance();

        let mut group = ScriptGroup::new();
        group.group_name = group_name;

        let mut last_script: Option<Box<Script>> = None;
        let mut first_script: Option<Box<Script>> = None;

        // Parse scripts until we hit EndScriptGroup
        while !context.is_at_end() {
            let line = context
                .current_line()
                .ok_or_else(|| context.error("Unexpected end of file in ScriptGroup"))?;

            let trimmed = line.content.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            if trimmed == "EndScriptGroup" || trimmed.starts_with("EndScriptGroup") {
                context.advance();
                break;
            }

            if trimmed.starts_with("Script ") {
                let script = self.parse_script(context)?;

                if first_script.is_none() {
                    first_script = Some(script.clone());
                }

                if let Some(mut prev) = last_script {
                    prev.next_script = Some(script.clone());
                    last_script = Some(prev);
                }

                last_script = Some(script);
            } else if trimmed.starts_with("IsActive") {
                let value = self.parse_boolean_value(trimmed)?;
                group.is_group_active = value;
                context.advance();
            } else if trimmed.starts_with("IsSubroutine") {
                let value = self.parse_boolean_value(trimmed)?;
                group.is_group_subroutine = value;
                context.advance();
            } else {
                context.advance();
            }
        }

        group.first_script = first_script;

        Ok(Box::new(group))
    }

    /// Parse a Script block
    fn parse_script(&mut self, context: &mut ParseContext) -> GameLogicResult<Box<Script>> {
        let line = context
            .current_line()
            .ok_or_else(|| context.error("Unexpected end of file"))?;

        let parts: Vec<&str> = line.content.trim().split_whitespace().collect();
        if parts.len() < 2 {
            return Err(context.error("Script requires a name"));
        }

        let script_name = parts[1].to_string();
        context.advance();

        let mut script = Script::new();
        script.script_name = script_name;

        // Parse script properties until we hit EndScript
        while !context.is_at_end() {
            let line = context
                .current_line()
                .ok_or_else(|| context.error("Unexpected end of file in Script"))?;

            let trimmed = line.content.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            if trimmed == "EndScript" || trimmed.starts_with("EndScript") {
                context.advance();
                break;
            }

            // Parse script properties
            if trimmed.starts_with("Conditions") {
                script.condition = Some(self.parse_conditions(context)?);
            } else if trimmed.starts_with("Actions") {
                script.action = Some(self.parse_actions(context)?);
            } else if trimmed.starts_with("ActionsIsFalse") || trimmed.starts_with("ActionsFalse") {
                script.action_false = Some(self.parse_actions(context)?);
            } else if trimmed.starts_with("IsActive") {
                script.is_active = self.parse_boolean_value(trimmed)?;
                context.advance();
            } else if trimmed.starts_with("IsOneShot")
                || trimmed.starts_with("DeactivateUponSuccess")
            {
                script.is_one_shot = self.parse_boolean_value(trimmed)?;
                context.advance();
            } else if trimmed.starts_with("IsSubroutine") {
                script.is_subroutine = self.parse_boolean_value(trimmed)?;
                context.advance();
            } else if trimmed.starts_with("EvaluationInterval") {
                script.delay_evaluation_seconds = self.parse_integer_value(trimmed)?;
                context.advance();
            } else if trimmed.starts_with("Easy") {
                script.easy = self.parse_boolean_value(trimmed)?;
                context.advance();
            } else if trimmed.starts_with("Normal") {
                script.normal = self.parse_boolean_value(trimmed)?;
                context.advance();
            } else if trimmed.starts_with("Hard") {
                script.hard = self.parse_boolean_value(trimmed)?;
                context.advance();
            } else {
                context.advance();
            }
        }

        Ok(Box::new(script))
    }

    /// Parse conditions block (OR conditions containing AND conditions)
    #[allow(unused_assignments)]
    fn parse_conditions(
        &mut self,
        context: &mut ParseContext,
    ) -> GameLogicResult<Box<OrCondition>> {
        context.advance(); // Skip the "Conditions = " line

        let mut first_or: Option<Box<OrCondition>> = None;
        let mut last_or: Option<Box<OrCondition>> = None;

        while !context.is_at_end() {
            let line = context
                .current_line()
                .ok_or_else(|| context.error("Unexpected end of file in Conditions"))?;

            let trimmed = line.content.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            if trimmed == "EndConditions" || trimmed.starts_with("EndConditions") {
                context.advance();
                break;
            }

            // Check for AND condition block
            if trimmed.starts_with("Condition") {
                let or_condition = self.parse_and_condition_block(context)?;

                if first_or.is_none() {
                    first_or = Some(or_condition.clone());
                }

                if let Some(mut prev) = last_or {
                    prev.next_or = Some(or_condition.clone());
                    last_or = Some(prev);
                }

                last_or = Some(or_condition);
            } else {
                context.advance();
            }
        }

        first_or.ok_or_else(|| context.error("No conditions found in Conditions block"))
    }

    /// Parse an AND condition block
    #[allow(unused_assignments)]
    fn parse_and_condition_block(
        &mut self,
        context: &mut ParseContext,
    ) -> GameLogicResult<Box<OrCondition>> {
        context.advance(); // Skip the "Condition = AND" line

        let mut or_condition = OrCondition::new();
        let mut first_and: Option<Box<Condition>> = None;
        let mut last_and: Option<Box<Condition>> = None;

        while !context.is_at_end() {
            let line = context
                .current_line()
                .ok_or_else(|| context.error("Unexpected end of file in AND condition block"))?;

            let trimmed = line.content.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            if trimmed == "EndCondition" || trimmed.starts_with("EndCondition") {
                context.advance();
                break;
            }

            // Parse individual condition
            let condition = self.parse_single_condition(trimmed)?;
            context.advance();

            if first_and.is_none() {
                first_and = Some(condition.clone());
            }

            if let Some(mut prev) = last_and {
                prev.next_and_condition = Some(condition.clone());
                last_and = Some(prev);
            }

            last_and = Some(condition);
        }

        or_condition.first_and = first_and;

        Ok(Box::new(or_condition))
    }

    /// Parse a single condition line
    fn parse_single_condition(&mut self, line: &str) -> GameLogicResult<Box<Condition>> {
        // Parse format: CONDITION_TYPE param1 param2 param3 ...
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Err(GameLogicError::Configuration(format!(
                "Empty condition line"
            )));
        }

        let condition_name = parts[0];
        let condition_type = self.parse_condition_type(condition_name)?;

        let mut condition = Condition::new(condition_type);

        // Sugar: `PLAYER_HAS_BUILDINGS player type count` / `PLAYER_HAS_UNITS player type count`
        // maps to `PlayerHasObjectComparison player type >= count`.
        let upper_name = condition_name.to_uppercase();
        if matches!(
            upper_name.as_str(),
            "PLAYER_HAS_BUILDINGS" | "PLAYER_HAS_UNITS"
        ) {
            if parts.len() >= 4 {
                // C++ order: player, comparison, count, object_type
                condition.add_parameter(self.parse_parameter(parts[1])?)?;
                // GreaterEqual (matches ScriptConditions.cpp comparison encoding)
                condition.add_parameter(Parameter::with_int(ParameterType::Int, 3))?;
                condition.add_parameter(self.parse_parameter(parts[3])?)?;
                condition.add_parameter(self.parse_parameter(parts[2])?)?;
                return Ok(Box::new(condition));
            }
        }

        fn is_coord_token(token: &str) -> bool {
            token.starts_with("X:")
                || token.starts_with("Y:")
                || token.starts_with("Z:")
                || token.contains("X:")
                || token.contains("Y:")
                || token.contains("Z:")
        }

        // Parse parameters (with coord token grouping).
        let mut idx = 1usize;
        let mut added = 0usize;
        while idx < parts.len() {
            if added >= MAX_PARMS {
                self.warnings.push(format!(
                    "Too many parameters for condition {}: maximum is {}",
                    condition_name, MAX_PARMS
                ));
                break;
            }

            let token = parts[idx];
            if is_coord_token(token) {
                let mut coord_parts = vec![token];
                idx += 1;
                while idx < parts.len() && is_coord_token(parts[idx]) {
                    coord_parts.push(parts[idx]);
                    idx += 1;
                }
                let joined = coord_parts.join(" ");
                let parameter = self.parse_parameter(&joined)?;
                condition.add_parameter(parameter)?;
            } else {
                let parameter = self.parse_parameter(token)?;
                condition.add_parameter(parameter)?;
                idx += 1;
            }
            added += 1;
        }

        Ok(Box::new(condition))
    }

    /// Parse actions block
    #[allow(unused_assignments)]
    fn parse_actions(&mut self, context: &mut ParseContext) -> GameLogicResult<Box<ScriptAction>> {
        context.advance(); // Skip the "Actions = " line

        let mut first_action: Option<Box<ScriptAction>> = None;
        let mut last_action: Option<Box<ScriptAction>> = None;

        while !context.is_at_end() {
            let line = context
                .current_line()
                .ok_or_else(|| context.error("Unexpected end of file in Actions"))?;

            let trimmed = line.content.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                context.advance();
                continue;
            }

            if trimmed == "EndActions" || trimmed.starts_with("EndActions") {
                context.advance();
                break;
            }

            // Parse individual action
            let action = self.parse_single_action(trimmed)?;
            context.advance();

            if first_action.is_none() {
                first_action = Some(action.clone());
            }

            if let Some(mut prev) = last_action {
                prev.next_action = Some(action.clone());
                last_action = Some(prev);
            }

            last_action = Some(action);
        }

        first_action.ok_or_else(|| context.error("No actions found in Actions block"))
    }

    /// Parse a single action line
    fn parse_single_action(&mut self, line: &str) -> GameLogicResult<Box<ScriptAction>> {
        // Parse format: ACTION_TYPE param1 param2 param3 ...
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Err(GameLogicError::Configuration(format!("Empty action line")));
        }

        let action_name = parts[0];
        let action_type = self.parse_action_type(action_name)?;

        let mut action = ScriptAction::new(action_type);

        fn is_coord_token(token: &str) -> bool {
            token.starts_with("X:")
                || token.starts_with("Y:")
                || token.starts_with("Z:")
                || token.contains("X:")
                || token.contains("Y:")
                || token.contains("Z:")
        }

        // Parse parameters (with coord token grouping).
        let mut idx = 1usize;
        let mut added = 0usize;
        while idx < parts.len() {
            if added >= MAX_PARMS {
                self.warnings.push(format!(
                    "Too many parameters for action {}: maximum is {}",
                    action_name, MAX_PARMS
                ));
                break;
            }

            let token = parts[idx];
            if is_coord_token(token) {
                let mut coord_parts = vec![token];
                idx += 1;
                while idx < parts.len() && is_coord_token(parts[idx]) {
                    coord_parts.push(parts[idx]);
                    idx += 1;
                }
                let joined = coord_parts.join(" ");
                let parameter = self.parse_parameter(&joined)?;
                action.add_parameter(parameter)?;
            } else {
                let parameter = self.parse_parameter(token)?;
                action.add_parameter(parameter)?;
                idx += 1;
            }
            added += 1;
        }

        Ok(Box::new(action))
    }

    /// Parse a parameter from a string
    fn parse_parameter(&self, param_str: &str) -> GameLogicResult<Parameter> {
        let trimmed = param_str.trim();

        // Check for quoted string
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            let content = &trimmed[1..trimmed.len() - 1];
            return Ok(Parameter::with_string(
                ParameterType::TextString,
                content.to_string(),
            ));
        }

        // Check for coordinate (X:100 Y:200 Z:0)
        if trimmed.contains("X:") || trimmed.contains("Y:") || trimmed.contains("Z:") {
            return self.parse_coordinate(trimmed);
        }

        // Check for boolean
        if trimmed.eq_ignore_ascii_case("TRUE") || trimmed.eq_ignore_ascii_case("YES") {
            return Ok(Parameter::with_int(ParameterType::Boolean, 1));
        }
        if trimmed.eq_ignore_ascii_case("FALSE") || trimmed.eq_ignore_ascii_case("NO") {
            return Ok(Parameter::with_int(ParameterType::Boolean, 0));
        }

        // Try to parse as integer
        if let Ok(value) = trimmed.parse::<i32>() {
            return Ok(Parameter::with_int(ParameterType::Int, value));
        }

        // Try to parse as float
        if let Ok(value) = trimmed.parse::<f32>() {
            return Ok(Parameter::with_real(ParameterType::Real, value));
        }

        // Default to string (could be a name reference)
        Ok(Parameter::with_string(
            ParameterType::TextString,
            trimmed.to_string(),
        ))
    }

    /// Parse coordinate parameter
    fn parse_coordinate(&self, coord_str: &str) -> GameLogicResult<Parameter> {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut z = 0.0f32;

        // Split by whitespace and parse X: Y: Z: components
        for part in coord_str.split_whitespace() {
            if let Some(x_val) = part.strip_prefix("X:") {
                x = x_val.parse().map_err(|_| {
                    GameLogicError::Configuration(format!("Invalid X coordinate: {}", x_val))
                })?;
            } else if let Some(y_val) = part.strip_prefix("Y:") {
                y = y_val.parse().map_err(|_| {
                    GameLogicError::Configuration(format!("Invalid Y coordinate: {}", y_val))
                })?;
            } else if let Some(z_val) = part.strip_prefix("Z:") {
                z = z_val.parse().map_err(|_| {
                    GameLogicError::Configuration(format!("Invalid Z coordinate: {}", z_val))
                })?;
            }
        }

        Ok(Parameter::with_coord(
            ParameterType::Coord3D,
            Coord3D::new(x, y, z),
        ))
    }

    /// Parse boolean value from line
    fn parse_boolean_value(&self, line: &str) -> GameLogicResult<bool> {
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() < 2 {
            return Err(GameLogicError::Configuration(format!(
                "Invalid boolean format: {}",
                line
            )));
        }

        let value = parts[1].trim();
        Ok(value.eq_ignore_ascii_case("TRUE") || value.eq_ignore_ascii_case("YES") || value == "1")
    }

    /// Parse integer value from line
    fn parse_integer_value(&self, line: &str) -> GameLogicResult<i32> {
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() < 2 {
            return Err(GameLogicError::Configuration(format!(
                "Invalid integer format: {}",
                line
            )));
        }

        let value = parts[1].trim();
        value
            .parse()
            .map_err(|_| GameLogicError::Configuration(format!("Invalid integer value: {}", value)))
    }

    /// Parse condition type from name
    fn parse_condition_type(&mut self, name: &str) -> GameLogicResult<ConditionType> {
        match name.to_uppercase().as_str() {
            "FALSE" => Ok(ConditionType::ConditionFalse),
            "TRUE" => Ok(ConditionType::ConditionTrue),
            "COUNTER" => Ok(ConditionType::Counter),
            "FLAG" => Ok(ConditionType::Flag),
            "TIMER_EXPIRED" => Ok(ConditionType::TimerExpired),
            // Parser keeps the legacy 'PLAYER_ALIVE player TRUE/FALSE' spellings by mapping onto
            // PlayerAllDestroyed with an optional invert parameter (handled in the evaluator).
            "PLAYER_ALIVE" => Ok(ConditionType::PlayerAllDestroyed),
            "PLAYER_ALL_DESTROYED" => Ok(ConditionType::PlayerAllDestroyed),
            "PLAYER_ALL_BUILDFACILITIES_DESTROYED" => {
                Ok(ConditionType::PlayerAllBuildfacilitiesDestroyed)
            }
            "PLAYER_HAS_BUILDINGS" => Ok(ConditionType::PlayerHasObjectComparison),
            "PLAYER_HAS_UNITS" => Ok(ConditionType::PlayerHasObjectComparison),
            "TEAM_INSIDE_AREA_PARTIALLY" => Ok(ConditionType::TeamInsideAreaPartially),
            "TEAM_DESTROYED" => Ok(ConditionType::TeamDestroyed),
            "CAMERA_MOVEMENT_FINISHED" => Ok(ConditionType::CameraMovementFinished),
            "TEAM_HAS_UNITS" => Ok(ConditionType::TeamHasUnits),
            "NAMED_DESTROYED" => Ok(ConditionType::NamedDestroyed),
            "NAMED_NOT_DESTROYED" => Ok(ConditionType::NamedNotDestroyed),
            "NAMED_INSIDE_AREA" => Ok(ConditionType::NamedInsideArea),
            _ => {
                self.warnings
                    .push(format!("Unknown condition type: {}", name));
                Ok(ConditionType::ConditionTrue) // Default to always true
            }
        }
    }

    /// Parse action type from name
    fn parse_action_type(&mut self, name: &str) -> GameLogicResult<ScriptActionType> {
        match name.to_uppercase().as_str() {
            "NO_OP" | "NOOP" => Ok(ScriptActionType::NoOp),
            "VICTORY" => Ok(ScriptActionType::Victory),
            "DEFEAT" => Ok(ScriptActionType::Defeat),
            "SET_FLAG" => Ok(ScriptActionType::SetFlag),
            "SET_COUNTER" => Ok(ScriptActionType::SetCounter),
            "INCREMENT_COUNTER" => Ok(ScriptActionType::IncrementCounter),
            "DECREMENT_COUNTER" => Ok(ScriptActionType::DecrementCounter),
            "SET_TIMER" => Ok(ScriptActionType::SetTimer),
            "SET_MILLISECOND_TIMER" => Ok(ScriptActionType::SetMillisecondTimer),
            "SET_RANDOM_MSEC_TIMER" => Ok(ScriptActionType::SetRandomMsecTimer),
            "DISPLAY_TEXT" => Ok(ScriptActionType::DisplayText),
            "DISPLAY_CINEMATIC_TEXT" => Ok(ScriptActionType::DisplayCinematicText),
            "PLAY_SOUND_EFFECT" => Ok(ScriptActionType::PlaySoundEffect),
            "PLAY_SOUND_EFFECT_AT" => Ok(ScriptActionType::PlaySoundEffectAt),
            "MOVE_CAMERA_TO" => Ok(ScriptActionType::MoveCameraTo),
            "MOVE_CAMERA_ALONG_WAYPOINT_PATH" => Ok(ScriptActionType::MoveCameraAlongWaypointPath),
            "CAMERA_LETTERBOX_BEGIN" => Ok(ScriptActionType::CameraLetterboxBegin),
            "CAMERA_LETTERBOX_END" => Ok(ScriptActionType::CameraLetterboxEnd),
            "ROTATE_CAMERA" => Ok(ScriptActionType::RotateCamera),
            "RESET_CAMERA" => Ok(ScriptActionType::ResetCamera),
            "CREATE_OBJECT" => Ok(ScriptActionType::CreateObject),
            "CREATE_NAMED_ON_TEAM_AT_WAYPOINT" => Ok(ScriptActionType::CreateNamedOnTeamAtWaypoint),
            "CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT" => {
                Ok(ScriptActionType::CreateUnnamedOnTeamAtWaypoint)
            }
            "CREATE_REINFORCEMENT_TEAM" => Ok(ScriptActionType::CreateReinforcementTeam),
            "TEAM_ATTACK_TEAM" => Ok(ScriptActionType::TeamAttackTeam),
            "TEAM_FOLLOW_WAYPOINTS" => Ok(ScriptActionType::TeamFollowWaypoints),
            "MOVE_NAMED_UNIT_TO" => Ok(ScriptActionType::MoveNamedUnitTo),
            "NAMED_ATTACK_NAMED" => Ok(ScriptActionType::NamedAttackNamed),
            "TEAM_ATTACK_NAMED" => Ok(ScriptActionType::TeamAttackNamed),
            "NAMED_ATTACK_TEAM" => Ok(ScriptActionType::NamedAttackTeam),
            "PLAYER_KILL" => Ok(ScriptActionType::PlayerKill),
            "PLAYER_GIVE_MONEY" => Ok(ScriptActionType::PlayerGiveMoney),
            "NAMED_DELETE" => Ok(ScriptActionType::NamedDelete),
            "TEAM_DELETE" => Ok(ScriptActionType::TeamDelete),
            "ENABLE_SCRIPT" => Ok(ScriptActionType::EnableScript),
            "DISABLE_SCRIPT" => Ok(ScriptActionType::DisableScript),
            "CALL_SUBROUTINE" => Ok(ScriptActionType::CallSubroutine),
            "MOVIE_PLAY_FULLSCREEN" => Ok(ScriptActionType::MoviePlayFullscreen),
            "MAP_REVEAL_AT_WAYPOINT" => Ok(ScriptActionType::MapRevealAtWaypoint),
            "MAP_REVEAL_ALL" => Ok(ScriptActionType::MapRevealAll),
            "MAP_SHROUD_AT_WAYPOINT" => Ok(ScriptActionType::MapShroudAtWaypoint),
            "MAP_SHROUD_ALL" => Ok(ScriptActionType::MapShroudAll),
            _ => {
                self.warnings.push(format!("Unknown action type: {}", name));
                Ok(ScriptActionType::NoOp) // Default to no-op
            }
        }
    }

    /// Skip to next major section after parse error
    fn skip_to_next_section(&self, context: &mut ParseContext) {
        while !context.is_at_end() {
            let line = match context.current_line() {
                Some(line) => line,
                None => break,
            };

            let trimmed = line.content.trim();

            if trimmed.starts_with("ScriptList")
                || trimmed.starts_with("EndScriptList")
                || trimmed.starts_with("ScriptGroup")
                || trimmed.starts_with("EndScriptGroup")
            {
                break;
            }

            context.advance();
        }
    }

    /// Get all parsed script lists
    pub fn get_script_lists(&self) -> &HashMap<String, ScriptList> {
        &self.script_lists
    }

    /// Get a specific script list by name
    pub fn get_script_list(&self, name: &str) -> Option<&ScriptList> {
        self.script_lists.get(name)
    }

    /// Get parse errors
    pub fn get_errors(&self) -> &[String] {
        &self.errors
    }

    /// Get parse warnings
    pub fn get_warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Check if parsing had errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get a formatted error report
    pub fn get_error_report(&self) -> String {
        let mut report = String::new();

        if !self.errors.is_empty() {
            report.push_str(&format!("=== ERRORS ({}) ===\n", self.errors.len()));
            for (i, error) in self.errors.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, error));
            }
            report.push('\n');
        }

        if !self.warnings.is_empty() {
            report.push_str(&format!("=== WARNINGS ({}) ===\n", self.warnings.len()));
            for (i, warning) in self.warnings.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, warning));
            }
        }

        report
    }
}

impl Default for IniScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_script() {
        let content = r#"
ScriptList TestScripts
  ScriptGroup Group1
    Script Script_001
      Conditions = OR
        Condition1 = AND
          TRUE
        EndCondition
      EndConditions
      Actions = SEQUENTIAL
        VICTORY
      EndActions
      IsActive = Yes
      IsOneShot = Yes
    EndScript
  EndScriptGroup
EndScriptList
"#;

        let mut parser = IniScriptParser::new();
        let result = parser.parse(content);

        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        assert_eq!(parser.get_errors().len(), 0);
        assert!(!parser.get_script_lists().is_empty());
    }

    #[test]
    fn test_parse_script_with_conditions() {
        let content = r#"
ScriptList TestScripts
  ScriptGroup Group1
    Script Script_002
      Conditions = OR
        Condition1 = AND
          PLAYER_ALL_DESTROYED Player_USA
          TIMER_EXPIRED 60000
        EndCondition
      EndConditions
      Actions = SEQUENTIAL
        DISPLAY_TEXT "Mission Complete!"
        VICTORY
      EndActions
      IsActive = Yes
    EndScript
  EndScriptGroup
EndScriptList
"#;

        let mut parser = IniScriptParser::new();
        let result = parser.parse(content);

        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        assert_eq!(parser.get_errors().len(), 0);

        let lists = parser.get_script_lists();
        assert!(!lists.is_empty());
    }

    #[test]
    fn test_parse_multiple_conditions() {
        let content = r#"
ScriptList TestScripts
  ScriptGroup Group1
    Script Script_003
      Conditions = OR
        Condition1 = AND
          COUNTER MyCounter > 10
          FLAG MyFlag TRUE
        EndCondition
        Condition2 = AND
          TIMER_EXPIRED 30000
        EndCondition
      EndConditions
      Actions = SEQUENTIAL
        SET_COUNTER MyCounter 0
        ENABLE_SCRIPT OtherScript
      EndActions
      IsActive = Yes
    EndScript
  EndScriptGroup
EndScriptList
"#;

        let mut parser = IniScriptParser::new();
        let result = parser.parse(content);

        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        assert_eq!(parser.get_errors().len(), 0);
    }

    #[test]
    fn test_parse_coordinate_parameters() {
        let mut parser = IniScriptParser::new();

        let param = parser.parse_coordinate("X:100 Y:200 Z:0").unwrap();
        let coord = param.get_coord();
        assert_eq!(coord.x, 100.0);
        assert_eq!(coord.y, 200.0);
        assert_eq!(coord.z, 0.0);
    }

    #[test]
    fn test_parse_boolean_parameters() {
        let mut parser = IniScriptParser::new();

        let param1 = parser.parse_parameter("TRUE").unwrap();
        assert_eq!(param1.get_int(), 1);

        let param2 = parser.parse_parameter("FALSE").unwrap();
        assert_eq!(param2.get_int(), 0);
    }

    #[test]
    fn test_parse_string_parameters() {
        let mut parser = IniScriptParser::new();

        let param = parser.parse_parameter("\"Hello World\"").unwrap();
        assert_eq!(param.get_string(), "Hello World");
    }

    #[test]
    fn test_parse_numeric_parameters() {
        let mut parser = IniScriptParser::new();

        let param1 = parser.parse_parameter("42").unwrap();
        assert_eq!(param1.get_int(), 42);

        let param2 = parser.parse_parameter("3.14").unwrap();
        assert_eq!(param2.get_real(), 3.14);
    }

    #[test]
    fn test_parse_complete_example() {
        let content = r#"
ScriptList MyMission
  ScriptGroup IntroSequence
    Script IntroCamera
      Conditions = OR
        Condition1 = AND
          TRUE
        EndCondition
      EndConditions
      Actions = SEQUENTIAL
        MOVE_CAMERA_TO X:1000 Y:2000 Z:100
        DISPLAY_TEXT "Welcome to the mission!"
        SET_FLAG IntroComplete TRUE
      EndActions
      IsActive = Yes
      IsOneShot = Yes
    EndScript
  EndScriptGroup

  ScriptGroup MainObjectives
    Script CheckPlayerStatus
      Conditions = OR
        Condition1 = AND
          FLAG IntroComplete TRUE
          PLAYER_ALL_DESTROYED Enemy
        EndCondition
      EndConditions
      Actions = SEQUENTIAL
        DISPLAY_TEXT "Enemy defeated!"
        VICTORY
      EndActions
      IsActive = Yes
      EvaluationInterval = 5
    EndScript
  EndScriptGroup
EndScriptList
"#;

        let mut parser = IniScriptParser::new();
        let result = parser.parse(content);

        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        assert_eq!(parser.get_errors().len(), 0);

        let lists = parser.get_script_lists();
        assert!(!lists.is_empty());

        if !parser.get_warnings().is_empty() {
            println!("Warnings:\n{}", parser.get_error_report());
        }
    }

    #[test]
    fn test_error_recovery() {
        let content = r#"
ScriptList TestScripts
  ScriptGroup Group1
    Script BadScript
      Conditions = OR
        Condition1 = AND
          INVALID_CONDITION
        EndCondition
      EndConditions
      Actions = SEQUENTIAL
        INVALID_ACTION
      EndActions
    EndScript
  EndScriptGroup
EndScriptList
"#;

        let mut parser = IniScriptParser::new();
        let result = parser.parse(content);

        // Should succeed but with warnings
        assert!(result.is_ok());
        assert!(!parser.get_warnings().is_empty());
    }
}

/// Parse script from INI section
///
/// This is a helper function that wraps the IniScriptParser for backwards compatibility
/// with code expecting a simple function interface.
pub fn parse_script_from_ini(content: &str) -> GameLogicResult<Box<ScriptList>> {
    let mut parser = IniScriptParser::new();
    parser.parse(content)?;

    // Get the first script list
    let script_lists = parser.get_script_lists();
    if let Some((_, script_list)) = script_lists.iter().next() {
        Ok(Box::new(script_list.clone()))
    } else {
        // Many embedded map script sections contain `Script ...` blocks directly under a
        // `[Scripts]` / `[ScriptEngine]` header without an explicit `ScriptList`.
        // For compatibility, wrap the content in a synthetic `ScriptList` and retry.
        if content.contains("\nScript ") || content.trim_start().starts_with("Script ") {
            let body = {
                let mut lines = content.lines();
                let mut collected = Vec::new();
                let mut skipped_header = false;
                while let Some(line) = lines.next() {
                    let trimmed = line.trim();
                    if !skipped_header && (trimmed == "[Scripts]" || trimmed == "[ScriptEngine]") {
                        skipped_header = true;
                        continue;
                    }
                    collected.push(line);
                }
                collected.join("\n")
            };

            let wrapped = format!("ScriptList Default\n{}\nEndScriptList\n", body);
            let mut fallback = IniScriptParser::new();
            fallback.parse(&wrapped)?;

            if let Some((_, script_list)) = fallback.get_script_lists().iter().next() {
                return Ok(Box::new(script_list.clone()));
            }
        }

        Err(GameLogicError::Configuration(
            "No script lists found".to_string(),
        ))
    }
}
