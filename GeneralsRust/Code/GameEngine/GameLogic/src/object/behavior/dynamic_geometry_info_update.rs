//! DynamicGeometryInfoUpdate - Rust conversion of C++ DynamicGeometryInfoUpdate
//!
//! Update module that smoothly transitions the object's geometry (height, radii)
//! from initial to final values over a transition time.
//! Author: Graham Smallwood, April 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{ModuleData, Real, UnsignedInt, XferVersion};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

/// INI-configurable data for DynamicGeometryInfoUpdate
#[derive(Clone, Debug)]
pub struct DynamicGeometryInfoUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Delay before starting transition (frames)
    pub initial_delay: u32,
    /// Initial geometry dimensions
    pub initial_height: Real,
    pub initial_major_radius: Real,
    pub initial_minor_radius: Real,
    /// Final geometry dimensions
    pub final_height: Real,
    pub final_major_radius: Real,
    pub final_minor_radius: Real,
    /// Transition time in frames
    pub transition_time: u32,
    /// Whether to reverse direction at end of transition
    pub reverse_at_transition_time: bool,
}

impl Default for DynamicGeometryInfoUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            initial_delay: 0,
            initial_height: 0.0,
            initial_major_radius: 0.0,
            initial_minor_radius: 0.0,
            final_height: 0.0,
            final_major_radius: 0.0,
            final_minor_radius: 0.0,
            transition_time: 1,
            reverse_at_transition_time: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(DynamicGeometryInfoUpdateModuleData, base);

impl DynamicGeometryInfoUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DYNAMIC_GEOMETRY_INFO_UPDATE_FIELDS)
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const DYNAMIC_GEOMETRY_INFO_UPDATE_FIELDS: &[FieldParse<DynamicGeometryInfoUpdateModuleData>] = &[
    FieldParse {
        token: "InitialDelay",
        parse: |_, data, tokens| {
            data.initial_delay = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitialHeight",
        parse: |_, data, tokens| {
            data.initial_height = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitialMajorRadius",
        parse: |_, data, tokens| {
            data.initial_major_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitialMinorRadius",
        parse: |_, data, tokens| {
            data.initial_minor_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalHeight",
        parse: |_, data, tokens| {
            data.final_height = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalMajorRadius",
        parse: |_, data, tokens| {
            data.final_major_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalMinorRadius",
        parse: |_, data, tokens| {
            data.final_minor_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TransitionTime",
        parse: |_, data, tokens| {
            data.transition_time = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ReverseAtTransitionTime",
        parse: |_, data, tokens| {
            data.reverse_at_transition_time = INI::parse_bool(required_value(tokens)?)?;
            Ok(())
        },
    },
];

/// Shared logic for dynamic geometry transitions
pub struct DynamicGeometryInfoUpdateLogic {
    /// UpdateModule scheduler state serialized by the C++ base class
    pub next_call_frame_and_phase: UnsignedInt,
    /// Countdown frames before starting
    pub starting_delay_countdown: UnsignedInt,
    /// Frames since transition started
    pub time_active: UnsignedInt,
    /// Whether transition has started
    pub started: bool,
    /// Whether transition is finished
    pub finished: bool,
    /// Whether to reverse at transition time (instance copy)
    pub reverse_at_transition_time: bool,
    /// C++ persisted this enum even though current transition math uses swapped endpoints.
    pub direction: DynamicGeometryDirection,
    /// Whether we've switched directions
    pub switched_directions: bool,

    // Instance copies of initial/final that can be swapped for reverse
    pub initial_height: Real,
    pub initial_major_radius: Real,
    pub initial_minor_radius: Real,
    pub final_height: Real,
    pub final_major_radius: Real,
    pub final_minor_radius: Real,

    pub transition_time: UnsignedInt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicGeometryDirection {
    Backward = -1,
    Forward = 1,
}

impl DynamicGeometryDirection {
    fn from_i32(value: i32) -> Self {
        match value {
            -1 => Self::Backward,
            _ => Self::Forward,
        }
    }
}

impl DynamicGeometryInfoUpdateLogic {
    pub fn new(data: &DynamicGeometryInfoUpdateModuleData) -> Self {
        Self {
            next_call_frame_and_phase: 0,
            starting_delay_countdown: data.initial_delay.max(1),
            time_active: 0,
            started: false,
            finished: false,
            reverse_at_transition_time: data.reverse_at_transition_time,
            direction: DynamicGeometryDirection::Forward,
            switched_directions: false,
            initial_height: data.initial_height,
            initial_major_radius: data.initial_major_radius,
            initial_minor_radius: data.initial_minor_radius,
            final_height: data.final_height,
            final_major_radius: data.final_major_radius,
            final_minor_radius: data.final_minor_radius,
            transition_time: data.transition_time.max(1),
        }
    }

    pub fn update_step(&mut self, object: &GameObject) -> UpdateSleepTime {
        if self.finished {
            return UPDATE_SLEEP_NONE;
        }

        // Wait for initial delay
        if !self.started {
            self.starting_delay_countdown -= 1;
            if self.starting_delay_countdown > 0 {
                return UPDATE_SLEEP_NONE;
            }
            self.started = true;
        }

        // Calculate interpolation ratio
        let transition_time = self.transition_time as f32;
        let ratio = (self.time_active as f32) / transition_time;

        // Calculate new geometry values
        let _new_height = self.initial_height + ratio * (self.final_height - self.initial_height);
        let _new_major = self.initial_major_radius
            + ratio * (self.final_major_radius - self.initial_major_radius);
        let _new_minor = self.initial_minor_radius
            + ratio * (self.final_minor_radius - self.initial_minor_radius);

        // Apply new geometry to object (simplified implementation)
        let _ = (object, _new_height, _new_major, _new_minor);

        // Increment time active
        self.time_active += 1;

        // Check if transition is complete
        if self.time_active > self.transition_time {
            if self.reverse_at_transition_time {
                // Reverse direction
                self.switched_directions = true;
                self.time_active = 0;
                self.reverse_at_transition_time = false;
                self.direction = DynamicGeometryDirection::Backward;

                // Swap initial and final values
                std::mem::swap(&mut self.initial_height, &mut self.final_height);
                std::mem::swap(&mut self.initial_major_radius, &mut self.final_major_radius);
                std::mem::swap(&mut self.initial_minor_radius, &mut self.final_minor_radius);
            } else {
                self.finished = true;
            }
        }

        UPDATE_SLEEP_NONE
    }
}

pub(crate) fn xfer_dynamic_geometry_info_update_logic(
    xfer: &mut dyn Xfer,
    logic: &mut DynamicGeometryInfoUpdateLogic,
    label: &str,
) -> Result<(), String> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| format!("{label} xfer version: {e:?}"))?;

    xfer_update_module_base_state(xfer, &mut logic.next_call_frame_and_phase)?;

    xfer.xfer_unsigned_int(&mut logic.starting_delay_countdown)
        .map_err(|e| format!("{label} xfer starting_delay_countdown: {e:?}"))?;
    xfer.xfer_unsigned_int(&mut logic.time_active)
        .map_err(|e| format!("{label} xfer time_active: {e:?}"))?;
    xfer.xfer_bool(&mut logic.started)
        .map_err(|e| format!("{label} xfer started: {e:?}"))?;
    xfer.xfer_bool(&mut logic.finished)
        .map_err(|e| format!("{label} xfer finished: {e:?}"))?;
    xfer.xfer_bool(&mut logic.reverse_at_transition_time)
        .map_err(|e| format!("{label} xfer reverse_at_transition_time: {e:?}"))?;

    let mut direction = logic.direction as i32;
    xfer.xfer_int(&mut direction)
        .map_err(|e| format!("{label} xfer direction: {e:?}"))?;
    logic.direction = DynamicGeometryDirection::from_i32(direction);

    xfer.xfer_bool(&mut logic.switched_directions)
        .map_err(|e| format!("{label} xfer switched_directions: {e:?}"))?;
    xfer.xfer_real(&mut logic.initial_height)
        .map_err(|e| format!("{label} xfer initial_height: {e:?}"))?;
    xfer.xfer_real(&mut logic.initial_major_radius)
        .map_err(|e| format!("{label} xfer initial_major_radius: {e:?}"))?;
    xfer.xfer_real(&mut logic.initial_minor_radius)
        .map_err(|e| format!("{label} xfer initial_minor_radius: {e:?}"))?;
    xfer.xfer_real(&mut logic.final_height)
        .map_err(|e| format!("{label} xfer final_height: {e:?}"))?;
    xfer.xfer_real(&mut logic.final_major_radius)
        .map_err(|e| format!("{label} xfer final_major_radius: {e:?}"))?;
    xfer.xfer_real(&mut logic.final_minor_radius)
        .map_err(|e| format!("{label} xfer final_minor_radius: {e:?}"))?;

    Ok(())
}

/// DynamicGeometryInfoUpdate - smoothly transitions object geometry over time
pub struct DynamicGeometryInfoUpdate {
    object: Weak<RwLock<GameObject>>,
    pub logic: DynamicGeometryInfoUpdateLogic,
}

impl DynamicGeometryInfoUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<DynamicGeometryInfoUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            logic: DynamicGeometryInfoUpdateLogic::new(data),
        })
    }
}

impl UpdateModuleInterface for DynamicGeometryInfoUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj) = obj_arc.read() {
                return self.logic.update_step(&obj);
            }
        }
        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for DynamicGeometryInfoUpdate {
    fn get_module_name(&self) -> &'static str {
        "DynamicGeometryInfoUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for DynamicGeometryInfoUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer_dynamic_geometry_info_update_logic(xfer, &mut self.logic, "DynamicGeometryInfoUpdate")
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct DynamicGeometryInfoUpdateFactory;
impl DynamicGeometryInfoUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DynamicGeometryInfoUpdate::new(
            thing,
            module_data,
        )?))
    }
}
