//! AutoDepositUpdate - Rust conversion of C++ AutoDepositUpdate
//!
//! Auto deposit money/resources over time.
//! Original C++: AutoDepositUpdate.cpp by Chris Huybregts, Aug 2002
//! Rust conversion: 2025
//!
//! FILE: AutoDepositUpdate.cpp lines 1-268

use crate::common::{
    AsciiString, Bool, Coord3D, Int, KindOf, ModuleData, UnsignedInt, CONSTRUCTION_COMPLETE,
};
use crate::helpers::{game_client_random_value_real, TheGameText, TheInGameUI};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use crate::upgrade::center::get_upgrade_center;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

// Upgrade pair structure for supply boost. Matches C++ lines 47-76
#[derive(Clone, Debug)]
pub struct UpgradePair {
    pub upgrade_type: AsciiString,
    pub boost_amount: Int,
}

/// AutoDepositUpdateModuleData - Configuration for auto deposit behavior
/// Matches C++ AutoDepositUpdate.cpp (module data structure)
#[derive(Clone, Debug)]
pub struct AutoDepositUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Frames between deposits. Matches C++ AutoDepositUpdate.cpp
    pub deposit_frame: UnsignedInt,
    /// Amount to deposit each time. Matches C++ line 143
    pub deposit_amount: Int,
    /// Bonus awarded on initial capture. Matches C++ lines 99-117
    pub initial_capture_bonus: Int,
    /// Whether this is actual money or just a counter. Matches C++ line 145
    pub is_actual_money: Bool,
    /// List of upgrade pairs that boost supply. Matches C++ lines 195-218
    pub upgrade_boost: Vec<UpgradePair>,
}

impl Default for AutoDepositUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            deposit_frame: 0,
            deposit_amount: 0,
            initial_capture_bonus: 0,
            is_actual_money: true,
            upgrade_boost: Vec::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(AutoDepositUpdateModuleData, base);

impl AutoDepositUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, AUTO_DEPOSIT_UPDATE_FIELDS)
    }
}

/// AutoDepositUpdate - Automatically deposits resources over time
///
/// Matches C++ AutoDepositUpdate.cpp lines 81-268
pub struct AutoDepositUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<AutoDepositUpdateModuleData>,
    /// Frame when next deposit occurs. Matches C++ line 83
    deposit_on_frame: UnsignedInt,
    /// Whether to award initial capture bonus. Matches C++ line 84
    award_initial_capture_bonus: Bool,
    /// Whether module has been initialized. Matches C++ line 85 (version 2)
    initialized: Bool,
}

impl AutoDepositUpdate {
    /// Creates a new AutoDepositUpdate. Matches C++ lines 81-86
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<AutoDepositUpdateModuleData>()
            .ok_or("Invalid module data for AutoDepositUpdate")?;

        // Get current frame from game logic
        // Matches C++ TheGameLogic->getFrame() (line 99)
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            // Matches C++ line 83
            deposit_on_frame: current_frame + specific_data.deposit_frame,
            // Matches C++ line 84
            award_initial_capture_bonus: false,
            // Matches C++ line 85
            initialized: false,
        })
    }

    /// Award the initial capture bonus. Matches C++ lines 96-118
    pub fn award_initial_capture_bonus(
        &mut self,
        player: Option<Arc<RwLock<crate::common::Player>>>,
    ) {
        // Reset deposit frame. Matches C++ line 98
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.deposit_on_frame = current_frame + self.module_data.deposit_frame;

        // Check conditions. Matches C++ lines 99-100
        if !self.award_initial_capture_bonus || self.module_data.initial_capture_bonus <= 0 {
            return;
        }

        if let Some(player_arc) = player {
            if let Ok(mut player_guard) = player_arc.write() {
                // Deposit money (with standard sound/academy bookkeeping). Matches C++ line 102
                let _ = player_guard
                    .get_money_mut()
                    .deposit(self.module_data.initial_capture_bonus as u32);

                // Add to score keeper. Matches C++ line 103
                player_guard
                    .get_score_keeper_mut()
                    .add_money_earned(self.module_data.initial_capture_bonus as u32);

                // Display floating text. Matches C++ lines 105-115
                let text = format_add_cash(self.module_data.initial_capture_bonus);
                let mut pos = self
                    .object
                    .upgrade()
                    .and_then(|obj| obj.read().ok().map(|g| *g.get_position()))
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));
                pos.z += 10.0;

                let mut color = player_guard.get_player_color();
                color.a = 230;
                let _ = TheInGameUI::add_floating_text(&text, &pos, color);
            }
        }

        // Clear the flag. Matches C++ line 117
        self.award_initial_capture_bonus = false;
    }

    /// Get the upgraded supply boost amount. Matches C++ lines 195-218
    fn get_upgraded_supply_boost(&self) -> Int {
        // Get controlling player
        let object = match self.object.upgrade() {
            Some(obj) => obj,
            None => return 0,
        };

        let obj_read = match object.read() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };

        let player = match obj_read.get_controlling_player() {
            Some(p) => p,
            None => return 0, // Matches C++ line 198
        };

        // Loop through upgrade pairs. Matches C++ lines 201-215
        let Ok(player_guard) = player.read() else {
            return 0;
        };

        let upgrade_center = get_upgrade_center();
        let Ok(center_guard) = upgrade_center.read() else {
            return 0;
        };

        for upgrade_pair in &self.module_data.upgrade_boost {
            if let Some(template) = center_guard.find_upgrade(upgrade_pair.upgrade_type.as_str()) {
                if player_guard.has_upgrade_complete(&template) {
                    return upgrade_pair.boost_amount;
                }
            }
        }

        0 // Matches C++ line 217
    }
}

impl UpdateModuleInterface for AutoDepositUpdate {
    /// Main update loop. Matches C++ lines 122-192
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Matches C++ TheGameLogic->getFrame() (line 124)
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Check if it's time to deposit. Matches C++ line 126
        if current_frame >= self.deposit_on_frame {
            // Initialize on first update. Matches C++ lines 128-133
            if !self.initialized {
                self.award_initial_capture_bonus = true;
                self.initialized = true;
            }

            // Schedule next deposit. Matches C++ line 134
            self.deposit_on_frame = current_frame + self.module_data.deposit_frame;

            let object = match self.object.upgrade() {
                Some(obj) => obj,
                None => return UpdateSleepTime::None,
            };

            let obj_read = match object.read() {
                Ok(guard) => guard,
                Err(_) => return UpdateSleepTime::None,
            };

            // Don't deposit if neutral or amount <= 0. Matches C++ lines 136-137
            if obj_read.is_neutral_controlled() || self.module_data.deposit_amount <= 0 {
                return UpdateSleepTime::None; // UPDATE_SLEEP_NONE
            }

            // Don't deposit for buildings under construction. Matches C++ lines 140-141
            if obj_read.get_construction_percent() != CONSTRUCTION_COMPLETE as i32 {
                return UpdateSleepTime::None; // UPDATE_SLEEP_NONE
            }

            // Calculate money amount with upgrades. Matches C++ line 143
            let money_amount = self.module_data.deposit_amount + self.get_upgraded_supply_boost();

            // Deposit actual money if configured. Matches C++ lines 145-149
            if self.module_data.is_actual_money {
                if let Some(player) = obj_read.get_controlling_player() {
                    if let Ok(mut player_guard) = player.write() {
                        if money_amount > 0 {
                            let _ = player_guard.get_money_mut().deposit(money_amount as u32);
                        }
                        if self.module_data.deposit_amount > 0 {
                            player_guard
                                .get_score_keeper_mut()
                                .add_money_earned(self.module_data.deposit_amount as u32);
                        }
                    }
                }
            }

            // Determine if we should display floating money text. Matches C++ lines 151-160
            let mut display_money = money_amount > 0;
            if obj_read.is_stealthed() {
                // Only show for local player if detected. Matches C++ lines 154-159
                if !obj_read.is_locally_controlled() && !obj_read.is_detected() {
                    display_money = false;
                }
            }

            // Display floating text. Matches C++ lines 162-188
            if display_money {
                let text = format_add_cash(money_amount);
                let mut pos = *obj_read.get_position();
                pos.z += 10.0;

                if obj_read.is_kind_of(KindOf::Structure) {
                    let geom = obj_read.get_geometry_info();
                    let width = geom.get_major_radius() * 0.3;
                    let depth = geom.get_minor_radius() * 0.3;
                    pos.x += game_client_random_value_real(-width, width);
                    pos.y += game_client_random_value_real(-depth, depth);
                }

                if let Some(player) = obj_read.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        let mut color = player_guard.get_player_color();
                        color.a = 230;
                        let _ = TheInGameUI::add_floating_text(&text, &pos, color);
                    }
                }
            }
        }

        UpdateSleepTime::None // UPDATE_SLEEP_NONE, matches C++ line 191
    }
}

impl BehaviorModuleInterface for AutoDepositUpdate {
    fn get_module_name(&self) -> &'static str {
        "AutoDepositUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

/// Factory for creating AutoDepositUpdate behaviors
pub struct AutoDepositUpdateFactory;

impl AutoDepositUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AutoDepositUpdate::new(thing, module_data)?))
    }
}

// Thread safety
unsafe impl Send for AutoDepositUpdate {}
unsafe impl Sync for AutoDepositUpdate {}

impl Snapshotable for AutoDepositUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("AutoDepositUpdate xfer version failed: {:?}", e))?;

        xfer.xfer_unsigned_int(&mut self.deposit_on_frame)
            .map_err(|e| format!("AutoDepositUpdate xfer deposit_on_frame failed: {:?}", e))?;
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.award_initial_capture_bonus).map_err(
            |e| {
                format!(
                    "AutoDepositUpdate xfer award_initial_capture_bonus failed: {:?}",
                    e
                )
            },
        )?;
        if version > 1 {
            game_engine::system::Xfer::xfer_bool(xfer, &mut self.initialized)
                .map_err(|e| format!("AutoDepositUpdate xfer initialized failed: {:?}", e))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes AutoDepositUpdate through the common Module trait.
pub struct AutoDepositUpdateModule {
    behavior: AutoDepositUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<AutoDepositUpdateModuleData>,
}

impl AutoDepositUpdateModule {
    pub fn new(
        behavior: AutoDepositUpdate,
        module_name: &AsciiString,
        module_data: Arc<AutoDepositUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut AutoDepositUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for AutoDepositUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for AutoDepositUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_deposit_timing(
    _ini: &mut INI,
    data: &mut AutoDepositUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.deposit_frame = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_deposit_amount(
    _ini: &mut INI,
    data: &mut AutoDepositUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.deposit_amount = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_initial_capture_bonus(
    _ini: &mut INI,
    data: &mut AutoDepositUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.initial_capture_bonus = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_actual_money(
    _ini: &mut INI,
    data: &mut AutoDepositUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let token = tokens[0].to_ascii_lowercase();
    data.is_actual_money = token == "true" || token == "yes" || token == "1";
    Ok(())
}

fn parse_upgrade_boost(
    _ini: &mut INI,
    data: &mut AutoDepositUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut parts: Vec<&str> = Vec::new();
    for token in tokens {
        for part in token.split(':') {
            if !part.is_empty() {
                parts.push(part);
            }
        }
    }

    let mut upgrade_type: Option<AsciiString> = None;
    let mut boost_amount: Option<Int> = None;
    let mut iter = parts.into_iter();

    while let Some(key) = iter.next() {
        match key.to_ascii_lowercase().as_str() {
            "upgradetype" => {
                let value = iter.next().ok_or(INIError::InvalidData)?;
                upgrade_type = Some(AsciiString::from(value));
            }
            "boost" => {
                let value = iter.next().ok_or(INIError::InvalidData)?;
                boost_amount = Some(value.parse().map_err(|_| INIError::InvalidData)?);
            }
            _ => {}
        }
    }

    let upgrade_type = upgrade_type.ok_or(INIError::InvalidData)?;
    let boost_amount = boost_amount.ok_or(INIError::InvalidData)?;
    data.upgrade_boost.push(UpgradePair {
        upgrade_type,
        boost_amount,
    });
    Ok(())
}

const AUTO_DEPOSIT_UPDATE_FIELDS: &[FieldParse<AutoDepositUpdateModuleData>] = &[
    FieldParse {
        token: "DepositTiming",
        parse: parse_deposit_timing,
    },
    FieldParse {
        token: "DepositAmount",
        parse: parse_deposit_amount,
    },
    FieldParse {
        token: "InitialCaptureBonus",
        parse: parse_initial_capture_bonus,
    },
    FieldParse {
        token: "ActualMoney",
        parse: parse_actual_money,
    },
    FieldParse {
        token: "UpgradedBoost",
        parse: parse_upgrade_boost,
    },
];

fn format_add_cash(amount: Int) -> String {
    let template = TheGameText::fetch("GUI:AddCash");
    if template.contains("%d") || template.contains("%i") {
        template
            .replace("%d", &amount.to_string())
            .replace("%i", &amount.to_string())
    } else if template.contains("%f") {
        template.replace("%f", &format!("{:.0}", amount))
    } else {
        format!("+${}", amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_defaults() {
        let data = AutoDepositUpdateModuleData::default();
        assert_eq!(data.deposit_frame, 0);
        assert_eq!(data.deposit_amount, 0);
        assert_eq!(data.initial_capture_bonus, 0);
        assert!(data.is_actual_money);
        assert!(data.upgrade_boost.is_empty());
    }

    #[test]
    fn test_upgrade_pair() {
        let pair = UpgradePair {
            upgrade_type: "TestUpgrade".into(),
            boost_amount: 100,
        };
        assert_eq!(pair.boost_amount, 100);
    }

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames(&["1500ms"]).expect("duration"), 45);
        assert_eq!(parse_duration_frames(&["1.5s"]).expect("duration"), 45);
    }
}
