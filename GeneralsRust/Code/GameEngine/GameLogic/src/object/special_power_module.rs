// FILE: special_power_module.rs
// Port of SpecialPowerModule.h and SpecialPowerModule.cpp
// Author: Rust Port
// Desc: Special power module interface and base implementation

use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::AsciiString;
pub use crate::common::ObjectID as ObjectId;
use crate::common::{Coord3D, KindOf, ObjectStatusMaskType, Relationship};
use crate::helpers::{TheAudio, TheEva, TheGameLogic, TheGlobalData, TheInGameUI, TheThingFactory};
use crate::modules::{
    BehaviorModule, BehaviorModuleInterface,
    SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::special_power_interface_cast::module_special_power_update_interface;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::special_power_template::{AudioEventRts, SpecialPowerTemplate};
use crate::object::special_power_types::SpecialPowerType;
use crate::object::update::special_power_update::SpecialPowerCommandOption;
use crate::player::player_list;
pub use crate::waypoint::Waypoint;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::academy_stats::AcademyClassificationType;
use std::fmt;
use std::sync::Arc;

/// Frame counter type
pub type FrameCount = u32;

/// Shared cooldown groups for special powers (matches C++ cooldown groups).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CooldownGroup {
    None,
    Airstrike,
}

/// Command options flags
pub type SpecialPowerCommandOptions = SpecialPowerCommandOption;

/// Module data for special power modules
#[derive(Debug, Clone)]
pub struct SpecialPowerModuleData {
    pub base: BehaviorModuleData,
    /// Pointer to the special power template
    pub special_power_template: Option<Arc<SpecialPowerTemplate>>,

    /// Initiate sound
    pub initiate_sound: AudioEventRts,

    /// Update module determines when the special power actually starts
    pub update_module_starts_attack: bool,

    /// Paused on creation, someone else will unpause (upgrade module or script)
    pub starts_paused: bool,

    /// Can only be fired via scripts
    pub scripted_special_power_only: bool,
}

impl Default for SpecialPowerModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template: None,
            initiate_sound: AudioEventRts::default(),
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        }
    }
}

/// Special power module interface trait
pub trait SpecialPowerModuleInterface: EngineSpecialPowerModuleInterface {
    /// Is this module for the specified special power
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool;

    /// Get the percent ready (1.0 = ready now, 0.5 = half charged, etc.)
    fn get_percent_ready(&self) -> f32;

    /// Get the power name
    fn get_power_name(&self) -> String;

    /// Get the special power template
    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>>;

    /// Get required science
    fn get_required_science(&self) -> ScienceType;

    /// Called by create module to start countdown
    fn on_special_power_creation(&mut self);

    /// Set ready frame (for scripting)
    fn set_ready_frame(&mut self, frame: FrameCount);

    /// Pause or unpause countdown
    fn pause_countdown(&mut self, pause: bool);

    /// Execute special power with no target
    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions);

    /// Execute special power at object
    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    );

    /// Execute special power at location
    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    );

    /// Execute special power using waypoints
    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    );

    /// Mark special power as triggered
    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>);

    /// Start power recharge
    fn start_power_recharge_at(&mut self, current_frame: FrameCount);

    /// Get initiate sound
    fn get_initiate_sound(&self) -> &AudioEventRts;

    /// Is this script only?
    fn is_script_only(&self) -> bool;

    /// Get reference thing template (for construction sites)
    fn get_reference_thing_template(&self) -> Option<String>;
}

/// Base special power module implementation
#[derive(Clone)]
pub struct SpecialPowerModule {
    /// Module data
    module_data: SpecialPowerModuleData,

    /// Owner object ID
    owner_object_id: ObjectId,

    /// Frame when power becomes available
    available_on_frame: FrameCount,

    /// Reference count of sources pausing
    paused_count: i32,

    /// Frame when paused
    paused_on_frame: FrameCount,

    /// Percent ready when paused
    paused_percent: f32,
}

impl SpecialPowerModule {
    /// Create a new special power module
    pub fn new(owner_object_id: ObjectId, module_data: SpecialPowerModuleData) -> Self {
        Self {
            module_data,
            owner_object_id,
            available_on_frame: 0,
            paused_count: 0,
            paused_on_frame: 0,
            paused_percent: 0.0,
        }
    }

    fn resolve_special_power(&mut self) {
        let Some(template) = self.module_data.special_power_template.as_ref() else {
            return;
        };
        let template_name = AsciiString::from(template.get_name());

        self.module_data.special_power_template =
            Some(find_or_create_special_power_template(&template_name));
    }

    /// Initialize the module (called after construction)
    pub fn initialize(
        &mut self,
        current_frame: FrameCount,
        is_under_construction: bool,
        is_structure: bool,
    ) {
        // If pre-built, start counting down
        if !is_under_construction {
            if let Some(template) = &self.module_data.special_power_template {
                if !template.is_shared_n_sync() {
                    self.start_power_recharge_at(current_frame);
                }
            }
        }

        // Some powers need to be activated by upgrade
        if self.module_data.starts_paused {
            SpecialPowerModuleInterface::pause_countdown(self, true);
        }

        self.resolve_special_power();

        // Register with UI if has public timer
        if self.paused_count == 0 {
            if let Some(template) = &self.module_data.special_power_template {
                if template.is_shared_n_sync() && template.has_public_timer() && is_structure {
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
                        if let Ok(owner_guard) = owner.read() {
                            if let Some(player) = owner_guard.get_controlling_player() {
                                if let Ok(player_guard) = player.read() {
                                    let player_index = player_guard.get_player_index();
                                    TheInGameUI::add_superweapon(
                                        player_index,
                                        self.get_power_name(),
                                        self.owner_object_id,
                                        template,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Initialize using the owner object's current state (C++ constructor flow).
    pub fn initialize_from_owner(&mut self) {
        let current_frame = TheGameLogic::get_frame();
        let mut is_under_construction = false;
        let mut is_structure = false;

        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                is_under_construction = owner_guard
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::UNDER_CONSTRUCTION);
                is_structure = owner_guard.is_kind_of(KindOf::Structure);
            }
        }

        self.initialize(current_frame, is_under_construction, is_structure);
    }

    /// Get the module data
    pub fn get_module_data(&self) -> &SpecialPowerModuleData {
        &self.module_data
    }

    /// Get the owner object ID
    pub fn get_owner_object_id(&self) -> ObjectId {
        self.owner_object_id
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        SpecialPowerModuleInterface::is_module_for_power(self, special_power_template)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn on_special_power_creation(&mut self) {
        SpecialPowerModuleInterface::on_special_power_creation(self);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn set_ready_frame(&mut self, frame: FrameCount) {
        SpecialPowerModuleInterface::set_ready_frame(self, frame);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        SpecialPowerModuleInterface::do_special_power_at_location(
            self,
            location,
            angle,
            command_options,
        );
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        SpecialPowerModuleInterface::mark_special_power_triggered(self, location);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_percent_ready(&self) -> f32 {
        SpecialPowerModuleInterface::get_percent_ready(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_power_name(&self) -> String {
        SpecialPowerModuleInterface::get_power_name(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        SpecialPowerModuleInterface::get_special_power_template_full(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_required_science(&self) -> ScienceType {
        SpecialPowerModuleInterface::get_required_science(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn pause_countdown(&mut self, pause: bool) {
        SpecialPowerModuleInterface::pause_countdown(self, pause);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        SpecialPowerModuleInterface::do_special_power(self, command_options);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        SpecialPowerModuleInterface::do_special_power_at_object(self, object_id, command_options);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        SpecialPowerModuleInterface::do_special_power_using_waypoints(
            self,
            waypoint,
            command_options,
        );
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        SpecialPowerModuleInterface::start_power_recharge_at(self, current_frame);
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_initiate_sound(&self) -> &AudioEventRts {
        SpecialPowerModuleInterface::get_initiate_sound(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn is_script_only(&self) -> bool {
        SpecialPowerModuleInterface::is_script_only(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_reference_thing_template(&self) -> Option<String> {
        SpecialPowerModuleInterface::get_reference_thing_template(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        EngineSpecialPowerModuleInterface::start_power_recharge(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn get_ready_frame(&self) -> u32 {
        EngineSpecialPowerModuleInterface::get_ready_frame(self)
    }

    /// Compatibility wrapper for call sites that invoke trait methods directly on the struct.
    pub fn is_ready(&self) -> bool {
        EngineSpecialPowerModuleInterface::is_ready(self)
    }

    /// Initiate intent to do special power
    fn initiate_intent_to_do_special_power(
        &mut self,
        target_object: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        waypoint: Option<&Waypoint>,
        command_options: SpecialPowerCommandOptions,
    ) -> bool {
        let Some(template) = &self.module_data.special_power_template else {
            return false;
        };

        let mut valid = false;
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                for module in owner_guard.behavior_modules() {
                    if valid {
                        break;
                    }
                    module.with_module(|module| {
                        if valid {
                            return;
                        }
                        if let Some(update) = module_special_power_update_interface(module) {
                            if self.is_module_for_power(template)
                                && update.does_special_power_update_pass_science_test()
                                && update.initiate_intent_to_do_special_power(
                                    template,
                                    target_object,
                                    target_pos,
                                    waypoint,
                                    command_options,
                                )
                            {
                                valid = true;
                            }
                        }
                    });
                }

                if let Some(player) = owner_guard.get_controlling_player() {
                    if let Ok(mut player_guard) = player.write() {
                        let classification = match template.get_academy_classification_type() {
                            crate::object::special_power_template::AcademyClassificationType::Superweapon => {
                                AcademyClassificationType::Superpower
                            }
                            _ => AcademyClassificationType::None,
                        };
                        player_guard
                            .get_academy_stats_mut()
                            .record_special_power_used(classification);
                    }
                }
            }
        }

        if !valid && self.module_data.update_module_starts_attack {
            log::error!(
                "SpecialPowerModule '{}' missing update module to execute special power.",
                template.get_name()
            );
        }

        valid
    }

    /// Trigger the special power
    fn trigger_special_power(&mut self, location: Option<&Coord3D>, current_frame: FrameCount) {
        self.about_to_do_special_power(location);
        self.create_view_object(location);
        self.start_power_recharge_at(current_frame);
    }

    /// Create view object at location
    fn create_view_object(&self, location: Option<&Coord3D>) {
        if let Some(template) = &self.module_data.special_power_template {
            let vision_range = template.get_view_object_range();
            let vision_duration = template.get_view_object_duration();

            if vision_range == 0.0 || vision_duration == 0 {
                return; // No view object needed
            }

            let Some(pos) = location else {
                return;
            };

            let Some(global_data) = TheGlobalData::get() else {
                return;
            };
            let view_object_name = global_data.get_special_power_view_object_name();
            if view_object_name.is_empty() {
                return;
            }

            let Some(view_object_template) = TheThingFactory::find_template(&view_object_name)
            else {
                return;
            };

            let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
                return;
            };
            let Ok(owner_guard) = owner.read() else {
                return;
            };
            let Some(player) = owner_guard.get_controlling_player() else {
                return;
            };
            let Ok(player_guard) = player.read() else {
                return;
            };
            let Some(team) = player_guard.get_default_team() else {
                return;
            };
            let Ok(team_guard) = team.read() else {
                return;
            };

            let Ok(factory) = TheThingFactory::get() else {
                return;
            };
            let Ok(view_object) = factory.new_object(view_object_template, &team_guard) else {
                return;
            };

            if let Ok(mut view_guard) = view_object.write() {
                let _ = view_guard.set_position(pos);
                view_guard.set_shroud_clearing_range(vision_range);

                if let Some(module) = view_guard.find_update_module("DeletionUpdate") {
                    module.with_module(|module| {
                        if let Some(deletion) = module.get_deletion_lifetime_interface() {
                            deletion.set_lifetime_range(vision_duration, vision_duration);
                        }
                    });
                } else if let Some(behavior) = view_guard.find_update_behavior("DeletionUpdate") {
                    if let Ok(mut behavior) = behavior.lock() {
                        if let Some(deletion) = behavior.get_deletion_lifetime_interface() {
                            deletion.set_lifetime_range(vision_duration, vision_duration);
                        }
                    }
                }
            };
        }
    }

    /// About to do special power (notifications and sounds)
    fn about_to_do_special_power(&self, location: Option<&Coord3D>) {
        // Notify script engine (matches C++ SpecialPowerModule.cpp lines 315-318)
        if let Some(template) = &self.module_data.special_power_template {
            if let Some(owner) =
                crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
            {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(player) = owner_guard.get_controlling_player() {
                        if let Ok(player_guard) = player.read() {
                            let player_index = player_guard.get_player_index().max(0) as usize;
                            if let Ok(mut engine_guard) =
                                crate::scripting::engine::get_script_engine().write()
                            {
                                if let Some(engine) = engine_guard.as_mut() {
                                    engine.notify_of_triggered_special_power(
                                        player_index,
                                        template.get_name(),
                                        self.owner_object_id,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Play EVA launch events with C++ relationship semantics.
        if let Some(template) = &self.module_data.special_power_template {
            let power_type = template.get_special_power_type();
            let local_player = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned());
            let owner = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id);
            if let (Some(local_player), Some(owner)) = (local_player, owner) {
                let (is_own, relation) = if let Ok(owner_guard) = owner.read() {
                    let own = owner_guard
                        .get_controlling_player()
                        .and_then(|player| {
                            player.read().ok().map(|p| {
                                local_player
                                    .read()
                                    .ok()
                                    .map(|lp| lp.get_player_index() == p.get_player_index())
                            })
                        })
                        .flatten()
                        .unwrap_or(false);
                    let relation = owner_guard
                        .get_team()
                        .and_then(|team| {
                            team.read().ok().and_then(|team_guard| {
                                local_player
                                    .read()
                                    .ok()
                                    .map(|lp| lp.get_relationship_with_team(&team_guard))
                            })
                        })
                        .unwrap_or(Relationship::Neutral);
                    (own, relation)
                } else {
                    (false, Relationship::Neutral)
                };

                let launched = |own_evt, ally_evt, enemy_evt| {
                    if is_own {
                        let _ = TheEva::set_should_play(own_evt);
                    } else if relation != Relationship::Enemies {
                        let _ = TheEva::set_should_play(ally_evt);
                    } else {
                        let _ = TheEva::set_should_play(enemy_evt);
                    }
                };

                match power_type {
                    SpecialPowerType::ParticleUplinkCannon
                    | SpecialPowerType::SupwParticleUplinkCannon
                    | SpecialPowerType::LazrParticleUplinkCannon => launched(
                        crate::helpers::EvaEvent::SuperweaponLaunchedOwnParticleCannon,
                        crate::helpers::EvaEvent::SuperweaponLaunchedAllyParticleCannon,
                        crate::helpers::EvaEvent::SuperweaponLaunchedEnemyParticleCannon,
                    ),
                    SpecialPowerType::NeutronMissile
                    | SpecialPowerType::NukeNeutronMissile
                    | SpecialPowerType::SupwNeutronMissile => launched(
                        crate::helpers::EvaEvent::SuperweaponLaunchedOwnNuke,
                        crate::helpers::EvaEvent::SuperweaponLaunchedAllyNuke,
                        crate::helpers::EvaEvent::SuperweaponLaunchedEnemyNuke,
                    ),
                    SpecialPowerType::ScudStorm => launched(
                        crate::helpers::EvaEvent::SuperweaponLaunchedOwnScudStorm,
                        crate::helpers::EvaEvent::SuperweaponLaunchedAllyScudStorm,
                        crate::helpers::EvaEvent::SuperweaponLaunchedEnemyScudStorm,
                    ),
                    SpecialPowerType::GpsScrambler | SpecialPowerType::SlthGpsScrambler => {
                        launched(
                            crate::helpers::EvaEvent::SuperweaponLaunchedOwnGpsScrambler,
                            crate::helpers::EvaEvent::SuperweaponLaunchedAllyGpsScrambler,
                            crate::helpers::EvaEvent::SuperweaponLaunchedEnemyGpsScrambler,
                        )
                    }
                    SpecialPowerType::SneakAttack => launched(
                        crate::helpers::EvaEvent::SuperweaponLaunchedOwnSneakAttack,
                        crate::helpers::EvaEvent::SuperweaponLaunchedAllySneakAttack,
                        crate::helpers::EvaEvent::SuperweaponLaunchedEnemySneakAttack,
                    ),
                    _ => {
                        log::debug!(
                            "SpecialPowerModule: unhandled EVA event for power type {:?}",
                            power_type
                        );
                    }
                }
            }
        }

        // Play initiate sound
        if let Some(template) = &self.module_data.special_power_template {
            if let Some(audio) = TheAudio::get() {
                let mut audio_event = template.get_initiate_sound().clone();
                audio_event.set_object_id(self.owner_object_id);
                audio.add_audio_event(&audio_event);

                if let Some(pos) = location {
                    let mut sound_at_location = template.get_initiate_at_target_sound().clone();
                    let audio_pos = (pos.x, pos.y, pos.z);
                    sound_at_location.set_position(&audio_pos);
                    if let Some(owner) =
                        crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                    {
                        if let Ok(owner_guard) = owner.read() {
                            if let Some(player) = owner_guard.get_controlling_player() {
                                if let Ok(player_guard) = player.read() {
                                    sound_at_location.set_player_index(
                                        player_guard.get_player_index().max(0) as u32,
                                    );
                                }
                            }
                        }
                    }
                    audio.add_audio_event(&sound_at_location);
                }
            }
        }
    }
}

impl SpecialPowerModuleInterface for SpecialPowerModule {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        if let Some(template) = &self.module_data.special_power_template {
            // Compare template IDs or names
            template.get_id() == special_power_template.get_id()
        } else {
            false
        }
    }

    fn get_percent_ready(&self) -> f32 {
        // If paused and was ready, return almost ready (not 1.0 to indicate paused state)
        // Matches C++ SpecialPowerModule::getPercentReady() lines 145-148
        if self.paused_count > 0 && self.paused_percent == 1.0 {
            return 0.99999;
        }

        // If delays are disabled in global data, return 1.0 immediately
        // Matches C++ SpecialPowerModule::getPercentReady() lines 150-152
        if let Some(global_data) = TheGlobalData::get() {
            if !global_data.get_special_power_uses_delay() {
                return 1.0;
            }
        }

        // Easy case - is ready
        if self.is_ready() {
            return 1.0;
        }

        // If paused, return paused percent (frozen progress)
        if self.paused_count > 0 {
            return self.paused_percent;
        }

        // Calculate percent based on ready frame vs current frame
        // Matches C++ SpecialPowerModule::getPercentReady() lines 154-163
        if let Some(template) = &self.module_data.special_power_template {
            let reload_time = template.get_reload_time() as f32;
            if reload_time > 0.0 {
                let current_frame = crate::helpers::TheGameLogic::get_frame();
                let mut ready_frame = self.available_on_frame;

                if template.is_shared_n_sync() {
                    if let Some(owner) =
                        crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                    {
                        if let Ok(owner_guard) = owner.read() {
                            if let Some(player) = owner_guard.get_controlling_player() {
                                if let Ok(mut player_guard) = player.write() {
                                    ready_frame = player_guard
                                        .get_or_start_special_power_ready_frame(template);
                                }
                            }
                        }
                    }
                }

                let frames_remaining = ready_frame.saturating_sub(current_frame) as f32;
                return 1.0 - (frames_remaining / reload_time);
            }
        }

        0.0
    }

    fn get_power_name(&self) -> String {
        if let Some(template) = &self.module_data.special_power_template {
            template.get_name().to_string()
        } else {
            String::from("Unknown")
        }
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.module_data.special_power_template.clone()
    }

    fn get_required_science(&self) -> ScienceType {
        if let Some(template) = &self.module_data.special_power_template {
            template.get_required_science()
        } else {
            SCIENCE_INVALID
        }
    }

    fn on_special_power_creation(&mut self) {
        // Matches C++ SpecialPowerModule::onSpecialPowerCreation() lines 189-214
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Start recharge timer
        self.start_power_recharge_at(current_frame);

        if let Some(template) = &self.module_data.special_power_template {
            if template.is_shared_n_sync() {
                if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(player) = owner_guard.get_controlling_player() {
                            if let Ok(mut player_guard) = player.write() {
                                player_guard
                                    .express_special_power_ready_frame(template, current_frame);
                                self.available_on_frame =
                                    player_guard.get_or_start_special_power_ready_frame(template);
                            }
                        }
                    }
                }
            }
        }

        // Some powers start paused (e.g., require upgrade to activate)
        if self.module_data.starts_paused {
            SpecialPowerModuleInterface::pause_countdown(self, true);
        }

        if let Some(template) = &self.module_data.special_power_template {
            if template.has_public_timer() {
                if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if owner_guard.is_kind_of(KindOf::Structure) {
                            if let Some(player) = owner_guard.get_controlling_player() {
                                if let Ok(player_guard) = player.read() {
                                    TheInGameUI::add_superweapon(
                                        player_guard.get_player_index(),
                                        self.get_power_name(),
                                        self.owner_object_id,
                                        template,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn set_ready_frame(&mut self, frame: FrameCount) {
        // Set the ready frame (used by scripts to control power availability)
        // Matches C++ SpecialPowerModule::setReadyFrame() lines 217-221
        self.available_on_frame = frame;

        // Update paused frame to current frame
        // This prevents the pause system from thinking we've been paused for a long time
        self.paused_on_frame = crate::helpers::TheGameLogic::get_frame();
    }

    fn pause_countdown(&mut self, pause: bool) {
        // Reference-counted pause system - multiple sources can pause the same power
        // Matches C++ SpecialPowerModule::pauseCountdown() lines 224-243
        if pause {
            if self.paused_count == 0 {
                // Only record on first pause (not on nested pauses)
                let current_frame = crate::helpers::TheGameLogic::get_frame();
                self.paused_on_frame = current_frame;
                self.paused_percent = SpecialPowerModuleInterface::get_percent_ready(self);
            }
            self.paused_count += 1;
        } else if self.paused_count > 0 {
            self.paused_count -= 1;

            // Update ready time if fully unpaused
            // This extends the ready frame by the amount of time paused
            if self.paused_count == 0 {
                let current_frame = crate::helpers::TheGameLogic::get_frame();
                let pause_duration = current_frame.saturating_sub(self.paused_on_frame);
                self.available_on_frame = self.available_on_frame.saturating_add(pause_duration);
            }
        }
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        // Execute a self-targeted special power (no location/object target needed)
        // Matches C++ SpecialPowerModule::doSpecialPower() lines 246-261

        if self.paused_count > 0 {
            return;
        }
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        // Initiate intent (notifies update modules)
        self.initiate_intent_to_do_special_power(None, None, None, command_options);

        // Trigger immediately if update module doesn't start attack
        // Some powers are executed by update modules (like aircraft strikes)
        // Others execute immediately (like emergency repair)
        if !self.module_data.update_module_starts_attack {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            self.trigger_special_power(None, current_frame);
        }
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Execute special power targeted at a specific object
        // Matches C++ SpecialPowerModule::doSpecialPowerAtObject() lines 264-279

        if self.paused_count > 0 {
            return;
        }
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        self.initiate_intent_to_do_special_power(Some(object_id), None, None, command_options);

        if !self.module_data.update_module_starts_attack {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            let target_pos = crate::helpers::TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()));
            self.trigger_special_power(target_pos.as_ref(), current_frame);
        }
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        _angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Execute special power targeted at a map location
        // Matches C++ SpecialPowerModule::doSpecialPowerAtLocation() lines 282-297

        if self.paused_count > 0 {
            return;
        }
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        self.initiate_intent_to_do_special_power(None, Some(location), None, command_options);

        if !self.module_data.update_module_starts_attack {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            self.trigger_special_power(Some(location), current_frame);
        }
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Execute special power using a waypoint path (e.g., for aircraft routes)
        // Matches C++ SpecialPowerModule::doSpecialPowerUsingWaypoints() lines 300-315

        if self.paused_count > 0 {
            return;
        }
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        self.initiate_intent_to_do_special_power(None, None, Some(waypoint), command_options);

        if !self.module_data.update_module_starts_attack {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            self.trigger_special_power(None, current_frame);
        }
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        // Mark that the special power was triggered (called by update modules)
        // Matches C++ SpecialPowerModule::markSpecialPowerTriggered() lines 318-321
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.trigger_special_power(location, current_frame);
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        // Start the cooldown timer after power use
        // Matches C++ SpecialPowerModule::startPowerRecharge() lines 368-396

        // If cheat flag is enabled, skip recharge entirely
        // Matches C++ SpecialPowerModule::startPowerRecharge() lines 369-372
        if let Some(global_data) = TheGlobalData::get() {
            if !global_data.get_special_power_uses_delay() {
                return;
            }
        }

        if let Some(template) = &self.module_data.special_power_template {
            if template.is_shared_n_sync() {
                // Reset or start the shared timer in player's special power manager
                if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(player) = owner_guard.get_controlling_player() {
                            if let Ok(mut player_guard) = player.write() {
                                player_guard.reset_or_start_special_power_ready_frame(template);
                                return;
                            }
                        }
                    }
                }
            } else {
                self.available_on_frame = current_frame + template.get_reload_time();
            }
        } else {
            log::error!("SpecialPowerModule missing special power template");
        }
    }

    fn get_initiate_sound(&self) -> &AudioEventRts {
        &self.module_data.initiate_sound
    }

    fn is_script_only(&self) -> bool {
        self.module_data.scripted_special_power_only
    }

    fn get_reference_thing_template(&self) -> Option<String> {
        None // Override in derived classes if needed
    }
}

impl EngineSpecialPowerModuleInterface for SpecialPowerModule {
    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.do_special_power(SpecialPowerCommandOptions::NONE);
        Ok(())
    }

    fn can_activate(&self) -> bool {
        if self.paused_count > 0 {
            return false;
        }
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return false;
                }
            }
        }
        self.is_ready()
    }

    fn get_power_type(&self) -> u32 {
        if let Some(template) = &self.module_data.special_power_template {
            template.get_id()
        } else {
            0
        }
    }

    fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.start_power_recharge_at(current_frame);
        Ok(())
    }

    fn get_ready_frame(&self) -> u32 {
        if let Some(template) = &self.module_data.special_power_template {
            if template.is_shared_n_sync() {
                if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(player) = owner_guard.get_controlling_player() {
                            if let Ok(mut player_guard) = player.write() {
                                return player_guard
                                    .get_or_start_special_power_ready_frame(template);
                            }
                        }
                    }
                }
            }
        }

        let mut ready_frame = self.available_on_frame;
        let mut is_disabled = false;
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                is_disabled = owner_guard.is_disabled();
            }
        }
        if self.paused_count > 0 || is_disabled {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            let paused_frames = current_frame.saturating_sub(self.paused_on_frame);
            ready_frame = ready_frame.saturating_add(paused_frames);
        }
        ready_frame
    }

    fn is_ready(&self) -> bool {
        // Cheat for debug builds - if global data disables delays, all powers are ready
        // Matches C++ SpecialPowerModule::isReady() lines 269-273
        if let Some(global_data) = TheGlobalData::get() {
            if !global_data.get_special_power_uses_delay() {
                return true;
            }
        }

        if let Some(template) = &self.module_data.special_power_template {
            if template.is_shared_n_sync() {
                if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(player) = owner_guard.get_controlling_player() {
                            if let Ok(mut player_guard) = player.write() {
                                return crate::helpers::TheGameLogic::get_frame()
                                    >= player_guard
                                        .get_or_start_special_power_ready_frame(template);
                            }
                        }
                    }
                }
            }
        }

        let current_frame = crate::helpers::TheGameLogic::get_frame();
        (self.paused_count == 0) && (current_frame >= self.available_on_frame)
    }

    fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>> {
        self.module_data
            .special_power_template
            .clone()
            .map(|t| t as Arc<dyn std::any::Any>)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.module_data.special_power_template.clone()
    }

    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        SpecialPowerModuleInterface::is_module_for_power(self, special_power_template)
    }

    fn set_ready_frame(&mut self, frame: u32) {
        SpecialPowerModuleInterface::set_ready_frame(self, frame)
    }

    fn get_power_name(&self) -> String {
        SpecialPowerModuleInterface::get_power_name(self)
    }

    fn get_percent_ready(&self) -> f32 {
        SpecialPowerModuleInterface::get_percent_ready(self)
    }

    fn pause_countdown(&mut self, pause: bool) {
        SpecialPowerModuleInterface::pause_countdown(self, pause)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&crate::common::Coord3D>) {
        SpecialPowerModuleInterface::mark_special_power_triggered(self, location)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        SpecialPowerModuleInterface::do_special_power(self, command_options);
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        SpecialPowerModuleInterface::do_special_power_at_object(self, object_id, command_options);
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        SpecialPowerModuleInterface::do_special_power_at_location(
            self,
            location,
            angle,
            command_options,
        );
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        SpecialPowerModuleInterface::do_special_power_using_waypoints(
            self,
            waypoint,
            command_options,
        );
    }
}

impl SpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPECIAL_POWER_MODULE_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(SpecialPowerModuleData, base);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut SpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(*token);
    data.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_audio_event(
    _ini: &mut INI,
    data: &mut SpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.initiate_sound = AudioEventRts::new(*token);
    Ok(())
}

const SPECIAL_POWER_MODULE_FIELDS: &[FieldParse<SpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "UpdateModuleStartsAttack",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.update_module_starts_attack = v, tokens)
        },
    },
    FieldParse {
        token: "StartsPaused",
        parse: |_, data, tokens| parse_bool_field(&mut |v| data.starts_paused = v, tokens),
    },
    FieldParse {
        token: "InitiateSound",
        parse: parse_audio_event,
    },
    FieldParse {
        token: "ScriptedSpecialPowerOnly",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.scripted_special_power_only = v, tokens)
        },
    },
];

impl BehaviorModuleInterface for SpecialPowerModule {
    fn get_special_power(&mut self) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface(
        &mut self,
    ) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface_const(
        &self,
    ) -> Option<&dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    /// Update module interface
    fn get_update(&mut self) -> Option<&mut dyn crate::modules::UpdateModuleInterface> {
        None
    }
}

impl game_engine::common::thing::module::Module for SpecialPowerModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn get_module_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("SpecialPowerModule")
    }
    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
            &self.module_data.base,
        )
    }

    fn get_module_data(&self) -> &dyn game_engine::common::thing::module::ModuleData {
        &self.module_data
    }

    fn on_object_created(&mut self) {
        self.initialize_from_owner();
    }
}

impl BehaviorModule for SpecialPowerModule {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_destroy(&mut self) {
        if let Some(template) = &self.module_data.special_power_template {
            if template.has_public_timer() {
                if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(player) = owner_guard.get_controlling_player() {
                            if let Ok(player_guard) = player.read() {
                                TheInGameUI::remove_superweapon(
                                    player_guard.get_player_index(),
                                    self.get_power_name(),
                                    self.owner_object_id,
                                    template,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

impl fmt::Debug for SpecialPowerModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpecialPowerModule")
            .field("owner", &self.owner_object_id)
            .field("available_on_frame", &self.available_on_frame)
            .field("paused_count", &self.paused_count)
            .finish()
    }
}

impl game_engine::common::system::snapshot::Snapshotable for SpecialPowerModule {
    fn crc(&self, xfer: &mut dyn crate::common::Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn crate::common::Xfer) -> Result<(), String> {
        let mut version: crate::common::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpecialPowerModule version xfer failed: {:?}", e))?;

        xfer.xfer_unsigned_int(&mut self.available_on_frame)
            .map_err(|e| format!("SpecialPowerModule available_on_frame xfer failed: {:?}", e))?;
        xfer.xfer_int(&mut self.paused_count)
            .map_err(|e| format!("SpecialPowerModule paused_count xfer failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.paused_on_frame)
            .map_err(|e| format!("SpecialPowerModule paused_on_frame xfer failed: {:?}", e))?;
        xfer.xfer_real(&mut self.paused_percent)
            .map_err(|e| format!("SpecialPowerModule paused_percent xfer failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.resolve_special_power();

        if let Some(template) = &self.module_data.special_power_template {
            if self.paused_count == 0 && template.is_shared_n_sync() && template.has_public_timer()
            {
                if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
                    if let Ok(owner_guard) = owner.read() {
                        if owner_guard.is_kind_of(KindOf::Structure) {
                            if let Some(player) = owner_guard.get_controlling_player() {
                                if let Ok(player_guard) = player.read() {
                                    TheInGameUI::add_superweapon(
                                        player_guard.get_player_index(),
                                        self.get_power_name(),
                                        self.owner_object_id,
                                        template,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_module_creation() {
        let data = SpecialPowerModuleData::default();
        let module = SpecialPowerModule::new(1, data);

        assert_eq!(module.owner_object_id, 1);
        assert_eq!(module.available_on_frame, 0);
        assert_eq!(module.paused_count, 0);
    }

    #[test]
    fn test_pause_countdown() {
        let data = SpecialPowerModuleData::default();
        let mut module = SpecialPowerModule::new(1, data);

        SpecialPowerModuleInterface::pause_countdown(&mut module, true);
        assert_eq!(module.paused_count, 1);

        SpecialPowerModuleInterface::pause_countdown(&mut module, true);
        assert_eq!(module.paused_count, 2);

        SpecialPowerModuleInterface::pause_countdown(&mut module, false);
        assert_eq!(module.paused_count, 1);

        SpecialPowerModuleInterface::pause_countdown(&mut module, false);
        assert_eq!(module.paused_count, 0);
    }

    #[test]
    fn test_is_script_only() {
        let mut data = SpecialPowerModuleData::default();
        data.scripted_special_power_only = true;

        let module = SpecialPowerModule::new(1, data);
        assert!(module.is_script_only());
    }

    #[test]
    fn test_resolve_special_power_binds_to_store_entry() {
        let name = AsciiString::from("TestResolveSpecialPowerBind");
        let canonical = find_or_create_special_power_template(&name);
        let mut data = SpecialPowerModuleData::default();
        data.special_power_template = Some(Arc::new(SpecialPowerTemplate::new(
            name.to_string(),
            canonical.get_id() + 1000,
        )));

        let mut module = SpecialPowerModule::new(1, data);
        module.resolve_special_power();

        let resolved = module
            .module_data
            .special_power_template
            .as_ref()
            .expect("template should be resolved");
        assert_eq!(resolved.get_name(), canonical.get_name());
        assert_eq!(resolved.get_id(), canonical.get_id());
    }
}
