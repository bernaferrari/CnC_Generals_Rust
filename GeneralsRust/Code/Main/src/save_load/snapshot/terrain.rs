//! Terrain and weather snapshot types and Xfer residual.

use super::xfer_helpers::{xfer_vec_bool, xfer_vec_default, xfer_vec_f32, xfer_vec_u8};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Terrain state snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerrainSnapshot {
    pub width: u32,
    pub height: u32,
    pub height_map: Vec<f32>,
    pub texture_map: Vec<u8>,
    pub passability_map: Vec<bool>,
    pub modifications: Vec<TerrainModification>,
    /// C++ `WorldHeightMap` sample extents (`getXExtent`/`getYExtent`).
    #[serde(default)]
    pub logic_width: u32,
    #[serde(default)]
    pub logic_height: u32,
    /// Raw u8 logic heights applied by `W3DTerrainVisual::xfer` v>=2.
    #[serde(default)]
    pub logic_heights: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainModification {
    pub position: glam::Vec3,
    pub radius: f32,
    pub height_delta: f32,
    pub modification_type: String,
}

/// Weather system snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherSnapshot {
    pub current_weather: String,
    pub weather_intensity: f32,
    pub weather_duration: f32,
    pub next_weather_change: f32,
    #[serde(default = "weather_visible_default")]
    pub visible: bool,
}

const fn weather_visible_default() -> bool {
    true
}

impl XferData for TerrainModification {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TerrainModification")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        xfer.xfer_marker_label("HeightDelta")?;
        xfer.xfer_f32(&mut self.height_delta)?;
        xfer.xfer_marker_label("ModificationType")?;
        self.modification_type.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for TerrainSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TerrainSnapshot")?;
        xfer.xfer_marker_label("Width")?;
        xfer.xfer_u32(&mut self.width)?;
        xfer.xfer_marker_label("Height")?;
        xfer.xfer_u32(&mut self.height)?;
        xfer.xfer_marker_label("HeightMap")?;
        xfer_vec_f32(xfer, &mut self.height_map)?;
        xfer.xfer_marker_label("TextureMap")?;
        xfer_vec_u8(xfer, &mut self.texture_map)?;
        xfer.xfer_marker_label("PassabilityMap")?;
        xfer_vec_bool(xfer, &mut self.passability_map)?;
        xfer.xfer_marker_label("Modifications")?;
        xfer_vec_default(
            xfer,
            &mut self.modifications,
            TerrainModification {
                position: glam::Vec3::ZERO,
                radius: 0.0,
                height_delta: 0.0,
                modification_type: String::new(),
            },
        )?;
        xfer.xfer_marker_label("LogicWidth")?;
        xfer.xfer_u32(&mut self.logic_width)?;
        xfer.xfer_marker_label("LogicHeight")?;
        xfer.xfer_u32(&mut self.logic_height)?;
        xfer.xfer_marker_label("LogicHeights")?;
        xfer_vec_u8(xfer, &mut self.logic_heights)?;
        Ok(())
    }
}

impl XferData for WeatherSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("WeatherSnapshot")?;
        xfer.xfer_marker_label("CurrentWeather")?;
        self.current_weather.xfer(xfer)?;
        xfer.xfer_marker_label("WeatherIntensity")?;
        xfer.xfer_f32(&mut self.weather_intensity)?;
        xfer.xfer_marker_label("WeatherDuration")?;
        xfer.xfer_f32(&mut self.weather_duration)?;
        xfer.xfer_marker_label("NextWeatherChange")?;
        xfer.xfer_f32(&mut self.next_weather_change)?;
        xfer.xfer_marker_label("Visible")?;
        xfer.xfer_bool(&mut self.visible)?;
        Ok(())
    }
}

impl Default for WeatherSnapshot {
    fn default() -> Self {
        Self {
            current_weather: "clear".to_string(),
            weather_intensity: 0.0,
            weather_duration: 0.0,
            next_weather_change: 0.0,
            visible: weather_visible_default(),
        }
    }
}
