//! FXList tracer spawn matching C++ `TracerFXNugget` + `W3DTracerDraw`.
//!
//! Oracle:
//! - `FXList.cpp` TracerFXNugget::doFXPos (newDrawable, buildTransformMatrix,
//!   setTracerParms, REAL_TO_INT_CEIL expiration)
//! - `W3DTracerDraw.cpp` Line3D from (0,0,0)→(length,0,0), opacity decay,
//!   local-X translation by speed each frame

use std::sync::{Mutex, OnceLock};

use gamelogic::object::draw::{DrawModule, TracerDrawInterface};
use glam::{Mat4, Vec3, Vec4};

/// Live tracer created by FXList (wgpu stand-in for the Drawable + Line3D).
#[derive(Debug, Clone, PartialEq)]
pub struct TracerFxInstance {
    pub id: u32,
    pub tracer_name: String,
    pub pos: [f32; 3],
    pub dir: [f32; 3],
    pub speed: f32,
    pub length: f32,
    pub width: f32,
    pub color: [f32; 3],
    pub opacity: f32,
    pub spawn_frame: u32,
    pub expire_frame: u32,
}

/// GPU line-strip quad for one tracer (camera-up billboard along local X).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TracerGpuVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TracerGpuMesh {
    pub vertices: Vec<TracerGpuVertex>,
    pub indices: Vec<u16>,
}

struct TracerStore {
    next_id: u32,
    tracers: Vec<TracerFxInstance>,
}

impl TracerStore {
    fn new() -> Self {
        Self {
            next_id: 1,
            tracers: Vec::new(),
        }
    }
}

fn global_tracers() -> &'static Mutex<TracerStore> {
    static STORE: OnceLock<Mutex<TracerStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(TracerStore::new()))
}

/// Serializes tests that touch the process-wide tracer store.
pub fn lock_tracer_fx_tests() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// C++ `calcDist`.
pub fn tracer_distance(primary: [f32; 3], secondary: [f32; 3]) -> f32 {
    let dx = secondary[0] - primary[0];
    let dy = secondary[1] - primary[1];
    let dz = secondary[2] - primary[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// C++ `REAL_TO_INT_CEIL(frames * m_decayAt)` with
/// `frames = (dist >= 0 && speed >= 0) ? dist/speed : 1`.
pub fn tracer_expiration_frames(dist_minus_length: f32, speed: f32, decay_at: f32) -> u32 {
    let frames = if dist_minus_length >= 0.0 && speed >= 0.0 {
        dist_minus_length / speed
    } else {
        1.0
    };
    (frames * decay_at).ceil().max(0.0) as u32
}

/// C++ `Matrix3D::buildTransformMatrix(pos, unitDir)`:
/// X axis points along `dir` (must be unitized).
pub fn build_tracer_transform(pos: [f32; 3], dir: [f32; 3]) -> Mat4 {
    let len2 = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    let sinp = dir[2];
    let cosp = len2;
    let (siny, cosy) = if len2 != 0.0 {
        (dir[1] / len2, dir[0] / len2)
    } else {
        (0.0, 1.0)
    };

    let translate = Mat4::from_translation(Vec3::new(pos[0], pos[1], pos[2]));
    // WWMath post-multiply Rotate_Z(siny, cosy) then Rotate_Y(-sinp, cosp).
    let rotate_z = Mat4::from_cols(
        Vec4::new(cosy, siny, 0.0, 0.0),
        Vec4::new(-siny, cosy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let s = -sinp;
    let c = cosp;
    let rotate_y = Mat4::from_cols(
        Vec4::new(c, 0.0, -s, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(s, 0.0, c, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    translate * rotate_z * rotate_y
}

/// C++ `Line3DClass` local endpoints: `(0,0,0)` → `(length,0,0)`.
pub fn tracer_line3d_local_endpoints(length: f32) -> ([f32; 3], [f32; 3]) {
    ([0.0, 0.0, 0.0], [length, 0.0, 0.0])
}

/// C++ `W3DTracerDraw::doDrawModule` opacity after `elapsed` draws:
/// `decay = opacity / (expDate - currentFrame); opacity -= decay`.
/// `elapsed == 0` returns `initial_opacity` (spawn / already-updated state).
pub fn tracer_opacity_after_frames(
    initial_opacity: f32,
    spawn_frame: u32,
    expire_frame: u32,
    elapsed_frames: u32,
) -> f32 {
    if expire_frame == 0 || elapsed_frames == 0 {
        return initial_opacity;
    }
    let mut opacity = initial_opacity;
    for i in 0..elapsed_frames {
        let current = spawn_frame.saturating_add(i);
        if current >= expire_frame {
            break;
        }
        let remaining = (expire_frame - current) as f32;
        if remaining > 0.0 {
            opacity -= opacity / remaining;
        }
    }
    opacity
}

/// World endpoints of the C++ Line3D after `elapsed` frames of local-X travel.
///
/// Line3D lives in local space `(0,0,0)→(length,0,0)`; each draw translates
/// that transform by `(speed, 0, 0)` on local X (`W3DTracerDraw.cpp`).
pub fn tracer_world_endpoints(
    tracer: &TracerFxInstance,
    elapsed_frames: u32,
) -> ([f32; 3], [f32; 3]) {
    let travel = tracer.speed * elapsed_frames as f32;
    let xform = build_tracer_transform(tracer.pos, tracer.dir);
    let (local_start, local_end) = tracer_line3d_local_endpoints(tracer.length);
    let start = xform.transform_point3(Vec3::new(
        local_start[0] + travel,
        local_start[1],
        local_start[2],
    ));
    let end = xform.transform_point3(Vec3::new(local_end[0] + travel, local_end[1], local_end[2]));
    ([start.x, start.y, start.z], [end.x, end.y, end.z])
}

/// Billboard quad along the tracer line (width matches `m_width`).
///
/// Color RGB is `setTracerParms` / Line3D `Re_Color`. Alpha is Line3D
/// `Set_Opacity` after `elapsed` C++ decay steps from the instance opacity.
pub fn bake_tracer_gpu_mesh(tracer: &TracerFxInstance, elapsed_frames: u32) -> TracerGpuMesh {
    let (start, end) = tracer_world_endpoints(tracer, elapsed_frames);
    let dir = Vec3::new(end[0] - start[0], end[1] - start[1], end[2] - start[2]);
    let mut perp = dir.cross(Vec3::Z);
    if perp.length_squared() < 1.0e-8 {
        perp = dir.cross(Vec3::Y);
    }
    let perp = perp.normalize_or_zero() * (tracer.width * 0.5);
    let s = Vec3::from(start);
    let e = Vec3::from(end);
    let alpha = tracer_opacity_after_frames(
        tracer.opacity,
        tracer.spawn_frame,
        tracer.expire_frame,
        elapsed_frames,
    );
    let color = [tracer.color[0], tracer.color[1], tracer.color[2], alpha];
    let v = |p: Vec3, uv: [f32; 2]| TracerGpuVertex {
        position: [p.x, p.y, p.z],
        color,
        uv,
    };
    TracerGpuMesh {
        vertices: vec![
            v(s - perp, [0.0, 0.0]),
            v(s + perp, [0.0, 1.0]),
            v(e + perp, [1.0, 1.0]),
            v(e - perp, [1.0, 0.0]),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// Result of C++ `TracerFXNugget::doFXPos` (GPU Line3D stand-in + drawable).
#[derive(Debug, Clone, PartialEq)]
pub struct TracerDrawableSpawn {
    pub fx: TracerFxInstance,
    pub drawable_id: Option<u32>,
    /// `TheThingFactory->findTemplate` hit (C++ `newDrawable` path).
    pub used_thing_factory: bool,
    /// At least one `TracerDrawInterface::setTracerParms` ran.
    pub tracer_parms_applied: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveTracerDrawable {
    pub id: u32,
    pub tracer_name: String,
    pub line_start: [f32; 3],
    pub line_end: [f32; 3],
    pub opacity: f32,
    pub length: f32,
    pub speed: f32,
    pub expire_frame: u32,
}

struct TracerDrawableStore {
    next_id: u32,
    drawables: Vec<LiveTracerDrawable>,
}

impl TracerDrawableStore {
    fn new() -> Self {
        Self {
            next_id: 1,
            drawables: Vec::new(),
        }
    }
}

fn global_tracer_drawables() -> &'static Mutex<TracerDrawableStore> {
    static STORE: OnceLock<Mutex<TracerDrawableStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(TracerDrawableStore::new()))
}

pub fn live_tracer_drawables() -> Vec<LiveTracerDrawable> {
    global_tracer_drawables()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drawables
        .clone()
}

fn clear_tracer_drawables() {
    let mut store = global_tracer_drawables()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    store.drawables.clear();
}

fn rgb_from_fx_color(color: [f32; 3]) -> gamelogic::object::draw::draw_module::RGBColor {
    gamelogic::object::draw::draw_module::RGBColor::new(
        (color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// C++ `TracerFXNugget::doFXPos` after probability:
/// `newDrawable` + `buildTransformMatrix` + `setTracerParms` + `setExpirationDate`.
///
/// wgpu `create_tracer_fx` stays the Line3D GPU mesh stand-in (presentation/tests).
/// When ThingFactory has the template (GenericTracer → W3DTracerDraw), the real
/// drawable path runs. Otherwise a local W3DTracerDraw is the GenericTracer module.
pub fn spawn_tracer_drawable_like_cpp(
    tracer_name: &str,
    primary: [f32; 3],
    secondary: [f32; 3],
    speed: f32,
    length: f32,
    width: f32,
    color: [f32; 3],
    decay_at: f32,
    current_frame: u32,
) -> Option<TracerDrawableSpawn> {
    let _ = gamelogic::helpers::TheThingFactory::ensure_system_ini_drawable_only_templates();
    let fx = create_tracer_fx(
        tracer_name,
        primary,
        secondary,
        speed,
        length,
        width,
        color,
        decay_at,
        current_frame,
    )?;
    let expire = fx.expire_frame;
    let rgb = rgb_from_fx_color(color);
    let xform = build_tracer_transform(primary, fx.dir);

    let mut used_thing_factory = false;
    let mut tracer_parms_applied = false;
    let mut drawable_id = None;

    if let Some(template) = gamelogic::helpers::TheThingFactory::find_template(tracer_name) {
        let id = gamelogic::helpers::TheGameClient.create_drawable(template.as_ref());
        if id != 0 {
            used_thing_factory = true;
            drawable_id = Some(id);
            if let Some(arc) = gamelogic::helpers::TheGameClient.get_drawable_arc(id) {
                if let Ok(mut drawable) = arc.write() {
                    drawable.set_transform(xform);
                    let applied =
                        drawable.apply_tracer_parms(speed, length, width, &rgb, 1.0, expire);
                    tracer_parms_applied = applied > 0;
                }
            }
            gamelogic::helpers::TheGameClient.set_drawable_expiration_date(id, expire);
        }
    }

    if !tracer_parms_applied {
        // C++ GenericTracer INI: `Draw = W3DTracerDraw` only.
        let mut draw = gamelogic::object::draw::W3DTracerDraw::new(
            gamelogic::object::draw::W3DTracerDrawModuleData::new(),
        );
        draw.set_tracer_parms(speed, length, width, &rgb, 1.0);
        draw.set_expiration_date(expire);
        draw.do_draw_module(&xform);
        tracer_parms_applied = true;
        let mut store = global_tracer_drawables()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let id = store.next_id;
        store.next_id = store.next_id.wrapping_add(1).max(1);
        let start = draw.line_start();
        let end = draw.line_end();
        store.drawables.push(LiveTracerDrawable {
            id,
            tracer_name: tracer_name.to_string(),
            line_start: [start.x, start.y, start.z],
            line_end: [end.x, end.y, end.z],
            opacity: draw.opacity(),
            length: draw.length(),
            speed: draw.speed_in_dist_per_frame(),
            expire_frame: expire,
        });
        drawable_id = Some(id);
    } else if used_thing_factory {
        // Template path already owns the drawable; record spawn for tests (no second Line3D).
        let mut store = global_tracer_drawables()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        store.drawables.push(LiveTracerDrawable {
            id: drawable_id.unwrap_or(0),
            tracer_name: tracer_name.to_string(),
            line_start: primary,
            line_end: [
                primary[0] + fx.dir[0] * length,
                primary[1] + fx.dir[1] * length,
                primary[2] + fx.dir[2] * length,
            ],
            opacity: 1.0,
            length,
            speed,
            expire_frame: expire,
        });
    }

    Some(TracerDrawableSpawn {
        fx,
        drawable_id,
        used_thing_factory,
        tracer_parms_applied,
    })
}

/// C++ TracerFXNugget::doFXPos GPU Line3D stand-in (probability already passed).
pub fn create_tracer_fx(
    tracer_name: &str,
    primary: [f32; 3],
    secondary: [f32; 3],
    speed: f32,
    length: f32,
    width: f32,
    color: [f32; 3],
    decay_at: f32,
    current_frame: u32,
) -> Option<TracerFxInstance> {
    let mut dir = Vec3::new(
        secondary[0] - primary[0],
        secondary[1] - primary[1],
        secondary[2] - primary[2],
    );
    let len = dir.length();
    if len > 0.0 {
        dir /= len;
    } else {
        dir = Vec3::X;
    }
    let dist = tracer_distance(primary, secondary);
    let frames = tracer_expiration_frames(dist - length, speed, decay_at);
    let instance = {
        let mut store = global_tracers().lock().unwrap_or_else(|e| e.into_inner());
        let id = store.next_id;
        store.next_id = store.next_id.wrapping_add(1).max(1);
        let inst = TracerFxInstance {
            id,
            tracer_name: tracer_name.to_string(),
            pos: primary,
            dir: [dir.x, dir.y, dir.z],
            speed,
            length,
            width,
            color,
            opacity: 1.0,
            spawn_frame: current_frame,
            expire_frame: current_frame.saturating_add(frames),
        };
        store.tracers.push(inst.clone());
        inst
    };
    Some(instance)
}

pub fn live_tracer_fx() -> Vec<TracerFxInstance> {
    global_tracers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .tracers
        .clone()
}

pub fn clear_tracer_fx() {
    let mut store = global_tracers().lock().unwrap_or_else(|e| e.into_inner());
    store.tracers.clear();
    clear_tracer_drawables();
}

/// C++ `W3DTracerDraw::doDrawModule` opacity decay + local-X translate, then
/// expire when `current_frame >= expirationDate`.
pub fn update_tracer_fx(current_frame: u32) {
    let mut store = global_tracers().lock().unwrap_or_else(|e| e.into_inner());
    store
        .tracers
        .retain(|t| t.expire_frame == 0 || current_frame < t.expire_frame);
    for tracer in &mut store.tracers {
        if tracer.expire_frame != 0 {
            let remaining = (tracer.expire_frame - current_frame) as f32;
            if remaining > 0.0 {
                let decay = tracer.opacity / remaining;
                tracer.opacity -= decay;
            }
        }
        if tracer.speed != 0.0 {
            tracer.pos[0] += tracer.dir[0] * tracer.speed;
            tracer.pos[1] += tracer.dir[1] * tracer.speed;
            tracer.pos[2] += tracer.dir[2] * tracer.speed;
        }
    }
}

pub fn bake_all_tracer_gpu_meshes() -> Vec<TracerGpuMesh> {
    live_tracer_fx()
        .iter()
        .map(|t| bake_tracer_gpu_mesh(t, 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::fxlist_integration::{FXContext, FXNugget, TracerFXNugget};
    use crate::effects::particle_manager::ParticleSystemManager;
    use nalgebra::Point3;

    fn mesh_centerline(mesh: &TracerGpuMesh) -> ([f32; 3], [f32; 3]) {
        assert_eq!(mesh.vertices.len(), 4);
        let s0 = Vec3::from(mesh.vertices[0].position);
        let s1 = Vec3::from(mesh.vertices[1].position);
        let e0 = Vec3::from(mesh.vertices[2].position);
        let e1 = Vec3::from(mesh.vertices[3].position);
        let start = (s0 + s1) * 0.5;
        let end = (e0 + e1) * 0.5;
        ([start.x, start.y, start.z], [end.x, end.y, end.z])
    }

    /// C++ formulas from the same inputs as the FXList nugget (not a table).
    fn cpp_tracer_expected(
        primary: [f32; 3],
        secondary: [f32; 3],
        speed: f32,
        length: f32,
        decay_at: f32,
        spawn_frame: u32,
        elapsed: u32,
        initial_opacity: f32,
    ) -> ([f32; 3], [f32; 3], f32, u32) {
        let mut dir = [
            secondary[0] - primary[0],
            secondary[1] - primary[1],
            secondary[2] - primary[2],
        ];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if len > 0.0 {
            dir = [dir[0] / len, dir[1] / len, dir[2] / len];
        } else {
            dir = [1.0, 0.0, 0.0];
        }
        let dist = len;
        let frames = if dist - length >= 0.0 && speed >= 0.0 {
            (dist - length) / speed
        } else {
            1.0
        };
        let expire_span = (frames * decay_at).ceil().max(0.0) as u32;
        let expire_frame = spawn_frame.saturating_add(expire_span);

        let travel = speed * elapsed as f32;
        let start = [
            primary[0] + dir[0] * travel,
            primary[1] + dir[1] * travel,
            primary[2] + dir[2] * travel,
        ];
        let end = [
            start[0] + dir[0] * length,
            start[1] + dir[1] * length,
            start[2] + dir[2] * length,
        ];

        let mut opacity = initial_opacity;
        for i in 0..elapsed {
            let current = spawn_frame.saturating_add(i);
            if expire_frame != 0 && current < expire_frame {
                let remaining = (expire_frame - current) as f32;
                opacity -= opacity / remaining;
            }
        }
        (start, end, opacity, expire_frame)
    }

    #[test]
    fn fxlist_tracer_nugget_create_tracer_fx_gpu_mesh_after_n_frames_matches_cpp() {
        let _guard = lock_tracer_fx_tests();
        clear_tracer_fx();

        let primary = [12.0_f32, -4.0, 8.0];
        let secondary = [60.0_f32, 20.0, 20.0];
        let speed = 8.0_f32;
        let length = 6.0_f32;
        let width = 1.5_f32;
        let color = [0.7_f32, 0.25, 0.1];
        let decay_at = 0.8_f32;
        let spawn_frame = 40_u32;
        let n_frames = 3_u32;

        let mut nugget = TracerFXNugget::new("GenericTracer".to_string());
        nugget.speed = speed;
        nugget.length = length;
        nugget.width = width;
        nugget.color = color;
        nugget.decay_at = decay_at;
        nugget.probability = 1.0;

        let mut manager = ParticleSystemManager::new();
        let mut ctx = FXContext {
            particle_manager: &mut manager,
            ray_effect_manager: None,
            decal_manager: None,
            bone_query: None,
            current_frame: spawn_frame,
            local_player_index: 0,
        };
        FXNugget::do_fx_pos(
            &nugget,
            Point3::new(primary[0], primary[1], primary[2]),
            None,
            99.0,
            Some(Point3::new(secondary[0], secondary[1], secondary[2])),
            0.0,
            &mut ctx,
        );

        let spawned = live_tracer_fx();
        assert_eq!(
            spawned.len(),
            1,
            "FXList TracerFXNugget must call create_tracer_fx"
        );
        let drawables = live_tracer_drawables();
        assert_eq!(
            drawables.len(),
            1,
            "C++ TracerFXNugget newDrawable + W3DTracerDraw setTracerParms"
        );
        assert_eq!(drawables[0].tracer_name, "GenericTracer");
        assert_eq!(drawables[0].length, length);
        assert_eq!(drawables[0].speed, speed);
        let tracer = spawned[0].clone();
        assert_eq!(tracer.tracer_name, "GenericTracer");
        assert_eq!(tracer.speed, speed);
        assert_eq!(tracer.length, length);
        assert_eq!(tracer.width, width);
        assert_eq!(tracer.color, color);
        assert_eq!(tracer.opacity, 1.0);
        assert_eq!(tracer.spawn_frame, spawn_frame);

        let (exp_start, exp_end, exp_alpha, exp_expire) = cpp_tracer_expected(
            primary,
            secondary,
            speed,
            length,
            decay_at,
            spawn_frame,
            n_frames,
            1.0,
        );
        assert_eq!(tracer.expire_frame, exp_expire);

        let local = tracer_line3d_local_endpoints(length);
        assert_eq!(local, ([0.0, 0.0, 0.0], [length, 0.0, 0.0]));

        let mesh_from_spawn = bake_tracer_gpu_mesh(&tracer, n_frames);
        let (mesh_start, mesh_end) = mesh_centerline(&mesh_from_spawn);
        for i in 0..3 {
            assert!(
                (mesh_start[i] - exp_start[i]).abs() < 1.0e-4,
                "spawn-bake start[{i}] {} vs cpp {}",
                mesh_start[i],
                exp_start[i]
            );
            assert!(
                (mesh_end[i] - exp_end[i]).abs() < 1.0e-4,
                "spawn-bake end[{i}] {} vs cpp {}",
                mesh_end[i],
                exp_end[i]
            );
        }
        for v in &mesh_from_spawn.vertices {
            assert_eq!(&v.color[0..3], &color);
            assert!(
                (v.color[3] - exp_alpha).abs() < 1.0e-5,
                "spawn-bake alpha {} vs cpp {}",
                v.color[3],
                exp_alpha
            );
        }

        for i in 0..n_frames {
            update_tracer_fx(spawn_frame + i);
        }
        let after = live_tracer_fx();
        assert_eq!(after.len(), 1);
        assert!((after[0].opacity - exp_alpha).abs() < 1.0e-5);
        for i in 0..3 {
            assert!((after[0].pos[i] - exp_start[i]).abs() < 1.0e-4);
        }

        let live_mesh = bake_tracer_gpu_mesh(&after[0], 0);
        let (live_start, live_end) = mesh_centerline(&live_mesh);
        for i in 0..3 {
            assert!((live_start[i] - exp_start[i]).abs() < 1.0e-4);
            assert!((live_end[i] - exp_end[i]).abs() < 1.0e-4);
        }
        for v in &live_mesh.vertices {
            assert_eq!(&v.color[0..3], &color);
            assert!((v.color[3] - exp_alpha).abs() < 1.0e-5);
        }

        update_tracer_fx(exp_expire);
        assert!(
            live_tracer_fx().is_empty(),
            "C++ drawable expires at spawn+REAL_TO_INT_CEIL(frames*decayAt)"
        );
        clear_tracer_fx();
    }

    #[test]
    fn tracer_fx_zero_nugget_speed_uses_primary_speed_like_cpp() {
        let _guard = lock_tracer_fx_tests();
        clear_tracer_fx();
        let mut nugget = TracerFXNugget::new("GenericTracer".to_string());
        nugget.speed = 0.0;
        nugget.probability = 1.0;
        nugget.length = 4.0;
        nugget.decay_at = 1.0;

        let mut manager = ParticleSystemManager::new();
        let mut ctx = FXContext {
            particle_manager: &mut manager,
            ray_effect_manager: None,
            decal_manager: None,
            bone_query: None,
            current_frame: 7,
            local_player_index: 0,
        };
        FXNugget::do_fx_pos(
            &nugget,
            Point3::new(0.0, 0.0, 0.0),
            None,
            12.0,
            Some(Point3::new(40.0, 0.0, 0.0)),
            0.0,
            &mut ctx,
        );
        let tracers = live_tracer_fx();
        assert_eq!(tracers.len(), 1);
        assert_eq!(
            tracers[0].speed, 12.0,
            "C++ TracerFXNugget uses primarySpeed when m_speed == 0"
        );
        assert_eq!(live_tracer_drawables().len(), 1);
        clear_tracer_fx();
    }

    #[test]
    fn spawn_tracer_drawable_like_cpp_applies_w3d_tracer_draw_local_x() {
        let _guard = lock_tracer_fx_tests();
        clear_tracer_fx();
        let primary = [10.0_f32, 20.0, 30.0];
        let secondary = [110.0_f32, 20.0, 30.0];
        let spawned = spawn_tracer_drawable_like_cpp(
            "GenericTracer",
            primary,
            secondary,
            5.0,
            10.0,
            0.5,
            [0.9, 0.8, 0.7],
            1.0,
            0,
        )
        .expect("spawn");
        assert!(spawned.drawable_id.is_some());
        assert!(
            spawned.used_thing_factory,
            "System.ini GenericTracer must register so ThingFactory::findTemplate hits"
        );
        assert!(
            gamelogic::helpers::TheThingFactory::generic_tracer_matches_system_ini(),
            "GenericTracer KindOf=DRAWABLE_ONLY Draw=W3DTracerDraw ModuleTag_01"
        );
        let draws = live_tracer_drawables();
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].speed, 5.0);
        assert_eq!(draws[0].length, 10.0);
        clear_tracer_fx();
    }
}
