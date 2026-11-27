//! Original object shapes and their derived pose bounds.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// C++ `GeometryType` (`Geometry.h:25-33`). SPHERE=0, CYLINDER=1, BOX=2.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename = "HostGeometryType")]
pub enum GeometryType {
    #[default]
    Sphere = 0,
    Cylinder = 1,
    Box = 2,
}

impl GeometryType {
    /// C++ `GeometryNames[]` / `INI::scanIndexList` (`Geometry.cpp:26-29`).
    pub fn from_ini(token: &str) -> Option<Self> {
        match token.trim() {
            t if t.eq_ignore_ascii_case("SPHERE") => Some(Self::Sphere),
            t if t.eq_ignore_ascii_case("CYLINDER") => Some(Self::Cylinder),
            t if t.eq_ignore_ascii_case("BOX") => Some(Self::Box),
            _ => None,
        }
    }
}

/// C++ `ThingTemplate::m_geometryInfo` (`ThingTemplate.cpp:966`, `Geometry.cpp:26-88`).
///
/// INI parse writes each field independently (no `set()` copy of sphere/cylinder
/// radii). Constructor default is SPHERE / not-small / 1 / 1 / 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename = "HostGeometryInfo")]
pub struct GeometryShape {
    pub geom_type: GeometryType,
    pub is_small: bool,
    pub height: f32,
    pub major_radius: f32,
    pub minor_radius: f32,
    /// True when any Object INI `Geometry*` field was parsed onto this template.
    #[serde(default)]
    pub authored: bool,
}

impl Default for GeometryShape {
    fn default() -> Self {
        // C++ `ThingTemplate::ThingTemplate()`: `m_geometryInfo(GEOMETRY_SPHERE, FALSE, 1, 1, 1)`.
        Self {
            geom_type: GeometryType::Sphere,
            is_small: false,
            height: 1.0,
            major_radius: 1.0,
            minor_radius: 1.0,
            authored: false,
        }
    }
}

impl GeometryShape {
    /// C++ `GeometryInfo::calcBoundingStuff` circle (`Geometry.cpp:468-495`).
    pub fn bounding_circle_radius(&self) -> f32 {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => self.major_radius,
            GeometryType::Box => (self.major_radius * self.major_radius
                + self.minor_radius * self.minor_radius)
                .sqrt(),
        }
    }

    /// C++ `GeometryInfo::calcBoundingStuff` sphere (`Geometry.cpp:468-495`).
    pub fn bounding_sphere_radius(&self) -> f32 {
        match self.geom_type {
            GeometryType::Sphere => self.major_radius,
            GeometryType::Cylinder => {
                let half_h = self.height * 0.5;
                if half_h < self.major_radius {
                    self.major_radius
                } else {
                    half_h
                }
            }
            GeometryType::Box => {
                let half_h = self.height * 0.5;
                (self.major_radius * self.major_radius
                    + self.minor_radius * self.minor_radius
                    + half_h * half_h)
                    .sqrt()
            }
        }
    }

    /// C++ `GeometryInfo::getMaxHeightAbovePosition` (Sphere→major; Box/Cylinder→height).
    pub fn max_height_above_position(&self) -> f32 {
        match self.geom_type {
            GeometryType::Sphere => self.major_radius,
            GeometryType::Cylinder | GeometryType::Box => self.height,
        }
    }

    /// Stamp host pose `GeometryInfo` (Y-up bounds) from C++ geom extents.
    pub fn to_host_geometry(&self) -> GeometryInfo {
        let (bounds_min, bounds_max, radius) = match self.geom_type {
            GeometryType::Sphere => {
                let r = self.major_radius;
                (Vec3::splat(-r), Vec3::splat(r), r)
            }
            GeometryType::Cylinder => {
                let r = self.major_radius;
                let h = self.height;
                (Vec3::new(-r, 0.0, -r), Vec3::new(r, h, r), r)
            }
            GeometryType::Box => {
                let a = self.major_radius;
                let b = self.minor_radius;
                let h = self.height;
                (
                    Vec3::new(-a, 0.0, -b),
                    Vec3::new(a, h, b),
                    self.bounding_circle_radius(),
                )
            }
        };
        GeometryInfo {
            shape: Some(*self),
            position: Vec3::ZERO,
            rotation: 0.0,
            bounds_min,
            bounds_max,
            radius,
        }
    }
}

/// Basic geometry information for objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryInfo {
    /// Canonical mutable original shape. Remaining fields are the existing
    /// derived bounds cache and pose projection, not another shape owner.
    /// Native saved continuation transfers shape at a structural tail boundary.
    #[serde(skip)]
    pub shape: Option<GeometryShape>,
    pub position: Vec3,
    pub rotation: f32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub radius: f32,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            shape: None,
            position: Vec3::ZERO,
            rotation: 0.0,
            bounds_min: Vec3::splat(-1.0),
            bounds_max: Vec3::splat(1.0),
            radius: 1.0,
        }
    }
}
