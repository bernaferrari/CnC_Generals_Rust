//! Frozen simulation inputs for a drawable's C++ shroud branch.

pub use generals_game_domain::object_status::ObjectShroudStatus as PresentationObjectShroudStatus;
use serde::{Deserialize, Serialize};

/// Whether this frame has authoritative direct-object facts for a drawable.
///
/// The client owns drawable associations and checks their generations.
/// The frame records whether the driving simulation supplied the status and
/// effective-death facts for a direct drawable.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresentationDrawableLifetime {
    /// No authoritative direct-object drawable facts are available.
    #[default]
    Unknown = 0,
    /// The simulation captured facts from a live object during frame build.
    DirectHostObject = 1,
}

impl PresentationDrawableLifetime {
    #[inline]
    pub const fn is_direct_host_object(self) -> bool {
        matches!(self, Self::DirectHostObject)
    }
}

/// Frozen input for the C++ direct-object shroud branch.
///
/// The raw status and effective-death bit are captured before rendering.  The
/// drawable-owned clear-frame timer is intentionally absent: C++ updates it at
/// view dispatch after resolving the live drawable, and Main must not emulate
/// that state from `ObjectVisibility` alpha.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct PresentationDrawableShroudFacts {
    pub lifetime: PresentationDrawableLifetime,
    pub raw_status: PresentationObjectShroudStatus,
    pub effectively_dead: bool,
}

impl PresentationDrawableShroudFacts {
    #[inline]
    pub const fn direct_host_object(
        raw_status: PresentationObjectShroudStatus,
        effectively_dead: bool,
    ) -> Self {
        Self {
            lifetime: PresentationDrawableLifetime::DirectHostObject,
            raw_status,
            effectively_dead,
        }
    }

    /// C++ RTS3DScene selects the shroud material when the status exceeds Clear.
    #[inline]
    pub const fn requires_scene_shroud_material(self) -> bool {
        self.lifetime.is_direct_host_object()
            && (self.raw_status as u8) > (PresentationObjectShroudStatus::Clear as u8)
    }

    /// Source-side payload for GameClient's direct-object update.  `Unknown`
    /// has no drawable/object association and therefore cannot enter that API.
    #[inline]
    pub fn direct_game_client_status(self) -> Option<(PresentationObjectShroudStatus, bool)> {
        self.lifetime
            .is_direct_host_object()
            .then_some((self.raw_status, self.effectively_dead))
    }
}
