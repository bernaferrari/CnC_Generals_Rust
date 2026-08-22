//! Host HelicopterSlowDeathUpdate / HelicopterSlowDeathBehavior residual.
//!
//! C++ file: `HelicopterSlowDeathUpdate.cpp` (class `HelicopterSlowDeathBehavior`).
//! Extends SlowDeath with spiral orbit, self-spin, blade fly-off, ground hit.
//!
//! Retail Comanche peel (`AmericaAir.ini`):
//! - SpiralOrbitTurnRate **140** deg/s → **~0.0814** rad/frame
//! - SpiralOrbitForwardSpeed **350** → **~11.67** world units/frame
//! - SpiralOrbitForwardSpeedDamping **0.9999**
//! - MinSelfSpin **100** / MaxSelfSpin **300** deg/s
//! - SelfSpinUpdateDelay **100**ms → **3**f, UpdateAmount **10** deg
//! - FallHowFast **12%** of gravity
//! - Min/MaxBladeFlyOffDelay **1500**ms → **45**f
//! - SoundDeathLoop `ComancheDamagedLoop`
//! - FXHitGround `FX_HelicopterHitGround` / OCLHitGround `OCL_HelicopterHitGround`
//! - FXFinalBlowUp `FX_GroundedHelicopterBlowUp` / OCLFinalBlowUp `OCL_GroundedHelicopterBlowUp`
//! - DelayFromGroundToFinalDeath **1500**ms → **45**f (`parseDurationReal`)
//! - FinalRubbleObject `ComancheRubbleHull`
//!
//! C++ `update` (:416-474): on ground hit `doFXObj(FXHitGround)` + OCL + HELD +
//! SPECIAL_DAMAGED + `removeAudioEvent(death loop)`; after
//! `frame - hitGround > DelayFromGroundToFinalDeath` fire FinalBlowUp FX/OCL,
//! spawn `FinalRubbleObject` at the copter transform, destroy.
//!
//! AttachParticle is created and attached on begin (C++ :215-250).
//! Fail-closed: not full blade-bone matrix / eject-pilot veterancy gate.

use serde::{Deserialize, Serialize};

pub const HELI_SLOW_DEATH_LOGIC_FPS: f32 = 30.0;
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

#[inline]
pub fn heli_deg_per_sec_to_rad_per_frame(deg_per_sec: f32) -> f32 {
    deg_per_sec * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS
}

#[inline]
pub fn heli_velocity_per_sec_to_per_frame(v: f32) -> f32 {
    v / HELI_SLOW_DEATH_LOGIC_FPS
}

#[inline]
pub fn heli_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * HELI_SLOW_DEATH_LOGIC_FPS / 1000.0).round() as u32
}

/// C++ `INI::parseDurationReal` / `ConvertDurationFromMsecsToFrames`.
#[inline]
pub fn heli_ms_to_duration_frames(ms: f32) -> f32 {
    ms * HELI_SLOW_DEATH_LOGIC_FPS / 1000.0
}

/// Retail SpiralOrbitTurnRate 140 deg/s.
pub const COMANCHE_SPIRAL_ORBIT_TURN_RATE_DEG_PER_SEC: f32 = 140.0;
pub const HELI_SPIRAL_TURN_RATE: f32 =
    COMANCHE_SPIRAL_ORBIT_TURN_RATE_DEG_PER_SEC * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS;
/// Retail SpiralOrbitForwardSpeed 350 (dist/sec → /frame).
pub const COMANCHE_SPIRAL_FORWARD_SPEED_PER_SEC: f32 = 350.0;
pub const HELI_SPIRAL_FORWARD_SPEED: f32 =
    COMANCHE_SPIRAL_FORWARD_SPEED_PER_SEC / HELI_SLOW_DEATH_LOGIC_FPS;
/// Retail SpiralOrbitForwardSpeedDamping.
pub const HELI_SPIRAL_FORWARD_SPEED_DAMPING: f32 = 0.9999;
/// Retail MinSelfSpin / MaxSelfSpin deg/s.
pub const COMANCHE_MIN_SELF_SPIN_DEG_PER_SEC: f32 = 100.0;
pub const COMANCHE_MAX_SELF_SPIN_DEG_PER_SEC: f32 = 300.0;
pub const HELI_MIN_SELF_SPIN: f32 =
    COMANCHE_MIN_SELF_SPIN_DEG_PER_SEC * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS;
pub const HELI_MAX_SELF_SPIN: f32 =
    COMANCHE_MAX_SELF_SPIN_DEG_PER_SEC * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS;
/// Retail SelfSpinUpdateDelay 100ms.
pub const HELI_SELF_SPIN_UPDATE_DELAY_FRAMES: u32 = 3;
/// Retail SelfSpinUpdateAmount 10 deg → rad.
pub const HELI_SELF_SPIN_UPDATE_AMOUNT: f32 = 10.0 * DEG_TO_RAD;
/// Retail FallHowFast 12% of gravity magnitude residual.
pub const HELI_FALL_HOW_FAST: f32 = 0.12;
/// Host gravity magnitude residual (world Y down acceleration per frame² peel).
pub const HELI_GRAVITY_MAG: f32 = 0.5;
pub const HELI_CRASH_GRAVITY: f32 = -HELI_GRAVITY_MAG * HELI_FALL_HOW_FAST;
/// Retail blade fly-off delay 1500ms.
pub const HELI_BLADE_FLY_OFF_FRAMES: u32 = 45;
/// Retail Comanche `DelayFromGroundToFinalDeath` 1500ms → 45 frames.
pub const HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_MS: u32 = 1500;
pub const HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES: u32 = 45;
/// Alias for dual-tick peel; value is C++ DelayFromGroundToFinalDeath, not the
/// invented 30-frame silent settle.
pub const HELI_GROUND_SETTLE_FRAMES: u32 = HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES;
/// Retail SoundDeathLoop peel.
pub const HELI_SOUND_DEATH_LOOP: &str = "ComancheDamagedLoop";
/// Retail AttachParticle peel.
pub const HELI_ATTACH_PARTICLE: &str = "SootySmokeTrail";
/// Retail FXHitGround / OCLHitGround (Comanche / Chinook / Helix).
pub const HELI_FX_HIT_GROUND: &str = "FX_HelicopterHitGround";
pub const HELI_OCL_HIT_GROUND: &str = "OCL_HelicopterHitGround";
/// Retail FXFinalBlowUp / OCLFinalBlowUp (Comanche / Chinook).
pub const HELI_FX_FINAL_BLOW_UP: &str = "FX_GroundedHelicopterBlowUp";
pub const HELI_OCL_FINAL_BLOW_UP: &str = "OCL_GroundedHelicopterBlowUp";
/// Retail FinalRubbleObject (Comanche).
pub const HELI_FINAL_RUBBLE_OBJECT: &str = "ComancheRubbleHull";
/// Retail FXBlade / OCLBlade.
pub const HELI_FX_BLADE: &str = "FX_HelicopterBladeExplosion";
pub const HELI_OCL_BLADE: &str = "OCL_HelicopterBladeExplosion";
/// Retail Helix-only peels.
pub const HELIX_SOUND_DEATH_LOOP: &str = "HelixDamagedLoop";
pub const HELIX_FX_FINAL_BLOW_UP: &str = "FX_HelixHelicopterBlowUpBig";
pub const HELIX_OCL_FINAL_BLOW_UP: &str = "OCL_HelixBlades";
pub const HELIX_FINAL_RUBBLE_OBJECT: &str = "HelixRubbleHull";
pub const CHINOOK_FINAL_RUBBLE_OBJECT: &str = "ChinookRubbleHull";
/// Chinook / Helix `DelayFromGroundToFinalDeath` 30ms → 0.9 frames.
pub const HELI_SHORT_GROUND_DELAY_MS: f32 = 30.0;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostHelicopterSlowDeathFx {
    pub fx_hit_ground: Option<String>,
    pub ocl_hit_ground: Option<String>,
    pub fx_final_blow_up: Option<String>,
    pub ocl_final_blow_up: Option<String>,
    pub fx_blade: Option<String>,
    pub ocl_blade: Option<String>,
    pub death_loop_sound: Option<String>,
    /// C++ `m_delayFromGroundToFinalDeath` (DurationReal frames).
    pub delay_from_ground_frames: f32,
    pub final_rubble_object: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HostHeliDeathPhaseEvent {
    pub fx: Option<String>,
    pub ocl: Option<String>,
    pub audio: Option<String>,
    pub stop_loop: bool,
    pub rubble: Option<String>,
    pub held: bool,
    pub special_damaged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostHelicopterSlowDeathData {
    pub active: bool,
    pub hit_ground: bool,
    pub hit_ground_frame: u32,
    pub activate_frame: u32,
    pub orbit_angle: f32,
    pub self_spin: f32,
    /// Direction of self-spin update (+1 / -1).
    pub self_spin_dir: f32,
    pub frames_since_spin_update: u32,
    pub forward_speed: f32,
    pub vertical_velocity: f32,
    pub orientation_delta: f32,
    pub blade_flew_off: bool,
    pub done: bool,
    /// C++ `m_attachParticleSystem` template name.
    #[serde(default = "default_heli_attach_particle")]
    pub attach_particle: String,
    #[serde(default)]
    pub attach_particle_bone: String,
    #[serde(default)]
    pub attach_particle_loc: [f32; 3],
    #[serde(default)]
    pub pending_attach: bool,
    #[serde(default)]
    pub attach_system_id: Option<u32>,
    #[serde(default)]
    pub fx: HostHelicopterSlowDeathFx,
    #[serde(default)]
    pub pending_fx: Option<String>,
    #[serde(default)]
    pub pending_ocl: Option<String>,
    #[serde(default)]
    pub pending_audio: Option<String>,
    #[serde(default)]
    pub pending_stop_loop: bool,
    #[serde(default)]
    pub pending_rubble: Option<String>,
    #[serde(default)]
    pub pending_held: bool,
    #[serde(default)]
    pub pending_special_damaged: bool,
}

fn default_heli_attach_particle() -> String {
    HELI_ATTACH_PARTICLE.to_string()
}

impl Default for HostHelicopterSlowDeathData {
    fn default() -> Self {
        Self {
            active: false,
            hit_ground: false,
            hit_ground_frame: 0,
            activate_frame: 0,
            orbit_angle: 0.0,
            self_spin: HELI_MIN_SELF_SPIN,
            self_spin_dir: 1.0,
            frames_since_spin_update: 0,
            forward_speed: HELI_SPIRAL_FORWARD_SPEED,
            vertical_velocity: 0.0,
            orientation_delta: 0.0,
            blade_flew_off: false,
            done: false,
            attach_particle: HELI_ATTACH_PARTICLE.to_string(),
            attach_particle_bone: String::new(),
            attach_particle_loc: [0.0, 0.0, 0.0],
            pending_attach: false,
            attach_system_id: None,
            fx: HostHelicopterSlowDeathFx::default(),
            pending_fx: None,
            pending_ocl: None,
            pending_audio: None,
            pending_stop_loop: false,
            pending_rubble: None,
            pending_held: false,
            pending_special_damaged: false,
        }
    }
}

impl HostHelicopterSlowDeathData {
    pub fn with_fx(fx: HostHelicopterSlowDeathFx) -> Self {
        Self {
            fx,
            ..Default::default()
        }
    }

    pub fn begin(&mut self) {
        self.begin_at_frame(0);
    }

    pub fn begin_at_frame(&mut self, frame: u32) {
        if self.active || self.done {
            return;
        }
        self.active = true;
        self.hit_ground = false;
        self.activate_frame = frame;
        self.vertical_velocity = 0.0;
        self.forward_speed = HELI_SPIRAL_FORWARD_SPEED;
        self.self_spin = HELI_MIN_SELF_SPIN;
        self.self_spin_dir = 1.0;
        self.frames_since_spin_update = 0;
        self.orientation_delta = 0.0;
        self.blade_flew_off = false;
        // C++ HelicopterSlowDeathUpdate.cpp:215-250 create + attachToObject.
        if self.attach_particle.is_empty() {
            self.attach_particle = HELI_ATTACH_PARTICLE.to_string();
        }
        self.pending_attach = !self.attach_particle.is_empty();
        // C++ :175-181 SoundDeathLoop.
        self.pending_audio = self.fx.death_loop_sound.clone();
    }

    pub fn take_pending_attach_particle(&mut self) -> Option<String> {
        if !self.pending_attach {
            return None;
        }
        self.pending_attach = false;
        let name = self.attach_particle.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(name.to_string())
        }
    }

    pub fn take_pending_effects(&mut self) -> HostHeliDeathPhaseEvent {
        HostHeliDeathPhaseEvent {
            fx: self.pending_fx.take(),
            ocl: self.pending_ocl.take(),
            audio: self.pending_audio.take(),
            stop_loop: std::mem::take(&mut self.pending_stop_loop),
            rubble: self.pending_rubble.take(),
            held: std::mem::take(&mut self.pending_held),
            special_damaged: std::mem::take(&mut self.pending_special_damaged),
        }
    }

    /// C++ createParticleSystem + attachToObject. Uses public spawn + template stamp.
    pub fn spawn_attach_particle(
        &mut self,
        registry: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
        position: glam::Vec3,
        frame: u32,
        owner: crate::game_logic::ObjectId,
    ) -> Option<u32> {
        let name = self.take_pending_attach_particle()?;
        let loc = glam::Vec3::new(
            position.x + self.attach_particle_loc[0],
            position.y + self.attach_particle_loc[1],
            position.z + self.attach_particle_loc[2],
        );
        let id = registry.spawn(
            crate::game_logic::combat_particles::CombatParticleKind::DeathSmoke,
            loc,
            frame,
            Some(owner),
            None,
        );
        if let Some(entry) = registry.get_mut(id) {
            entry.template_name = name;
        }
        self.attach_system_id = Some(id);
        Some(id)
    }

    pub fn sync_attach_particle_position(
        &self,
        registry: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
        position: glam::Vec3,
    ) {
        let Some(id) = self.attach_system_id else {
            return;
        };
        if let Some(entry) = registry.get_mut(id) {
            if entry.active {
                entry.position = glam::Vec3::new(
                    position.x + self.attach_particle_loc[0],
                    position.y + self.attach_particle_loc[1],
                    position.z + self.attach_particle_loc[2],
                );
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active && !self.done
    }

    fn delay_from_ground_frames(&self) -> f32 {
        if self.fx.delay_from_ground_frames > 0.0 {
            self.fx.delay_from_ground_frames
        } else {
            HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES as f32
        }
    }

    /// Tick crash. Returns (dx, dy, dz, d_orient, should_destroy, blade_fly_off_event).
    pub fn tick(
        &mut self,
        current_frame: u32,
        height_above_terrain: f32,
    ) -> (f32, f32, f32, f32, bool, bool) {
        if !self.active || self.done {
            return (0.0, 0.0, 0.0, 0.0, false, false);
        }

        let mut blade_event = false;
        if !self.blade_flew_off
            && current_frame.saturating_sub(self.activate_frame) >= HELI_BLADE_FLY_OFF_FRAMES
        {
            self.blade_flew_off = true;
            blade_event = true;
            // C++ :381-382 FXBlade + OCLBlade at blade bone (center residual).
            if self.pending_fx.is_none() {
                self.pending_fx = self.fx.fx_blade.clone();
            }
            if self.pending_ocl.is_none() {
                self.pending_ocl = self.fx.ocl_blade.clone();
            }
        }

        // Self-spin update residual (oscillate between min/max).
        self.frames_since_spin_update = self.frames_since_spin_update.saturating_add(1);
        if self.frames_since_spin_update >= HELI_SELF_SPIN_UPDATE_DELAY_FRAMES {
            self.frames_since_spin_update = 0;
            self.self_spin += self.self_spin_dir * HELI_SELF_SPIN_UPDATE_AMOUNT;
            if self.self_spin >= HELI_MAX_SELF_SPIN {
                self.self_spin = HELI_MAX_SELF_SPIN;
                self.self_spin_dir = -1.0;
            } else if self.self_spin <= HELI_MIN_SELF_SPIN {
                self.self_spin = HELI_MIN_SELF_SPIN;
                self.self_spin_dir = 1.0;
            }
        }

        if !self.hit_ground {
            self.orbit_angle += HELI_SPIRAL_TURN_RATE;
            let d_orient = self.self_spin + HELI_SPIRAL_TURN_RATE;
            self.orientation_delta += d_orient;
            let dx = self.orbit_angle.cos() * self.forward_speed;
            let dz = self.orbit_angle.sin() * self.forward_speed;
            self.forward_speed *= HELI_SPIRAL_FORWARD_SPEED_DAMPING;
            self.vertical_velocity += HELI_CRASH_GRAVITY;
            let dy = self.vertical_velocity;
            if height_above_terrain + dy <= 0.5 {
                self.hit_ground = true;
                self.hit_ground_frame = current_frame;
                self.vertical_velocity = 0.0;
                // C++ :435-447 FXHitGround + OCL + HELD + SPECIAL_DAMAGED + stop loop.
                self.pending_fx = self.fx.fx_hit_ground.clone();
                self.pending_ocl = self.fx.ocl_hit_ground.clone();
                self.pending_stop_loop = true;
                self.pending_held = true;
                self.pending_special_damaged = true;
                return (
                    dx,
                    -height_above_terrain.max(0.0),
                    dz,
                    d_orient,
                    false,
                    blade_event,
                );
            }
            return (dx, dy, dz, d_orient, false, blade_event);
        }

        // C++ :453-454 `getFrame() - m_hitGroundFrame > m_delayFromGroundToFinalDeath`.
        let elapsed = current_frame.saturating_sub(self.hit_ground_frame) as f32;
        if elapsed > self.delay_from_ground_frames() {
            self.done = true;
            self.active = false;
            self.pending_fx = self.fx.fx_final_blow_up.clone();
            self.pending_ocl = self.fx.ocl_final_blow_up.clone();
            self.pending_rubble = self.fx.final_rubble_object.clone();
            return (0.0, 0.0, 0.0, 0.0, true, blade_event);
        }
        (0.0, 0.0, 0.0, 0.0, false, blade_event)
    }
}

/// Alias: C++ source file name residual for port matrix matching.
pub type HelicopterSlowDeathUpdateData = HostHelicopterSlowDeathData;

pub fn is_helicopter_slow_death_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("comanche")
        || n.contains("chinook")
        || n.contains("helicopter")
        || n.contains("combatcopter")
        || (n.contains("helix") && !n.contains("napalm") && !n.contains("nuke"))
}

fn opt_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(t.to_string())
    }
}

fn parse_duration_frames(raw: &str) -> Option<f32> {
    let t = raw.trim().trim_end_matches("ms").trim();
    t.parse::<f32>().ok().map(heli_ms_to_duration_frames)
}

pub fn heli_slow_death_fx_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostHelicopterSlowDeathFx {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    HostHelicopterSlowDeathFx {
        fx_hit_ground: get("FXHitGround").and_then(opt_name),
        ocl_hit_ground: get("OCLHitGround").and_then(opt_name),
        fx_final_blow_up: get("FXFinalBlowUp").and_then(opt_name),
        ocl_final_blow_up: get("OCLFinalBlowUp").and_then(opt_name),
        fx_blade: get("FXBlade").and_then(opt_name),
        ocl_blade: get("OCLBlade").and_then(opt_name),
        death_loop_sound: get("SoundDeathLoop").and_then(opt_name),
        delay_from_ground_frames: get("DelayFromGroundToFinalDeath")
            .and_then(parse_duration_frames)
            .unwrap_or(HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES as f32),
        final_rubble_object: get("FinalRubbleObject").and_then(opt_name),
    }
}

fn authored_heli_fx(name: &str) -> Option<HostHelicopterSlowDeathFx> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    for module in &definition.behavior_modules {
        if !module
            .class_name
            .eq_ignore_ascii_case("HelicopterSlowDeathBehavior")
        {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        return Some(heli_slow_death_fx_from_behavior_attrs(&attrs));
    }
    None
}

fn default_comanche_fx() -> HostHelicopterSlowDeathFx {
    HostHelicopterSlowDeathFx {
        fx_hit_ground: Some(HELI_FX_HIT_GROUND.into()),
        ocl_hit_ground: Some(HELI_OCL_HIT_GROUND.into()),
        fx_final_blow_up: Some(HELI_FX_FINAL_BLOW_UP.into()),
        ocl_final_blow_up: Some(HELI_OCL_FINAL_BLOW_UP.into()),
        fx_blade: Some(HELI_FX_BLADE.into()),
        ocl_blade: Some(HELI_OCL_BLADE.into()),
        death_loop_sound: Some(HELI_SOUND_DEATH_LOOP.into()),
        delay_from_ground_frames: HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES as f32,
        final_rubble_object: Some(HELI_FINAL_RUBBLE_OBJECT.into()),
    }
}

pub fn heli_slow_death_fx_for_template(name: &str) -> HostHelicopterSlowDeathFx {
    if let Some(authored) = authored_heli_fx(name) {
        return authored;
    }
    let n = name.to_ascii_lowercase();
    if n.contains("helix") && !n.contains("napalm") && !n.contains("nuke") {
        return HostHelicopterSlowDeathFx {
            fx_hit_ground: Some(HELI_FX_HIT_GROUND.into()),
            ocl_hit_ground: Some(HELI_OCL_HIT_GROUND.into()),
            fx_final_blow_up: Some(HELIX_FX_FINAL_BLOW_UP.into()),
            ocl_final_blow_up: Some(HELIX_OCL_FINAL_BLOW_UP.into()),
            fx_blade: Some(HELI_FX_BLADE.into()),
            ocl_blade: Some(HELI_OCL_BLADE.into()),
            death_loop_sound: Some(HELIX_SOUND_DEATH_LOOP.into()),
            delay_from_ground_frames: heli_ms_to_duration_frames(HELI_SHORT_GROUND_DELAY_MS),
            final_rubble_object: Some(HELIX_FINAL_RUBBLE_OBJECT.into()),
        };
    }
    if n.contains("chinook") {
        let mut fx = default_comanche_fx();
        fx.delay_from_ground_frames = heli_ms_to_duration_frames(HELI_SHORT_GROUND_DELAY_MS);
        fx.final_rubble_object = Some(CHINOOK_FINAL_RUBBLE_OBJECT.into());
        return fx;
    }
    default_comanche_fx()
}

pub fn honesty_helicopter_slow_death_update_residual_ok() -> bool {
    (HELI_SPIRAL_TURN_RATE - heli_deg_per_sec_to_rad_per_frame(140.0)).abs() < 1.0e-5
        && (HELI_SPIRAL_FORWARD_SPEED - heli_velocity_per_sec_to_per_frame(350.0)).abs() < 1.0e-5
        && (HELI_SPIRAL_FORWARD_SPEED_DAMPING - 0.9999).abs() < 1.0e-6
        && HELI_BLADE_FLY_OFF_FRAMES == heli_ms_to_frames(1500)
        && HELI_SELF_SPIN_UPDATE_DELAY_FRAMES == heli_ms_to_frames(100)
        && HELI_SOUND_DEATH_LOOP == "ComancheDamagedLoop"
        && HELI_ATTACH_PARTICLE == "SootySmokeTrail"
        && HELI_FX_HIT_GROUND == "FX_HelicopterHitGround"
        && HELI_OCL_HIT_GROUND == "OCL_HelicopterHitGround"
        && HELI_FX_FINAL_BLOW_UP == "FX_GroundedHelicopterBlowUp"
        && HELI_OCL_FINAL_BLOW_UP == "OCL_GroundedHelicopterBlowUp"
        && HELI_FINAL_RUBBLE_OBJECT == "ComancheRubbleHull"
        && HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_MS == 1500
        && HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES == heli_ms_to_frames(1500)
        && HELI_GROUND_SETTLE_FRAMES == HELI_DELAY_FROM_GROUND_TO_FINAL_DEATH_FRAMES
        && (heli_ms_to_duration_frames(1500.0) - 45.0).abs() < 1.0e-5
        && is_helicopter_slow_death_template("AmericaHelicopterComanche")
        && !is_helicopter_slow_death_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_crash() {
        assert!(honesty_helicopter_slow_death_update_residual_ok());
        let mut h = HostHelicopterSlowDeathData::with_fx(default_comanche_fx());
        h.begin_at_frame(0);
        let ev0 = h.take_pending_effects();
        assert_eq!(ev0.audio.as_deref(), Some(HELI_SOUND_DEATH_LOOP));
        let mut height = 40.0;
        let mut destroyed = false;
        let mut blade = false;
        let mut hit_fx = false;
        let mut final_fx = false;
        for f in 0..800 {
            let (dx, dy, dz, _, done, blade_ev) = h.tick(f, height);
            height = (height + dy).max(0.0);
            let _ = (dx, dz);
            if blade_ev {
                blade = true;
            }
            let ev = h.take_pending_effects();
            if ev.fx.as_deref() == Some(HELI_FX_HIT_GROUND) {
                hit_fx = true;
                assert!(ev.stop_loop);
                assert!(ev.held);
            }
            if ev.fx.as_deref() == Some(HELI_FX_FINAL_BLOW_UP) {
                final_fx = true;
                assert_eq!(ev.rubble.as_deref(), Some(HELI_FINAL_RUBBLE_OBJECT));
            }
            if done {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed);
        assert!(h.hit_ground);
        assert!(blade || h.blade_flew_off);
        assert!(hit_fx);
        assert!(final_fx);
    }

    #[test]
    fn self_spin_stays_in_band() {
        let mut h = HostHelicopterSlowDeathData::default();
        h.begin();
        for f in 0..100 {
            let _ = h.tick(f, 100.0);
            assert!(h.self_spin >= HELI_MIN_SELF_SPIN - 1e-4);
            assert!(h.self_spin <= HELI_MAX_SELF_SPIN + 1e-4);
        }
    }

    #[test]
    fn begin_queues_sooty_smoke_trail() {
        let mut h = HostHelicopterSlowDeathData::default();
        h.begin_at_frame(0);
        assert_eq!(
            h.take_pending_attach_particle().as_deref(),
            Some(HELI_ATTACH_PARTICLE)
        );
        assert!(h.take_pending_attach_particle().is_none());
    }
}
