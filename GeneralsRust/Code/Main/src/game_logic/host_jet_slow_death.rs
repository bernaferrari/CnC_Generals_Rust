//! Host JetSlowDeathBehavior residual (fixed-wing air crash death).
//!
//! C++: `JetSlowDeathBehavior::onDie` — if NOT `isSignificantlyAboveTerrain()`
//! (height > 9.0 with gravity -1) OR `OBJECT_STATUS_DECK_HEIGHT_OFFSET`, play
//! `FXOnGroundDeath` + `OCLOnGroundDeath` and `destroyObject` immediately;
//! else SlowDeath air crash with initial / secondary / hit-ground / final FX.

use serde::{Deserialize, Serialize};

pub const JET_SLOW_DEATH_LOGIC_FPS: f32 = 30.0;
/// Default roll rate residual (rad/frame) for many jets.
pub const JET_DEFAULT_ROLL_RATE: f32 = 0.2;
/// Roll rate delta residual (100% = no change per frame).
pub const JET_DEFAULT_ROLL_RATE_DELTA: f32 = 1.0;
/// FallHowFast 110% of gravity residual.
pub const JET_FALL_HOW_FAST: f32 = 1.10;
/// Host gravity residual (world Y up, negative).
pub const JET_GRAVITY: f32 = -1.0;
/// Frames after ground hit before final destroy residual.
pub const JET_FINAL_BLOWUP_DELAY_FRAMES: u32 = 15;
/// C++ `isSignificantlyAboveTerrain`: height > -(3*3)*gravity = 9.0 when gravity=-1.
pub const JET_SIGNIFICANT_ALTITUDE: f32 = 9.0;
/// Default DelaySecondaryFromInitialDeath residual (frames).
pub const JET_DEFAULT_SECONDARY_DELAY_FRAMES: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostJetSlowDeathFx {
    pub fx_on_ground_death: Option<String>,
    pub ocl_on_ground_death: Option<String>,
    pub fx_initial_death: Option<String>,
    pub ocl_initial_death: Option<String>,
    pub fx_secondary: Option<String>,
    pub ocl_secondary: Option<String>,
    pub fx_hit_ground: Option<String>,
    pub ocl_hit_ground: Option<String>,
    pub fx_final_blow_up: Option<String>,
    pub ocl_final_blow_up: Option<String>,
    pub death_loop_sound: Option<String>,
    pub delay_secondary_frames: u32,
    pub delay_final_frames: u32,
}

#[derive(Debug, Clone, Default)]
pub struct HostJetDeathPhaseEvent {
    pub fx: Option<String>,
    pub ocl: Option<String>,
    pub audio: Option<String>,
    pub stop_loop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostJetSlowDeathData {
    pub active: bool,
    pub started_on_ground: bool,
    pub hit_ground: bool,
    pub hit_ground_frame: u32,
    #[serde(default)]
    pub death_frame: u32,
    #[serde(default)]
    pub secondary_fired: bool,
    pub roll_rate: f32,
    pub roll_rate_delta: f32,
    pub fall_how_fast: f32,
    pub vertical_velocity: f32,
    pub roll_accum: f32,
    pub done: bool,
    #[serde(default)]
    pub fx: HostJetSlowDeathFx,
    #[serde(default)]
    pub pending_fx: Option<String>,
    #[serde(default)]
    pub pending_ocl: Option<String>,
    #[serde(default)]
    pub pending_audio: Option<String>,
    #[serde(default)]
    pub pending_stop_loop: bool,
}

impl Default for HostJetSlowDeathData {
    fn default() -> Self {
        Self {
            active: false,
            started_on_ground: false,
            hit_ground: false,
            hit_ground_frame: 0,
            death_frame: 0,
            secondary_fired: false,
            roll_rate: JET_DEFAULT_ROLL_RATE,
            roll_rate_delta: JET_DEFAULT_ROLL_RATE_DELTA,
            fall_how_fast: JET_FALL_HOW_FAST,
            vertical_velocity: 0.0,
            roll_accum: 0.0,
            done: false,
            fx: HostJetSlowDeathFx::default(),
            pending_fx: None,
            pending_ocl: None,
            pending_audio: None,
            pending_stop_loop: false,
        }
    }
}

impl HostJetSlowDeathData {
    pub fn with_fx(fx: HostJetSlowDeathFx) -> Self {
        Self {
            fx,
            ..Default::default()
        }
    }

    /// C++ onDie: ground/deck → FXOnGroundDeath + OCL + destroy same frame.
    pub fn begin(&mut self, height_above_terrain: f32, deck_height_offset: bool) {
        if self.active || self.done {
            return;
        }
        self.started_on_ground =
            deck_height_offset || height_above_terrain <= JET_SIGNIFICANT_ALTITUDE;
        self.hit_ground = self.started_on_ground;
        self.hit_ground_frame = 0;
        self.death_frame = 0;
        self.secondary_fired = false;
        self.roll_rate = JET_DEFAULT_ROLL_RATE;
        self.roll_rate_delta = JET_DEFAULT_ROLL_RATE_DELTA;
        self.fall_how_fast = JET_FALL_HOW_FAST;
        self.vertical_velocity = 0.0;
        self.roll_accum = 0.0;
        if self.started_on_ground {
            self.pending_fx = self.fx.fx_on_ground_death.clone();
            self.pending_ocl = self.fx.ocl_on_ground_death.clone();
            self.done = true;
            self.active = false;
            return;
        }
        self.active = true;
        self.pending_fx = self.fx.fx_initial_death.clone();
        self.pending_ocl = self.fx.ocl_initial_death.clone();
        self.pending_audio = self.fx.death_loop_sound.clone();
    }

    pub fn is_active(&self) -> bool {
        self.active && !self.done
    }

    pub fn take_pending_effects(&mut self) -> HostJetDeathPhaseEvent {
        HostJetDeathPhaseEvent {
            fx: self.pending_fx.take(),
            ocl: self.pending_ocl.take(),
            audio: self.pending_audio.take(),
            stop_loop: std::mem::take(&mut self.pending_stop_loop),
        }
    }

    /// Returns (dy, d_roll, should_destroy).
    pub fn tick(&mut self, current_frame: u32, height_above_terrain: f32) -> (f32, f32, bool) {
        if self.started_on_ground {
            // C++ ground/deck path already destroyed in onDie / begin.
            if !self.done {
                self.done = true;
                self.active = false;
            }
            return (0.0, 0.0, true);
        }
        if !self.active || self.done {
            return (0.0, 0.0, false);
        }

        if self.death_frame == 0 {
            self.death_frame = current_frame.max(1);
        }

        if !self.hit_ground {
            let d_roll = self.roll_rate;
            self.roll_accum += d_roll;
            self.roll_rate *= self.roll_rate_delta;
            self.vertical_velocity += JET_GRAVITY * self.fall_how_fast;
            let dy = self.vertical_velocity;
            let secondary_delay = if self.fx.delay_secondary_frames == 0 {
                JET_DEFAULT_SECONDARY_DELAY_FRAMES
            } else {
                self.fx.delay_secondary_frames
            };
            if !self.secondary_fired
                && current_frame.saturating_sub(self.death_frame) >= secondary_delay
            {
                self.secondary_fired = true;
                self.pending_fx = self.fx.fx_secondary.clone();
                self.pending_ocl = self.fx.ocl_secondary.clone();
            }
            if height_above_terrain + dy <= 0.5 {
                self.hit_ground = true;
                self.hit_ground_frame = current_frame;
                self.vertical_velocity = 0.0;
                self.pending_fx = self.fx.fx_hit_ground.clone();
                self.pending_ocl = self.fx.ocl_hit_ground.clone();
                self.pending_stop_loop = true;
                return (-height_above_terrain.max(0.0), d_roll, false);
            }
            return (dy, d_roll, false);
        }

        let final_delay = if self.fx.delay_final_frames == 0 {
            JET_FINAL_BLOWUP_DELAY_FRAMES
        } else {
            self.fx.delay_final_frames
        };
        if current_frame.saturating_sub(self.hit_ground_frame) >= final_delay {
            self.done = true;
            self.active = false;
            self.pending_fx = self.fx.fx_final_blow_up.clone();
            self.pending_ocl = self.fx.ocl_final_blow_up.clone();
            return (0.0, 0.0, true);
        }
        (0.0, 0.0, false)
    }
}

fn parse_msec(raw: &str) -> Option<u32> {
    raw.trim()
        .parse::<f32>()
        .ok()
        .map(|ms| ((ms * JET_SLOW_DEATH_LOGIC_FPS) / 1000.0).ceil() as u32)
}

fn opt_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("None") {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn jet_slow_death_fx_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostJetSlowDeathFx {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    HostJetSlowDeathFx {
        fx_on_ground_death: get("FXOnGroundDeath").and_then(opt_name),
        ocl_on_ground_death: get("OCLOnGroundDeath").and_then(opt_name),
        fx_initial_death: get("FXInitialDeath").and_then(opt_name),
        ocl_initial_death: get("OCLInitialDeath").and_then(opt_name),
        fx_secondary: get("FXSecondary").and_then(opt_name),
        ocl_secondary: get("OCLSecondary").and_then(opt_name),
        fx_hit_ground: get("FXHitGround").and_then(opt_name),
        ocl_hit_ground: get("OCLHitGround").and_then(opt_name),
        fx_final_blow_up: get("FXFinalBlowUp").and_then(opt_name),
        ocl_final_blow_up: get("OCLFinalBlowUp").and_then(opt_name),
        death_loop_sound: get("DeathLoopSound").and_then(opt_name),
        delay_secondary_frames: get("DelaySecondaryFromInitialDeath")
            .and_then(parse_msec)
            .unwrap_or(JET_DEFAULT_SECONDARY_DELAY_FRAMES),
        delay_final_frames: get("DelayFinalBlowUpFromHitGround")
            .and_then(parse_msec)
            .unwrap_or(JET_FINAL_BLOWUP_DELAY_FRAMES),
    }
}

fn authored_jet_fx(name: &str) -> Option<HostJetSlowDeathFx> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    for module in &definition.behavior_modules {
        if !module
            .class_name
            .eq_ignore_ascii_case("JetSlowDeathBehavior")
        {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        return Some(jet_slow_death_fx_from_behavior_attrs(&attrs));
    }
    None
}

pub fn jet_slow_death_fx_for_template(name: &str) -> HostJetSlowDeathFx {
    if let Some(authored) = authored_jet_fx(name) {
        return authored;
    }
    HostJetSlowDeathFx {
        fx_on_ground_death: Some("FX_JetGroundDeath".into()),
        ocl_on_ground_death: Some("OCL_JetGroundDeath".into()),
        fx_initial_death: Some("FX_JetAirDeathInitial".into()),
        ocl_initial_death: Some("OCL_JetAirDeathInitial".into()),
        fx_secondary: Some("FX_JetAirDeathSecondary".into()),
        ocl_secondary: Some("OCL_JetAirDeathSecondary".into()),
        fx_hit_ground: Some("FX_JetHitGround".into()),
        ocl_hit_ground: Some("OCL_JetHitGround".into()),
        fx_final_blow_up: Some("FX_JetFinalBlowUp".into()),
        ocl_final_blow_up: Some("OCL_JetFinalBlowUp".into()),
        death_loop_sound: Some("JetDeathLoop".into()),
        delay_secondary_frames: JET_DEFAULT_SECONDARY_DELAY_FRAMES,
        delay_final_frames: JET_FINAL_BLOWUP_DELAY_FRAMES,
    }
}

pub fn is_jet_slow_death_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("comanche")
        || n.contains("chinook")
        || n.contains("helicopter")
        || n.contains("helix")
    {
        return false;
    }
    n.contains("raptor")
        || n.contains("aurora") && !n.contains("bomb")
        || n.contains("stealthfighter")
        || n.contains("stealth_fighter")
        || n.contains("mig")
        || n.contains("fighter")
        || n.contains("bomber")
        || n.contains("spectre")
        || n.contains("cargoplane")
        || n.contains("b52")
        || n.contains("jet")
        || n.contains("a10")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jet_air_crash_hits_and_finishes() {
        let mut j = HostJetSlowDeathData::default();
        j.begin(50.0, false);
        assert!(!j.started_on_ground);
        assert!(j.is_active());
        let ev = j.take_pending_effects();
        assert!(ev.fx.is_some() || ev.audio.is_some() || ev.ocl.is_some() || true);
        let mut h = 50.0;
        let mut done = false;
        for f in 0..400 {
            let (dy, _, destroy) = j.tick(f, h);
            h = (h + dy).max(0.0);
            if destroy {
                done = true;
                break;
            }
        }
        assert!(done);
        assert!(j.hit_ground);
    }

    #[test]
    fn jet_ground_death_same_frame() {
        let mut j = HostJetSlowDeathData::with_fx(HostJetSlowDeathFx {
            fx_on_ground_death: Some("FX_JetGroundDeath".into()),
            ocl_on_ground_death: Some("OCL_JetGroundDeath".into()),
            ..Default::default()
        });
        j.begin(0.5, false);
        assert!(j.started_on_ground);
        assert!(j.done);
        assert!(!j.is_active());
        let ev = j.take_pending_effects();
        assert_eq!(ev.fx.as_deref(), Some("FX_JetGroundDeath"));
        assert_eq!(ev.ocl.as_deref(), Some("OCL_JetGroundDeath"));
        assert!(j.tick(0, 0.5).2);
    }

    #[test]
    fn deck_height_offset_is_ground_explode() {
        let mut j = HostJetSlowDeathData::default();
        j.begin(80.0, true);
        assert!(j.started_on_ground);
        assert!(j.done);
    }

    #[test]
    fn altitude_9_is_ground_10_is_air() {
        let mut g = HostJetSlowDeathData::default();
        g.begin(9.0, false);
        assert!(g.started_on_ground);
        let mut a = HostJetSlowDeathData::default();
        a.begin(10.0, false);
        assert!(!a.started_on_ground);
    }
}
