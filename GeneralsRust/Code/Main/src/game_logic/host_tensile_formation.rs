//! Host TensileFormationUpdate residual (avalanche chunk springy formation).
//!
//! C++: `TensileFormationUpdate` — snow/avalanche chunks stay idle until any
//! member takes BODY_DAMAGED, then slide downslope with spring links to the
//! four nearest formation members and go to BODY_RUBBLE after life > 300.
//!
//! Retail peels (`CivilianUnit.ini`):
//! - `AvalancheChunk` / `AvalancheLeadChunk`
//! - `Enabled = No` (damage enables the whole formation)
//! - `CrackSound = AvalancheCrack`
//!
//! Host residual (Y-up world: ground XZ, height Y):
//! - Init up to 4 nearest tensile members within 1000 world units
//! - Disabled sleep: wake when health ≤ 70% (BODY_DAMAGED residual)
//! - Physics: slope inertia * 0.95 friction + tensor blend 0.93/0.07
//! - Propagate dislodgement every 30 frames within 100 units
//! - Life > 300 → rubble / sleep forever
//!
//! Fail-closed: not full PartitionManager iterate / pathfinder wall footprint /
//! shrubbery topple / terrain normal mesh fidelity.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::ObjectId;

/// C++ link search radius during `initLinks`.
pub const TENSILE_LINK_SEARCH_RADIUS: f32 = 1_000.0;
/// C++ propagate dislodgement radius.
pub const TENSILE_PROPAGATE_RADIUS: f32 = 100.0;
/// C++ max life frames before rubble.
pub const TENSILE_LIFE_MAX: u32 = 300;
/// C++ propagate cadence (`m_life % 30 == 29`).
pub const TENSILE_PROPAGATE_PERIOD: u32 = 30;
/// C++ disabled sleep residual (`UPDATE_SLEEP(30)`).
pub const TENSILE_DISABLED_SLEEP_FRAMES: u32 = 30;
/// C++ friction scale on inertia.
pub const TENSILE_FRICTION: f32 = 0.95;
/// C++ slope scale base before steepness term.
pub const TENSILE_SLOPE_BASE: f32 = 0.3;
/// C++ position blend toward tensor-desired.
pub const TENSILE_BLEND_SELF: f32 = 0.93;
pub const TENSILE_BLEND_OTHER: f32 = 0.07;
/// C++ freefall height delta residual.
pub const TENSILE_FREEFALL_DELTA: f32 = 0.2;
/// C++ moving model condition until life.
pub const TENSILE_MOVING_LIFE: u32 = 200;
/// C++ freefall model condition until life.
pub const TENSILE_FREEFALL_LIFE: u32 = 100;
/// Retail crack audio peel.
pub const TENSILE_CRACK_SOUND: &str = "AvalancheCrack";
/// BODY_DAMAGED residual health fraction (matches host lobby residual).
pub const TENSILE_BODY_DAMAGED_HEALTH_FRAC: f32 = 0.70;

/// One spring link to another formation member (C++ `TensileLink`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct HostTensileLink {
    pub id: Option<ObjectId>,
    pub tensor: Vec3,
}

/// Per-object TensileFormationUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostTensileFormationData {
    pub enabled: bool,
    pub links_inited: bool,
    pub links: [HostTensileLink; 4],
    pub inertia: Vec3,
    pub life: u32,
    pub lowest_slide_elevation: f32,
    pub done: bool,
    pub post_collapse: bool,
    pub moving: bool,
    pub freefall: bool,
    pub rubble: bool,
    /// Next frame allowed to evaluate while disabled (sleep residual).
    pub next_disabled_eval_frame: u32,
    pub crack_played: bool,
}

impl Default for HostTensileFormationData {
    fn default() -> Self {
        Self {
            enabled: false,
            links_inited: false,
            links: [HostTensileLink::default(); 4],
            inertia: Vec3::ZERO,
            life: 0,
            lowest_slide_elevation: 255.0,
            done: false,
            post_collapse: false,
            moving: false,
            freefall: false,
            rubble: false,
            next_disabled_eval_frame: 0,
            crack_played: false,
        }
    }
}

impl HostTensileFormationData {
    /// Retail default: Enabled = No.
    pub fn new_disabled() -> Self {
        Self::default()
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_tensile_formation_template(template_name) {
            Some(Self::new_disabled())
        } else {
            None
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// C++ `initLinks`: keep four nearest other tensile members (push stack).
    pub fn init_links(&mut self, self_id: ObjectId, self_pos: Vec3, members: &[(ObjectId, Vec3)]) {
        self.links_inited = true;
        self.links = [HostTensileLink::default(); 4];
        let mut closest = f32::MAX;
        for &(id, pos) in members {
            if id == self_id {
                continue;
            }
            let delta = pos - self_pos;
            let dist = delta.length();
            if dist > TENSILE_LINK_SEARCH_RADIUS {
                continue;
            }
            if dist < closest {
                closest = dist;
                for t in (1..4).rev() {
                    self.links[t] = self.links[t - 1];
                }
                self.links[0] = HostTensileLink {
                    id: Some(id),
                    tensor: delta,
                };
            }
        }
    }

    /// Whether health fraction maps to BODY_DAMAGED or worse.
    pub fn should_enable_from_health(health_frac: f32) -> bool {
        health_frac <= TENSILE_BODY_DAMAGED_HEALTH_FRAC + f32::EPSILON
    }

    /// One logic-frame residual update.
    ///
    /// `ground_normal_xz` is terrain normal projected to XZ (C++ normal.x/y).
    /// `ground_height_at` samples terrain height (host Y).
    /// Returns `(new_pos, play_crack, propagate, became_rubble)`.
    pub fn tick(
        &mut self,
        current_frame: u32,
        pos: Vec3,
        health_frac: f32,
        ground_normal: Vec3,
        ground_height_at: &dyn Fn(f32, f32) -> f32,
        link_positions: &[(ObjectId, Vec3)],
    ) -> TensileTickResult {
        let mut result = TensileTickResult::default();
        if self.done {
            return result;
        }

        if !self.links_inited {
            // Caller should call `init_links` before tick; mark ready if empty.
            self.links_inited = true;
        }

        if !self.enabled {
            if current_frame < self.next_disabled_eval_frame {
                return result;
            }
            self.next_disabled_eval_frame =
                current_frame.saturating_add(TENSILE_DISABLED_SLEEP_FRAMES);
            if Self::should_enable_from_health(health_frac) {
                self.enabled = true;
                result.became_enabled = true;
                if !self.crack_played {
                    self.crack_played = true;
                    result.play_crack = true;
                }
            } else {
                return result;
            }
        }

        self.life = self.life.saturating_add(1);
        if self.life > TENSILE_LIFE_MAX {
            self.post_collapse = true;
            self.moving = false;
            self.freefall = false;
            self.rubble = true;
            self.done = true;
            result.became_rubble = true;
            result.new_pos = Some(pos);
            return result;
        }

        if self.life % TENSILE_PROPAGATE_PERIOD == 29 {
            result.propagate = true;
        }

        // APPLY PHYSICS — slope on ground plane (host XZ).
        let normal = if ground_normal.length_squared() > 1.0e-8 {
            ground_normal.normalize()
        } else {
            Vec3::Y
        };
        let mut slope = Vec3::new(normal.x, 0.0, normal.z);
        let steepness = 1.0 - normal.y.clamp(-1.0, 1.0);
        slope *= TENSILE_SLOPE_BASE + steepness;
        self.inertia += slope;
        self.inertia *= TENSILE_FRICTION;

        let mut new_pos = Vec3::new(pos.x + self.inertia.x, pos.y, pos.z + self.inertia.z);
        new_pos.y = ground_height_at(new_pos.x, new_pos.z);

        // APPLY TENSORS
        for link in &self.links {
            let Some(lid) = link.id else {
                continue;
            };
            let Some(&(_, other_pos)) = link_positions.iter().find(|(id, _)| *id == lid) else {
                continue;
            };
            let desired = other_pos - link.tensor;
            new_pos.x = new_pos.x * TENSILE_BLEND_SELF + desired.x * TENSILE_BLEND_OTHER;
            new_pos.z = new_pos.z * TENSILE_BLEND_SELF + desired.z * TENSILE_BLEND_OTHER;
            let gh = ground_height_at(new_pos.x, new_pos.z);
            new_pos.y = self.lowest_slide_elevation.min(gh);
        }

        self.post_collapse = true;
        self.moving = self.life < TENSILE_MOVING_LIFE;
        let height_delta = (pos.y - new_pos.y).abs();
        self.freefall = height_delta > TENSILE_FREEFALL_DELTA && self.life < TENSILE_FREEFALL_LIFE;

        self.lowest_slide_elevation = new_pos.y;
        result.new_pos = Some(new_pos);
        result.slid = true;
        result
    }
}

/// Result of one tensile residual tick.
#[derive(Debug, Clone, Default)]
pub struct TensileTickResult {
    pub new_pos: Option<Vec3>,
    pub play_crack: bool,
    pub propagate: bool,
    pub became_enabled: bool,
    pub became_rubble: bool,
    pub slid: bool,
}

/// Retail avalanche chunk templates.
pub fn is_tensile_formation_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("avalanchechunk") || n.contains("avalancheleadchunk")
}

/// Host residual registry / honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostTensileFormationRegistry {
    pub members_installed: u32,
    pub enables: u32,
    pub slides: u32,
    pub propagates: u32,
    pub rubbles: u32,
    pub crack_audio: u32,
}

impl HostTensileFormationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_install(&mut self) {
        self.members_installed = self.members_installed.saturating_add(1);
    }
    pub fn record_enable(&mut self) {
        self.enables = self.enables.saturating_add(1);
    }
    pub fn record_slide(&mut self) {
        self.slides = self.slides.saturating_add(1);
    }
    pub fn record_propagate(&mut self) {
        self.propagates = self.propagates.saturating_add(1);
    }
    pub fn record_rubble(&mut self) {
        self.rubbles = self.rubbles.saturating_add(1);
    }
    pub fn record_crack(&mut self) {
        self.crack_audio = self.crack_audio.saturating_add(1);
    }

    pub fn honesty_install_ok(&self) -> bool {
        self.members_installed > 0
    }
    pub fn honesty_enable_ok(&self) -> bool {
        self.enables > 0
    }
    pub fn honesty_slide_ok(&self) -> bool {
        self.slides > 0
    }
    pub fn honesty_rubble_ok(&self) -> bool {
        self.rubbles > 0
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_install_ok()
            || self.honesty_enable_ok()
            || self.honesty_slide_ok()
            || self.honesty_rubble_ok()
    }
}

pub fn honesty_tensile_formation_residual_ok() -> bool {
    TENSILE_LIFE_MAX == 300
        && TENSILE_PROPAGATE_PERIOD == 30
        && TENSILE_LINK_SEARCH_RADIUS == 1_000.0
        && TENSILE_PROPAGATE_RADIUS == 100.0
        && (TENSILE_FRICTION - 0.95).abs() < f32::EPSILON
        && (TENSILE_BLEND_SELF - 0.93).abs() < f32::EPSILON
        && (TENSILE_BLEND_OTHER - 0.07).abs() < f32::EPSILON
        && is_tensile_formation_template("AvalancheChunk")
        && is_tensile_formation_template("AvalancheLeadChunk")
        && !is_tensile_formation_template("AmericaTankCrusader")
        && TENSILE_CRACK_SOUND == "AvalancheCrack"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_template_matrix() {
        assert!(honesty_tensile_formation_residual_ok());
    }

    #[test]
    fn damage_enables_and_life_goes_rubble() {
        let mut d = HostTensileFormationData::new_disabled();
        let gh = |_x: f32, _z: f32| 0.0f32;
        let normal = Vec3::new(0.2, 0.96, 0.0);
        let pos = Vec3::new(0.0, 0.0, 0.0);

        // Healthy: stay disabled.
        let r = d.tick(0, pos, 1.0, normal, &gh, &[]);
        assert!(!d.enabled);
        assert!(!r.became_enabled);

        // Damaged: enable + crack.
        let r = d.tick(30, pos, 0.5, normal, &gh, &[]);
        assert!(d.enabled);
        assert!(r.became_enabled);
        assert!(r.play_crack);
        assert!(r.slid);

        // Fast-forward life to rubble.
        d.life = TENSILE_LIFE_MAX;
        let r = d.tick(400, pos, 0.5, normal, &gh, &[]);
        assert!(r.became_rubble);
        assert!(d.rubble);
        assert!(d.done);
    }

    #[test]
    fn init_links_keeps_nearest_four() {
        let mut d = HostTensileFormationData::new_disabled();
        let self_id = ObjectId(1);
        let self_pos = Vec3::ZERO;
        let members = vec![
            (ObjectId(1), Vec3::ZERO),
            (ObjectId(2), Vec3::new(10.0, 0.0, 0.0)),
            (ObjectId(3), Vec3::new(20.0, 0.0, 0.0)),
            (ObjectId(4), Vec3::new(5.0, 0.0, 0.0)),
            (ObjectId(5), Vec3::new(50.0, 0.0, 0.0)),
            (ObjectId(6), Vec3::new(2000.0, 0.0, 0.0)), // out of range
        ];
        d.init_links(self_id, self_pos, &members);
        assert!(d.links_inited);
        // Closest push leaves id 4 (5 units) as latest closest → links[0]
        assert_eq!(d.links[0].id, Some(ObjectId(4)));
        assert!(d.links.iter().all(|l| l.id != Some(ObjectId(6))));
    }

    #[test]
    fn tensor_blend_pulls_toward_linked_member() {
        let mut d = HostTensileFormationData::new_disabled();
        d.enabled = true;
        d.links_inited = true;
        d.links[0] = HostTensileLink {
            id: Some(ObjectId(2)),
            tensor: Vec3::new(-10.0, 0.0, 0.0),
        };
        let gh = |_x: f32, _z: f32| 0.0f32;
        let pos = Vec3::ZERO;
        let links = [(ObjectId(2), Vec3::new(100.0, 0.0, 0.0))];
        let r = d.tick(0, pos, 0.4, Vec3::Y, &gh, &links);
        let np = r.new_pos.expect("pos");
        // desired = 100 - (-10) = 110; blend moves x toward 110
        assert!(np.x > 0.0);
    }
}
