//! Command Translator - Port of C++ CommandXlat system
//!
//! This module handles:
//! - Right-click context commands (move, attack, enter, repair, etc.)
//! - Keyboard shortcuts for unit commands (S for stop, A for attack-move, G for guard)
//! - Force attack mode (Ctrl held)
//! - Waypoint mode (Shift held)
//! - Command evaluation and validation

use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};

use super::game_message::*;
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::helpers::TheInGameUI;
use crate::input::{KeyCode, KeyModifiers};

/// Command evaluation type
/// Matches C++ CommandTranslator::CommandEvaluateType from CommandXlat.h:21
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEvaluateType {
    DoCommand,    // Actually execute the command
    DoHint,       // Generate hint message for cursor feedback
    EvaluateOnly, // Just check if command is valid
}

/// Can attack result
/// Matches C++ CanAttackResult from CommandXlat.cpp:152-160
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanAttackResult {
    Possible,
    PossibleAfterMoving,
    NotPossible,
}

/// Object information for command evaluation
/// Minimal info needed from C++ Object class
#[derive(Debug, Clone)]
pub struct CommandableObject {
    pub id: ObjectID,
    pub position: Coord3D,
    pub is_dead: bool,
    pub is_locally_controlled: bool,
    pub can_attack: bool,
    pub can_repair: bool,
    pub can_capture: bool,
    pub can_enter: bool,
    pub is_dozer: bool,
    pub is_salvager: bool,
    pub kind_of_flags: u32,
    pub relationship_to_target: Relationship,
}

/// Relationship between objects
/// Matches C++ Relationship enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    Neutral,
    Allies,
    Enemies,
}

impl CommandableObject {
    /// Check if object can attack a target
    /// Port of C++ canObjectForceAttack() from CommandXlat.cpp:152-244
    pub fn can_force_attack(
        &self,
        victim: Option<&CommandableObject>,
        pos: Option<&Coord3D>,
    ) -> CanAttackResult {
        if !self.can_attack {
            return CanAttackResult::NotPossible;
        }

        if let Some(target) = victim {
            // Check if we can attack this specific object
            // Matches C++ Object::getAbleToAttackSpecificObject() logic

            // Can't attack if target is dead
            if target.is_dead {
                return CanAttackResult::NotPossible;
            }

            // Check relationship
            match self.relationship_to_target {
                Relationship::Enemies => CanAttackResult::Possible,
                Relationship::Allies => CanAttackResult::NotPossible, // Can't force attack allies
                Relationship::Neutral => CanAttackResult::Possible,   // Can force attack neutrals
            }
        } else if let Some(_target_pos) = pos {
            // Force attack ground
            // Almost every combat unit can force attack a position
            // Matches C++ CommandXlat.cpp:203-240

            // Immobile units need range check
            if self.has_kindof(KINDOF_IMMOBILE) {
                // Would check if position is within weapon range
                CanAttackResult::PossibleAfterMoving
            } else {
                CanAttackResult::Possible
            }
        } else {
            CanAttackResult::NotPossible
        }
    }

    /// Check if object can repair/heal a target
    pub fn can_repair_target(&self, target: &CommandableObject) -> bool {
        if !self.can_repair {
            return false;
        }

        // Can't repair dead targets
        if target.is_dead {
            return false;
        }

        // Can only repair allies
        if self.relationship_to_target != Relationship::Allies {
            return false;
        }

        // Must be appropriate type (vehicles can repair vehicles, etc.)
        true
    }

    /// Check if object can enter a target
    pub fn can_enter_target(&self, target: &CommandableObject) -> bool {
        if !self.can_enter {
            return false;
        }

        // Can't enter dead structures
        if target.is_dead {
            return false;
        }

        // Check if target can contain this unit
        // (Would check ContainModule capacity in full implementation)
        true
    }

    /// Check if object can salvage a target
    pub fn can_salvage_target(&self, target: &CommandableObject) -> bool {
        if !self.is_salvager {
            return false;
        }

        // Can only salvage crates/wrecks
        target.has_kindof(KINDOF_SALVAGE_CRATE)
    }

    /// Check if object has a specific KINDOF flag
    fn has_kindof(&self, flag: u32) -> bool {
        (self.kind_of_flags & flag) != 0
    }
}

// KINDOF flags from C++ ThingTemplate.h
pub const KINDOF_IMMOBILE: u32 = 0x00000100;
pub const KINDOF_SALVAGE_CRATE: u32 = 0x00000200;
pub const KINDOF_HEAL_PAD: u32 = 0x00000400;
pub const KINDOF_SPAWNS_ARE_THE_WEAPONS: u32 = 0x00000800;

/// Pick and play info for voice responses
/// Matches C++ PickAndPlayInfo from CommandXlat.h:92-101
#[derive(Debug, Clone)]
pub struct PickAndPlayInfo {
    pub air: bool,                           // Are we attacking an airborne target?
    pub draw_target: Option<DrawableID>,     // Override draw target
    pub weapon_slot: Option<WeaponSlotType>, // Specific weapon slot
    pub special_power_type: Option<u32>,     // Which special power
}

impl Default for PickAndPlayInfo {
    fn default() -> Self {
        Self {
            air: false,
            draw_target: None,
            weapon_slot: None,
            special_power_type: None,
        }
    }
}

/// Weapon slot type
/// Matches C++ WeaponSlotType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary,
    Secondary,
    Tertiary,
}

/// Command Translator - Port of C++ CommandTranslator
/// Original: GeneralsMD/Code/GameEngine/Include/GameClient/CommandXlat.h:14-48
pub struct CommandTranslator {
    // State tracking
    // Matches C++ CommandXlat.h:29-30
    objective: i32,
    team_exists: bool,

    // Mouse drag tracking
    // Matches C++ CommandXlat.h:33-36
    mouse_right_drag_anchor: ICoord2D,
    mouse_right_drag_lift: ICoord2D,
    mouse_right_down: u32,
    mouse_right_up: u32,

    // Mode flags
    force_attack_mode: bool,
    force_move_mode: bool,
    waypoint_mode: bool,

    // Current selection
    current_selection: HashSet<ObjectID>,

    // Object registry (would be provided by game client in real implementation)
    object_registry: HashMap<ObjectID, CommandableObject>,
}

impl CommandTranslator {
    pub fn new() -> Self {
        Self {
            objective: 0,
            team_exists: false,
            mouse_right_drag_anchor: ICoord2D::default(),
            mouse_right_drag_lift: ICoord2D::default(),
            mouse_right_down: 0,
            mouse_right_up: 0,
            force_attack_mode: false,
            force_move_mode: false,
            waypoint_mode: false,
            current_selection: HashSet::new(),
            object_registry: HashMap::new(),
        }
    }

    /// Register an object for command evaluation
    pub fn register_object(&mut self, obj: CommandableObject) {
        self.object_registry.insert(obj.id, obj);
    }

    /// Issue move to location command
    /// Port of C++ issueMoveToLocationCommand() from CommandXlat.cpp:841-898
    fn issue_move_to_location_command(
        &mut self,
        pos: &Coord3D,
        drawable_in_way: Option<DrawableID>,
        command_type: CommandEvaluateType,
    ) -> GameMessageType {
        let msg_type;

        if self.team_exists {
            if self.waypoint_mode {
                msg_type = GameMessageType::AddWaypoint(pos.clone());
            } else if TheInGameUI::is_in_attack_move_to_mode() {
                msg_type = GameMessageType::DoAttackMoveTo(pos.clone());
            } else if self.force_move_mode {
                msg_type = GameMessageType::DoForceMoveTO(pos.clone());
            } else if self.force_attack_mode && drawable_in_way.is_some() {
                // In force attack mode with drawable in way, attack it
                msg_type = GameMessageType::DoAttackObject(drawable_in_way.unwrap() as ObjectID);
            } else {
                msg_type = GameMessageType::DoMoveTo(pos.clone());
            }

            if matches!(command_type, CommandEvaluateType::DoCommand) {
                debug!("Issuing move command to {:?}", pos);
                // Would append to message stream here
            }
        } else {
            msg_type = GameMessageType::Invalid;
        }

        // Play unit voice response
        if matches!(command_type, CommandEvaluateType::DoCommand) {
            let mut info = PickAndPlayInfo::default();
            info.draw_target = drawable_in_way;
            // Would call pickAndPlayUnitVoiceResponse() here
        }

        msg_type
    }

    /// Issue attack command
    /// Port of C++ issueAttackCommand() from CommandXlat.cpp:939-1000
    fn issue_attack_command(
        &mut self,
        target: DrawableID,
        command_type: CommandEvaluateType,
        gui_command: u32,
    ) -> GameMessageType {
        let target_obj_id = target as ObjectID; // Simplified conversion
        let msg_type;

        if self.team_exists {
            // Determine message type based on GUI command mode
            // Matches C++ CommandXlat.cpp:958-972
            msg_type = if gui_command == 0 {
                GameMessageType::DoAttackObject(target_obj_id)
            } else {
                // Other special commands (pick up prisoner, etc.)
                GameMessageType::DoAttackObject(target_obj_id)
            };

            if matches!(command_type, CommandEvaluateType::DoCommand) {
                debug!("Issuing attack command on object {}", target_obj_id);
                // Would append to message stream here
                // Matches C++ CommandXlat.cpp:975-986
            }
        } else {
            debug!("Issuing non-team attack");
            msg_type = GameMessageType::DoAttackObject(target_obj_id);
        }

        msg_type
    }

    /// Evaluate context command for drawable
    /// Port of C++ evaluateContextCommand() from CommandXlat.cpp (complex function)
    fn evaluate_context_command(
        &mut self,
        drawable: DrawableID,
        pos: &Coord3D,
        eval_type: CommandEvaluateType,
    ) -> GameMessageType {
        // This is a complex function in C++ that determines what command to issue
        // based on the target and current selection

        // Get object info
        let target_obj_id = drawable as ObjectID;
        let target = self.object_registry.get(&target_obj_id);

        if target.is_none() {
            return GameMessageType::Invalid;
        }

        let target = target.unwrap();

        // Check what command we can issue to this target
        // Matches C++ logic from CommandXlat.cpp evaluateContextCommand()

        // Can we enter it?
        for sel_id in &self.current_selection {
            if let Some(sel_obj) = self.object_registry.get(sel_id) {
                if sel_obj.can_enter_target(target) {
                    return GameMessageType::Enter(*sel_id, target_obj_id);
                }
            }
        }

        // Can we repair it?
        for sel_id in &self.current_selection {
            if let Some(sel_obj) = self.object_registry.get(sel_id) {
                if sel_obj.can_repair_target(target) {
                    return GameMessageType::DoRepair(target_obj_id);
                }
            }
        }

        // Can we salvage it?
        for sel_id in &self.current_selection {
            if let Some(sel_obj) = self.object_registry.get(sel_id) {
                if sel_obj.can_salvage_target(target) {
                    return GameMessageType::DoSalvage(pos.clone());
                }
            }
        }

        // Can we attack it?
        if self.force_attack_mode {
            for sel_id in &self.current_selection {
                if let Some(sel_obj) = self.object_registry.get(sel_id) {
                    if !matches!(
                        sel_obj.can_force_attack(Some(target), None),
                        CanAttackResult::NotPossible
                    ) {
                        return GameMessageType::DoForceAttackObject(target_obj_id);
                    }
                }
            }
        }

        // Default to move
        GameMessageType::DoMoveTo(pos.clone())
    }

    /// Evaluate force attack
    /// Port of C++ evaluateForceAttack() from CommandXlat.h:24
    fn evaluate_force_attack(
        &mut self,
        drawable: DrawableID,
        pos: &Coord3D,
        eval_type: CommandEvaluateType,
    ) -> GameMessageType {
        let target_obj_id = drawable as ObjectID;

        // Check if any selected unit can force attack
        for sel_id in &self.current_selection {
            if let Some(sel_obj) = self.object_registry.get(sel_id) {
                let target = self.object_registry.get(&target_obj_id);

                let result = if target.is_some() {
                    sel_obj.can_force_attack(target, None)
                } else {
                    sel_obj.can_force_attack(None, Some(pos))
                };

                if !matches!(result, CanAttackResult::NotPossible) {
                    if drawable != 0 {
                        return GameMessageType::DoForceAttackObject(target_obj_id);
                    } else {
                        return GameMessageType::DoForceAttackGround(pos.clone());
                    }
                }
            }
        }

        GameMessageType::Invalid
    }

    /// Handle keyboard shortcuts
    /// Port of C++ keyboard message handling
    fn handle_keyboard_command(&mut self, key: KeyCode, down: bool) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        if !down {
            return messages;
        }

        match key {
            // S - Stop
            // Matches C++ CommandXlat.cpp meta stop handling
            KeyCode::S => {
                debug!("Stop command");
                messages.push(GameMessageType::MetaStop);
            }

            // A - Attack move toggle
            KeyCode::A => {
                debug!("Attack move toggle");
                messages.push(GameMessageType::MetaToggleAttackMove);
            }

            // G - Guard
            KeyCode::G => {
                debug!("Guard command");
                if let Some(first_sel) = self.current_selection.iter().next() {
                    let pos = self
                        .object_registry
                        .get(first_sel)
                        .map(|obj| obj.position.clone())
                        .unwrap_or_default();
                    messages.push(GameMessageType::DoGuardPosition(pos, 0));
                }
            }

            // H - Halt (same as stop)
            KeyCode::H => {
                debug!("Halt command");
                messages.push(GameMessageType::MetaStop);
            }

            // X - Scatter
            KeyCode::X => {
                debug!("Scatter command");
                messages.push(GameMessageType::MetaScatter);
            }

            // D - Delete/self-destruct
            KeyCode::D if self.current_selection.len() > 0 => {
                debug!("Delete/self-destruct command");
                // Would implement self-destruct logic
            }

            _ => {}
        }

        messages
    }

    /// Handle mode toggle keys
    fn handle_modifier_key(&mut self, key: KeyCode, down: bool) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        match key {
            // Ctrl - Prefer selection mode / Force move in some cases
            KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                if down {
                    TheInGameUI::set_prefer_selection_mode(true);
                    messages.push(GameMessageType::MetaBeginPreferSelection);
                } else {
                    TheInGameUI::set_prefer_selection_mode(false);
                    messages.push(GameMessageType::MetaEndPreferSelection);
                }
            }

            // Alt - Force attack mode
            KeyCode::LeftAlt | KeyCode::RightAlt => {
                if down {
                    self.force_attack_mode = true;
                    TheInGameUI::set_force_attack_mode(true);
                    messages.push(GameMessageType::MetaBeginForceAttack);
                } else {
                    self.force_attack_mode = false;
                    TheInGameUI::set_force_attack_mode(false);
                    messages.push(GameMessageType::MetaEndForceAttack);
                }
            }

            // Shift - Waypoint mode
            KeyCode::LeftShift | KeyCode::RightShift => {
                self.waypoint_mode = down;
            }

            _ => {}
        }

        messages
    }

    /// Update team exists flag based on selection
    pub fn update_selection(&mut self, selection: HashSet<ObjectID>) {
        self.current_selection = selection;
        self.team_exists = !self.current_selection.is_empty();
    }
}

impl Default for CommandTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for CommandTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let new_messages = match msg.get_type() {
            // Right-click commands
            GameMessageType::RawMouseRightButtonUp(pos, _modifiers, _time) => {
                // Evaluate context command at mouse position
                let world_pos = Coord3D {
                    x: pos.x as f32,
                    y: pos.y as f32,
                    z: 0.0,
                };

                // In full implementation, would pick drawable at position
                let cmd = if self.force_attack_mode {
                    self.evaluate_force_attack(0, &world_pos, CommandEvaluateType::DoCommand)
                } else {
                    self.issue_move_to_location_command(
                        &world_pos,
                        None,
                        CommandEvaluateType::DoCommand,
                    )
                };

                if !matches!(cmd, GameMessageType::Invalid) {
                    vec![cmd]
                } else {
                    Vec::new()
                }
            }

            // Keyboard shortcuts
            GameMessageType::RawKeyDown(key) => {
                self.handle_keyboard_command(KeyCode::from(*key as u32), true)
            }

            GameMessageType::RawKeyUp(key) => {
                self.handle_modifier_key(KeyCode::from(*key as u32), false)
            }

            // Selection updates
            GameMessageType::CreateSelectedGroup(_create_new, objects) => {
                if *_create_new {
                    self.current_selection = objects.iter().copied().collect();
                } else {
                    for obj in objects {
                        self.current_selection.insert(*obj);
                    }
                }
                self.team_exists = !self.current_selection.is_empty();
                return GameMessageDisposition::KeepMessage;
            }

            GameMessageType::CreateSelectedGroupNoSound(_create_new, objects) => {
                if *_create_new {
                    self.current_selection = objects.iter().copied().collect();
                } else {
                    for obj in objects {
                        self.current_selection.insert(*obj);
                    }
                }
                self.team_exists = !self.current_selection.is_empty();
                return GameMessageDisposition::KeepMessage;
            }

            GameMessageType::RemoveFromSelectedGroup(objects) => {
                for obj in objects {
                    self.current_selection.remove(obj);
                }
                self.team_exists = !self.current_selection.is_empty();
                return GameMessageDisposition::KeepMessage;
            }

            // Pass through other messages
            _ => {
                return GameMessageDisposition::KeepMessage;
            }
        };

        // Dispatch translated messages into the message stream.
        for new_msg in new_messages {
            emit_message(GameMessage::new(new_msg));
        }

        // Destroy raw input messages after processing
        GameMessageDisposition::DestroyMessage
    }
}

// Temporary KeyCode conversion (would use proper input module in full implementation)
impl KeyCode {
    fn from(code: u32) -> Self {
        match code {
            0x53 => KeyCode::S,
            0x41 => KeyCode::A,
            0x47 => KeyCode::G,
            0x48 => KeyCode::H,
            0x58 => KeyCode::X,
            0x44 => KeyCode::D,
            0x11 => KeyCode::LeftCtrl,
            0x12 => KeyCode::LeftAlt,
            0x10 => KeyCode::LeftShift,
            _ => KeyCode::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_translator_creation() {
        let translator = CommandTranslator::new();
        assert!(!translator.team_exists);
        assert!(!translator.force_attack_mode);
        assert_eq!(translator.current_selection.len(), 0);
    }

    #[test]
    fn test_can_force_attack() {
        let attacker = CommandableObject {
            id: 1,
            position: Coord3D::default(),
            is_dead: false,
            is_locally_controlled: true,
            can_attack: true,
            can_repair: false,
            can_capture: false,
            can_enter: false,
            is_dozer: false,
            is_salvager: false,
            kind_of_flags: 0,
            relationship_to_target: Relationship::Enemies,
        };

        let enemy = CommandableObject {
            id: 2,
            position: Coord3D::default(),
            is_dead: false,
            is_locally_controlled: false,
            can_attack: false,
            can_repair: false,
            can_capture: false,
            can_enter: false,
            is_dozer: false,
            is_salvager: false,
            kind_of_flags: 0,
            relationship_to_target: Relationship::Enemies,
        };

        // Can attack enemy
        assert_eq!(
            attacker.can_force_attack(Some(&enemy), None),
            CanAttackResult::Possible
        );

        // Can attack ground
        let pos = Coord3D {
            x: 100.0,
            y: 100.0,
            z: 0.0,
        };
        assert_eq!(
            attacker.can_force_attack(None, Some(&pos)),
            CanAttackResult::Possible
        );

        // Can't attack if dead
        let mut dead_enemy = enemy.clone();
        dead_enemy.is_dead = true;
        assert_eq!(
            attacker.can_force_attack(Some(&dead_enemy), None),
            CanAttackResult::NotPossible
        );
    }

    #[test]
    fn test_can_repair_target() {
        let repairer = CommandableObject {
            id: 1,
            position: Coord3D::default(),
            is_dead: false,
            is_locally_controlled: true,
            can_attack: false,
            can_repair: true,
            can_capture: false,
            can_enter: false,
            is_dozer: false,
            is_salvager: false,
            kind_of_flags: 0,
            relationship_to_target: Relationship::Allies,
        };

        let ally = CommandableObject {
            id: 2,
            position: Coord3D::default(),
            is_dead: false,
            is_locally_controlled: true,
            can_attack: false,
            can_repair: false,
            can_capture: false,
            can_enter: false,
            is_dozer: false,
            is_salvager: false,
            kind_of_flags: 0,
            relationship_to_target: Relationship::Allies,
        };

        // Can repair ally
        assert!(repairer.can_repair_target(&ally));

        // Can't repair dead target
        let mut dead_ally = ally.clone();
        dead_ally.is_dead = true;
        assert!(!repairer.can_repair_target(&dead_ally));
    }

    #[test]
    fn test_issue_move_command() {
        let mut translator = CommandTranslator::new();
        translator.team_exists = true;

        let pos = Coord3D {
            x: 100.0,
            y: 100.0,
            z: 0.0,
        };

        // Normal move
        let cmd = translator.issue_move_to_location_command(
            &pos,
            None,
            CommandEvaluateType::EvaluateOnly,
        );
        assert!(matches!(cmd, GameMessageType::DoMoveTo(_)));

        // Waypoint mode
        translator.waypoint_mode = true;
        let cmd = translator.issue_move_to_location_command(
            &pos,
            None,
            CommandEvaluateType::EvaluateOnly,
        );
        assert!(matches!(cmd, GameMessageType::AddWaypoint(_)));

        // Force move mode
        translator.waypoint_mode = false;
        translator.force_move_mode = true;
        let cmd = translator.issue_move_to_location_command(
            &pos,
            None,
            CommandEvaluateType::EvaluateOnly,
        );
        assert!(matches!(cmd, GameMessageType::DoForceMoveTO(_)));
    }

    #[test]
    fn test_keyboard_commands() {
        let mut translator = CommandTranslator::new();
        translator.current_selection.insert(100);

        // Test stop command
        let messages = translator.handle_keyboard_command(KeyCode::S, true);
        assert_eq!(messages.len(), 1);
        assert!(matches!(messages[0], GameMessageType::MetaStop));

        // Test attack move toggle
        let messages = translator.handle_keyboard_command(KeyCode::A, true);
        assert_eq!(messages.len(), 1);
        assert!(matches!(messages[0], GameMessageType::MetaToggleAttackMove));

        // Test guard command
        let messages = translator.handle_keyboard_command(KeyCode::G, true);
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            messages[0],
            GameMessageType::DoGuardPosition(_, _)
        ));
    }

    #[test]
    fn test_modifier_keys() {
        let mut translator = CommandTranslator::new();

        // Test Alt key (force attack mode)
        assert!(!translator.force_attack_mode);

        let messages = translator.handle_modifier_key(KeyCode::LeftAlt, true);
        assert!(translator.force_attack_mode);
        assert_eq!(messages.len(), 1);

        let messages = translator.handle_modifier_key(KeyCode::LeftAlt, false);
        assert!(!translator.force_attack_mode);
        assert_eq!(messages.len(), 1);

        // Test Shift key (waypoint mode)
        assert!(!translator.waypoint_mode);

        translator.handle_modifier_key(KeyCode::LeftShift, true);
        assert!(translator.waypoint_mode);

        translator.handle_modifier_key(KeyCode::LeftShift, false);
        assert!(!translator.waypoint_mode);
    }

    #[test]
    fn test_selection_update() {
        let mut translator = CommandTranslator::new();

        assert!(!translator.team_exists);

        let mut selection = HashSet::new();
        selection.insert(100);
        selection.insert(101);

        translator.update_selection(selection);

        assert!(translator.team_exists);
        assert_eq!(translator.current_selection.len(), 2);
    }
}
