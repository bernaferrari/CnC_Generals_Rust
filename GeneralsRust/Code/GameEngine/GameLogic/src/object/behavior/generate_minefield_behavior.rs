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
use crate::object::behavior::behavior_module::{
    xfer_behavior_module_base_versions, BehaviorModuleData,
};
use crate::object::Object as GameObject;
use crate::upgrade::center::THE_UPGRADE_CENTER;
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, PayloadTargetControlInterface,
};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

trait Coord3DExt {
    #[allow(dead_code)]
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

impl GeometryInfo {
    fn expand(&mut self, amount: f32) {
        self.major_radius += amount;
        self.minor_radius += amount;
    }

    fn bounding_circle_radius(&self) -> f32 {
        if self.is_circular {
            self.major_radius
        } else {
            (self.major_radius * self.major_radius + self.minor_radius * self.minor_radius).sqrt()
        }
    }

    fn footprint_area(&self) -> f32 {
        if self.is_circular {
            std::f32::consts::PI * self.major_radius * self.major_radius
        } else {
            self.major_radius * 2.0 * self.minor_radius * 2.0
        }
    }

    fn contains_point_2d(&self, center: &Coord3D, point: &Coord3D) -> bool {
        let dx = point.x - center.x;
        let dy = point.y - center.y;
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        let local_x = dx * cos + dy * sin;
        let local_y = -dx * sin + dy * cos;

        if self.is_circular {
            local_x * local_x + local_y * local_y <= self.major_radius * self.major_radius
        } else {
            local_x.abs() <= self.major_radius && local_y.abs() <= self.minor_radius
        }
    }

    fn offset_to_world(&self, center: &Coord3D, local_x: f32, local_y: f32) -> Coord3D {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        Coord3D::new(
            center.x + local_x * cos - local_y * sin,
            center.y + local_x * sin + local_y * cos,
            center.z,
        )
    }
}

fn line_segment_count(length: f32, mine_radius: f32) -> u32 {
    let mine_diameter = mine_radius * 2.0;
    if length <= 0.0 || mine_diameter <= 0.0 {
        return 1;
    }
    ((length / mine_diameter).ceil() as u32).max(1)
}

fn circle_mine_count(radius: f32, mine_radius: f32) -> u32 {
    line_segment_count(2.0 * std::f32::consts::PI * radius, mine_radius)
}

fn footprint_mine_count(area: f32, mines_per_square_foot: f32) -> u32 {
    (area * mines_per_square_foot).ceil().max(1.0) as u32
}

fn is_any_position_too_close_2d(
    positions: &[Coord3D],
    position: &Coord3D,
    min_dist_sqr: f32,
) -> bool {
    positions.iter().any(|existing| {
        let dx = existing.x - position.x;
        let dy = existing.y - position.y;
        dx * dx + dy * dy < min_dist_sqr
    })
}

fn rotated_rect_corners(
    center: &Coord3D,
    major_radius: f32,
    minor_radius: f32,
    rotation: f32,
) -> [Coord3D; 4] {
    let cos = rotation.cos();
    let sin = rotation.sin();
    let offsets = [
        (major_radius, minor_radius),
        (-major_radius, minor_radius),
        (-major_radius, -minor_radius),
        (major_radius, -minor_radius),
    ];

    offsets.map(|(x, y)| {
        Coord3D::new(
            center.x + x * cos - y * sin,
            center.y + x * sin + y * cos,
            center.z,
        )
    })
}

#[cfg(test)]
fn smart_border_ring_count(
    mut bounding_circle_radius: f32,
    distance: f32,
    mine_radius: f32,
) -> u32 {
    let mine_diameter = mine_radius * 2.0;
    bounding_circle_radius += mine_radius;
    let mut rings = 0;
    loop {
        rings += 1;
        if bounding_circle_radius >= distance {
            return rings;
        }
        bounding_circle_radius += mine_diameter;
    }
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
        let (distance_around_object, mines_per_square_foot) = get_global_data()
            .map(|global| {
                let data = global.read();
                (
                    data.standard_minefield_distance,
                    data.standard_minefield_density,
                )
            })
            .unwrap_or((40.0, 0.01));

        Self {
            base: BehaviorModuleData::default(),
            mine_name: String::new(),
            mine_name_upgraded: None,
            mine_upgrade_trigger: None,
            generation_fx: None,
            distance_around_object,
            mines_per_square_foot,
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

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
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
    let token = required_value(tokens)?;
    data.mine_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_upgraded_mine_name(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    let value = INI::parse_ascii_string(token)?;
    data.mine_name_upgraded = if value.is_empty() { None } else { Some(value) };
    Ok(())
}

fn parse_upgrade_trigger(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    let value = INI::parse_ascii_string(token)?;
    data.mine_upgrade_trigger = if value.is_empty() { None } else { Some(value) };
    Ok(())
}

fn parse_generation_fx(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    let value = INI::parse_ascii_string(token)?;
    data.generation_fx = if value.is_empty() { None } else { Some(value) };
    Ok(())
}

fn parse_distance_around_object(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.distance_around_object = INI::parse_real(token)?;
    Ok(())
}

fn parse_mines_per_square_foot(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.mines_per_square_foot = INI::parse_real(token)?;
    Ok(())
}

fn parse_generate_only_on_death(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.on_death = INI::parse_bool(token)?;
    Ok(())
}

fn parse_border_only(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.border_only = INI::parse_bool(token)?;
    Ok(())
}

fn parse_smart_border(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.smart_border = INI::parse_bool(token)?;
    Ok(())
}

fn parse_smart_border_skip_interior(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.smart_border_skip_interior = INI::parse_bool(token)?;
    Ok(())
}

fn parse_always_circular(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.always_circular = INI::parse_bool(token)?;
    Ok(())
}

fn parse_upgradable(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.upgradable = INI::parse_bool(token)?;
    Ok(())
}

fn parse_random_jitter(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.random_jitter = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_skip_if_this_much_under_structure(
    _ini: &mut INI,
    data: &mut GenerateMinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
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
    /// C++ UpgradeMux::m_upgradeExecuted for the behavior's upgrade facet.
    upgrade_executed: bool,
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
            upgrade_executed: false,
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
        } else if self.config.border_only {
            if self.config.always_circular || geometry.is_circular {
                MinePlacementPattern::Circular
            } else {
                MinePlacementPattern::Rectangular
            }
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
        let mine_radius = self.mine_template_radius(mine_template)?;

        match pattern {
            MinePlacementPattern::Circular => {
                let radius = geometry.major_radius + self.config.distance_around_object;
                self.place_mines_around_circle(
                    state,
                    &geometry.center,
                    radius,
                    mine_template,
                    mine_radius,
                )?;
            }
            MinePlacementPattern::Rectangular => {
                self.place_mines_around_rect(
                    state,
                    &geometry.center,
                    geometry.major_radius + self.config.distance_around_object,
                    geometry.minor_radius + self.config.distance_around_object,
                    geometry.rotation,
                    mine_template,
                    mine_radius,
                )?;
            }
            MinePlacementPattern::FootprintBased => {
                self.place_mines_in_footprint(state, geometry, mine_template, mine_radius)?;
            }
            MinePlacementPattern::SmartBorder => {
                self.place_mines_smart_border(state, geometry, mine_template)?;
            }
        }
        Ok(())
    }

    fn mine_template_radius(&self, mine_template: &str) -> BehaviorResult<f32> {
        Ok(
            crate::helpers::TheThingFactory::find_template(mine_template)
                .map(|template| {
                    template
                        .get_template_geometry_info()
                        .get_bounding_circle_radius()
                })
                .unwrap_or(1.0),
        )
    }

    /// Place mines in a circular pattern
    fn place_mines_around_circle(
        &self,
        state: &mut MinefieldState,
        center: &Coord3D,
        radius: f32,
        mine_template: &str,
        mine_radius: f32,
    ) -> BehaviorResult<()> {
        let num_mines = circle_mine_count(radius, mine_radius);
        let angle_inc = (2.0 * std::f32::consts::PI) / num_mines as f32;
        let angle_limit = (2.0 * std::f32::consts::PI) - angle_inc * 0.5;
        let mine_jitter = mine_radius * self.config.random_jitter;

        let mut angle = 0.0;
        while angle < angle_limit {
            let mut position = Coord3D::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
                center.z,
            );

            if mine_jitter > 0.0 {
                position.x += self.random_value(-mine_jitter, mine_jitter);
                position.y += self.random_value(-mine_jitter, mine_jitter);
            }

            if let Ok(mine_id) = self.place_mine_at(&position, mine_template) {
                if self.config.upgradable {
                    state.mine_list.push(mine_id);
                }
            }

            angle += angle_inc;
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
        rotation: f32,
        mine_template: &str,
        mine_radius: f32,
    ) -> BehaviorResult<()> {
        let corners = rotated_rect_corners(center, major_radius, minor_radius, rotation);

        self.place_mines_along_line(
            state,
            &corners[0],
            &corners[1],
            mine_template,
            mine_radius,
            true,
        )?;

        self.place_mines_along_line(
            state,
            &corners[1],
            &corners[2],
            mine_template,
            mine_radius,
            true,
        )?;

        self.place_mines_along_line(
            state,
            &corners[2],
            &corners[3],
            mine_template,
            mine_radius,
            true,
        )?;

        self.place_mines_along_line(
            state,
            &corners[3],
            &corners[0],
            mine_template,
            mine_radius,
            true,
        )?;

        Ok(())
    }

    /// Place mines in footprint-based pattern
    fn place_mines_in_footprint(
        &self,
        state: &mut MinefieldState,
        geometry: &GeometryInfo,
        mine_template: &str,
        mine_radius: f32,
    ) -> BehaviorResult<()> {
        let mut expanded_geometry = geometry.clone();
        expanded_geometry.expand(self.config.distance_around_object);
        if self.config.always_circular {
            let radius = expanded_geometry.bounding_circle_radius();
            expanded_geometry.major_radius = radius;
            expanded_geometry.minor_radius = radius;
            expanded_geometry.is_circular = true;
        }

        let num_mines = footprint_mine_count(
            expanded_geometry.footprint_area(),
            self.config.mines_per_square_foot,
        );
        let min_dist_sqr = (mine_radius * 2.0) * (mine_radius * 2.0);
        let mut created_positions = Vec::new();

        for _ in 0..num_mines {
            let mut max_retry = 100;
            let position = loop {
                let candidate = self.random_point_in_footprint(&expanded_geometry);
                max_retry -= 1;
                if !is_any_position_too_close_2d(&created_positions, &candidate, min_dist_sqr)
                    || max_retry == 0
                {
                    break candidate;
                }
            };

            if geometry.contains_point_2d(&geometry.center, &position) {
                continue;
            }

            if let Ok(mine_id) = self.place_mine_at(&position, mine_template) {
                created_positions.push(position);
                if self.config.upgradable {
                    state.mine_list.push(mine_id);
                }
            }
        }

        Ok(())
    }

    fn random_point_in_footprint(&self, geometry: &GeometryInfo) -> Coord3D {
        if geometry.is_circular {
            let angle = self.random_value(0.0, 2.0 * std::f32::consts::PI);
            let radius = self.random_value(0.0, 1.0).sqrt() * geometry.major_radius;
            geometry.offset_to_world(&geometry.center, radius * angle.cos(), radius * angle.sin())
        } else {
            let x = self.random_value(-geometry.major_radius, geometry.major_radius);
            let y = self.random_value(-geometry.minor_radius, geometry.minor_radius);
            geometry.offset_to_world(&geometry.center, x, y)
        }
    }

    /// Place mines using smart border algorithm
    fn place_mines_smart_border(
        &self,
        state: &mut MinefieldState,
        geometry: &GeometryInfo,
        mine_template: &str,
    ) -> BehaviorResult<()> {
        let mine_radius = self.mine_template_radius(mine_template)?;
        let mine_diameter = mine_radius * 2.0;
        let mut ring_geometry = if self.config.smart_border_skip_interior {
            GeometryInfo {
                center: geometry.center,
                major_radius: geometry.major_radius,
                minor_radius: geometry.minor_radius,
                rotation: geometry.rotation,
                is_circular: geometry.is_circular,
            }
        } else {
            if let Ok(mine_id) = self.place_mine_at(&geometry.center, mine_template) {
                if self.config.upgradable {
                    state.mine_list.push(mine_id);
                }
            }
            GeometryInfo {
                center: geometry.center,
                major_radius: mine_radius,
                minor_radius: mine_radius,
                rotation: geometry.rotation,
                is_circular: true,
            }
        };

        if self.config.always_circular {
            let radius = ring_geometry.bounding_circle_radius();
            ring_geometry.major_radius = radius;
            ring_geometry.minor_radius = radius;
            ring_geometry.is_circular = true;
        }

        ring_geometry.expand(mine_radius);

        loop {
            if !ring_geometry.is_circular && !self.config.always_circular {
                self.place_mines_around_rect(
                    state,
                    &ring_geometry.center,
                    ring_geometry.major_radius,
                    ring_geometry.minor_radius,
                    ring_geometry.rotation,
                    mine_template,
                    mine_radius,
                )?;
            } else {
                self.place_mines_around_circle(
                    state,
                    &ring_geometry.center,
                    ring_geometry.major_radius,
                    mine_template,
                    mine_radius,
                )?;
            }

            if ring_geometry.bounding_circle_radius() >= self.config.distance_around_object {
                break;
            }
            ring_geometry.expand(mine_diameter);
        }

        Ok(())
    }

    /// Place mines along a line between two points
    fn place_mines_along_line(
        &self,
        state: &mut MinefieldState,
        start: &Coord3D,
        end: &Coord3D,
        mine_template: &str,
        mine_radius: f32,
        skip_one_at_start: bool,
    ) -> BehaviorResult<()> {
        let distance = start.distance_2d(end);
        if distance <= f32::EPSILON {
            return Ok(());
        }
        let num_mines = line_segment_count(distance, mine_radius);
        let spacing = distance / num_mines as f32;
        let mine_jitter = mine_radius * self.config.random_jitter;

        let mut place = if skip_one_at_start { spacing } else { 0.0 };
        while place <= distance {
            let t = place / distance;
            let mut position = Coord3D::new(
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t,
                start.z + (end.z - start.z) * t,
            );

            if mine_jitter > 0.0 {
                position.x += self.random_value(-mine_jitter, mine_jitter);
                position.y += self.random_value(-mine_jitter, mine_jitter);
            }

            if let Ok(mine_id) = self.place_mine_at(&position, mine_template) {
                if self.config.upgradable {
                    state.mine_list.push(mine_id);
                }
            }

            place += spacing;
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
        state.upgrade_executed = false;
        Ok(())
    }
}

impl PayloadTargetControlInterface for GenerateMinefieldBehavior {
    fn set_payload_target_position(&mut self, target: [f32; 3]) {
        self.set_minefield_target(Some(Coord3D {
            x: target[0],
            y: target[1],
            z: target[2],
        }));
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer version failed: {:?}", e))?;

        xfer_behavior_module_base_versions(xfer)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer behavior base failed: {}", e))?;

        let mut state = self.state.write().unwrap();

        let mut upgrade_mux_version: u8 = 1;
        xfer.xfer_version(&mut upgrade_mux_version, 1)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer upgrade mux failed: {:?}", e))?;
        xfer.xfer_bool(&mut state.upgrade_executed).map_err(|e| {
            format!(
                "GenerateMinefieldBehavior xfer upgrade executed failed: {:?}",
                e
            )
        })?;

        xfer.xfer_bool(&mut state.generated)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer generated failed: {:?}", e))?;

        let mut has_target = state.target.is_some();
        xfer.xfer_bool(&mut has_target)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer target flag failed: {:?}", e))?;

        xfer.xfer_bool(&mut state.upgraded)
            .map_err(|e| format!("GenerateMinefieldBehavior xfer upgraded failed: {:?}", e))?;

        let mut target = state.target.unwrap_or_default();
        xfer.xfer_coord3d(&mut target);
        state.target = has_target.then_some(target);

        let mut mine_count = state.mine_list.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut mine_count)
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
        let applied = if self.config.upgradable {
            self.upgrade_minefield().is_ok()
        } else {
            self.place_mines().is_ok()
        };

        if applied {
            if let Ok(mut state) = self.state.write() {
                state.upgrade_executed = true;
            }
        }

        applied
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

    fn get_payload_target_control_interface(
        &mut self,
    ) -> Option<&mut dyn PayloadTargetControlInterface> {
        Some(&mut self.behavior)
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
    use game_engine::common::ini::ini_game_data::{ensure_global_data, GlobalData};
    use parking_lot::RwLock;
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

    struct GlobalMinefieldRestore {
        global: Arc<RwLock<GlobalData>>,
        distance: Real,
        density: Real,
        _guard: MutexGuard<'static, ()>,
    }

    impl Drop for GlobalMinefieldRestore {
        fn drop(&mut self) {
            let mut data = self.global.write();
            data.standard_minefield_distance = self.distance;
            data.standard_minefield_density = self.density;
        }
    }

    fn set_global_minefield_defaults(distance: Real, density: Real) -> GlobalMinefieldRestore {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let global = ensure_global_data();
        let (original_distance, original_density) = {
            let data = global.read();
            (
                data.standard_minefield_distance,
                data.standard_minefield_density,
            )
        };
        {
            let mut data = global.write();
            data.standard_minefield_distance = distance;
            data.standard_minefield_density = density;
        }

        GlobalMinefieldRestore {
            global,
            distance: original_distance,
            density: original_density,
            _guard: guard,
        }
    }

    fn parse_field(data: &mut GenerateMinefieldBehaviorModuleData, token: &str, values: &[&str]) {
        let field = GENERATE_MINEFIELD_BEHAVIOR_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

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
    fn default_minefield_distance_matches_cpp_global_data() {
        let _restore = set_global_minefield_defaults(40.0, 0.01);

        let data = GenerateMinefieldBehaviorModuleData::default();

        assert_eq!(data.distance_around_object, 40.0);
        assert_eq!(data.mines_per_square_foot, 0.01);
        assert!(data.border_only);
    }

    #[test]
    fn minefield_defaults_use_runtime_global_data_and_ini_overrides_win() {
        let _restore = set_global_minefield_defaults(48.0, 0.004);

        let mut data = GenerateMinefieldBehaviorModuleData::default();
        assert_eq!(data.distance_around_object, 48.0);
        assert_eq!(data.mines_per_square_foot, 0.004);

        parse_field(&mut data, "DistanceAroundObject", &["=", "64"]);
        parse_field(&mut data, "MinesPerSquareFoot", &["=", "0.02"]);
        assert_eq!(data.distance_around_object, 64.0);
        assert_eq!(data.mines_per_square_foot, 0.02);
    }

    #[test]
    fn generate_minefield_fields_accept_ini_equals_token() {
        let mut data = GenerateMinefieldBehaviorModuleData::default();

        parse_field(&mut data, "MineName", &["=", "DemoMine"]);
        parse_field(&mut data, "UpgradedMineName", &["=", "DemoMineUpgraded"]);
        parse_field(&mut data, "UpgradedTriggeredBy", &["=", "Upgrade_Test"]);
        parse_field(&mut data, "GenerationFX", &["=", "FX_Test"]);
        parse_field(&mut data, "DistanceAroundObject", &["=", "60.5"]);
        parse_field(&mut data, "MinesPerSquareFoot", &["=", "0.02"]);
        parse_field(&mut data, "GenerateOnlyOnDeath", &["=", "Yes"]);
        parse_field(&mut data, "BorderOnly", &["=", "No"]);
        parse_field(&mut data, "SmartBorder", &["=", "Yes"]);
        parse_field(&mut data, "SmartBorderSkipInterior", &["=", "No"]);
        parse_field(&mut data, "AlwaysCircular", &["=", "Yes"]);
        parse_field(&mut data, "Upgradable", &["=", "Yes"]);
        parse_field(&mut data, "RandomJitter", &["=", "25%"]);
        parse_field(&mut data, "SkipIfThisMuchUnderStructure", &["=", "50%"]);

        assert_eq!(data.mine_name, "DemoMine");
        assert_eq!(data.mine_name_upgraded.as_deref(), Some("DemoMineUpgraded"));
        assert_eq!(data.mine_upgrade_trigger.as_deref(), Some("Upgrade_Test"));
        assert_eq!(data.generation_fx.as_deref(), Some("FX_Test"));
        assert_eq!(data.distance_around_object, 60.5);
        assert_eq!(data.mines_per_square_foot, 0.02);
        assert!(data.on_death);
        assert!(!data.border_only);
        assert!(data.smart_border);
        assert!(!data.smart_border_skip_interior);
        assert!(data.always_circular);
        assert!(data.upgradable);
        assert_eq!(data.random_jitter, 0.25);
        assert_eq!(data.skip_if_this_much_under_structure, 0.5);
    }

    #[test]
    fn border_spacing_uses_cpp_mine_diameter_counts() {
        assert_eq!(line_segment_count(95.0, 5.0), 10);
        assert_eq!(line_segment_count(4.0, 5.0), 1);
        assert_eq!(circle_mine_count(40.0, 5.0), 26);
    }

    #[test]
    fn smart_border_expands_multiple_cpp_mine_rings_until_distance() {
        let box_bounds = GeometryInfo {
            center: Coord3D::new(0.0, 0.0, 0.0),
            major_radius: 10.0,
            minor_radius: 10.0,
            rotation: 0.0,
            is_circular: false,
        };

        assert!((box_bounds.bounding_circle_radius() - 14.142136).abs() < 0.0001);
        assert_eq!(
            smart_border_ring_count(box_bounds.bounding_circle_radius(), 40.0, 5.0),
            4
        );
        assert_eq!(smart_border_ring_count(45.0, 40.0, 5.0), 1);
    }

    #[test]
    fn rectangular_border_corners_follow_object_rotation() {
        let center = Coord3D::new(100.0, 200.0, 7.0);
        let corners = rotated_rect_corners(&center, 20.0, 10.0, std::f32::consts::FRAC_PI_2);

        assert!((corners[0].x - 90.0).abs() < 0.0001);
        assert!((corners[0].y - 220.0).abs() < 0.0001);
        assert!((corners[1].x - 90.0).abs() < 0.0001);
        assert!((corners[1].y - 180.0).abs() < 0.0001);
        assert!((corners[2].x - 110.0).abs() < 0.0001);
        assert!((corners[2].y - 180.0).abs() < 0.0001);
        assert!((corners[3].x - 110.0).abs() < 0.0001);
        assert!((corners[3].y - 220.0).abs() < 0.0001);
        assert!(corners.iter().all(|corner| corner.z == center.z));
    }

    #[test]
    fn footprint_density_uses_expanded_area_with_minimum_one_mine() {
        let mut geom = GeometryInfo {
            center: Coord3D::new(0.0, 0.0, 0.0),
            major_radius: 10.0,
            minor_radius: 5.0,
            rotation: 0.0,
            is_circular: false,
        };
        geom.expand(10.0);

        assert_eq!(geom.footprint_area(), 1200.0);
        assert_eq!(footprint_mine_count(geom.footprint_area(), 0.01), 12);
        assert_eq!(footprint_mine_count(0.0, 0.01), 1);
    }

    #[test]
    fn footprint_contains_point_respects_rotation_and_circle() {
        let center = Coord3D::new(10.0, 20.0, 0.0);
        let rect = GeometryInfo {
            center,
            major_radius: 20.0,
            minor_radius: 10.0,
            rotation: std::f32::consts::FRAC_PI_2,
            is_circular: false,
        };
        let inside = Coord3D::new(0.0, 20.0, 0.0);
        let outside = Coord3D::new(-5.0, 20.0, 0.0);

        assert!(rect.contains_point_2d(&center, &inside));
        assert!(!rect.contains_point_2d(&center, &outside));

        let circle = GeometryInfo {
            center,
            major_radius: 5.0,
            minor_radius: 5.0,
            rotation: 0.0,
            is_circular: true,
        };
        assert!(circle.contains_point_2d(&center, &Coord3D::new(13.0, 24.0, 0.0)));
        assert!(!circle.contains_point_2d(&center, &Coord3D::new(14.0, 24.0, 0.0)));
    }

    #[test]
    fn footprint_spacing_check_matches_cpp_strict_less_than() {
        let positions = vec![Coord3D::new(0.0, 0.0, 0.0)];

        assert!(is_any_position_too_close_2d(
            &positions,
            &Coord3D::new(9.0, 0.0, 0.0),
            100.0
        ));
        assert!(!is_any_position_too_close_2d(
            &positions,
            &Coord3D::new(10.0, 0.0, 0.0),
            100.0
        ));
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
