//! Split-out inherent `special powers and command buttons` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// Pause or unpause all special power countdowns for this object.
    /// C++ Reference: Object.cpp lines 2389-2399
    ///
    /// When `pausing` is true, increments the pause count for all special powers.
    /// When `pausing` is false, decrements the pause count (unpausing).
    pub(super) fn pause_all_special_powers(&self, pausing: bool) {
        for entry in &self.modules {
            entry.with_module(|module| {
                if let Some(sp) = Self::get_special_power_from_module(module) {
                    sp.pause_countdown(pausing);
                }
            });
        }

        for behavior in &self.behaviors {
            if let Ok(mut guard) = behavior.lock() {
                if let Some(sp) = guard.get_special_power_module_interface() {
                    sp.pause_countdown(pausing);
                }
            }
        }
    }

    pub(super) fn get_special_power_from_module(
        module: &mut dyn Module,
    ) -> Option<&mut dyn SpecialPowerModuleInterface> {
        crate::object::special_power_interface_cast::module_special_power_interface(module)
    }

    /// Set creator id on SpecialPowerCompletionDie modules, if present.
    pub fn set_special_power_completion_creator(&mut self, creator_id: ObjectID) {
        for entry in &self.die_module_handles {
            entry.with_module(|module| {
                if let Some(die_module) = module_die_kind(module) {
                    die_module.into_interface().set_creator(creator_id);
                }
            });
        }
    }

    /// Notify script engine via SpecialPowerCompletionDie if present.
    /// Returns true if a matching die module was found.
    pub fn notify_special_power_completion_die(&self) -> bool {
        let player_index = self.get_controlling_player().and_then(|player| {
            player
                .read()
                .ok()
                .map(|guard| guard.get_player_index() as usize)
        });

        let mut found = false;
        for entry in &self.die_module_handles {
            entry.with_module(|module| {
                if let Some(die_module) = module_die_kind(module) {
                    if die_module
                        .into_interface()
                        .notify_script_engine_with_player_index(player_index)
                    {
                        found = true;
                    }
                }
            });
        }
        found
    }

    pub(super) fn is_valid_command_target(
        &self,
        target: &Object,
        options: crate::object::update::special_power_update::SpecialPowerCommandOption,
    ) -> bool {
        if target.is_destroyed() {
            return false;
        }
        if target
            .get_status_bits()
            .test(crate::common::ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }
        if options.contains(crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_PRISONER)
            && !target.is_captured()
        {
            return false;
        }

        let needs_relationship = options.intersects(
            crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
        );
        if !needs_relationship {
            return true;
        }

        use crate::object::contain::open_contain::ObjectRelationship;
        let relationship = self.get_relationship_to(target);
        if options.contains(crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
            && relationship == ObjectRelationship::Enemy
        {
            return true;
        }
        if options.contains(crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
            && relationship == ObjectRelationship::Neutral
        {
            return true;
        }
        if options.contains(crate::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
            && (relationship == ObjectRelationship::Ally || relationship == ObjectRelationship::Self_)
        {
            return true;
        }

        false
    }

    /// Set rally point on any compatible exit/production behavior.
    pub fn set_rally_point(&mut self, pos: &Coord3D) -> bool {
        let mut applied = false;
        for entry in &self.modules {
            let applied_module = entry.with_module(|module| {
                module_production_behavior_kind(module).and_then(|kind| {
                    if kind.set_rally_point(pos) {
                        Some(())
                    } else {
                        None
                    }
                })
            });

            if applied_module.is_some() {
                applied = true;
            }
        }

        for behavior in &self.behaviors {
            let Ok(mut behavior_guard) = behavior.lock() else {
                continue;
            };

            if let Some(kind) = behavior_production_rally_kind(&mut *behavior_guard) {
                kind.set_rally_point(pos);
                applied = true;
            }
        }

        if let Some(contain) = &self.contain {
            if let Ok(mut contain_guard) = contain.lock() {
                contain_guard.set_rally_point(*pos);
                applied = true;
            }
        }

        applied
    }

    pub(crate) fn forward_command_to_flight_deck(&self, params: &crate::ai::AiCommandParams) {
        for entry in &self.modules {
            let forwarded = entry.with_module(|module| {
                module_production_behavior_kind(module)
                    .and_then(ProductionBehaviorModuleKindMut::into_flight_deck_behavior)
                    .map(|flight| {
                        flight.ai_do_command(
                            params.cmd,
                            Some(params.pos),
                            params.obj.map(|id| id as ObjectID),
                            params.cmd_source,
                        );
                    })
            });
            if forwarded.is_some() {
                return;
            }
        }

        for behavior_arc in &self.behaviors {
            let Ok(mut behavior_guard) = behavior_arc.lock() else {
                continue;
            };
            if let Some(flight) = behavior_production_rally_kind(&mut *behavior_guard)
                .and_then(ProductionBehaviorRallyKindMut::into_flight_deck)
            {
                flight.ai_do_command(
                    params.cmd,
                    Some(params.pos),
                    params.obj.map(|id| id as ObjectID),
                    params.cmd_source,
                );
            }
        }
    }

    /// Execute a command button ability with no target.
    pub fn do_command_button(
        &mut self,
        button_id: u32,
        source: CommandSource,
    ) -> Result<(), String> {
        use crate::ai::{AiCommandParams, AiCommandType};
        use crate::commands::command::CommandType;
        use crate::control_bar::get_control_bar_bridge;
        use crate::modules::AIUpdateInterfaceExt;
        use crate::object::special_power_module::SpecialPowerCommandOptions;
        use crate::object::update::special_power_update::SpecialPowerCommandOption;

        if self.is_disabled() {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        let Some(command_button) = control_bar.get_command_button(button_id) else {
            return Ok(());
        };

        let ai = self.get_ai_update_interface();
        match command_button.get_command_type() {
            CommandType::SpecialPower => {
                if let Some(template) = command_button.get_special_power_template() {
                    let mut options = SpecialPowerCommandOptions::from_bits_truncate(
                        command_button.get_options_bits(),
                    );
                    options.insert(SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT);
                    self.command_button_special_power_no_target(
                        template.get_name(),
                        options,
                        source,
                    );
                    return Ok(());
                }
            }
            CommandType::DoStop => {
                if let Some(ai) = ai {
                    ai.ai_idle(source);
                    return Ok(());
                }
            }
            CommandType::SwitchWeapons => {
                self.lock_switch_weapon_from_command(command_button);
                return Ok(());
            }
            CommandType::FireWeapon => {
                if let Some(ai) = ai {
                    let options = SpecialPowerCommandOptions::from_bits_truncate(
                        command_button.get_options_bits(),
                    );
                    let needs_target = options.intersects(
                        SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_PRISONER
                            | SpecialPowerCommandOption::NEED_TARGET_POS,
                    );
                    if needs_target {
                        return Ok(());
                    }
                    self.lock_fire_weapon_from_command(command_button);
                    if let Ok(mut guard) = ai.try_lock() {
                        let mut params =
                            AiCommandParams::new(AiCommandType::AttackPosition, source);
                        params.int_value = command_button.get_max_shots_to_fire();
                        self.forward_command_to_flight_deck(&params);
                        let _ = guard.execute_command(&params);
                        return Ok(());
                    }
                }
            }
            CommandType::QueueUpgrade => {
                if let Some(upgrade) = command_button.get_upgrade_template() {
                    if upgrade.get_upgrade_type() == crate::upgrade::UpgradeType::Object {
                        if self.has_upgrade(upgrade) || !self.affected_by_upgrade(upgrade) {
                            return Ok(());
                        }
                    }

                    if self.queue_upgrade_via_production(upgrade) {
                        return Ok(());
                    }
                }
            }
            CommandType::QueueUnitCreate | CommandType::DozerConstruct => {
                if let Some(template) = command_button.get_thing_template() {
                    if self.command_button_dozer_construct_no_target(template) {
                        return Ok(());
                    }
                }
            }
            CommandType::InternetHack => {
                if let Some(ai) = ai {
                    if let Ok(mut guard) = ai.try_lock() {
                        let params = AiCommandParams::new(AiCommandType::HackInternet, source);
                        self.forward_command_to_flight_deck(&params);
                        let _ = guard.execute_command(&params);
                        return Ok(());
                    }
                }
            }
            CommandType::Sell => {
                if let Some(mut assistant) =
                    game_engine::common::system::build_assistant::get_build_assistant()
                {
                    let object = game_engine::common::system::build_assistant::Object {
                        id: self.get_id(),
                        position: game_engine::common::system::build_assistant::Coord3D {
                            x: self.get_position().x,
                            y: self.get_position().y,
                            z: self.get_position().z,
                        },
                        orientation: self.get_orientation(),
                        command_set: None,
                    };
                    assistant.sell_object(&object, crate::helpers::TheGameLogic::get_frame());
                    return Ok(());
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn do_command_button_at_object(
        &mut self,
        button_id: u32,
        target: &Object,
        source: CommandSource,
    ) -> Result<(), String> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        use crate::ai::{AiCommandParams, AiCommandType};
        use crate::commands::command::CommandType;
        use crate::control_bar::get_control_bar_bridge;
        use crate::modules::AIUpdateInterfaceExt;
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::special_power_module::SpecialPowerCommandOptions;
        use crate::object::update::special_power_update::SpecialPowerCommandOption;

        if self.is_disabled() {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        let Some(command_button) = control_bar.get_command_button(button_id) else {
            return Ok(());
        };

        let ai = self.get_ai_update_interface();
        #[allow(unreachable_patterns)]
        match command_button.get_command_type() {
            CommandType::CombatDropAtLocation | CommandType::CombatDropAtObject => {
                if let Some(ai) = ai {
                    if let Ok(mut guard) = ai.try_lock() {
                        let mut params = crate::ai::AiCommandParams::new(
                            crate::ai::AiCommandType::CombatDrop,
                            source,
                        );
                        params.obj = Some(target.get_id());
                        params.pos = *target.get_position();
                        self.forward_command_to_flight_deck(&params);
                        let _ = guard.execute_command(&params);
                        return Ok(());
                    }
                }
            }
            CommandType::SpecialPower => {
                if let Some(template) = command_button.get_special_power_template() {
                    let mut options = SpecialPowerCommandOptions::from_bits_truncate(
                        command_button.get_options_bits(),
                    );
                    options.insert(SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT);
                    self.command_button_special_power_at_object(
                        template.get_name(),
                        target.get_id(),
                        options,
                        source,
                    );
                    return Ok(());
                }
            }
            CommandType::DoStop => {
                if let Some(ai) = ai {
                    let params = AiCommandParams::new(AiCommandType::Idle, source);
                    self.forward_command_to_flight_deck(&params);
                    ai.ai_idle(source);
                    return Ok(());
                }
            }
            CommandType::FireWeapon => {
                if let Some(ai) = ai {
                    let options = SpecialPowerCommandOptions::from_bits_truncate(
                        command_button.get_options_bits(),
                    );
                    let needs_object_target = options.intersects(
                        SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
                    );
                    if !needs_object_target {
                        return Ok(());
                    }

                    if !self.is_valid_command_target(target, options) {
                        return Ok(());
                    }

                    self.lock_fire_weapon_from_command(command_button);
                    if options.contains(SpecialPowerCommandOption::ATTACK_OBJECTS_POSITION) {
                        let mut params =
                            AiCommandParams::new(AiCommandType::AttackPosition, source);
                        params.pos = *target.get_position();
                        params.int_value = command_button.get_max_shots_to_fire();
                        self.forward_command_to_flight_deck(&params);
                        ai.ai_attack_position(
                            target.get_position(),
                            command_button.get_max_shots_to_fire(),
                            source,
                        );
                    } else {
                        let mut params = AiCommandParams::new(AiCommandType::AttackObject, source);
                        params.obj = Some(target.get_id());
                        params.int_value = command_button.get_max_shots_to_fire();
                        self.forward_command_to_flight_deck(&params);
                        ai.ai_attack_object_id(
                            target.get_id(),
                            command_button.get_max_shots_to_fire(),
                            source,
                        );
                    }
                    return Ok(());
                }
            }
            CommandType::Enter
            | CommandType::HijackVehicle
            | CommandType::ConvertToCarBomb
            | CommandType::SabotageBuilding => {
                if let Some(ai) = ai {
                    let mut params = AiCommandParams::new(AiCommandType::Enter, source);
                    params.obj = Some(target.get_id());
                    self.forward_command_to_flight_deck(&params);
                    ai.ai_enter(target.get_id(), source);
                    return Ok(());
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Execute a command button ability directed at a location.
    pub fn do_command_button_at_position(
        &mut self,
        button_id: u32,
        pos: &Coord3D,
        source: CommandSource,
    ) -> Result<(), String> {
        use crate::ai::{AiCommandParams, AiCommandType};
        use crate::commands::command::CommandType;
        use crate::control_bar::get_control_bar_bridge;
        use crate::modules::AIUpdateInterfaceExt;
        use crate::object::special_power_module::SpecialPowerCommandOptions;
        use crate::object::update::special_power_update::SpecialPowerCommandOption;

        if self.is_disabled() {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        let Some(command_button) = control_bar.get_command_button(button_id) else {
            return Ok(());
        };

        let ai = self.get_ai_update_interface();
        match command_button.get_command_type() {
            CommandType::SpecialPower => {
                if let Some(template) = command_button.get_special_power_template() {
                    let mut options = SpecialPowerCommandOptions::from_bits_truncate(
                        command_button.get_options_bits(),
                    );
                    options.insert(SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT);
                    self.command_button_special_power_at_location(
                        template.get_name(),
                        pos,
                        INVALID_ANGLE,
                        options,
                        source,
                    );
                    return Ok(());
                }
            }
            CommandType::DoAttackMoveTo => {
                if let Some(ai) = ai {
                    let mut params =
                        AiCommandParams::new(AiCommandType::AttackMoveToPosition, source);
                    params.pos = *pos;
                    params.int_value = command_button.get_max_shots_to_fire();
                    self.forward_command_to_flight_deck(&params);
                    ai.ai_attack_move_to_position(
                        pos,
                        command_button.get_max_shots_to_fire(),
                        source,
                    );
                    return Ok(());
                }
            }
            CommandType::DoStop => {
                if let Some(ai) = ai {
                    let params = AiCommandParams::new(AiCommandType::Idle, source);
                    self.forward_command_to_flight_deck(&params);
                    ai.ai_idle(source);
                    return Ok(());
                }
            }
            CommandType::DozerConstruct => {
                if let Some(template) = command_button.get_thing_template() {
                    self.command_button_dozer_construct_at_position(template, pos);
                    return Ok(());
                }
            }
            CommandType::FireWeapon => {
                if let Some(ai) = ai {
                    let options = SpecialPowerCommandOptions::from_bits_truncate(
                        command_button.get_options_bits(),
                    );
                    if !options.contains(SpecialPowerCommandOption::NEED_TARGET_POS) {
                        return Ok(());
                    }

                    self.lock_fire_weapon_from_command(command_button);
                    ai.ai_attack_position(pos, command_button.get_max_shots_to_fire(), source);
                    return Ok(());
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Execute a command button ability using a waypoint path.
    pub fn do_command_button_using_waypoints(
        &self,
        button_id: u32,
        waypoint: &crate::object::special_power_module::Waypoint,
        source: CommandSource,
    ) -> Result<(), String> {
        use crate::commands::command::CommandType;
        use crate::control_bar::get_control_bar_bridge;
        use crate::object::special_power_module::SpecialPowerCommandOptions;
        use crate::object::update::special_power_update::SpecialPowerCommandOption;

        if self.is_disabled() {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        let Some(command_button) = control_bar.get_command_button(button_id) else {
            return Ok(());
        };

        let options =
            SpecialPowerCommandOptions::from_bits_truncate(command_button.get_options_bits());
        if !options.contains(SpecialPowerCommandOption::CAN_USE_WAYPOINTS) {
            return Ok(());
        }

        if command_button.get_command_type() == CommandType::SpecialPower {
            if let Some(template) = command_button.get_special_power_template() {
                let mut command_options = options;
                command_options.insert(SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT);
                self.command_button_special_power_using_waypoints(
                    template.get_name(),
                    waypoint,
                    command_options,
                    source,
                );
                return Ok(());
            }
        }

        // Update registered module entries that mirror C++ UpdateModule behavior.
        for module in self.modules_with_interface(ModuleInterfaceType::UPDATE) {
            module.with_module(|module| {
                if let Some(ocl_update) = module.get_ocl_update_control_interface() {
                    ocl_update.tick_ocl_update();
                }
            });
        }

        Ok(())
    }

    pub fn do_special_power_using_waypoints(
        &self,
        special_power_name: &str,
        waypoint: &crate::object::special_power_module::Waypoint,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) -> Result<(), String> {
        self.do_special_power_using_waypoints_forced(
            special_power_name,
            waypoint,
            command_options,
            false,
        )
    }

    pub fn do_special_power_using_waypoints_forced(
        &self,
        special_power_name: &str,
        waypoint: &crate::object::special_power_module::Waypoint,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
        forced: bool,
    ) -> Result<(), String> {
        if self.is_disabled() {
            return Ok(());
        }

        if !self.can_dispatch_special_power(special_power_name, forced) {
            return Ok(());
        }

        self.with_special_power_module_mut_by_name(special_power_name, |sp_module| {
            sp_module.do_special_power_using_waypoints(waypoint, command_options);
        });
        Ok(())
    }

    /// Set or clear special power availability on this object.
    pub fn set_special_power_available(&mut self, power_type: SpecialPowerType, available: bool) {
        self.special_power_bits.set_power(power_type, available);
    }

    /// Check if a special power is marked as available on this object.
    pub fn has_special_power(&self, power_type: SpecialPowerType) -> bool {
        self.special_power_bits.test_power(power_type)
    }

    /// Find a special ability update module by special power type
    /// C++ Reference: Object.cpp - Special power system
    ///
    /// # Arguments
    /// * `power_type` - The special power type to search for
    ///
    /// # Returns
    /// An optional reference to the special ability update module
    pub fn find_special_ability_update(
        &self,
        power_type: crate::common::types::SpecialPowerType,
    ) -> Option<Arc<Mutex<dyn crate::modules::SpecialAbilityUpdate>>> {
        for behavior in &self.behaviors {
            let matches = {
                let Ok(guard) = behavior.lock() else {
                    continue;
                };
                guard
                    .as_any()
                    .downcast_ref::<SpecialAbilityUpdateBehavior>()
                    .and_then(|update| update.get_special_power_type())
                    .map(|update_type| update_type == power_type)
                    .unwrap_or(false)
            };

            if matches {
                return Some(Arc::new(Mutex::new(SpecialAbilityUpdateProxy {
                    behavior: behavior.clone(),
                })));
            }
        }

        None
    }

    pub(super) fn module_special_power_interface(
        module: &mut dyn Module,
    ) -> Option<&mut dyn SpecialPowerModuleInterface> {
        crate::object::special_power_interface_cast::module_special_power_interface(module)
    }

    /// Return whether this object owns a special-power module capable of executing `template`.
    /// Matches the module-presence gate in C++ `Object::getSpecialPowerModule`.
    pub fn has_special_power_module_for_power(&self, template: &SpecialPowerTemplate) -> bool {
        for behavior_arc in &self.behaviors {
            let Ok(behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            if behavior_lock
                .get_special_power_module_interface_const()
                .map(|sp_module| sp_module.is_module_for_power(template))
                .unwrap_or(false)
            {
                return true;
            }
        }

        for module_handle in self.modules_with_interface(ModuleInterfaceType::SPECIAL_POWER) {
            let mut matched = false;
            module_handle.with_module(|module| {
                if let Some(sp_module) = Self::module_special_power_interface(module) {
                    matched = sp_module.is_module_for_power(template);
                }
            });
            if matched {
                return true;
            }
        }

        false
    }

    /// Get special power module for a given template ID
    /// C++ Reference: Object.cpp - Special power system
    ///
    /// # Arguments
    /// * `template_id` - The special power template ID
    ///
    /// # Returns
    /// An optional special power module ID
    pub fn get_special_power_module(&self, template_id: u32) -> Option<u32> {
        for behavior_arc in &self.behaviors {
            let Ok(behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            if let Some(sp_module) = behavior_lock.get_special_power_module_interface_const() {
                if let Some(template_any) = sp_module.get_special_power_template() {
                    if let Some(template) = template_any
                        .as_ref()
                        .downcast_ref::<crate::object::SpecialPowerTemplate>()
                    {
                        if template.get_id() == template_id {
                            return Some(template_id);
                        }
                    }
                }
            }
        }

        for module_handle in self.modules_with_interface(ModuleInterfaceType::SPECIAL_POWER) {
            let mut matched = false;
            module_handle.with_module(|module| {
                if let Some(sp_module) = Self::module_special_power_interface(module) {
                    if let Some(template) = sp_module.get_special_power_template_full() {
                        if template.get_id() == template_id {
                            matched = true;
                        }
                    }
                }
            });
            if matched {
                return Some(template_id);
            }
        }

        None
    }

    /// Get special power module by its template name
    pub fn get_special_power_module_by_name(
        &self,
        template_name: &str,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior_arc in &self.behaviors {
            let Ok(behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            if let Some(sp_module) = behavior_lock.get_special_power_module_interface_const() {
                if sp_module.get_power_name() == template_name {
                    return Some(behavior_arc.clone());
                }
            }
        }
        None
    }

    pub fn with_special_power_module_mut_by_name<F, R>(
        &self,
        template_name: &str,
        func: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut dyn SpecialPowerModuleInterface) -> R,
    {
        let mut func = Some(func);

        for behavior_arc in &self.behaviors {
            let Ok(mut behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            if let Some(sp_module) = behavior_lock.get_special_power_module_interface() {
                if sp_module.get_power_name() == template_name {
                    let func = func.take().expect("special power callback already used");
                    return Some(func(sp_module));
                }
            }
        }

        for module_handle in self.modules_with_interface(ModuleInterfaceType::SPECIAL_POWER) {
            let mut result = None;
            module_handle.with_module(|module| {
                if let Some(sp_module) = Self::module_special_power_interface(module) {
                    if sp_module.get_power_name() == template_name {
                        let func = func.take().expect("special power callback already used");
                        result = Some(func(sp_module));
                    }
                }
            });
            if result.is_some() {
                return result;
            }
        }

        None
    }

    pub fn with_special_power_module_interface_by_name<F, R>(
        &self,
        template_name: &str,
        mut func: F,
    ) -> Option<R>
    where
        F: FnMut(&dyn SpecialPowerModuleInterface) -> R,
    {
        for behavior_arc in &self.behaviors {
            let Ok(behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            if let Some(sp_module) = behavior_lock.get_special_power_module_interface_const() {
                if sp_module.get_power_name() == template_name {
                    return Some(func(sp_module));
                }
            }
        }

        for module_handle in self.modules_with_interface(ModuleInterfaceType::SPECIAL_POWER) {
            let mut result = None;
            module_handle.with_module(|module| {
                if let Some(sp_module) = Self::module_special_power_interface(module) {
                    if sp_module.get_power_name() == template_name {
                        result = Some(func(sp_module));
                    }
                }
            });
            if result.is_some() {
                return result;
            }
        }

        None
    }

    // ========================================================================
    // SPECIAL POWER DISPATCH (3 methods)
    // C++ Reference: Object.cpp doSpecialPower, doSpecialPowerAtObject, doSpecialPowerAtLocation
    // ========================================================================

    pub(super) fn can_dispatch_special_power(
        &self,
        special_power_template_name: &str,
        forced: bool,
    ) -> bool {
        if forced {
            return true;
        }

        let Some(store) = crate::object::special_power_template::get_special_power_store() else {
            return false;
        };
        let Some(template) = store.find_special_power_template(special_power_template_name) else {
            return false;
        };

        store.can_use_special_power_for_object(self, template)
    }

    pub fn do_special_power(
        &self,
        special_power_template_name: &str,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
        forced: bool,
    ) {
        if self.is_disabled() {
            return;
        }

        if !self.can_dispatch_special_power(special_power_template_name, forced) {
            return;
        }

        self.with_special_power_module_mut_by_name(special_power_template_name, |sp_module| {
            sp_module.do_special_power(command_options);
        });
    }

    pub fn do_special_power_at_object(
        &self,
        special_power_template_name: &str,
        target_obj_id: ObjectID,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
        forced: bool,
    ) {
        if self.is_disabled() {
            return;
        }

        if !self.can_dispatch_special_power(special_power_template_name, forced) {
            return;
        }

        self.with_special_power_module_mut_by_name(special_power_template_name, |sp_module| {
            sp_module.do_special_power_at_object(target_obj_id, command_options);
        });
    }

    pub fn do_special_power_at_location(
        &self,
        special_power_template_name: &str,
        location: &Coord3D,
        angle: f32,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
        forced: bool,
    ) {
        if self.is_disabled() {
            return;
        }

        if !self.can_dispatch_special_power(special_power_template_name, forced) {
            return;
        }

        self.with_special_power_module_mut_by_name(special_power_template_name, |sp_module| {
            sp_module.do_special_power_at_location(location, angle, command_options);
        });
    }

    // ========================================================================
    // SPECIAL POWER LOOKUP (5 methods)
    // C++ Reference: Object.cpp findSpecialPowerModuleInterface, etc.
    // ========================================================================

    pub fn find_special_power_module_interface(
        &self,
        special_power_type: SpecialPowerType,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(sp) = guard.get_special_power_module_interface() {
                if let Some(template_any) = sp.get_special_power_template() {
                    if let Some(template) = template_any.downcast_ref::<Arc<SpecialPowerTemplate>>()
                    {
                        if template.get_special_power_type() == special_power_type
                            || special_power_type == SpecialPowerType::Invalid
                        {
                            drop(guard);
                            return Some(behavior.clone());
                        }
                    }
                }
            }
        }
        None
    }

    pub fn find_any_shortcut_special_power_module_interface(
        &self,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(sp) = guard.get_special_power_module_interface() {
                if let Some(template_any) = sp.get_special_power_template() {
                    if let Some(template) = template_any.downcast_ref::<Arc<SpecialPowerTemplate>>()
                    {
                        if template.is_shortcut_power() {
                            drop(guard);
                            return Some(behavior.clone());
                        }
                    }
                }
            }
        }
        None
    }

    pub fn find_special_power_with_overridable_destination_active(
        &self,
        _special_power_type: SpecialPowerType,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(sp_interface) = guard.get_special_power_update_interface() {
                if sp_interface.does_special_power_have_overridable_destination_active() {
                    drop(guard);
                    return Some(behavior.clone());
                }
            }
        }
        None
    }

    pub fn find_special_power_with_overridable_destination(
        &self,
        _special_power_type: SpecialPowerType,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(sp_interface) = guard.get_special_power_update_interface() {
                if sp_interface.does_special_power_have_overridable_destination() {
                    drop(guard);
                    return Some(behavior.clone());
                }
            }
        }
        None
    }

    pub fn has_any_special_power(&self) -> bool {
        !self.special_power_bits.is_empty()
    }
}
