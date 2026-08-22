//! Host StructureToppleUpdate residual (buildings fall after HP death).
//!
//! C++: `StructureToppleUpdate::onDie` → `beginStructureTopple` → delayed fall
//! with structural integrity decay, crushing sweep, then done/rubble.
//!
//! Residual playability slice:
//! - States: Standing → WaitingForStart → Toppling → WaitingForDone → Done
//! - Delay frames before fall (default min/max 0 → immediate start residual)
//! - Angular accumulation to π/2 with accel factor 0.02
//! - Presentation lean via `lean_radians` (shared with tree topple presentation)
//! - On done: mark destroyed + DEATH_TOPPLED (rubble phase residual)
//!
//! - Crush sweep: authored CrushingWeaponName (`ToppledStructureWeapon`) at
//!   25/25 spacing (C++ `doDamageLine`), plus CrushingFX at each fire point
//!
//! Fail-closed:
//! - Not full OCL rubble / BoneFX / DieMux death-type filters

use serde::{Deserialize, Serialize};

/// C++ TOPPLE_ACCELERATION_FACTOR
pub const STRUCTURE_TOPPLE_ACCEL_FACTOR: f32 = 0.02;
/// Default structural integrity residual (INI StructuralIntegrity).
pub const STRUCTURE_TOPPLE_INTEGRITY_DEFAULT: f32 = 0.5;
/// Default structural decay residual per frame (INI StructuralDecay).
pub const STRUCTURE_TOPPLE_DECAY_DEFAULT: f32 = 0.1;
/// Default min/max topple delay frames when unset.
pub const STRUCTURE_TOPPLE_DELAY_MIN: u32 = 0;
pub const STRUCTURE_TOPPLE_DELAY_MAX: u32 = 0;
/// Waiting-for-done frames residual (brief settle).
pub const STRUCTURE_TOPPLE_DONE_DELAY_FRAMES: u32 = 1;
/// C++ THETA_CEILING — crush only when remaining angle to ground ≤ this.
pub const STRUCTURE_TOPPLE_THETA_CEILING: f32 = std::f32::consts::PI / 6.0;
/// C++ WEAPON_SPACING_PERPENDICULAR residual (along fall).
pub const STRUCTURE_TOPPLE_WEAPON_SPACING: f32 = 25.0;
/// C++ WEAPON_SPACING_PARALLEL residual (across facing).
pub const STRUCTURE_TOPPLE_WEAPON_SPACING_PARALLEL: f32 = 25.0;
/// Retail Object INI `CrushingWeaponName` residual.
pub const STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME: &str = "ToppledStructureWeapon";
/// Retail Object INI `CrushingFX` residual.
pub const STRUCTURE_TOPPLE_CRUSHING_FX: &str = "FX_DefaultStructureCrushing";
/// Retail `Weapon ToppledStructureWeapon` PrimaryDamage.
pub const TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE: f32 = 9999.0;
/// Retail `Weapon ToppledStructureWeapon` PrimaryDamageRadius.
pub const TOPPLED_STRUCTURE_WEAPON_PRIMARY_RADIUS: f32 = 20.0;
/// Default building height residual when geometry missing.
pub const STRUCTURE_TOPPLE_DEFAULT_HEIGHT: f32 = 40.0;
/// Default facing half-width residual.
pub const STRUCTURE_TOPPLE_DEFAULT_FACING_WIDTH: f32 = 20.0;

fn default_crushing_weapon_name() -> String {
    STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME.to_string()
}

fn default_crushing_fx() -> String {
    STRUCTURE_TOPPLE_CRUSHING_FX.to_string()
}

fn default_crush_damage() -> f32 {
    TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE
}

fn default_crush_radius() -> f32 {
    TOPPLED_STRUCTURE_WEAPON_PRIMARY_RADIUS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostStructureToppleState {
    #[default]
    Standing = 0,
    WaitingForStart = 1,
    Toppling = 2,
    WaitingForDone = 3,
    Done = 4,
}

/// Per-structure StructureToppleUpdate residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStructureToppleData {
    pub state: HostStructureToppleState,
    pub topple_start_frame: u32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub topple_velocity: f32,
    pub accumulated_angle: f32,
    pub structural_integrity: f32,
    pub structural_decay: f32,
    pub done_frame: u32,
    /// Presentation lean (radians) — mirrors tree topple lean field consumers.
    pub lean_radians: f32,
    /// C++ m_lastCrushedLocation residual (distance along fall already crushed).
    pub last_crushed_location: f32,
    /// Building height residual for crush projection.
    pub building_height: f32,
    /// Facing half-width residual for crush line.
    pub facing_width: f32,
    /// Authored CrushingWeaponName (empty → C++ `wt == NULL` no-op).
    #[serde(default = "default_crushing_weapon_name")]
    pub crushing_weapon_name: String,
    /// Authored CrushingFX list name.
    #[serde(default = "default_crushing_fx")]
    pub crushing_fx: String,
    /// Resolved CrushingWeaponName PrimaryDamage (0 → no fire).
    #[serde(default = "default_crush_damage")]
    pub crush_damage: f32,
    /// Resolved CrushingWeaponName PrimaryDamageRadius.
    #[serde(default = "default_crush_radius")]
    pub crush_radius: f32,
    /// Authored geometry major radius (0 → use `facing_width`).
    #[serde(default)]
    pub major_radius: f32,
    /// Authored geometry minor radius.
    #[serde(default)]
    pub minor_radius: f32,
    /// Building yaw used for C++ facingWidth projection.
    #[serde(default)]
    pub orientation: f32,
}

impl Default for HostStructureToppleData {
    fn default() -> Self {
        Self {
            state: HostStructureToppleState::Standing,
            topple_start_frame: 0,
            dir_x: 1.0,
            dir_y: 0.0,
            topple_velocity: 0.0,
            accumulated_angle: 0.0,
            structural_integrity: STRUCTURE_TOPPLE_INTEGRITY_DEFAULT,
            structural_decay: STRUCTURE_TOPPLE_DECAY_DEFAULT,
            done_frame: 0,
            lean_radians: 0.0,
            last_crushed_location: 0.0,
            building_height: STRUCTURE_TOPPLE_DEFAULT_HEIGHT,
            facing_width: STRUCTURE_TOPPLE_DEFAULT_FACING_WIDTH,
            crushing_weapon_name: default_crushing_weapon_name(),
            crushing_fx: default_crushing_fx(),
            crush_damage: TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE,
            crush_radius: TOPPLED_STRUCTURE_WEAPON_PRIMARY_RADIUS,
            major_radius: 0.0,
            minor_radius: 0.0,
            orientation: 0.0,
        }
    }
}

impl HostStructureToppleData {
    pub fn is_standing(&self) -> bool {
        self.state == HostStructureToppleState::Standing
    }

    pub fn is_active(&self) -> bool {
        !matches!(
            self.state,
            HostStructureToppleState::Standing | HostStructureToppleState::Done
        )
    }

    /// C++ beginStructureTopple residual.
    pub fn begin(&mut self, current_frame: u32, dir_x: f32, dir_y: f32, delay_frames: u32) {
        if !self.is_standing() {
            return;
        }
        let mut dx = dir_x;
        let mut dy = dir_y;
        let len = (dx * dx + dy * dy).sqrt();
        if len > 1e-6 {
            dx /= len;
            dy /= len;
        } else {
            dx = 1.0;
            dy = 0.0;
        }
        self.dir_x = dx;
        self.dir_y = dy;
        self.topple_start_frame = current_frame.saturating_add(delay_frames);
        self.topple_velocity = 0.0;
        self.accumulated_angle = 0.0;
        self.lean_radians = 0.0;
        self.last_crushed_location = 0.0;
        self.structural_integrity = STRUCTURE_TOPPLE_INTEGRITY_DEFAULT;
        self.state = HostStructureToppleState::WaitingForStart;
    }

    /// One logic frame. Returns true when topple completes (doToppleDoneStuff).
    pub fn tick(&mut self, current_frame: u32) -> bool {
        match self.state {
            HostStructureToppleState::Standing | HostStructureToppleState::Done => false,
            HostStructureToppleState::WaitingForStart => {
                if current_frame >= self.topple_start_frame {
                    self.state = HostStructureToppleState::Toppling;
                    self.structural_integrity = STRUCTURE_TOPPLE_INTEGRITY_DEFAULT;
                }
                false
            }
            HostStructureToppleState::Toppling => {
                let integrity_term = (1.0 - self.structural_integrity).max(0.0);
                let topple_acceleration =
                    STRUCTURE_TOPPLE_ACCEL_FACTOR * self.accumulated_angle.sin() * integrity_term;
                // C++ also accelerates from rest: give a small kick if still zero.
                let accel = if self.topple_velocity <= 1e-6 && self.accumulated_angle <= 1e-6 {
                    STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.05
                } else {
                    topple_acceleration.max(STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.01)
                };
                self.topple_velocity += accel;
                if self.structural_integrity > 0.0 {
                    self.structural_integrity *= self.structural_decay;
                    if self.structural_integrity < 0.0 {
                        self.structural_integrity = 0.0;
                    }
                }
                self.accumulated_angle += self.topple_velocity;
                self.lean_radians = self.accumulated_angle;
                if self.accumulated_angle >= std::f32::consts::FRAC_PI_2 {
                    self.accumulated_angle = std::f32::consts::FRAC_PI_2;
                    self.lean_radians = self.accumulated_angle;
                    self.state = HostStructureToppleState::WaitingForDone;
                    self.done_frame =
                        current_frame.saturating_add(STRUCTURE_TOPPLE_DONE_DELAY_FRAMES);
                }
                false
            }
            HostStructureToppleState::WaitingForDone => {
                if current_frame >= self.done_frame {
                    self.state = HostStructureToppleState::Done;
                    return true;
                }
                false
            }
        }
    }
}

/// Name/kind peel: structures that should structure-topple on death.
pub fn is_structure_topple_candidate(template_name: &str, is_structure: bool) -> bool {
    if !is_structure {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    // Skip pure base pads / holes / walls that may not topple in retail.
    if n.contains("rebuildhole")
        || n.contains("bunker") && n.contains("tunnel")
        || n.contains("supplydock")
        || n.contains("oil")
    {
        return false;
    }
    true
}

/// Authored StructureToppleUpdate CrushingWeaponName + CrushingFX from leftover ThingFactory.
#[derive(Debug, Clone, Default)]
pub struct AuthoredStructureTopplePeel {
    pub weapon: String,
    pub fx: String,
}

pub fn leftover_structure_topple_module_peel(
    template_name: &str,
) -> Option<AuthoredStructureTopplePeel> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("StructureToppleUpdate")
        {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::StructureToppleUpdateModuleData>(
            )
        {
            let weapon = data.crushing_weapon_name.as_str().trim().to_string();
            let fx = data
                .crushing_fx_list
                .as_ref()
                .map(|fx| fx.name().to_string())
                .unwrap_or_default();
            if weapon.is_empty() && fx.is_empty() {
                return None;
            }
            return Some(AuthoredStructureTopplePeel { weapon, fx });
        }
        let weapon = entry
            .data
            .get_ini_field("CrushingWeaponName")
            .unwrap_or("")
            .trim()
            .to_string();
        let fx = entry
            .data
            .get_ini_field("CrushingFX")
            .unwrap_or("")
            .trim()
            .to_string();
        if weapon.is_empty() && fx.is_empty() {
            return None;
        }
        return Some(AuthoredStructureTopplePeel { weapon, fx });
    }
    None
}


/// World-space crush sample from structure topple sweep.
#[derive(Debug, Clone, Copy)]
pub struct StructureToppleCrushSample {
    pub x: f32,
    pub z: f32,
    pub damage: f32,
    pub radius: f32,
}

/// Resolve CrushingWeaponName PrimaryDamage / PrimaryDamageRadius.
/// Store miss on the retail name uses Weapon.ini residuals; empty name is a no-op.
pub fn resolve_crushing_weapon(name: &str) -> (f32, f32) {
    if name.trim().is_empty() {
        return (0.0, 0.0);
    }
    let from_store = gamelogic::weapon::with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| (wt.primary_damage, wt.primary_damage_radius))
    })
    .ok()
    .flatten();
    if let Some((dmg, radius)) = from_store {
        return (dmg, radius.max(0.0));
    }
    if name.eq_ignore_ascii_case(STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME) {
        (
            TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE,
            TOPPLED_STRUCTURE_WEAPON_PRIMARY_RADIUS,
        )
    } else {
        (0.0, 0.0)
    }
}

impl HostStructureToppleData {
    /// Remaining angle to ground (C++ theta passed to applyCrushingDamage).
    pub fn remaining_theta(&self) -> f32 {
        (std::f32::consts::FRAC_PI_2 - self.accumulated_angle).max(0.0)
    }

    /// C++ facingWidth = length(major*sin(orient-topple), minor*cos) / 2.
    pub fn crush_facing_width(&self) -> f32 {
        if self.major_radius > 1e-6 || self.minor_radius > 1e-6 {
            gamelogic::object::behavior::leftover_structure_topple_facing_width(
                self.orientation,
                self.dir_y.atan2(self.dir_x),
                self.major_radius,
                self.minor_radius,
            )
        } else {
            self.facing_width
        }
    }

    /// Bind authored CrushingWeaponName (C++ findWeaponTemplate).
    pub fn bind_crushing_weapon(&mut self, name: &str) {
        self.crushing_weapon_name = name.to_string();
        let (dmg, radius) = resolve_crushing_weapon(name);
        self.crush_damage = dmg;
        self.crush_radius = radius;
        if self.crushing_fx.is_empty() {
            self.crushing_fx = default_crushing_fx();
        }
    }

    /// Stamp geometry used for height + facingWidth (C++ getMaxHeightAbovePosition).
    pub fn bind_geometry(
        &mut self,
        height: f32,
        major_radius: f32,
        minor_radius: f32,
        orientation: f32,
    ) {
        self.building_height = height;
        self.major_radius = major_radius.max(0.0);
        self.minor_radius = minor_radius.max(0.0);
        self.orientation = orientation;
        self.facing_width = self.crush_facing_width();
    }

    /// C++ applyCrushingDamage / doDamageLine via leftover 25/25 sweep.
    /// Updates `last_crushed_location`. Empty if theta above ceiling or no weapon.
    pub fn take_crush_sweep_samples(
        &mut self,
        building_x: f32,
        building_z: f32,
    ) -> Vec<StructureToppleCrushSample> {
        if self.crushing_weapon_name.trim().is_empty() || self.crush_damage <= 0.0 {
            return Vec::new();
        }
        let theta = self.remaining_theta();
        if theta > STRUCTURE_TOPPLE_THETA_CEILING
            && self.state == HostStructureToppleState::Toppling
        {
            return Vec::new();
        }
        let theta = if matches!(
            self.state,
            HostStructureToppleState::WaitingForDone | HostStructureToppleState::Done
        ) {
            0.0
        } else {
            theta
        };
        if self.state == HostStructureToppleState::Toppling
            && theta > STRUCTURE_TOPPLE_THETA_CEILING
        {
            return Vec::new();
        }

        let max_distance = gamelogic::object::behavior::leftover_structure_topple_max_crush_distance(
            self.building_height,
            theta,
        );
        let topple_angle = self.dir_y.atan2(self.dir_x);
        let facing = self.crush_facing_width();
        let (pts, new_last) = gamelogic::object::behavior::leftover_structure_topple_crush_points(
            building_x,
            building_z,
            self.last_crushed_location,
            max_distance,
            facing,
            topple_angle,
        );
        self.last_crushed_location = new_last;
        pts.into_iter()
            .map(|(x, z)| StructureToppleCrushSample {
                x,
                z,
                damage: self.crush_damage,
                radius: self.crush_radius,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crush_sweep_emits_near_ground() {
        let mut t = HostStructureToppleData::default();
        t.begin(0, 1.0, 0.0, 0);
        t.state = HostStructureToppleState::Toppling;
        t.accumulated_angle = 0.1;
        assert!(t.take_crush_sweep_samples(0.0, 0.0).is_empty());
        t.accumulated_angle = std::f32::consts::FRAC_PI_2 - 0.1;
        t.last_crushed_location = 0.0;
        let s = t.take_crush_sweep_samples(0.0, 0.0);
        assert!(!s.is_empty(), "expected crush samples near ground");
        assert!(s.iter().all(|p| {
            (p.damage - TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE).abs() < 1e-3
                && (p.radius - TOPPLED_STRUCTURE_WEAPON_PRIMARY_RADIUS).abs() < 1e-3
        }));
    }

    #[test]
    fn crush_sweep_uses_parallel_25_not_three_points() {
        let mut t = HostStructureToppleData::default();
        t.begin(0, 1.0, 0.0, 0);
        t.state = HostStructureToppleState::WaitingForDone;
        t.accumulated_angle = std::f32::consts::FRAC_PI_2;
        t.bind_geometry(50.0, 80.0, 10.0, 0.0);
        t.last_crushed_location = 0.0;
        let _ = t.take_crush_sweep_samples(0.0, 0.0);
        // facingWidth = length(80*sin(0-0), 10*cos) / 2 = 5, so 25-step still
        // two points per line. Widen via stored facing fallback:
        t.major_radius = 0.0;
        t.minor_radius = 0.0;
        t.facing_width = 60.0;
        t.last_crushed_location = 0.0;
        let s = t.take_crush_sweep_samples(0.0, 0.0);
        let first_line: Vec<_> = s
            .iter()
            .filter(|p| p.x.abs() < 1e-2)
            .map(|p| p.z)
            .collect();
        assert!(
            first_line.len() >= 4,
            "25-parallel across facing=60 must exceed 3-point line, got {:?}",
            first_line
        );
        assert!(s.iter().all(|p| p.damage < 99999.0));
        assert_eq!(t.crushing_weapon_name, STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME);
    }

    #[test]
    fn empty_crushing_weapon_name_is_noop() {
        let mut t = HostStructureToppleData::default();
        t.begin(0, 1.0, 0.0, 0);
        t.state = HostStructureToppleState::WaitingForDone;
        t.bind_crushing_weapon("");
        assert!(t.take_crush_sweep_samples(0.0, 0.0).is_empty());
    }

    #[test]
    fn structure_topple_reaches_done() {
        let mut t = HostStructureToppleData::default();
        t.begin(0, 1.0, 0.0, 0);
        assert_eq!(t.state, HostStructureToppleState::WaitingForStart);
        let mut done = false;
        for f in 0..600 {
            if t.tick(f) {
                done = true;
                break;
            }
        }
        assert!(done, "should complete topple");
        assert_eq!(t.state, HostStructureToppleState::Done);
        assert!((t.lean_radians - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }
}
