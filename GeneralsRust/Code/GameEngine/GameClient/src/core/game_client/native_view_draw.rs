// W3DView's CPU Drawable region, independent of GPU visibility.
use crate::display::client_draw_schedule::ViewAabb;

/// One driving view in original C++ Z-up coordinates.
#[derive(Debug, Clone, Copy)]
pub struct NativeDrawView {
    pub region: ViewAabb,
}

impl NativeDrawView {
    /// `W3DView.cpp:1374-1415`: corners projected by the driving view to Z=0.
    /// Negative guard bias is preserved, including an inverted (empty) region.
    pub fn from_ground_corners(
        corners: [[f32; 3]; 4],
        guard_band: [f32; 2],
        terrain_vertical: [f32; 2],
    ) -> GameClientResult<Self> {
        if !corners
            .iter()
            .flatten()
            .chain(guard_band.iter())
            .chain(terrain_vertical.iter())
            .all(|value| value.is_finite())
        {
            return Err(GameClientError::InvalidOperation(
                "non-finite native view input".into(),
            ));
        }
        let mut lo = corners[0];
        let mut hi = corners[0];
        for point in &corners {
            lo[0] = lo[0].min(point[0]);
            lo[1] = lo[1].min(point[1]);
            hi[0] = hi[0].max(point[0]);
            hi[1] = hi[1].max(point[1]);
        }
        lo[2] = terrain_vertical[0] - 999_999.;
        hi[2] = terrain_vertical[1] + 999_999.;
        lo[0] -= 75. + guard_band[0];
        lo[1] -= 75. + guard_band[1] + 60.;
        hi[0] += 75. + guard_band[0];
        hi[1] += 75. + guard_band[1];
        let view = Self {
            region: ViewAabb { lo, hi },
        };
        view.validate()?;
        Ok(view)
    }

    fn validate(&self) -> GameClientResult<()> {
        if self
            .region
            .lo
            .iter()
            .chain(self.region.hi.iter())
            .all(|value| value.is_finite())
        {
            Ok(())
        } else {
            Err(GameClientError::InvalidOperation(
                "non-finite native view region".into(),
            ))
        }
    }
}

impl GameClient {
    /// `GameClient.cpp:757`: raw position, inclusive XYZ, constructor-list order.
    /// Visibility and Object existence are later Drawable::draw decisions.
    pub(crate) fn native_roster_for_view(
        &self,
        view: &NativeDrawView,
    ) -> GameClientResult<Vec<DrawableId>> {
        view.validate()?;
        let mut selected = Vec::new();
        for &id in &self.drawables.native_drawable_order {
            let drawable = self.drawables.drawable_map.get(&id).ok_or_else(|| {
                GameClientError::InvalidOperation("native view roster receiver absent".into())
            })?;
            let position = drawable.get_position();
            let point = if let Some(binding) = self
                .drawables
                .presentation_direct_drawable_bindings
                .get(&id)
            {
                let key = binding.binding_key;
                if key.drawable_id != id
                    || self
                        .presentation_direct_drawable_state(key.host_epoch, key.object_id)
                        .is_none_or(|state| state.binding_key != key)
                    || !drawable.as_any().is::<BasicDrawable>()
                {
                    return Err(GameClientError::InvalidOperation(
                        "stale native view binding".into(),
                    ));
                }
                // Main stores raw host positions Y-up. Instance matrices,
                // float pose, scale and recoil are not getPosition().
                [position.x, position.z, position.y]
            } else {
                [position.x, position.y, position.z]
            };
            if view.region.contains(point) {
                selected.push(id);
            }
        }
        Ok(selected)
    }
}

#[cfg(test)]
#[path = "native_view_draw_tests.rs"]
mod native_view_draw_tests;

#[cfg(test)]
#[path = "native_presentation_tests.rs"]
mod native_presentation_tests;

#[cfg(test)]
#[path = "region_iteration_tests.rs"]
mod region_iteration_tests;
