//! C++ `Drawable::calcPhysicsXformTreads` (`Drawable.cpp:1637-1891`).

use super::PhysicsVisualXform;
use super::loco_state::PhysicsVisualLocoState;
use super::rng::ClientVisualRng;
use super::spring::{cross3, finish_accel_totals, ground_pitch_roll, normalize3};
use super::types::{CPP_PI, LocomotorVisualParams, OverlapVisualTarget, PhysicsVisualBody};

const OVERLAP_SHRINK_FACTOR: f32 = 0.8;
const FLATTENED_OBJECT_HEIGHT: f32 = 0.5;
const LEAVE_OVERLAP_PITCH_KICK: f32 = CPP_PI / 128.0;
const OVERLAP_ROUGH_VIBRATION_FACTOR: f32 = 5.0;
const MAX_ROUGH_VIBRATION: f32 = 0.5;

/// Treads overlap mount, leave-kick, ground spring, and half-pre-integrate Z.
pub fn calc_treads(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    rng: &mut impl ClientVisualRng,
    info: &mut PhysicsVisualXform,
) {
    if !body.has_object || !body.has_ai || !body.has_physics {
        return;
    }

    let mut normal_x = body.terrain_normal_x;
    let mut normal_y = body.terrain_normal_y;
    let mut normal_z = body.terrain_normal_z;
    let mut overlap_z = 0.0;
    let overlapped = effective_overlap(body.current_overlap);

    if let Some(target) = overlapped {
        apply_overlap_surface(
            body,
            target,
            rng,
            &mut overlap_z,
            &mut normal_x,
            &mut normal_y,
            &mut normal_z,
        );
    } else if body.previous_overlap_valid && loco.overlap_z > 0.0 {
        // C++ Drawable.cpp:1776-1777 — leave-overlap pitch kick.
        loco.pitch_rate += LEAVE_OVERLAP_PITCH_KICK;
    }

    let mut terrain = *body;
    terrain.terrain_normal_x = normal_x;
    terrain.terrain_normal_y = normal_y;
    terrain.terrain_normal_z = normal_z;
    let (ground_pitch, ground_roll) = ground_pitch_roll(&terrain);

    // Ground spring only while overlapped or leftover overlap Z has settled.
    if overlapped.is_some() || loco.overlap_z <= 0.0 {
        loco.pitch_rate += (-params.pitch_stiffness * (loco.pitch - ground_pitch))
            + (-params.pitch_damping * loco.pitch_rate);
        if loco.pitch_rate > 0.0 {
            loco.pitch_rate *= 0.5;
        }
        loco.roll_rate += (-params.roll_stiffness * (loco.roll - ground_roll))
            + (-params.roll_damping * loco.roll_rate);
    }
    loco.pitch += loco.pitch_rate * params.uniform_axial_damping;
    loco.roll += loco.roll_rate * params.uniform_axial_damping;

    finish_accel_totals(
        loco,
        body,
        params,
        &mut info.total_pitch,
        &mut info.total_roll,
    );
    integrate_overlap_z(loco, overlap_z, info);
}

fn effective_overlap(current: Option<OverlapVisualTarget>) -> Option<OverlapVisualTarget> {
    current.filter(|target| !target.is_shrubbery)
}

fn apply_overlap_surface(
    body: &PhysicsVisualBody,
    target: OverlapVisualTarget,
    rng: &mut impl ClientVisualRng,
    overlap_z: &mut f32,
    normal_x: &mut f32,
    normal_y: &mut f32,
    normal_z: &mut f32,
) {
    let dx = target.pos_x - body.pos_x;
    let dy = target.pos_y - body.pos_y;
    let center_dist_sqr = dx * dx + dy * dy;
    let max_center_dist =
        (target.bounding_circle_radius + body.bounding_circle_radius) * OVERLAP_SHRINK_FACTOR;
    if center_dist_sqr >= max_center_dist * max_center_dist {
        return;
    }

    let center_dist = center_dist_sqr.sqrt();
    let amount = (1.0 - center_dist / max_center_dist).clamp(0.0, 1.0);
    let mut rough =
        (body.vel_x * body.vel_x + body.vel_y * body.vel_y) * OVERLAP_ROUGH_VIBRATION_FACTOR;
    if rough > MAX_ROUGH_VIBRATION {
        rough = MAX_ROUGH_VIBRATION;
    }

    let flat = target.is_low_overlappable
        || target.is_infantry
        || (target.front_crushed && target.back_crushed);
    let height = if flat {
        FLATTENED_OBJECT_HEIGHT
    } else {
        target.max_height_above_position
    };

    // Flat kinds always sit on top (branch B), even at tiny amount.
    if amount < FLATTENED_OBJECT_HEIGHT && !flat {
        *overlap_z = height * 2.0 * amount;
        let v_x = dx / center_dist;
        let v_y = dy / center_dist;
        let v_z = 0.2;
        let (up_x, up_y, up_z) = normalize3(
            rng.random_real(-rough, rough),
            rng.random_real(-rough, rough),
            1.0,
        );
        let (prp_x, prp_y, prp_z) = cross3(v_x, v_y, v_z, up_x, up_y, up_z);
        let (nx, ny, nz) = cross3(prp_x, prp_y, prp_z, v_x, v_y, v_z);
        let (nx, ny, nz) = normalize3(nx, ny, nz);
        *normal_x = nx;
        *normal_y = ny;
        *normal_z = nz;
    } else {
        *overlap_z = height;
        let (nx, ny, nz) = normalize3(
            rng.random_real(-rough, rough),
            rng.random_real(-rough, rough),
            1.0,
        );
        *normal_x = nx;
        *normal_y = ny;
        *normal_z = nz;
    }
}

/// Visual Z is half of overlapZ **before** this frame's fake gravity.
///
/// C++ `Drawable.cpp:1868-1890`.
fn integrate_overlap_z(
    loco: &mut PhysicsVisualLocoState,
    overlap_z: f32,
    info: &mut PhysicsVisualXform,
) {
    if overlap_z > loco.overlap_z {
        loco.overlap_z = overlap_z;
        loco.overlap_z_vel = 0.0;
    }
    let ztmp = loco.overlap_z / 2.0;
    if loco.overlap_z > 0.0 {
        loco.overlap_z_vel -= 0.2;
        loco.overlap_z += loco.overlap_z_vel;
    }
    if loco.overlap_z <= 0.0 {
        loco.overlap_z = 0.0;
        loco.overlap_z_vel = 0.0;
    }
    info.total_z = ztmp;
}
