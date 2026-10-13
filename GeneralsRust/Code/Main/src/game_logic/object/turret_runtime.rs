//! TurretAI runtime owned by one canonical Main Object.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum HostTurretTargetKind {
    #[default]
    None,
    Object,
    Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct HostTurretExtra {
    pub(crate) target_pos: Option<glam::Vec3>,
    #[serde(skip)]
    pub(crate) victim_team: Option<super::super::Team>,
    pub(crate) enable_sweep_until: u32,
    pub(crate) positive_sweep: bool,
    #[serde(skip)]
    pub(crate) fire_angle_sweep: [f32; 3],
    #[serde(skip)]
    pub(crate) sweep_speed_mod: [f32; 3],
    #[serde(skip)]
    pub(crate) allows_pitch: bool,
    #[serde(skip)]
    pub(crate) fire_pitch: f32,
    #[serde(skip)]
    pub(crate) min_pitch: f32,
    #[serde(skip)]
    pub(crate) ground_unit_pitch: f32,
    #[serde(skip)]
    pub(crate) pitch_rate: f32,
    #[serde(skip)]
    pub(crate) min_idle_scan_angle: f32,
    #[serde(skip)]
    pub(crate) max_idle_scan_angle: f32,
    #[serde(skip)]
    pub(crate) min_idle_scan_interval: u32,
    #[serde(skip)]
    pub(crate) max_idle_scan_interval: u32,
    pub(crate) play_rot_sound: bool,
    pub(crate) play_pitch_sound: bool,
    pub(crate) did_fire: bool,
    // Receipt state refers to native delivery, not a C++ saved sound handle.
    #[serde(skip)]
    pub(crate) move_loop_playing: bool,
    pub(crate) idle_scan_desired_rad: f32,
    pub(crate) idle_scan_entered: bool,
    pub(crate) targeter_adds: u32,
    pub(crate) preventing_aim: bool,
    #[serde(skip)]
    pub(crate) fires_while_turning: bool,
    pub(crate) sweep_sound_until: u32,
    #[serde(skip)]
    pub(crate) seeded: bool,
    #[serde(skip)]
    pub(crate) has_authored_turret: bool,
    #[serde(skip)]
    pub(crate) definition_overridden: bool,
}

impl Default for HostTurretExtra {
    fn default() -> Self {
        Self {
            target_pos: None,
            victim_team: None,
            enable_sweep_until: 0,
            positive_sweep: true,
            fire_angle_sweep: [0.0; 3],
            sweep_speed_mod: [1.0; 3],
            allows_pitch: false,
            fire_pitch: 0.0,
            min_pitch: 0.0,
            ground_unit_pitch: 0.0,
            pitch_rate: crate::game_logic::object::default_turret_turn_rate(),
            min_idle_scan_angle: 0.0,
            max_idle_scan_angle: 0.0,
            min_idle_scan_interval: 9_999_999,
            max_idle_scan_interval: 9_999_999,
            play_rot_sound: false,
            play_pitch_sound: false,
            did_fire: false,
            move_loop_playing: false,
            idle_scan_desired_rad: 0.0,
            idle_scan_entered: false,
            targeter_adds: 0,
            preventing_aim: false,
            fires_while_turning: false,
            sweep_sound_until: 0,
            seeded: false,
            has_authored_turret: false,
            definition_overridden: false,
        }
    }
}

impl HostTurretExtra {
    pub(crate) fn from_spawn(spec: &super::TurretSpawnSpec) -> Self {
        let mut state = Self::default();
        state.has_authored_turret = spec.has_turret;
        if spec.has_turret {
            state.pitch_rate = spec.pitch_rate_rad;
            state.allows_pitch = spec.allows_pitch;
            state.fire_pitch = spec.fire_pitch_rad;
            state.min_pitch = spec.min_pitch_rad;
            state.ground_unit_pitch = spec.ground_unit_pitch_rad;
            state.min_idle_scan_angle = spec.min_idle_scan_angle_rad;
            state.max_idle_scan_angle = spec.max_idle_scan_angle_rad;
            state.min_idle_scan_interval = spec.min_idle_scan_interval;
            state.max_idle_scan_interval = spec.max_idle_scan_interval;
            state.fire_angle_sweep = spec.fire_angle_sweep;
            state.sweep_speed_mod = spec.sweep_speed_mod;
            state.fires_while_turning = spec.fires_while_turning;
        }
        state
    }
}

pub(crate) fn seed_host_turret_extra(e: &mut HostTurretExtra, turn_rate: f32) {
    if e.seeded {
        return;
    }
    e.seeded = true;
    if e.has_authored_turret {
        e.pitch_rate = e.pitch_rate.max(turn_rate);
    } else {
        e.pitch_rate = turn_rate.max(crate::game_logic::object::default_turret_turn_rate());
    }
}

impl super::Object {
    pub(crate) fn turret_target_kind(&self) -> HostTurretTargetKind {
        if self.turret_target_id.is_some() {
            HostTurretTargetKind::Object
        } else if self.turret_runtime.target_pos.is_some() {
            HostTurretTargetKind::Position
        } else {
            HostTurretTargetKind::None
        }
    }
}
