// Client shadow types and shroud-status mapping.
// Split from `core/game_client.rs` dump. Included by `game_client_impl/mod.rs`
// so this stays one logical `game_client` module (public API identical).

// ==================================================================================
// Shadow System Types
// C++ reference: ShadowManager, W3DShadow, GameClient::releaseShadows/allocateShadows
// ==================================================================================

/// Shadow projection type — mirrors C++ `ShadowType` (Shadow.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowType {
    /// No shadow rendered.
    None,
    /// Simple circular blob projected onto terrain.
    #[default]
    Blob,
    /// Volumetric shadow projected from the model silhouette.
    Volume,
    /// Shadow rendered as a decal on the terrain.
    Decal,
}

/// Shadow instance projected onto terrain beneath an object.
///
/// C++ reference: `Shadow` / `W3DShadow` — each object that casts a shadow has
/// a Shadow record stored in the GameClient shadow table.  The renderer projects
/// the shadow geometry (blob circle or model silhouette) onto terrain.
#[derive(Debug, Clone)]
pub struct Shadow {
    /// World position of the shadow centre (projected onto terrain Z).
    pub position: crate::system::Coord3D,
    /// Shadow radius for blob-type shadows.
    pub radius: f32,
    /// Shadow opacity [0..1]. C++ reduces opacity for partially transparent objects.
    pub opacity: f32,
    /// Which shadow technique to use.
    pub shadow_type: ShadowType,
    /// Orientation angle of the shadow (for directional light projection).
    pub angle: f32,
    /// Whether the shadow is currently visible (within frustum, not culled).
    pub visible: bool,
}

impl Shadow {
    /// Create a new blob shadow at the given position with default radius and full opacity.
    pub fn new_blob(position: crate::system::Coord3D, radius: f32) -> Self {
        Self {
            position,
            radius: radius.max(0.0),
            opacity: 1.0,
            shadow_type: ShadowType::Blob,
            angle: 0.0,
            visible: true,
        }
    }

    /// Create a new volumetric shadow at the given position.
    pub fn new_volume(position: crate::system::Coord3D) -> Self {
        Self {
            position,
            radius: 0.0,
            opacity: 1.0,
            shadow_type: ShadowType::Volume,
            angle: 0.0,
            visible: true,
        }
    }
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            position: crate::system::Coord3D::default(),
            radius: 0.0,
            opacity: 0.0,
            shadow_type: ShadowType::None,
            angle: 0.0,
            visible: false,
        }
    }
}

// ==================================================================================
// Shroud Status for Client Queries
// C++ reference: PartitionManager::getShroudStatusForPlayer
// ==================================================================================

/// Client-visible shroud status for a world position.
///
/// This is the *client-facing* version of `ObjectShroudStatus`.  The C++ code
/// uses the same underlying `PartitionManager` shroud data but the client
/// collapses the status into three discrete states for rendering decisions:
/// `Clear` (fully visible), `Fogged` (previously seen, now dimmed),
/// `Shrouded` (never explored or fully obscured).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShroudStatus {
    /// Position is fully visible — no shroud or fog.
    Clear,
    /// Position was previously seen but is now in fog of war.
    Fogged,
    /// Position has never been explored or is fully shrouded.
    #[default]
    Shrouded,
}

impl From<gamelogic::common::types::ObjectShroudStatus> for ShroudStatus {
    fn from(status: gamelogic::common::types::ObjectShroudStatus) -> Self {
        match status {
            gamelogic::common::types::ObjectShroudStatus::Clear
            | gamelogic::common::types::ObjectShroudStatus::PartialClear => ShroudStatus::Clear,
            gamelogic::common::types::ObjectShroudStatus::Fogged
            | gamelogic::common::types::ObjectShroudStatus::InvalidButPreviousValid => {
                ShroudStatus::Fogged
            }
            gamelogic::common::types::ObjectShroudStatus::Shrouded
            | gamelogic::common::types::ObjectShroudStatus::Invalid => ShroudStatus::Shrouded,
        }
    }
}

impl From<gamelogic::system::shroud_manager::ShroudState> for ShroudStatus {
    fn from(status: gamelogic::system::shroud_manager::ShroudState) -> Self {
        match status {
            gamelogic::system::shroud_manager::ShroudState::Visible => ShroudStatus::Clear,
            gamelogic::system::shroud_manager::ShroudState::Explored => ShroudStatus::Fogged,
            gamelogic::system::shroud_manager::ShroudState::Hidden => ShroudStatus::Shrouded,
        }
    }
}
