//! W3D-specific game logic factory selection.
//!
//! C++ source: `GameEngineDevice/Source/W3DDevice/GameLogic/W3DGameLogic.cpp`.
//! That source file is intentionally empty; the class behavior lives inline in
//! `W3DGameLogic.h`, where `W3DGameLogic` derives from `GameLogic` only to
//! create W3D terrain logic and a W3D ghost-object manager.

/// Terrain logic implementation selected by `W3DGameLogic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainLogicFactory {
    /// C++ `NEW W3DTerrainLogic`.
    W3DTerrainLogic,
}

/// Ghost-object manager implementation selected by `W3DGameLogic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostObjectManagerFactory {
    /// C++ `NEW W3DGhostObjectManager`.
    W3DGhostObjectManager,
}

/// W3D-specific `GameLogic` adapter.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct W3DGameLogic;

impl W3DGameLogic {
    /// Construct a W3D game-logic adapter.
    pub const fn new() -> Self {
        Self
    }

    /// C++ `createTerrainLogic`.
    pub const fn create_terrain_logic(&self) -> TerrainLogicFactory {
        TerrainLogicFactory::W3DTerrainLogic
    }

    /// C++ `createGhostObjectManager`.
    pub const fn create_ghost_object_manager(&self) -> GhostObjectManagerFactory {
        GhostObjectManagerFactory::W3DGhostObjectManager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factories_match_w3d_game_logic_header() {
        let logic = W3DGameLogic::new();

        assert_eq!(
            logic.create_terrain_logic(),
            TerrainLogicFactory::W3DTerrainLogic
        );
        assert_eq!(
            logic.create_ghost_object_manager(),
            GhostObjectManagerFactory::W3DGhostObjectManager
        );
    }
}
