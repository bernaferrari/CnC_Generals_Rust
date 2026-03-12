// terrain_types.rs - Terrain type definitions placeholder

/// Terrain types enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainType {
    Grass,
    Sand,
    Snow,
    Rock,
    Water,
    Lava,
    Swamp,
    Urban,
}

impl Default for TerrainType {
    fn default() -> Self {
        TerrainType::Grass
    }
}

impl TerrainType {
    /// Get all terrain types
    pub fn all_types() -> &'static [TerrainType] {
        &[
            TerrainType::Grass,
            TerrainType::Sand,
            TerrainType::Snow,
            TerrainType::Rock,
            TerrainType::Water,
            TerrainType::Lava,
            TerrainType::Swamp,
            TerrainType::Urban,
        ]
    }

    /// Get terrain name
    pub fn name(self) -> &'static str {
        match self {
            TerrainType::Grass => "Grass",
            TerrainType::Sand => "Sand",
            TerrainType::Snow => "Snow",
            TerrainType::Rock => "Rock",
            TerrainType::Water => "Water",
            TerrainType::Lava => "Lava",
            TerrainType::Swamp => "Swamp",
            TerrainType::Urban => "Urban",
        }
    }

    /// Check if terrain is passable for infantry
    pub fn is_infantry_passable(self) -> bool {
        !matches!(self, TerrainType::Water | TerrainType::Lava)
    }

    /// Check if terrain is passable for vehicles
    pub fn is_vehicle_passable(self) -> bool {
        !matches!(
            self,
            TerrainType::Water | TerrainType::Lava | TerrainType::Swamp
        )
    }

    /// Get movement speed modifier
    pub fn movement_speed_modifier(self) -> f32 {
        match self {
            TerrainType::Grass => 1.0,
            TerrainType::Sand => 0.8,
            TerrainType::Snow => 0.7,
            TerrainType::Rock => 0.9,
            TerrainType::Water => 0.0, // Not passable
            TerrainType::Lava => 0.0,  // Not passable
            TerrainType::Swamp => 0.5,
            TerrainType::Urban => 0.9,
        }
    }
}

/// Terrain properties
#[derive(Debug, Clone)]
pub struct TerrainProperties {
    pub terrain_type: TerrainType,
    pub elevation: f32,
    pub hardness: f32,
    pub fertility: f32,
}

impl Default for TerrainProperties {
    fn default() -> Self {
        Self {
            terrain_type: TerrainType::default(),
            elevation: 0.0,
            hardness: 1.0,
            fertility: 1.0,
        }
    }
}

impl TerrainProperties {
    pub fn new(terrain_type: TerrainType) -> Self {
        Self {
            terrain_type,
            elevation: 0.0,
            hardness: Self::default_hardness(terrain_type),
            fertility: Self::default_fertility(terrain_type),
        }
    }

    fn default_hardness(terrain_type: TerrainType) -> f32 {
        match terrain_type {
            TerrainType::Rock => 2.0,
            TerrainType::Urban => 1.5,
            TerrainType::Sand | TerrainType::Snow => 0.5,
            TerrainType::Swamp => 0.3,
            _ => 1.0,
        }
    }

    fn default_fertility(terrain_type: TerrainType) -> f32 {
        match terrain_type {
            TerrainType::Grass => 1.5,
            TerrainType::Swamp => 1.2,
            TerrainType::Sand | TerrainType::Snow | TerrainType::Rock => 0.3,
            TerrainType::Water | TerrainType::Lava => 0.0,
            _ => 1.0,
        }
    }
}
