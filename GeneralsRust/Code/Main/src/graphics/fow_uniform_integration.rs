use crate::fow_rendering::ObjectVisibility;

/// The exact scalar values that an already-collected render item contributes
/// to its model uniform. This module intentionally has no FOW bridge or game
/// object dependency: querying visibility belongs to collection, never draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrozenFowModelFields {
    pub visibility_alpha: f32,
    pub visibility_falloff: f32,
    pub is_explored: f32,
}

/// Preserve a render item's frozen visibility in the order expected by
/// `WgpuMaterialBinds::model`.
#[inline]
pub(crate) fn frozen_fow_model_fields(visibility: ObjectVisibility) -> FrozenFowModelFields {
    FrozenFowModelFields {
        visibility_alpha: visibility.visibility_alpha,
        visibility_falloff: visibility.visibility_falloff,
        is_explored: visibility.is_explored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_fow_fields_preserve_each_snapshot_scalar() {
        let fields = frozen_fow_model_fields(ObjectVisibility {
            visibility_alpha: 0.23,
            is_explored: 0.47,
            visibility_falloff: 0.61,
        });

        assert_eq!(fields.visibility_alpha, 0.23);
        assert_eq!(fields.visibility_falloff, 0.61);
        assert_eq!(fields.is_explored, 0.47);
    }
}
