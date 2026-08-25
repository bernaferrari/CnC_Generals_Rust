//! C++ `Drawable::calcPhysicsXform` dispatcher (`Drawable.cpp:1390-1441`).

use super::PhysicsVisualXform;
use super::hover::calc_hover_or_wings;
use super::loco_state::PhysicsVisualLocoState;
use super::motorcycle::calc_motorcycle;
use super::rng::ClientVisualRng;
use super::thrust::calc_thrust;
use super::treads::calc_treads;
use super::types::{LocomotorVisualParams, PhysicsVisualAppearance, PhysicsVisualBody};
use super::wheels::calc_wheels;

/// Dispatch by appearance, mutate loco state, return denormal-cleaned totals.
///
/// `None` matches C++ `false` (no AI, no current locomotor, or legs/climber/other).
pub fn calc_physics_visual_xform(
    appearance: PhysicsVisualAppearance,
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    rng: &mut impl ClientVisualRng,
) -> Option<PhysicsVisualXform> {
    if !appearance.has_physics_xform() {
        return None;
    }

    let mut info = PhysicsVisualXform::default();
    match appearance {
        PhysicsVisualAppearance::WheelsFour => calc_wheels(loco, params, body, rng, &mut info),
        PhysicsVisualAppearance::Motorcycle => calc_motorcycle(loco, params, body, rng, &mut info),
        PhysicsVisualAppearance::Treads => calc_treads(loco, params, body, rng, &mut info),
        PhysicsVisualAppearance::Hover | PhysicsVisualAppearance::Wings => {
            calc_hover_or_wings(loco, params, body, &mut info)
        }
        PhysicsVisualAppearance::Thrust => calc_thrust(loco, params, &mut info),
        PhysicsVisualAppearance::LegsTwo
        | PhysicsVisualAppearance::Climber
        | PhysicsVisualAppearance::Other => return None,
    }
    Some(info.without_denormals())
}
