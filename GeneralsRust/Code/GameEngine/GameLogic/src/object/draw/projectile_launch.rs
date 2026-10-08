//! Frozen outputs for one projectile query across a pristine-bone lookup.

use crate::common::*;

/// One selected condition state's values; no module handles or mutable state.
#[derive(Debug)]
pub struct ProjectileLaunchPlan {
    pub(crate) launch: Option<Matrix3D>,
    pub(crate) apply_attachment: bool,
    pub(crate) turret_positions: [Coord3D; 2],
    pub(crate) fallback_pivots: [NameKeyType; 2],
    pub(crate) attachment_bone: Option<AsciiString>,
    pub(crate) cached_attachment: Option<Coord3D>,
}

impl ProjectileLaunchPlan {
    pub(crate) fn finish(
        self,
        queried_attachment: Option<Coord3D>,
        fallback_positions: [Option<Coord3D>; 2],
        launch: &mut Matrix3D,
        turret_rotation: &mut Coord3D,
        turret_pitch: &mut Coord3D,
    ) -> bool {
        let offset = if self.apply_attachment {
            self.cached_attachment
                .or(queried_attachment)
                .unwrap_or(Coord3D::origin())
        } else {
            Coord3D::origin()
        };
        let positions = std::array::from_fn::<_, 2, _>(|index| {
            fallback_positions[index].unwrap_or(self.turret_positions[index]) + offset
        });
        *turret_rotation = positions[0];
        *turret_pitch = positions[1];
        if let Some(transform) = self.launch {
            *launch = transform;
            true
        } else {
            false
        }
    }
}
