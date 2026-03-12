//! Generate Minefield Behavior Module
//!
//! Creates minefields around objects, either on construction, on death, or when upgraded.
//! Supports various patterns like circular, rectangular, and border-only placement.
//!
//! Author: Colin Day, December 2001 (Original C++)
//! Converted to Rust: 2025

use crate::common::xfer::XferExt;
use crate::common::{AsciiString, Coord3D, ModuleData, PathfindLayerEnum, Real, UpgradeMaskType};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, UpdateModuleInterface, UpdateSleepTime,
    UpgradeModuleInterface, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use crate::upgrade::center::THE_UPGRADE_CENTER;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

trait Coord3DExt {
    fn distance_to(&self, other: &Coord3D) -> f32;
    fn distance_2d(&self, other: &Coord3D) -> f32;
}

impl Coord3DExt for Coord3D {
    fn distance_to(&self, other: &Coord3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    fn distance_2d(&self, other: &Coord3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Object ID type
pub type ObjectId = u32;

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectId = 0;

/// Team ID type
pub type TeamId = u32;

/// Mine template identifier
pub type MineTemplateId = String;

/// FX template identifier
pub type FXTemplateId = String;

/// Geometry information for object footprints
#[derive(Debug, Clone)]
pub struct GeometryInfo {
    pub center: Coord3D,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub rotation: f32,
    pub is_circular: bool,
}

/// Configuration data for generate minefield behavior
#[derive(Debug, Clone)]
pub struct GenerateMinefieldBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Name of the mine template to create
    pub mine_name: MineTemplateId,
    /// Name of upgraded mine template
    pub mine_name_upgraded: Option<MineTemplateId>,
    /// Upgrade that triggers mine upgrade
    pub mine_upgrade_trigger: Option<String>,
    /// FX to play when generating mines
    pub generation_fx: Option<FXTemplateId>,
    /// Distance around object to place mines
    pub distance_around_object: f32,
    /// Density of mines per square foot
    pub mines_per_square_foot: f32,
    /// Random jitter for mine placement
    pub random_jitter: f32,
    /// Skip mine placement if this much area is under structure
    pub skip_if_this_much_under_structure: f32,
    /// Generate mines only on death
    pub on_death: bool,
    /// Place mines only on border
    pub border_only: bool,
    /// Always use circular pattern
    pub always_circular: bool,
    /// Whether minefield can be upgraded
    pub upgradable: bool,
    /// Use smart border algorithm
    pub smart_border: bool,
    /// Skip interior when using smart border
    pub smart_border_skip_interior: bool,
}

impl Default for GenerateMinefieldBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            mine_name: String::new(),
            mine_name_upgraded: None,
            mine_upgrade_trigger: None,
            generation_fx: None,
            distance_around_object: 50.0,
            mines_per_square_foot: 0.01,
            random_jitter: 0.0,
            skip_if_this_much_under_structure: 0.33,
            on_death: false,
            border_only: true,
            always_circular: false,
            upgradable: false,
            smart_border: false,
            smart_border_skip_interior: true,
        }
    }
}

crate::impl_behavior_module_data_via_base!(GenerateMinefieldBehaviorModuleData, base);

impl GenerateMinefieldBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, GENERATE_MINEFIELD_BEHAVIOR_FIELDS)
    }
}

fn parse_mine_name(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.mine_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_upgraded_mine_name(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    let value = INI::parse_ascii_string(token)?;
    data.mine_name_upgraded = if value.is_empty() { None } else { Some(value) };
    Ok(())
}

fn parse_upgrade_trigger(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    let value = INI::parse_ascii_string(token)?;
    data.mine_upgrade_trigger = if value.is_empty() { None } else { Some(value) };
    Ok(())
}

fn parse_generation_fx(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    let value = INI::parse_ascii_string(token)?;
    data.generation_fx = if value.is_empty() { None } else { Some(value) };
    Ok(())
}

fn parse_distance_around_object(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.distance_around_object = INI::parse_real(token)?;
    Ok(())
}

fn parse_mines_per_square_foot(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.mines_per_square_foot = INI::parse_real(token)?;
    Ok(())
}

fn parse_generate_only_on_death(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.on_death = INI::parse_bool(token)?;
    Ok(())
}

fn parse_border_only(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.border_only = INI::parse_bool(token)?;
    Ok(())
}

fn parse_smart_border(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.smart_border = INI::parse_bool(token)?;
    Ok(())
}

fn parse_smart_border_skip_interior(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.smart_border_skip_interior = INI::parse_bool(token)?;
    Ok(())
}

fn parse_always_circular(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.always_circular = INI::parse_bool(token)?;
    Ok(())
}

fn parse_upgradable(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.upgradable = INI::parse_bool(token)?;
    Ok(())
}

fn parse_random_jitter(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.random_jitter = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_skip_if_this_much_under_structure(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.skip_if_this_much_under_structure = INI::parse_percent_to_real(token)?;
    Ok(())
}

const GENERATE_MINEFIELD_BEHAVIOR_FIELDS: &[FieldParse<GenerateMinefieldBehaviorModuleData>] = &[
    FieldParse {
        token: "MineName",
        parse: parse_mine_name,
    },
    FieldParse {
        token: "UpgradedMineName",
        parse: parse_upgraded_mine_name,
    },
    FieldParse {
        token: "UpgradedTriggeredBy",
        parse: parse_upgrade_trigger,
    },
    FieldParse {
        token: "GenerationFX",
        parse: parse_generation_fx,
    },
    FieldParse {
        token: "DistanceAroundObject",
        parse: parse_distance_around_object,
    },
    FieldParse {
        token: "MinesPerSquareFoot",
        parse: parse_mines_per_square_foot,
    },
    FieldParse {
        token: "GenerateOnlyOnDeath",
        parse: parse_generate_only_on_death,
    },
    FieldParse {
        token: "BorderOnly",
        parse: parse_border_only,
    },
    FieldParse {
        token: "SmartBorder",
        parse: parse_smart_border,
    },
    FieldParse {
        token: "SmartBorderSkipInterior",
        parse: parse_smart_border_skip_interior,
    },
    FieldParse {
        token: "AlwaysCircular",
        parse: parse_always_circular,
    },
    FieldParse {
        token: "Upgradable",
        parse: parse_upgradable,
    },
    FieldParse {
        token: "RandomJitter",
        parse: parse_random_jitter,
    },
    FieldParse {
        token: "SkipIfThisMuchUnderStructure",
        parse: parse_skip_if_this_much_under_structure,
    },
];

/// Result type for behavior operations
pub type BehaviorResult<T> = Result<T, BehaviorError>;

/// Error types for behavior operations
#[derive(Debug, thiserror::Error)]
pub enum BehaviorError {
    #[error("Object not found: {id}")]
    ObjectNotFound { id: ObjectId },
    #[error("Mine template not found: {template}")]
    MineTemplateNotFound { template: String },
    #[error("Invalid position")]
    InvalidPosition,
    #[error("No space available for mine placement")]
    NoSpaceAvailable,
    #[error("Minefield already generated")]
    MinefieldAlreadyGenerated,
}

/// Mine placement pattern
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinePlacementPattern {
    Circular,
    Rectangular,
    FootprintBased,
    SmartBorder,
}

/// Thread-safe generate minefield behavior implementation
#[derive(Debug)]
pub struct GenerateMinefieldBehavior {
    /// Configuration data
    config: GenerateMinefieldBehaviorModuleData,
    /// Internal state
    state: Arc<RwLock<MinefieldState>>,
    /// Object ID this behavior belongs to
    object_id: ObjectId,
}

/// Internal state for the minefield behavior
#[derive(Debug)]
struct MinefieldState {
    /// Target position for minefield generation
    target: Option<Coord3D>,
    /// Whether minefield has been generated
    generated: bool,
    /// Whether mines have been upgraded
    upgraded: bool,
    /// List of placed mine IDs
    mine_list: Vec<ObjectId>,
    /// Current mine template being used
    current_mine_template: MineTemplateId,
}

impl GenerateMinefieldBehavior {
    /// Create a new generate minefield behavior from explicit config (builder/tests).
    pub fn new_from_config(
        object_id: ObjectId,
        config: GenerateMinefieldBehaviorModuleData,
    ) -> Self {
        let state = MinefieldState {
            target: None,
            generated: false,
            upgraded: false,
            mine_list: Vec::new(),
            current_mine_template: config.mine_name.clone(),
        };

        Self {
            config,
            state: Arc::new(RwLock::new(state)),
            object_id,
        }
    }

    /// Create a new generate minefield behavior from module data.
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<GenerateMinefieldBehaviorModuleData>()
            .ok_or("Invalid module data")?;
        let object_id = object.read().map(|guard| guard.get_id()).unwrap_or(0);
        Ok(Self::new_from_config(object_id, specific_data.clone()))
    }

    /// Update the behavior (called each frame)
    pub fn update(&mut self, _current_frame: u32) -> BehaviorResult<UpdateSleepTime> {
        let mut state = self.state.write().unwrap();

        if self.config.upgradable && !state.upgraded && state.generated {
            let upgrade_name = self
                .config
                .mine_upgrade_trigger
                .as_deref()
                .unwrap_or("Upgrade_ChinaEMPMines");
            let upgrade = THE_UPGRADE_CENTER
                .read()
                .ok()
                .and_then(|center| center.find_upgrade(upgrade_name));

            if let Some(upgrade) = upgrade {
                let mask_bits = UpgradeMaskType::from_bits_retain(upgrade.mask().bits());
                let has_upgrade = crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .and_then(|obj| {
                        obj.read()
                            .ok()
                            .map(|guard| guard.completed_upgrades().contains(mask_bits))
                    })
                    .unwrap_or(false);

                if has_upgrade {
                    let mine_ids = state.mine_list.clone();
                    state.mine_list.clear();
                    state.generated = false;
                    state.upgraded = true;
                    drop(state);

                    for mine_id in mine_ids {
                        let _ = self.remove_mine(mine_id);
                    }

                    self.place_mines()?;
                    return Ok(UpdateSleepTime::None);
                }
            }

            return Ok(UpdateSleepTime::None);
        }

        if self.config.upgradable && !state.upgraded {
            return Ok(UpdateSleepTime::None);
        }

        Ok(UpdateSleepTime::Forever)
    }

    /// Set minefield target position
    pub fn set_minefield_target(&self, position: Option<Coord3D>) {
        let mut state = self.state.write().unwrap();
        state.target = position;
    }

    /// Get minefield target position
    pub fn get_minefield_target(&self) -> Option<Coord3D> {
        let state = self.state.read().unwrap();
        if let Some(target) = state.target {
            return Some(target);
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_position().clone()))
    }

    /// Place mines in the minefield
    pub fn place_mines(&self) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();

        if state.generated {
            return Err(BehaviorError::MinefieldAlreadyGenerated);
        }

        // Get object geometry information
        let geometry = self.get_object_geometry()?;
        // Resolve target without calling `get_minefield_target` while holding the write lock.
        let target = state
            .target
            .or_else(|| {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .and_then(|obj| obj.read().ok().map(|guard| guard.get_position().clone()))
            })
            .unwrap_or(geometry.center);
        let mut placement_geometry = geometry.clone();
        placement_geometry.center = target;

        // Determine mine template to use
        let mine_template = if state.upgraded && self.config.mine_name_upgraded.is_some() {
            self.config.mine_name_upgraded.as_ref().unwrap()
        } else {
            &self.config.mine_name
        };

        // Play generation FX if specified
        if let Some(ref fx_template) = self.config.generation_fx {
            self.play_fx(fx_template, &geometry.center)?;
        }

        // Place mines based on configuration
        let pattern = self.determine_placement_pattern(&placement_geometry);
        self.place_mines_with_pattern(&mut state, &placement_geometry, mine_template, pattern)?;

        state.generated = true;
        state.current_mine_template = mine_template.clone();

        Ok(())
    }

    /// Determine mine placement pattern based on configuration
    fn determine_placement_pattern(&self, geometry: &GeometryInfo) -> MinePlacementPattern {
        if self.config.smart_border {
            MinePlacementPattern::SmartBorder
        } else if self.config.always_circular || geometry.is_circular {
            MinePlacementPattern::Circular
        } else if self.config.border_only {
            MinePlacementPattern::Rectangular
        } else {
            MinePlacementPattern::FootprintBased
        }
    }

    /// Place mines using specified pattern
    fn place_mines_with_pattern(
        &self,
        state: &mut MinefieldState,
        geometry: &GeometryInfo,
        mine_template: &str,
        pattern: MinePlacementPattern,
    ) -> BehaviorResult<()> {
        match pattern {
            MinePlacementPattern::Circular => {
                self.place_mines_around_circle(
                    state,
                    &geometry.center,
                    geometry.major_radius,
                    mine_template,
                )?;
            }
            MinePlacementPattern::Rectangular => {
                self.place_mines_around_rect(
                    state,
                    &geometry.center,
                    geometry.major_radius,
                    geometry.minor_radius,
                    mine_template,
                )?;
            }
            MinePlacementPattern::FootprintBased => {
                self.place_mines_in_footprint(state, geometry, mine_template)?;
            }
            MinePlacementPattern::SmartBorder => {
                self.place_mines_smart_border(state, geometry, mine_template)?;
            }
        }
        Ok(())
    }

    /// Place mines in a circular pattern
    fn place_mines_around_circle(
        &self,
        state: &mut MinefieldState,
        center: &Coord3D,
        radius: f32,
        mine_template: &str,
    ) -> BehaviorResult<()> {
        let effective_radius = radius + self.config.distance_around_object;
        let circumference = 2.0 * std::f32::consts::PI * effective_radius;
        let mine_spacing = (1.0 / self.config.mines_per_square_foot).sqrt();
        let num_mines = (circumference / mine_spacing).round() as u32;

        for i in 0..num_mines {
            let angle = (i as f32 / num_mines as f32) * 2.0 * std::f32::consts::PI;
            let mut position = Coord3D::new(
                center.x + effective_radius * angle.cos(),
                center.y + effective_radius * angle.sin(),
                center.z,
            );

            // Apply random jitter
            if self.config.random_jitter > 0.0 {
                position.x +=
                    self.random_value(-self.config.random_jitter, self.config.random_jitter);
                position.y +=
                    self.random_value(-self.config.random_jitter, self.config.random_jitter);
            }

            if let Ok(mine_id) = self.place_mine_at(&position, mine_template) {
                if self.config.upgradable {
                    state.mine_list.push(mine_id);
                }
            }
        }

        Ok(())
    }

    /// Place mines in a rectangular pattern
    fn place_mines_around_rect(
        &self,
        state: &mut MinefieldState,
        center: &Coord3D,
        major_radius: f32,
        minor_radius: f32,
        mine_template: &str,
    ) -> BehaviorResult<()> {
        let effective_major = major_radius + self.config.distance_around_object;
        let effective_minor = minor_radius + self.config.distance_around_object;
        let mine_spacing = (1.0 / self.config.mines_per_square_foot).sqrt();

        // Place mines along the four sides of the rectangle
        self.place_mines_along_line(
            state,
            &Coord3D::new(
                center.x - effective_major,
                center.y - effective_minor,
                center.z,
            ),
            &Coord3D::new(
                center.x + effective_major,
                center.y - effective_minor,
                center.z,
            ),
            mine_template,
            mine_spacing,
        )?;

        self.place_mines_along_line(
            state,
            &Coord3D::new(
                center.x + effective_major,
                center.y - effective_minor,
                center.z,
            ),
            &Coord3D::new(
                center.x + effective_major,
                center.y + effective_minor,
                center.z,
            ),
            mine_template,
            mine_spacing,
        )?;

        self.place_mines_along_line(
            state,
            &Coord3D::new(
                center.x + effective_major,
                center.y + effective_minor,
                center.z,
            ),
            &Coord3D::new(
                center.x - effective_major,
                center.y + effective_minor,
                center.z,
            ),
            mine_template,
            mine_spacing,
        )?;

        self.place_mines_along_line(
            state,
            &Coord3D::new(
                center.x - effective_major,
                center.y + effective_minor,
                center.z,
            ),
            &Coord3D::new(
                center.x - effective_major,
                center.y - effective_minor,
                center.z,
            ),
            mine_template,
            mine_spacing,
        )?;

        Ok(())
    }

    /// Place mines in footprint-based pattern
    fn place_mines_in_footprint(
        &self,
        state: &mut MinefieldState,
        geometry: &GeometryInfo,
        mine_template: &str,
    ) -> BehaviorResult<()> {
        // For footprint-based placement, use a grid pattern within the expanded geometry
        let effective_major = geometry.major_radius + self.config.distance_around_object;
        let effective_minor = geometry.minor_radius + self.config.distance_around_object;
        let mine_spacing = (1.0 / self.config.mines_per_square_foot).sqrt();

        let x_count = ((2.0 * effective_major) / mine_spacing).ceil() as i32;
        let y_count = ((2.0 * effective_minor) / mine_spacing).ceil() as i32;

        for x in 0..x_count {
            for y in 0..y_count {
                let position = Coord3D::new(
                    geometry.center.x - effective_major + (x as f32 * mine_spacing),
                    geometry.center.y - effective_minor + (y as f32 * mine_spacing),
                    geometry.center.z,
                );

                // Check if position is valid for mine placement
                if self.is_position_valid_for_mine(&position)? {
                    if let Ok(mine_id) = self.place_mine_at(&position, mine_template) {
                        if self.config.upgradable {
                            state.mine_list.push(mine_id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Place mines using smart border algorithm
    fn place_mines_smart_border(
        &self,
        state: &mut MinefieldState,
        geometry: &GeometryInfo,
        mine_template: &str,
    ) -> BehaviorResult<()> {
        // Smart border uses more intelligent placement based on terrain and existing objects
        // For now, use circular pattern as fallback
        self.place_mines_around_circle(
            state,
            &geometry.center,
            geometry.major_radius,
            mine_template,
        )
    }

    /// Place mines along a line between two points
    fn place_mines_along_line(
        &self,
        state: &mut MinefieldState,
        start: &Coord3D,
        end: &Coord3D,
        mine_template: &str,
        spacing: f32,
    ) -> BehaviorResult<()> {
        let distance = start.distance_2d(end);
        let num_mines = (distance / spacing).round() as u32;

        if num_mines == 0 {
            return Ok(());
        }

        for i in 0..=num_mines {
            let t = if num_mines > 0 {
                i as f32 / num_mines as f32
            } else {
                0.0
            };
            let mut position = Coord3D::new(
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t,
                start.z + (end.z - start.z) * t,
            );

            // Apply random jitter
            if self.config.random_jitter > 0.0 {
                position.x +=
                    self.random_value(-self.config.random_jitter, self.config.random_jitter);
                position.y +=
                    self.random_value(-self.config.random_jitter, self.config.random_jitter);
            }

            if let Ok(mine_id) = self.place_mine_at(&position, mine_template) {
                if self.config.upgradable {
                    state.mine_list.push(mine_id);
                }
            }
        }

        Ok(())
    }

    /// Place a single mine at a specific position
    /// C++ Reference: GenerateMinefieldBehavior.cpp - placeMineAt() lines 171-221
    fn place_mine_at(&self, position: &Coord3D, mine_template: &str) -> BehaviorResult<ObjectId> {
        let owner = crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .ok_or(BehaviorError::ObjectNotFound { id: self.object_id })?;

        let (owner_pos, owner_team) = owner
            .read()
            .map(|guard| (guard.get_position().clone(), guard.get_team()))
            .map_err(|_| BehaviorError::ObjectNotFound { id: self.object_id })?;

        // C++ lines 173-181: Check terrain validity
        // layer = TheTerrainLogic->getHighestLayerForDestination(&tmp)
        // if (layer == LAYER_GROUND && TheTerrainLogic->isUnderwater(pt.x, pt.y)) return NULL
        // if (layer == LAYER_GROUND && TheTerrainLogic->isCliffCell(pt.x, pt.y)) return NULL
        if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
            let mut tmp = *position;
            tmp.z = 99999.0;
            let layer = terrain.get_highest_layer_for_destination(&tmp);

            if layer == PathfindLayerEnum::Ground {
                if terrain.is_underwater(position.x, position.y, None, None) {
                    return Err(BehaviorError::InvalidPosition);
                }
                if terrain.is_cliff_cell(position.x, position.y) {
                    return Err(BehaviorError::InvalidPosition);
                }
            }
        }

        let template =
            crate::helpers::TheThingFactory::find_template(mine_template).ok_or_else(|| {
                BehaviorError::MineTemplateNotFound {
                    template: mine_template.to_string(),
                }
            })?;

        // C++ lines 187-197: Check for structure overlap using partition manager
        // Uses ThePartitionManager->iteratePotentialCollisions() and checks KINDOF_STRUCTURE
        let mine_radius = template
            .get_template_geometry_info()
            .get_bounding_circle_radius();
        let shrink_radius = mine_radius * (1.0 - self.config.skip_if_this_much_under_structure);
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            let objects_near = partition.get_objects_in_range(position, shrink_radius.max(0.0));
            for obj_id in objects_near {
                if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) {
                    if let Ok(obj_guard) = obj.read() {
                        if obj_guard.is_kind_of(crate::common::KindOf::Structure) {
                            return Err(BehaviorError::NoSpaceAvailable);
                        }
                    }
                }
            }
        }

        // C++ lines 183, 199-202: Create mine with random orientation
        // Real orient = GameLogicRandomValueReal(-PI, PI);
        // Object* mine = TheThingFactory->newObject(mineTemplate, team);
        // mine->setPosition(&pt);
        // mine->setOrientation(orient);
        let orientation = self.random_value(-std::f32::consts::PI, std::f32::consts::PI);

        let factory = crate::helpers::TheThingFactory::get().map_err(|_| {
            BehaviorError::MineTemplateNotFound {
                template: mine_template.to_string(),
            }
        })?;
        let mine = if let Some(team) = owner_team.as_ref().and_then(|team| team.read().ok()) {
            factory.new_object(template, &*team).map_err(|_| {
                BehaviorError::MineTemplateNotFound {
                    template: mine_template.to_string(),
                }
            })?
        } else {
            factory
                .new_object_optional_team(template, None)
                .map_err(|_| BehaviorError::MineTemplateNotFound {
                    template: mine_template.to_string(),
                })?
        };

        if let Ok(mut mine_guard) = mine.write() {
            let _ = mine_guard.set_position(position);
            let _ = mine_guard.set_orientation(orientation);
            if let Ok(owner_guard) = owner.read() {
                mine_guard.set_producer(Some(&owner_guard));
            }
        }

        let mine_id = mine
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(INVALID_OBJECT_ID);

        // C++ lines 204-212: Set scoot parameters for land mine interface
        // Handled by the mine's own behavior modules
        if mine_id != INVALID_OBJECT_ID {
            let behaviors = mine
                .read()
                .map(|guard| guard.get_behavior_modules())
                .unwrap_or_default();
            for behavior in behaviors {
                if let Ok(mut guard) = behavior.lock() {
                    if let Some(lmi) = guard.get_land_mine_interface() {
                        lmi.set_scoot_parms(&owner_pos, position);
                        break;
                    }
                }
            }
        }

        Ok(mine_id)
    }

    /// Check if a position is valid for mine placement
    fn is_position_valid_for_mine(&self, position: &Coord3D) -> BehaviorResult<bool> {
        // Delegate to place_mine_at validation logic
        match self.place_mine_at(position, &self.config.mine_name) {
            Ok(_) => Ok(true),
            Err(BehaviorError::InvalidPosition) | Err(BehaviorError::NoSpaceAvailable) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get object geometry information
    /// C++ Reference: Uses getObject()->getGeometryInfo()
    fn get_object_geometry(&self) -> BehaviorResult<GeometryInfo> {
        if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(obj_guard) = obj.read() {
                let pos = obj_guard.get_position();
                let geom = obj_guard.get_geometry_info();
                let bounds = geom.bounds.clone();
                let major_radius = ((bounds.max.x - bounds.min.x).abs()) * 0.5;
                let minor_radius = ((bounds.max.y - bounds.min.y).abs()) * 0.5;
                return Ok(GeometryInfo {
                    center: Coord3D::new(pos.x, pos.y, pos.z),
                    major_radius,
                    minor_radius,
                    rotation: obj_guard.get_orientation(),
                    is_circular: false,
                });
            }
        }
        // Fallback defaults
        Ok(GeometryInfo {
            center: Coord3D::new(0.0, 0.0, 0.0),
            major_radius: 20.0,
            minor_radius: 20.0,
            rotation: 0.0,
            is_circular: false,
        })
    }

    /// Play FX at specified position
    /// C++ Reference: Uses FXList::doFXObj() at line 431
    fn play_fx(&self, fx_template: &str, position: &Coord3D) -> BehaviorResult<()> {
        // C++ line 431: FXList::doFXObj(d->m_genFX, obj);
        if let Some(fx_system) = crate::helpers::TheFXList::get() {
            let pos = crate::common::Coord3D::new(position.x, position.y, position.z);
            fx_system.do_fx_at_position(fx_template, &pos);
        }
        Ok(())
    }

    /// Generate random value between min and max
    /// C++ Reference: Uses GameLogicRandomValueReal(-PI, PI) at line 183
    fn random_value(&self, min: f32, max: f32) -> f32 {
        // Use game logic random for replay compatibility
        crate::helpers::get_game_logic_random_value_real(min, max)
    }

    /// Upgrade the minefield
    pub fn upgrade_minefield(&self) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();

        if !self.config.upgradable {
            return Ok(());
        }

        if state.upgraded {
            return Ok(());
        }

        // If mines already exist, replace them with upgraded mines.
        if state.generated && self.config.mine_name_upgraded.is_some() {
            let mine_ids = state.mine_list.clone();
            state.mine_list.clear();
            state.generated = false;
            state.upgraded = true;
            drop(state);

            for mine_id in mine_ids {
                let _ = self.remove_mine(mine_id);
            }

            self.place_mines()?;
            return Ok(());
        }

        if !state.generated {
            state.upgraded = true;
            drop(state);
            self.place_mines()?;
            return Ok(());
        }

        state.upgraded = true;
        Ok(())
    }

    /// Clear all mines
    pub fn clear_mines(&self) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();

        for &mine_id in &state.mine_list {
            self.remove_mine(mine_id)?;
        }

        state.mine_list.clear();
        state.generated = false;

        Ok(())
    }

    /// Remove a single mine from the game
    /// C++ Reference: GenerateMinefieldBehavior.cpp lines 459-463
    fn remove_mine(&self, mine_id: ObjectId) -> BehaviorResult<()> {
        // C++ lines 459-463:
        // Object *obj = TheGameLogic->findObjectByID(objID);
        // if (obj) { TheGameLogic->destroyObject(obj); }
        if crate::object::registry::OBJECT_REGISTRY
            .get_object(mine_id)
            .is_some()
        {
            if let Some(mut mgr) = crate::object_manager::get_object_manager().write().ok() {
                mgr.destroy_object(mine_id);
            }
        }
        Ok(())
    }

    fn on_upgrade_removed(&self) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();
        state.upgraded = false;
        Ok(())
    }
}

impl UpdateModuleInterface for GenerateMinefieldBehavior {
    fn update_simple(&mut self) -> UpdateSleepTime {
        match self.update(crate::helpers::TheGameLogic::get_frame()) {
            Ok(sleep) => sleep,
            Err(err) => {
                log::warn!("GenerateMinefieldBehavior update error: {:?}", err);
                UPDATE_SLEEP_FOREVER
            }
        }
    }
}

impl DieModuleInterface for GenerateMinefieldBehavior {
    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.config.on_death {
            self.place_mines()?;
        }
        Ok(())
    }
}

impl Snapshotable for GenerateMinefieldBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer version failed: {:?}", e))?;

        let mut state = self.state.write().unwrap();
        xfer.xfer_bool(&mut state.generated)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer generated failed: {:?}", e))?;

        let mut has_target = state.target.is_some();
        xfer.xfer_bool(&mut has_target)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer target flag failed: {:?}", e))?;
        if has_target {
            let mut target = state.target.unwrap_or_default();
            xfer.xfer_coord3d(&mut target);
            state.target = Some(target);
        } else {
            state.target = None;
        }

        xfer.xfer_bool(&mut state.upgraded)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer upgraded failed: {:?}", e))?;

        let mut mine_count = state.mine_list.len() as u32;
        xfer.xfer_unsigned_int(&mut mine_count)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer mine count failed: {:?}", e))?;

        if xfer.is_loading() {
            state.mine_list.clear();
            for _ in 0..mine_count {
                let mut id = 0;
                xfer.xfer_object_id(&mut id).map_err(|e| {
                    format!("GenerateMinefieldBehavior xfer mine id failed: {:?}", e)
                })?;
                state.mine_list.push(id);
            }
        } else {
            for id in &mut state.mine_list {
                let mut id_copy = *id;
                xfer.xfer_object_id(&mut id_copy).map_err(|e| {
                    format!("GenerateMinefieldBehavior xfer mine id failed: {:?}", e)
                })?;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for GenerateMinefieldBehavior {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        true
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.config.upgradable {
            self.upgrade_minefield().is_ok()
        } else {
            self.place_mines().is_ok()
        }
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        let _ = self.on_upgrade_removed();
    }
}

/// Glue that exposes GenerateMinefieldBehavior through the common Module trait.
pub struct GenerateMinefieldBehaviorModule {
    behavior: GenerateMinefieldBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<GenerateMinefieldBehaviorModuleData>,
}

impl GenerateMinefieldBehaviorModule {
    pub fn new(
        behavior: GenerateMinefieldBehavior,
        module_name: &AsciiString,
        module_data: Arc<GenerateMinefieldBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut GenerateMinefieldBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for GenerateMinefieldBehaviorModule {
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

impl Module for GenerateMinefieldBehaviorModule {
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

/// Get statistics about the minefield
impl GenerateMinefieldBehavior {
    pub fn get_statistics(&self) -> MinefieldStatistics {
        let state = self.state.read().unwrap();

        MinefieldStatistics {
            is_generated: state.generated,
            is_upgraded: state.upgraded,
            mine_count: state.mine_list.len(),
            current_mine_template: state.current_mine_template.clone(),
            has_target: state.target.is_some(),
            target_position: state.target,
        }
    }

    /// Get list of mine IDs
    pub fn get_mine_list(&self) -> Vec<ObjectId> {
        let state = self.state.read().unwrap();
        state.mine_list.clone()
    }
}

/// Statistics for the minefield behavior
#[derive(Debug, Clone)]
pub struct MinefieldStatistics {
    pub is_generated: bool,
    pub is_upgraded: bool,
    pub mine_count: usize,
    pub current_mine_template: MineTemplateId,
    pub has_target: bool,
    pub target_position: Option<Coord3D>,
}

/// Builder for creating GenerateMinefieldBehavior with fluent interface
#[derive(Debug, Default)]
pub struct GenerateMinefieldBehaviorBuilder {
    config: GenerateMinefieldBehaviorModuleData,
}

impl GenerateMinefieldBehaviorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mine_name<S: Into<String>>(mut self, name: S) -> Self {
        self.config.mine_name = name.into();
        self
    }

    pub fn upgraded_mine_name<S: Into<String>>(mut self, name: S) -> Self {
        self.config.mine_name_upgraded = Some(name.into());
        self
    }

    pub fn distance_around_object(mut self, distance: f32) -> Self {
        self.config.distance_around_object = distance;
        self
    }

    pub fn mines_per_square_foot(mut self, density: f32) -> Self {
        self.config.mines_per_square_foot = density;
        self
    }

    pub fn on_death(mut self, on_death: bool) -> Self {
        self.config.on_death = on_death;
        self
    }

    pub fn border_only(mut self, border_only: bool) -> Self {
        self.config.border_only = border_only;
        self
    }

    pub fn always_circular(mut self, circular: bool) -> Self {
        self.config.always_circular = circular;
        self
    }

    pub fn upgradable(mut self, upgradable: bool) -> Self {
        self.config.upgradable = upgradable;
        self
    }

    pub fn random_jitter(mut self, jitter: f32) -> Self {
        self.config.random_jitter = jitter;
        self
    }

    pub fn smart_border(mut self, smart: bool) -> Self {
        self.config.smart_border = smart;
        self
    }

    pub fn generation_fx<S: Into<String>>(mut self, fx: S) -> Self {
        self.config.generation_fx = Some(fx.into());
        self
    }

    pub fn build(self, object_id: ObjectId) -> GenerateMinefieldBehavior {
        GenerateMinefieldBehavior::new_from_config(object_id, self.config)
    }
}

impl BehaviorModuleInterface for GenerateMinefieldBehavior {
    fn get_module_name(&self) -> &'static str {
        "GenerateMinefieldBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

/// Factory for creating GenerateMinefieldBehavior instances
pub struct GenerateMinefieldBehaviorFactory;

impl GenerateMinefieldBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<crate::object::Object>>,
        module_data: Arc<dyn crate::common::ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        Ok(Box::new(GenerateMinefieldBehavior::new(
            thing,
            module_data,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_behavior() -> GenerateMinefieldBehavior {
        GenerateMinefieldBehaviorBuilder::new()
            .mine_name("test_mine")
            .distance_around_object(30.0)
            .mines_per_square_foot(0.02)
            .border_only(true)
            .build(1)
    }

    #[test]
    fn test_behavior_creation() {
        let behavior = create_test_behavior();
        let stats = behavior.get_statistics();

        assert!(!stats.is_generated);
        assert!(!stats.is_upgraded);
        assert_eq!(stats.mine_count, 0);
        assert_eq!(stats.current_mine_template, "test_mine");
    }

    #[test]
    fn test_minefield_target() {
        let behavior = create_test_behavior();
        let target = Coord3D::new(100.0, 200.0, 0.0);

        behavior.set_minefield_target(Some(target));
        let retrieved_target = behavior.get_minefield_target();

        assert!(retrieved_target.is_some());
        let pos = retrieved_target.unwrap();
        assert!((pos.x - 100.0).abs() < 0.001);
        assert!((pos.y - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_mine_placement() {
        let behavior = create_test_behavior();

        let result = behavior.place_mines();
        assert!(result.is_ok());

        let stats = behavior.get_statistics();
        assert!(stats.is_generated);
        // Note: mine_count would be > 0 in actual implementation
    }

    #[test]
    fn test_upgrade_behavior() {
        let mut behavior = GenerateMinefieldBehaviorBuilder::new()
            .mine_name("basic_mine")
            .upgraded_mine_name("advanced_mine")
            .upgradable(true)
            .build(1);

        let result = behavior.apply_upgrade(UpgradeMaskType::from_bits_retain(1u128));
        assert!(result);

        let stats = behavior.get_statistics();
        assert!(stats.is_upgraded);
    }

    #[test]
    fn test_death_behavior() {
        let mut behavior = GenerateMinefieldBehaviorBuilder::new()
            .mine_name("death_mine")
            .on_death(true)
            .build(1);

        let damage_info =
            DamageInfo::with_simple(1000.0, 42, DamageType::Explosion, DeathType::Normal);

        let result = crate::modules::DieModuleInterface::on_die(&mut behavior, &damage_info);
        assert!(result.is_ok());

        let stats = behavior.get_statistics();
        assert!(stats.is_generated);
    }

    #[test]
    fn test_coordinate_calculations() {
        let pos1 = Coord3D::new(0.0, 0.0, 0.0);
        let pos2 = Coord3D::new(3.0, 4.0, 0.0);

        let distance = pos1.distance_to(&pos2);
        assert!((distance - 5.0).abs() < 0.001);

        let distance_2d = pos1.distance_2d(&pos2);
        assert!((distance_2d - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_builder_pattern() {
        let behavior = GenerateMinefieldBehaviorBuilder::new()
            .mine_name("builder_test_mine")
            .upgraded_mine_name("builder_advanced_mine")
            .distance_around_object(50.0)
            .mines_per_square_foot(0.01)
            .on_death(true)
            .border_only(false)
            .always_circular(true)
            .upgradable(true)
            .random_jitter(0.1)
            .smart_border(true)
            .generation_fx("mine_generation_fx")
            .build(999);

        let stats = behavior.get_statistics();
        assert_eq!(stats.current_mine_template, "builder_test_mine");
        assert!(!stats.is_generated); // Not generated until triggered
    }

    #[test]
    fn test_mine_clearing() {
        let behavior = create_test_behavior();

        // Generate mines first
        let _ = behavior.place_mines();
        assert!(behavior.get_statistics().is_generated);

        // Clear mines
        let result = behavior.clear_mines();
        assert!(result.is_ok());

        let stats = behavior.get_statistics();
        assert!(!stats.is_generated);
        assert_eq!(stats.mine_count, 0);
    }
}
