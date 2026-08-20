/// Locomotor instance - runtime state for a unit's locomotor
#[derive(Debug, Clone)]
pub struct Locomotor {
    /// Reference to template
    pub template: Arc<LocomotorTemplate>,

    /// Current maximum speed (can be modified by upgrades)
    max_speed: Real,
    /// Current maximum turn rate
    max_turn_rate: Real,
    /// Current maximum acceleration
    max_accel: Real,
    /// Current maximum lift
    max_lift: Real,
    /// Current maximum braking
    max_braking: Real,

    /// Current preferred height (can be modified)
    pub preferred_height: Real,
    /// Preferred height damping
    pub preferred_height_damping: Real,

    /// Close enough distance (can be modified)
    close_enough_dist: Real,

    /// Braking factor for smooth deceleration
    braking_factor: Real,

    /// Wander angle offset for infantry
    angle_offset: Real,
    /// Wander offset increment
    offset_increment: Real,

    /// Maintain position for hover/circle — Matches C++ Locomotor::m_maintainPos
    maintain_pos: Coord3D,

    /// Active path being followed
    pub active_path: Option<ActivePath>,

    /// Last obstacle detection time
    last_obstacle_check: u32,

    /// Donut timer frame for wheels braking near destination
    /// Matches C++ Locomotor::m_donutTimer
    donut_timer: u32,

    /// Last 3D motive acceleration (wu/frame^2) from appearance movers.
    last_motive_accel: Coord3D,

    /// Flags
    flags: u32,
}

// Locomotor flags — C++ LocoFlag (Locomotor.h:395-407), saved as 1<<enum.
const FLAG_IS_BRAKING: u32 = 1 << 0;
const FLAG_ALLOW_INVALID_POS: u32 = 1 << 1;
const FLAG_MAINTAIN_POS_VALID: u32 = 1 << 2;
const FLAG_PRECISE_Z_POS: u32 = 1 << 3;
const FLAG_NO_SLOW_DOWN: u32 = 1 << 4;
const FLAG_OVER_WATER: u32 = 1 << 5;
const FLAG_ULTRA_ACCURATE: u32 = 1 << 6;
const FLAG_MOVING_BACKWARDS: u32 = 1 << 7;
const FLAG_DOING_THREE_POINT_TURN: u32 = 1 << 8;
const FLAG_CLIMBING: u32 = 1 << 9;
const FLAG_CLOSE_ENOUGH_3D: u32 = 1 << 10;
const FLAG_OFFSET_INCREASING: u32 = 1 << 11;
/// Runtime-only (not in C++ LocoFlag / not in the saved word).
const FLAG_SLIDING_INTO_PLACE: u32 = 1 << 12;
const LOCO_FLAG_XFER_MASK: u32 = 0x0FFF;

const TURN_FACTOR_ULTRA_ACCURATE: Real = 2.0;

impl Locomotor {
    /// Create new locomotor from template
    /// Matches C++ Locomotor.cpp:629-651
    pub fn new(template: Arc<LocomotorTemplate>) -> Self {
        // Random initial wander offset (C++ lines 647-649)
        let angle_offset = get_game_logic_random_value_real(
            -std::f32::consts::PI / 6.0,
            std::f32::consts::PI / 6.0,
        );
        let wander_len = if template.wander_length_factor.abs() < 1.0e-6 {
            1.0
        } else {
            template.wander_length_factor
        };
        let offset_increment = (std::f32::consts::PI / 40.0)
            * (get_game_logic_random_value_real(0.8, 1.2) / wander_len);
        let offset_increasing = get_game_logic_random_value(0, 1) != 0;
        let donut_timer = TheGameLogic::get_frame()
            + (DONUT_TIME_DELAY_SECONDS * LOGICFRAMES_PER_SECOND as Real) as u32;

        Self {
            max_speed: template.max_speed,
            max_turn_rate: template.max_turn_rate,
            max_accel: template.acceleration,
            max_lift: template.lift,
            max_braking: template.braking,
            preferred_height: template.preferred_height,
            preferred_height_damping: template.preferred_height_damping,
            close_enough_dist: template.close_enough_dist,
            braking_factor: 1.0,
            angle_offset,
            offset_increment,
            maintain_pos: Coord3D::new(0.0, 0.0, 0.0),
            active_path: None,
            last_obstacle_check: 0,
            donut_timer,
            last_motive_accel: Coord3D::new(0.0, 0.0, 0.0),
            flags: if template.is_close_enough_dist_3d {
                FLAG_CLOSE_ENOUGH_3D
                    | (if offset_increasing {
                        FLAG_OFFSET_INCREASING
                    } else {
                        0
                    })
            } else if offset_increasing {
                FLAG_OFFSET_INCREASING
            } else {
                0
            },
            template,
        }
    }

    /// C++ `IS_CONDITION_BETTER(condition, TheGlobalData->m_movementPenaltyDamageState)`.
    fn uses_undamaged_loco_stats(&self, condition: BodyDamageType) -> bool {
        loco_is_condition_better(condition, movement_penalty_damage_state())
    }

    /// Get maximum speed for given damage condition
    /// Matches C++ Locomotor::getMaxSpeedForCondition (Locomotor.cpp:768-781)
    pub fn get_max_speed_for_condition(&self, condition: BodyDamageType) -> Real {
        let speed = if self.uses_undamaged_loco_stats(condition) {
            self.template.max_speed
        } else {
            heal_damaged_stat(self.template.max_speed_damaged, self.template.max_speed)
        };
        speed.min(self.max_speed)
    }

    /// Get maximum turn rate for given damage condition
    /// Matches C++ Locomotor::getMaxTurnRate (Locomotor.cpp:784-801)
    pub fn get_max_turn_rate(&self, condition: BodyDamageType) -> Real {
        let mut turn = if self.uses_undamaged_loco_stats(condition) {
            self.template.max_turn_rate
        } else {
            heal_damaged_stat(
                self.template.max_turn_rate_damaged,
                self.template.max_turn_rate,
            )
        };
        turn = turn.min(self.max_turn_rate);
        if self.is_ultra_accurate() {
            turn *= TURN_FACTOR_ULTRA_ACCURATE;
        }
        turn
    }

    /// Get maximum acceleration for given damage condition
    /// Matches C++ Locomotor::getMaxAcceleration (Locomotor.cpp:804-817)
    pub fn get_max_acceleration(&self, condition: BodyDamageType) -> Real {
        let accel = if self.uses_undamaged_loco_stats(condition) {
            self.template.acceleration
        } else {
            heal_damaged_stat(
                self.template.acceleration_damaged,
                self.template.acceleration,
            )
        };
        accel.min(self.max_accel)
    }

    /// Get maximum lift for given damage condition
    /// Matches C++ Locomotor::getMaxLift (Locomotor.cpp:831-844)
    pub fn get_max_lift(&self, condition: BodyDamageType) -> Real {
        let lift = if self.uses_undamaged_loco_stats(condition) {
            self.template.lift
        } else {
            heal_damaged_stat(self.template.lift_damaged, self.template.lift)
        };
        lift.min(self.max_lift)
    }

    pub fn is_over_water(&self) -> bool {
        self.get_flag(FLAG_OVER_WATER)
    }

    /// Get braking
    /// Matches C++ Locomotor::getBraking (Locomotor.cpp:820-828)
    pub fn get_braking(&self) -> Real {
        self.template.braking.min(self.max_braking)
    }

    /// Get appearance
    pub fn get_appearance(&self) -> LocomotorAppearance {
        self.template.appearance
    }

    /// Check if locomotor uses 3D close-enough distance.
    pub fn is_close_enough_dist_3d(&self) -> Bool {
        (self.flags & FLAG_CLOSE_ENOUGH_3D) != 0
    }

    /// Get legal surfaces
    pub fn get_legal_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.template.surfaces
    }

    /// Get template name
    pub fn get_template_name(&self) -> &str {
        &self.template.name
    }

    /// Calculate slow down distance needed to reach desired speed
    /// Matches C++ Locomotor.cpp:62-73 calcSlowDownDist
    fn calc_slow_down_dist(cur_speed: Real, desired_speed: Real, max_braking: Real) -> Real {
        let delta = cur_speed - desired_speed;
        if delta <= 0.0 {
            return 0.0;
        }

        let dist = (delta * delta / max_braking.abs()) * 0.5;

        // Use a little fudge so that things can stop "on a dime" more easily
        const FUDGE: Real = 1.05;
        dist * FUDGE
    }

}
