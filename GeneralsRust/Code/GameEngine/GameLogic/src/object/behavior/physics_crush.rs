//! C++ PhysicsBehavior::checkForOverlapCollision (PhysicsUpdate.cpp:1424-1748).
//! Sibling of `physics_update.rs`.

use super::{PhysicsBehaviorHandle, is_very_small3d};
use crate::common::{Coord3D, Real};
use crate::damage::{DamageInfo, DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::object::{CrushSquishTestType, Object as GameObject};

const PERP_RANGE: Real = 0.15;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CrushTarget {
    NoCrush,
    FrontEndCrush,
    BackEndCrush,
    TotalCrush,
}

fn perps_logically_equal(a: Real, b: Real) -> bool {
    (a - b).abs() <= PERP_RANGE
}

fn vec2_len(x: Real, y: Real) -> Real {
    (x * x + y * y).sqrt()
}

/// Returns true when this collision should skip bounce-apart (overlap/crush).
pub(super) fn check_for_overlap_collision(
    handle: &mut PhysicsBehaviorHandle,
    crusher: &GameObject,
    crushee: &mut GameObject,
) -> bool {
    if is_very_small3d(handle.state.vel) {
        return false;
    }

    let self_crushing_other =
        crusher.can_crush_or_squish(crushee, CrushSquishTestType::TestCrushOnly);
    let self_being_crushed =
        crushee.can_crush_or_squish(crusher, CrushSquishTestType::TestCrushOnly);

    if self_crushing_other && self_being_crushed {
        return false;
    }
    if self_being_crushed {
        return true;
    }
    if !self_crushing_other {
        return false;
    }

    handle.add_overlap(crushee.get_id());
    if !handle.was_previously_overlapped(crushee.get_id()) {
        let mut fx =
            DamageInfo::with_simple(0.0, crusher.get_id(), DamageType::Crush, DeathType::Crushed);
        let _ = crushee.attempt_damage(&mut fx);
    }

    let Some(body) = crushee.get_body_module() else {
        return true;
    };
    let Ok(body) = body.try_lock() else {
        return true;
    };
    let front_crushed = body.get_front_crushed();
    let back_crushed = body.get_back_crushed();
    drop(body);

    if front_crushed && back_crushed {
        return true;
    }

    let (dir_x, dir_y) = crusher.get_unit_direction_vector_2d();
    let (crushee_dir_x, crushee_dir_y) = crushee.get_unit_direction_vector_2d();
    let crush_point_offset_distance = crushee.get_geometry_info().get_major_radius() / 2.0;
    let crush_off = Coord3D::new(
        crushee_dir_x * crush_point_offset_distance,
        crushee_dir_y * crush_point_offset_distance,
        0.0,
    );
    let crushee_pos = *crushee.get_position();
    let crusher_pos = *crusher.get_position();

    let crush_target = if front_crushed || back_crushed {
        if front_crushed {
            CrushTarget::BackEndCrush
        } else {
            CrushTarget::FrontEndCrush
        }
    } else {
        pick_crush_target(&crusher_pos, &crushee_pos, dir_x, dir_y, &crush_off)
    };

    let distance_too_far_squared = 2.25 * crush_point_offset_distance * crush_point_offset_distance;
    let crush_it = match crush_target {
        CrushTarget::TotalCrush => past_crush_point(
            crushee_pos.x - crusher_pos.x,
            crushee_pos.y - crusher_pos.y,
            dir_x,
            dir_y,
            distance_too_far_squared,
        ),
        CrushTarget::FrontEndCrush => past_crush_point(
            (crushee_pos.x + crush_off.x) - crusher_pos.x,
            (crushee_pos.y + crush_off.y) - crusher_pos.y,
            dir_x,
            dir_y,
            distance_too_far_squared,
        ),
        CrushTarget::BackEndCrush => past_crush_point(
            (crushee_pos.x - crush_off.x) - crusher_pos.x,
            (crushee_pos.y - crush_off.y) - crusher_pos.y,
            dir_x,
            dir_y,
            distance_too_far_squared,
        ),
        CrushTarget::NoCrush => false,
    };

    if crush_it {
        let mut lethal = DamageInfo::with_simple(
            HUGE_DAMAGE_AMOUNT,
            crusher.get_id(),
            DamageType::Crush,
            DeathType::Crushed,
        );
        let _ = crushee.attempt_damage(&mut lethal);
    }

    true
}

fn past_crush_point(dx: Real, dy: Real, dir_x: Real, dir_y: Real, max_dist_sq: Real) -> bool {
    let dot = dir_x * dx + dir_y * dy;
    let distance_squared = dx * dx + dy * dy;
    dot < 0.0 && distance_squared < max_dist_sq
}

fn perp_length(
    from_crusher_x: Real,
    from_crusher_y: Real,
    dir_x: Real,
    dir_y: Real,
) -> (Real, Real, Real) {
    let ray_length = from_crusher_x * dir_x + from_crusher_y * dir_y;
    let dir_vx = ray_length * dir_x;
    let dir_vy = ray_length * dir_y;
    let perp_x = dir_vx - from_crusher_x;
    let perp_y = dir_vy - from_crusher_y;
    (vec2_len(perp_x, perp_y), from_crusher_x, from_crusher_y)
}

fn pick_crush_target(
    crusher_pos: &Coord3D,
    crushee_pos: &Coord3D,
    dir_x: Real,
    dir_y: Real,
    crush_off: &Coord3D,
) -> CrushTarget {
    let front = perp_length(
        (crushee_pos.x + crush_off.x) - crusher_pos.x,
        (crushee_pos.y + crush_off.y) - crusher_pos.y,
        dir_x,
        dir_y,
    );
    let back = perp_length(
        (crushee_pos.x - crush_off.x) - crusher_pos.x,
        (crushee_pos.y - crush_off.y) - crusher_pos.y,
        dir_x,
        dir_y,
    );
    let center = perp_length(
        crushee_pos.x - crusher_pos.x,
        crushee_pos.y - crusher_pos.y,
        dir_x,
        dir_y,
    );

    let (front_perp, front_vx, front_vy) = front;
    let (back_perp, back_vx, back_vy) = back;
    let (center_perp, center_vx, center_vy) = center;

    if front_perp <= center_perp && front_perp <= back_perp {
        if perps_logically_equal(front_perp, center_perp)
            || perps_logically_equal(front_perp, back_perp)
        {
            let front_len = vec2_len(front_vx, front_vy);
            if perps_logically_equal(front_perp, center_perp) {
                let center_len = vec2_len(center_vx, center_vy);
                if front_len < center_len {
                    CrushTarget::FrontEndCrush
                } else {
                    CrushTarget::TotalCrush
                }
            } else {
                let back_len = vec2_len(back_vx, back_vy);
                if front_len < back_len {
                    CrushTarget::FrontEndCrush
                } else {
                    CrushTarget::BackEndCrush
                }
            }
        } else {
            CrushTarget::FrontEndCrush
        }
    } else if back_perp <= center_perp && back_perp <= front_perp {
        if perps_logically_equal(back_perp, center_perp)
            || perps_logically_equal(back_perp, front_perp)
        {
            let back_len = vec2_len(back_vx, back_vy);
            if perps_logically_equal(back_perp, center_perp) {
                let center_len = vec2_len(center_vx, center_vy);
                if back_len < center_len {
                    CrushTarget::BackEndCrush
                } else {
                    CrushTarget::TotalCrush
                }
            } else {
                let front_len = vec2_len(front_vx, front_vy);
                if back_len < front_len {
                    CrushTarget::BackEndCrush
                } else {
                    CrushTarget::FrontEndCrush
                }
            }
        } else {
            CrushTarget::BackEndCrush
        }
    } else if perps_logically_equal(center_perp, back_perp)
        || perps_logically_equal(center_perp, front_perp)
    {
        let center_len = vec2_len(center_vx, center_vy);
        if perps_logically_equal(center_perp, front_perp) {
            let front_len = vec2_len(front_vx, front_vy);
            if center_len < front_len {
                CrushTarget::TotalCrush
            } else {
                CrushTarget::FrontEndCrush
            }
        } else {
            let back_len = vec2_len(back_vx, back_vy);
            if center_len < back_len {
                CrushTarget::TotalCrush
            } else {
                CrushTarget::BackEndCrush
            }
        }
    } else {
        CrushTarget::TotalCrush
    }
}
