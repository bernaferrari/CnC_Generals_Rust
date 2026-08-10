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

    /// Flags
    flags: u32,
}

// Locomotor flags - Matches C++ Locomotor.h:395-407
const FLAG_IS_BRAKING: u32 = 0x01;
const FLAG_ALLOW_INVALID_POS: u32 = 0x02;
const FLAG_MAINTAIN_POS_VALID: u32 = 0x04;
const FLAG_PRECISE_Z_POS: u32 = 0x08;
const FLAG_NO_SLOW_DOWN: u32 = 0x10;
const FLAG_ULTRA_ACCURATE: u32 = 0x20;
const FLAG_CLOSE_ENOUGH_3D: u32 = 0x40;
const FLAG_MOVING_BACKWARDS: u32 = 0x80;
const FLAG_DOING_THREE_POINT_TURN: u32 = 0x100;
const FLAG_CLIMBING: u32 = 0x200;
const FLAG_OVER_WATER: u32 = 0x400;
const FLAG_OFFSET_INCREASING: u32 = 0x800;
const FLAG_SLIDING_INTO_PLACE: u32 = 0x1000;

impl Locomotor {
    /// Create new locomotor from template
    /// Matches C++ Locomotor.cpp:629-651
    pub fn new(template: Arc<LocomotorTemplate>) -> Self {
        // Random initial wander offset (C++ lines 647-649)
        let angle_offset = get_game_logic_random_value_real(
            -std::f32::consts::PI / 6.0,
            std::f32::consts::PI / 6.0,
        );
        let offset_increment = (std::f32::consts::PI / 40.0)
            * (get_game_logic_random_value_real(0.8, 1.2) / template.wander_length_factor);
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
            flags: if template.is_close_enough_dist_3d {
                FLAG_CLOSE_ENOUGH_3D
                    | (if offset_increasing {
                        FLAG_OFFSET_INCREASING
                    } else {
                        0
                    })
            } else {
                if offset_increasing {
                    FLAG_OFFSET_INCREASING
                } else {
                    0
                }
            },
            template,
        }
    }

    /// Get maximum speed for given damage condition
    pub fn get_max_speed_for_condition(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_speed,
            BodyDamageType::Damaged => self.template.max_speed_damaged,
            BodyDamageType::ReallyDamaged => self.template.max_speed_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get maximum turn rate for given damage condition
    pub fn get_max_turn_rate(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_turn_rate,
            BodyDamageType::Damaged => self.template.max_turn_rate_damaged,
            BodyDamageType::ReallyDamaged => self.template.max_turn_rate_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get maximum acceleration for given damage condition
    pub fn get_max_acceleration(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_accel,
            BodyDamageType::Damaged => self.template.acceleration_damaged,
            BodyDamageType::ReallyDamaged => self.template.acceleration_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get maximum lift for given damage condition
    pub fn get_max_lift(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_lift,
            BodyDamageType::Damaged => self.template.lift_damaged,
            BodyDamageType::ReallyDamaged => self.template.lift_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get braking
    pub fn get_braking(&self) -> Real {
        self.max_braking
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
