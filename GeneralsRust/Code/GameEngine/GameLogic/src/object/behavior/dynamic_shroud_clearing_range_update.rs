//! DynamicShroudClearingRangeUpdate - Rust conversion of C++ DynamicShroudClearingRangeUpdate
//!
//! Changes the object's shroud clearing range over time with grow/sustain/shrink phases.
//! Used for spy satellite and similar revealing effects.
//! Author: Graham Smallwood, August 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Coord3D, CoordOrigin, ModuleData, RadiusDecal, RadiusDecalTemplate, Real,
    SHADOW_NAMES,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use crate::player::ThePlayerList;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

const GRID_FX_DECAL_COUNT: usize = 30;

/// State machine states for shroud clearing
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DSCRUState {
    NotStartedYet,
    Growing,
    Sustaining,
    Shrinking,
    DoneForever,
    Sleeping,
}

impl Default for DSCRUState {
    fn default() -> Self {
        DSCRUState::NotStartedYet
    }
}

/// INI-configurable data for DynamicShroudClearingRangeUpdate
#[derive(Clone, Debug)]
pub struct DynamicShroudClearingRangeUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Wait until shrink starts (frames)
    pub shrink_delay: u32,
    /// Shrink duration (frames)
    pub shrink_time: u32,
    /// Wait until grow starts (frames)
    pub grow_delay: u32,
    /// Grow duration (frames)
    pub grow_time: u32,
    /// Final vision range
    pub final_vision: Real,
    /// How often to update vision range (frames)
    pub change_interval: u32,
    /// Update interval during grow phase (frames)
    pub grow_interval: u32,
    /// Whether to show spy satellite visual effects
    pub do_spy_sat_fx: bool,
    /// Template for grid decals
    pub grid_decal_template: RadiusDecalTemplate,
}

impl Default for DynamicShroudClearingRangeUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            shrink_delay: 0,
            shrink_time: 0,
            grow_delay: 0,
            grow_time: 0,
            final_vision: 0.0,
            change_interval: 0,
            grow_interval: 0,
            do_spy_sat_fx: false,
            grid_decal_template: RadiusDecalTemplate::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(DynamicShroudClearingRangeUpdateModuleData, base);

impl DynamicShroudClearingRangeUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DYNAMIC_SHROUD_UPDATE_FIELDS)
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<u32, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_radius_decal_texture(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.texture_name = crate::common::AsciiString::from(*token);
    Ok(())
}

fn parse_radius_decal_style(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.shadow_type = INI::parse_bit_string_32(tokens, &SHADOW_NAMES)?;
    Ok(())
}

fn parse_radius_decal_opacity_min(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.min_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.min_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_max(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.max_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.max_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_throb_time(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.opacity_throb_time = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_radius_decal_color(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens.len() == 1 {
        if let Ok(value) = tokens[0].parse::<u32>() {
            data.color = value;
            return Ok(());
        }
    }

    let mut r: u8 = 0;
    let mut g: u8 = 0;
    let mut b: u8 = 0;
    let mut a: u8 = 255;

    for token in tokens {
        let (key, value) = match token.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => ("", token.trim()),
        };
        let parsed = value.parse::<u8>().map_err(|_| INIError::InvalidData)?;
        match key.to_ascii_uppercase().as_str() {
            "R" => r = parsed,
            "G" => g = parsed,
            "B" => b = parsed,
            "A" => a = parsed,
            _ => {}
        }
    }

    data.color = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
    Ok(())
}

fn parse_radius_decal_only_visible(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.only_visible_to_owning_player = INI::parse_bool(token)?;
    Ok(())
}

const RADIUS_DECAL_TEMPLATE_FIELDS: &[FieldParse<RadiusDecalTemplate>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_radius_decal_texture,
    },
    FieldParse {
        token: "Style",
        parse: parse_radius_decal_style,
    },
    FieldParse {
        token: "OpacityMin",
        parse: parse_radius_decal_opacity_min,
    },
    FieldParse {
        token: "OpacityMax",
        parse: parse_radius_decal_opacity_max,
    },
    FieldParse {
        token: "OpacityThrobTime",
        parse: parse_radius_decal_opacity_throb_time,
    },
    FieldParse {
        token: "Color",
        parse: parse_radius_decal_color,
    },
    FieldParse {
        token: "OnlyVisibleToOwningPlayer",
        parse: parse_radius_decal_only_visible,
    },
];

fn parse_grid_decal_template(
    ini: &mut INI,
    data: &mut DynamicShroudClearingRangeUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(&mut data.grid_decal_template, RADIUS_DECAL_TEMPLATE_FIELDS)
}

const DYNAMIC_SHROUD_UPDATE_FIELDS: &[FieldParse<DynamicShroudClearingRangeUpdateModuleData>] = &[
    FieldParse {
        token: "ChangeInterval",
        parse: |_, data, tokens| {
            data.change_interval = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "GrowInterval",
        parse: |_, data, tokens| {
            data.grow_interval = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ShrinkDelay",
        parse: |_, data, tokens| {
            data.shrink_delay = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ShrinkTime",
        parse: |_, data, tokens| {
            data.shrink_time = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "GrowDelay",
        parse: |_, data, tokens| {
            data.grow_delay = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "GrowTime",
        parse: |_, data, tokens| {
            data.grow_time = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalVision",
        parse: |_, data, tokens| {
            data.final_vision = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "GridDecalTemplate",
        parse: parse_grid_decal_template,
    },
];

/// DynamicShroudClearingRangeUpdate - manages shroud clearing range over time
pub struct DynamicShroudClearingRangeUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<DynamicShroudClearingRangeUpdateModuleData>,

    /// Current state machine state
    state: DSCRUState,
    /// Countdown timer for state transitions
    state_countdown: i32,
    /// Total frames for animation
    total_frames: i32,
    /// Deadline for grow start
    grow_start_deadline: u32,
    /// Deadline for sustain phase
    sustain_deadline: u32,
    /// Deadline for shrink start  
    shrink_start_deadline: u32,
    /// Failsafe frame to force shutdown
    done_forever_frame: u32,
    /// Countdown for change interval
    change_interval_countdown: u32,
    /// Whether decals have been created
    decals_created: bool,
    /// Vision change per interval
    vision_change_per_interval: Real,
    /// Object's native shroud clearing range
    native_clearing_range: Real,
    /// Current shroud clearing range
    current_clearing_range: Real,
    /// Grid decals for visual effect
    grid_decals: Vec<RadiusDecal>,
}

impl DynamicShroudClearingRangeUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<DynamicShroudClearingRangeUpdateModuleData>()
            .ok_or("Invalid module data")?;

        // Calculate state timeline (see C++ diagram comment)
        let state_countdown = (data.shrink_delay + data.shrink_time) as i32;
        let total_frames = state_countdown.max(1);
        let shrink_start_deadline = state_countdown as u32 - data.shrink_delay;
        let grow_start_deadline = state_countdown as u32 - data.grow_delay;
        let sustain_deadline = grow_start_deadline - data.grow_time;

        debug_assert!(
            sustain_deadline >= shrink_start_deadline,
            "DynamicShroudClearingRangeUpdate: sustain deadline before shrink start"
        );
        debug_assert!(
            grow_start_deadline >= shrink_start_deadline,
            "DynamicShroudClearingRangeUpdate: grow start before shrink start"
        );

        let done_forever_frame = TheGameLogic::get_frame() + state_countdown as u32;

        // Get native clearing range from object
        let native_clearing_range = if let Ok(obj) = object.read() {
            obj.get_shroud_clearing_range()
        } else {
            200.0 // Sensible default
        };

        // Initialize grid decals
        let mut grid_decals = Vec::with_capacity(GRID_FX_DECAL_COUNT);
        for _ in 0..GRID_FX_DECAL_COUNT {
            let mut decal = RadiusDecal::new(Coord3D::origin(), 0.0);
            decal.clear();
            grid_decals.push(decal);
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(data.clone()),
            state: DSCRUState::NotStartedYet,
            state_countdown,
            total_frames,
            grow_start_deadline,
            sustain_deadline,
            shrink_start_deadline,
            done_forever_frame,
            change_interval_countdown: 0,
            decals_created: false,
            vision_change_per_interval: 0.0,
            native_clearing_range,
            current_clearing_range: 0.0,
            grid_decals,
        })
    }

    /// Create grid decals for visual effect
    fn create_grid_decals(&mut self, template: &RadiusDecalTemplate, radius: Real, pos: &Coord3D) {
        let owner_index = self
            .object
            .upgrade()
            .and_then(|obj| obj.read().ok().and_then(|o| o.get_controlling_player()))
            .and_then(|player| player.read().ok().map(|p| p.get_player_index()));
        let local_index = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index());
        let allow_decal = if template.only_visible_to_owning_player {
            matches!((local_index, owner_index), (Some(local), Some(owner)) if local == owner)
        } else {
            true
        };
        if !allow_decal {
            return;
        }
        for decal in &mut self.grid_decals {
            decal.clear();
            let mut created = template.create_radius_decal_with_radius(*pos, radius);
            if !created.is_empty() {
                created.set_position(*pos);
                if template.color == 0 {
                    if let (Some(owner), Ok(list)) = (owner_index, ThePlayerList().read()) {
                        if let Some(player) = list.get_player(owner).and_then(|p| p.read().ok()) {
                            created.color = player.get_player_color().to_argb_u32();
                        }
                    }
                }
                *decal = created;
            }
        }
    }

    /// Kill all grid decals
    fn kill_grid_decals(&mut self) {
        for decal in &mut self.grid_decals {
            decal.clear();
        }
    }

    /// Animate grid decals based on current state
    fn animate_grid_decals(&mut self) {
        let center = if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj) = obj_arc.read() {
                *obj.get_position()
            } else {
                return;
            }
        } else {
            return;
        };

        let radius =
            self.current_clearing_range + ((self.total_frames - self.state_countdown) as f32 * 2.0);
        let angle_inc = (std::f32::consts::PI * 2.0) / GRID_FX_DECAL_COUNT as f32;
        let opacity = 1.0 - (self.current_clearing_range / self.native_clearing_range);

        for (i, decal) in self.grid_decals.iter_mut().enumerate() {
            let angle = i as f32 * angle_inc;
            let mut pos = Coord3D::new(
                center.x + angle.sin() * radius,
                center.y + angle.cos() * radius,
                0.0,
            );

            // Grid snapping effect from C++ (pos.x -= ((Int)pos.x)%23)
            pos.x -= (pos.x as i32 % 23) as f32;
            pos.y -= (pos.y as i32 % 23) as f32;

            decal.set_position(pos);
            decal.set_opacity(opacity);
        }
    }
}

impl UpdateModuleInterface for DynamicShroudClearingRangeUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.state == DSCRUState::Sleeping {
            return UPDATE_SLEEP_NONE;
        }

        let current_frame = TheGameLogic::get_frame();

        // Create decals on first update
        if !self.decals_created {
            if let Some(obj_arc) = self.object.upgrade() {
                if let Ok(obj) = obj_arc.read() {
                    let pos = *obj.get_position();
                    self.create_grid_decals(
                        &self.module_data.grid_decal_template.clone(),
                        100.0,
                        &pos,
                    );
                    self.decals_created = true;
                }
            }
        }

        // State transition logic
        if self.state_countdown <= 0 || current_frame > self.done_forever_frame {
            self.state = DSCRUState::DoneForever;
        } else if self.state_countdown <= self.shrink_start_deadline as i32 {
            self.state = DSCRUState::Shrinking;
        } else if self.state_countdown <= self.sustain_deadline as i32 {
            self.state = DSCRUState::Sustaining;
        } else if self.state_countdown <= self.grow_start_deadline as i32 {
            self.state = DSCRUState::Growing;
        }

        // Execute state behavior
        match self.state {
            DSCRUState::NotStartedYet => {
                self.animate_grid_decals();
            }
            DSCRUState::Growing => {
                self.animate_grid_decals();
                let grow_time = self.module_data.grow_time.max(1) as f32;
                self.current_clearing_range += self.native_clearing_range / grow_time;
                if self.current_clearing_range >= self.native_clearing_range {
                    self.state = DSCRUState::Sustaining;
                }
            }
            DSCRUState::Sustaining => {
                self.current_clearing_range = self.native_clearing_range;
                self.kill_grid_decals();
            }
            DSCRUState::Shrinking => {
                let shrink_time = self.module_data.shrink_time.max(1) as f32;
                let range_diff = self.native_clearing_range - self.module_data.final_vision;
                self.current_clearing_range -= range_diff / shrink_time;
            }
            DSCRUState::DoneForever => {
                self.kill_grid_decals();
                self.current_clearing_range = self.module_data.final_vision;
            }
            DSCRUState::Sleeping => {}
        }

        // Decrement state countdown
        if self.state_countdown > 0 {
            self.state_countdown -= 1;
        }

        // Update object shroud clearing range at intervals
        if self.change_interval_countdown > 0 {
            self.change_interval_countdown -= 1;
        } else {
            let interval = if self.state == DSCRUState::Growing {
                self.module_data.grow_interval
            } else {
                self.module_data.change_interval
            };
            self.change_interval_countdown = interval;

            // Apply range to object
            if let Some(obj_arc) = self.object.upgrade() {
                if let Ok(mut obj) = obj_arc.write() {
                    obj.set_shroud_clearing_range(self.current_clearing_range);
                }
            }

            // Transition to sleeping when done
            if self.state == DSCRUState::DoneForever {
                self.state = DSCRUState::Sleeping;
            }
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for DynamicShroudClearingRangeUpdate {
    fn get_module_name(&self) -> &'static str {
        "DynamicShroudClearingRangeUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for DynamicShroudClearingRangeUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1).map_err(|e| {
            format!(
                "DynamicShroudClearingRangeUpdate xfer version failed: {:?}",
                e
            )
        })?;

        let mut state = self.state as i32;
        xfer.xfer_i32(&mut state).map_err(|e| e.to_string())?;
        self.state = match state {
            1 => DSCRUState::Growing,
            2 => DSCRUState::Sustaining,
            3 => DSCRUState::Shrinking,
            4 => DSCRUState::DoneForever,
            5 => DSCRUState::Sleeping,
            _ => DSCRUState::NotStartedYet,
        };

        xfer.xfer_i32(&mut self.state_countdown)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.total_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.grow_start_deadline)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.sustain_deadline)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.shrink_start_deadline)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.done_forever_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.change_interval_countdown)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.decals_created)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.vision_change_per_interval)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.native_clearing_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.current_clearing_range)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes DynamicShroudClearingRangeUpdate through the common Module trait.
pub struct DynamicShroudClearingRangeUpdateModule {
    behavior: DynamicShroudClearingRangeUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<DynamicShroudClearingRangeUpdateModuleData>,
}

impl DynamicShroudClearingRangeUpdateModule {
    pub fn new(
        behavior: DynamicShroudClearingRangeUpdate,
        module_name: &AsciiString,
        module_data: Arc<DynamicShroudClearingRangeUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut DynamicShroudClearingRangeUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for DynamicShroudClearingRangeUpdateModule {
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

impl Module for DynamicShroudClearingRangeUpdateModule {
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

pub struct DynamicShroudClearingRangeUpdateFactory;
impl DynamicShroudClearingRangeUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DynamicShroudClearingRangeUpdate::new(
            thing,
            module_data,
        )?))
    }
}
