//! Live bridge scaffold motion, tower links, damage state and timed effects.

use generals_game_domain::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identity assigned during map admission. Overflow spans share a layer, and
/// the reconstructed actor ID can differ from its ID in the save.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapBridgeSource {
    Drawable(i32),
    Landmark(usize),
}

impl MapBridgeSource {
    pub fn from_map_indices(drawable: i32, landmark: Option<usize>) -> Option<Self> {
        match (drawable, landmark) {
            (0.., None) => Some(Self::Drawable(drawable)),
            (-1, Some(index)) => Some(Self::Landmark(index)),
            _ => None,
        }
    }

    pub fn of(info: &crate::terrain::BridgeInfo) -> Option<Self> {
        Self::from_map_indices(info.bridge_index, info.source_map_object_index)
    }
}

/// C++ FRAMES while scaffolds rise (lateral then vertical). Host residual: 45.
pub const BRIDGE_SCAFFOLD_RISE_FRAMES: u32 = 45;

/// Retail scaffold object name (`BridgeScaffold01`).
pub const BRIDGE_SCAFFOLD_TEMPLATE: &str = "BridgeScaffold01";

/// C++ `HUGE_DAMAGE_AMOUNT` residual used for falling splat.
pub const BRIDGE_SPLAT_DAMAGE: f32 = 999999.0;

/// C++ scaffold tile spacing fallback when template radius is unknown.
pub const BRIDGE_SCAFFOLD_TILE_SPACING: f32 = 16.0;

/// Staged C++ `BridgeDieFX` delays (frames after `m_deathFrame`).
pub const BRIDGE_DIE_FX_DELAYS: &[u32] = &[0, 8, 16, 32];

/// Residual authored FX list name for staged bridge death.
pub const BRIDGE_DIE_FX_NAME: &str = "FX_GenericBridgeDie";
/// Retail support-layer scaffold (`BridgeScaffoldSupport01`).
pub const BRIDGE_SCAFFOLD_SUPPORT_TEMPLATE: &str = "BridgeScaffoldSupport01";
/// C++ scaffold geometry height fallback when the template is unknown.
pub const BRIDGE_SCAFFOLD_HEIGHT: f32 = 16.0;
/// C++ `setScaffoldData` sink fudge (`BridgeBehavior.cpp:955`).
pub const BRIDGE_SCAFFOLD_SINK_FUDGE: f32 = 8.0;
/// C++ `BridgeBehaviorModuleData` default lateral/vertical speeds.
pub const BRIDGE_SCAFFOLD_LATERAL_SPEED: f32 = 1.0;
pub const BRIDGE_SCAFFOLD_VERTICAL_SPEED: f32 = 1.0;
/// Residual authored OCL name for staged bridge death.
pub const BRIDGE_DIE_OCL_NAME: &str = "OCL_GenericBridgeDie";
/// C++ Roads.ini `DamagedToSound` residual.
pub const BRIDGE_DAMAGED_TO_SOUND: &str = "BridgeDamaged";
/// C++ Roads.ini `RepairedToSound` residual.
pub const BRIDGE_REPAIRED_TO_SOUND: &str = "BridgeRepaired";

/// C++ `ScaffoldTargetMotion` residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostScaffoldMotion {
    Rise,
    BuildAcross,
    #[default]
    Still,
    TearDownAcross,
    Sink,
}

#[derive(Debug, Clone, Copy)]
pub struct HostScaffoldTile {
    pub pos: Vec3,
    pub angle: f32,
    pub create_pos: Vec3,
    pub rise_to: Vec3,
    pub build_pos: Vec3,
    pub is_support: bool,
}

impl HostScaffoldTile {
    fn from_rise_build(
        rise_to: Vec3,
        build_pos: Vec3,
        angle: f32,
        height: f32,
        is_support: bool,
    ) -> Self {
        let create_pos = Vec3::new(
            rise_to.x,
            rise_to.y - height.max(0.0) - BRIDGE_SCAFFOLD_SINK_FUDGE,
            rise_to.z,
        );
        Self {
            pos: build_pos,
            angle,
            create_pos,
            rise_to,
            build_pos,
            is_support,
        }
    }
}

/// Per-object C++ `BridgeScaffoldBehavior` rise/build-across state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostScaffoldAnim {
    pub id: ObjectId,
    pub create_pos: Vec3,
    pub rise_to: Vec3,
    pub build_pos: Vec3,
    pub motion: HostScaffoldMotion,
    pub lateral_speed: f32,
    pub vertical_speed: f32,
    pub last_pos: Vec3,
}

impl HostScaffoldAnim {
    pub fn from_tile(id: ObjectId, tile: &HostScaffoldTile, center: Vec3) -> Self {
        let dist_build = (tile.build_pos - tile.rise_to).length();
        let dist_rise = (center - tile.rise_to).length();
        let ratio = if dist_rise > f32::EPSILON {
            dist_build / dist_rise
        } else {
            1.0
        };
        Self {
            id,
            create_pos: tile.create_pos,
            rise_to: tile.rise_to,
            build_pos: tile.build_pos,
            motion: HostScaffoldMotion::Rise,
            lateral_speed: BRIDGE_SCAFFOLD_LATERAL_SPEED * ratio,
            vertical_speed: BRIDGE_SCAFFOLD_VERTICAL_SPEED,
            last_pos: tile.create_pos,
        }
    }

    fn target(&self) -> Vec3 {
        match self.motion {
            HostScaffoldMotion::Rise => self.rise_to,
            HostScaffoldMotion::Sink => self.create_pos,
            HostScaffoldMotion::BuildAcross => self.build_pos,
            HostScaffoldMotion::TearDownAcross => self.rise_to,
            HostScaffoldMotion::Still => self.build_pos,
        }
    }

    /// One C++ `BridgeScaffoldBehavior::update` step. Returns the new pose.
    pub fn step(&mut self, current: Vec3) -> Vec3 {
        if self.motion == HostScaffoldMotion::Still {
            return current;
        }
        let target = self.target();
        let dir = target - current;
        let dir_len = dir.length();
        if dir_len <= f32::EPSILON {
            self.arrive();
            return target;
        }
        let (top_speed, start, end) = match self.motion {
            HostScaffoldMotion::Rise => (self.vertical_speed, self.create_pos, self.rise_to),
            HostScaffoldMotion::Sink => (self.vertical_speed, self.rise_to, self.create_pos),
            HostScaffoldMotion::BuildAcross => (self.lateral_speed, self.rise_to, self.build_pos),
            HostScaffoldMotion::TearDownAcross => {
                (self.lateral_speed, self.build_pos, self.rise_to)
            }
            HostScaffoldMotion::Still => return current,
        };
        let total_distance = (end - start).length() * 0.25;
        let our_distance = (end - current).length();
        let mut speed = if total_distance > f32::EPSILON {
            (our_distance / total_distance) * top_speed
        } else {
            top_speed
        };
        let min_speed = top_speed * 0.08;
        if speed < min_speed {
            speed = min_speed;
        }
        if speed > top_speed {
            speed = top_speed;
        }
        if speed < 0.001 {
            speed = 0.001;
        }
        let new_pos = current + (dir / dir_len) * speed;
        let to_target_new = target - new_pos;
        if to_target_new.dot(dir) <= 0.0 {
            self.arrive();
            return target;
        }
        new_pos
    }

    fn arrive(&mut self) {
        self.motion = match self.motion {
            HostScaffoldMotion::Rise => HostScaffoldMotion::BuildAcross,
            HostScaffoldMotion::BuildAcross => HostScaffoldMotion::Still,
            HostScaffoldMotion::TearDownAcross => HostScaffoldMotion::Sink,
            HostScaffoldMotion::Sink | HostScaffoldMotion::Still => HostScaffoldMotion::Still,
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBridgeSpan {
    pub object_id: ObjectId,
    pub from_left: Vec3,
    pub from_right: Vec3,
    pub to_left: Vec3,
    pub to_right: Vec3,
    pub was_rubble: bool,
    pub scaffold_present: bool,
    pub scaffold_motion_frames: u32,
    pub scaffold_ids: Vec<ObjectId>,
    #[serde(default)]
    pub tower_ids: [ObjectId; 4],
    #[serde(default)]
    pub death_frame: u32,
    #[serde(default)]
    pub last_body_state: u8,
    #[serde(default)]
    pub die_fx_fired_mask: u8,
    #[serde(default)]
    pub die_ocl_fired_mask: u8,
    #[serde(default)]
    pub scaffold_anims: Vec<HostScaffoldAnim>,
}

impl HostBridgeSpan {
    pub fn is_scaffold_in_motion(&self) -> bool {
        if !self.scaffold_present {
            return false;
        }
        if self
            .scaffold_anims
            .iter()
            .any(|a| a.motion != HostScaffoldMotion::Still)
        {
            return true;
        }
        self.scaffold_motion_frames > 0
    }

    pub fn point_on_deck(&self, pos: Vec3) -> bool {
        point_in_quad(
            pos.x,
            pos.z,
            &[self.from_left, self.from_right, self.to_right, self.to_left],
        )
    }

    /// C++ `BridgeBehavior::getRandomSurfacePosition` (BridgeBehavior.cpp:492-536)
    /// on the host deck (C++ Z is host Y). Draws, in order, from the world's
    /// logic stream: `GameLogicRandomValueReal(0,1)` scale for the to-left edge,
    /// the same for the from-right edge, then
    /// `GameLogicRandomValueReal(0, TransitionEffectsHeight)` added to height
    /// (a non-positive range returns `hi` without drawing).
    pub fn random_surface_position(
        &self,
        mut next_real: impl FnMut(f32, f32) -> f32,
        transition_effects_height: f32,
    ) -> Vec3 {
        let v1 = (self.to_left - self.from_left) * next_real(0.0, 1.0);
        let v2 = (self.from_right - self.from_left) * next_real(0.0, 1.0);
        let mut pos = self.from_left + v1 + v2;
        pos.y += next_real(0.0, transition_effects_height);
        pos
    }

    fn members(&self) -> impl Iterator<Item = ObjectId> + '_ {
        std::iter::once(self.object_id).chain(
            self.tower_ids
                .iter()
                .copied()
                .filter(|id| id.0 != 0 && *id != self.object_id),
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBridgeBehaviorRegistry {
    pub scaffolds_created: u32,
    pub rubble_restamps: u32,
    pub splat_kills: u32,
    #[serde(default)]
    pub mirrors: u32,
    #[serde(default)]
    pub death_links: u32,
    #[serde(default)]
    pub die_fx: u32,
    #[serde(default)]
    pub die_ocl: u32,
    #[serde(default)]
    pub damaged_syncs: u32,

    spans: HashMap<u32, HostBridgeSpan>,
    #[serde(default)]
    tower_to_span: HashMap<u32, ObjectId>,
}

impl HostBridgeBehaviorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace live spans from a save payload. Does not call
    /// `create_scaffolding` so a load cannot spawn a fresh rise.
    pub fn restore(&mut self, other: Self) {
        *self = other;
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn unregister_span(&mut self, id: ObjectId) {
        self.spans.remove(&id.0);
        self.tower_to_span.retain(|_, span| *span != id);
    }

    pub fn span(&self, id: ObjectId) -> Option<&HostBridgeSpan> {
        self.spans.get(&id.0)
    }

    pub fn span_mut(&mut self, id: ObjectId) -> Option<&mut HostBridgeSpan> {
        self.spans.get_mut(&id.0)
    }

    pub fn register_span(
        &mut self,
        object_id: ObjectId,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
    ) {
        match self.spans.entry(object_id.0) {
            std::collections::hash_map::Entry::Occupied(mut occ) => {
                let span = occ.get_mut();
                span.from_left = from_left;
                span.from_right = from_right;
                span.to_left = to_left;
                span.to_right = to_right;
            }
            std::collections::hash_map::Entry::Vacant(vac) => {
                vac.insert(HostBridgeSpan {
                    object_id,
                    from_left,
                    from_right,
                    to_left,
                    to_right,
                    was_rubble: false,
                    scaffold_present: false,
                    scaffold_motion_frames: 0,
                    scaffold_ids: Vec::new(),
                    tower_ids: [ObjectId(0); 4],
                    death_frame: 0,
                    last_body_state: 0,
                    die_fx_fired_mask: 0,
                    die_ocl_fired_mask: 0,
                    scaffold_anims: Vec::new(),
                });
            }
        }
    }

    /// C++ `BridgeBehavior::setTower` — bind the four targetable towers.
    pub fn bind_towers(&mut self, span: ObjectId, towers: [ObjectId; 4]) {
        self.tower_to_span.retain(|_, sid| sid.0 != span.0);
        if let Some(s) = self.spans.get_mut(&span.0) {
            s.tower_ids = towers;
        }
        for tid in towers {
            if tid.0 != 0 {
                self.tower_to_span.insert(tid.0, span);
            }
        }
    }

    pub fn span_id_for(&self, id: ObjectId) -> Option<ObjectId> {
        if self.spans.contains_key(&id.0) {
            return Some(id);
        }
        self.tower_to_span.get(&id.0).copied()
    }

    pub fn is_linked_member(&self, id: ObjectId) -> bool {
        self.span_id_for(id).is_some()
    }

    pub fn linked_members(&self, id: ObjectId) -> Vec<ObjectId> {
        let Some(span_id) = self.span_id_for(id) else {
            return Vec::new();
        };
        self.spans
            .get(&span_id.0)
            .map(|s| s.members().collect())
            .unwrap_or_default()
    }

    /// One synchronous callback slot. `span_id` is captured at callback entry;
    /// reread its current routing before each recipient, after prior callbacks.
    pub fn callback_target_at(
        &self,
        span_id: ObjectId,
        victim: ObjectId,
        slot: usize,
    ) -> Option<ObjectId> {
        if slot == 4 {
            // Tower callbacks finish on the originally resolved span, even if
            // a prior sibling callback changes the tower-to-span lookup.
            return (span_id != victim).then_some(span_id);
        }
        self.spans
            .get(&span_id.0)?
            .tower_ids
            .get(slot)
            .copied()
            .filter(|id| id.0 != 0 && *id != victim)
    }

    /// C++ tower/span onDamage/onHealing: siblings excluding `victim`.
    pub fn mirror_targets(&self, victim: ObjectId) -> Vec<ObjectId> {
        let Some(span_id) = self.span_id_for(victim) else {
            return Vec::new();
        };
        let Some(span) = self.spans.get(&span_id.0) else {
            return Vec::new();
        };
        let mut targets: Vec<_> = span
            .tower_ids
            .iter()
            .copied()
            .filter(|id| id.0 != 0 && *id != victim)
            .collect();
        if span_id != victim {
            targets.push(span_id);
        }
        targets
    }

    /// C++ `BridgeBehavior::createScaffolding` — start rise, close deck.
    /// Returns true only when scaffolding is newly created.
    pub fn create_scaffolding(&mut self, bridge: ObjectId) -> bool {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return false;
        };
        if span.scaffold_present {
            return false;
        }
        span.scaffold_present = true;
        span.scaffold_motion_frames = BRIDGE_SCAFFOLD_RISE_FRAMES;
        self.scaffolds_created = self.scaffolds_created.saturating_add(1);
        true
    }

    pub fn is_scaffold_in_motion(&self, bridge: ObjectId) -> bool {
        self.spans
            .get(&bridge.0)
            .is_some_and(|s| s.is_scaffold_in_motion())
    }

    pub fn is_scaffold_present(&self, bridge: ObjectId) -> bool {
        self.spans
            .get(&bridge.0)
            .is_some_and(|s| s.scaffold_present)
    }

    /// Advance each owned scaffold; the final flag requests direct deletion
    /// when the original sink phase completes.
    pub fn tick_scaffolds(&mut self) -> Vec<(ObjectId, Vec3, bool)> {
        let mut moved = Vec::new();
        for span in self.spans.values_mut() {
            if span.scaffold_motion_frames > 0 {
                span.scaffold_motion_frames -= 1;
            }
            span.scaffold_anims.retain_mut(|anim| {
                if anim.motion == HostScaffoldMotion::Still {
                    return true;
                }
                let was_sinking = anim.motion == HostScaffoldMotion::Sink;
                let next = anim.step(anim.last_pos);
                anim.last_pos = next;
                let finished_sink = was_sinking && anim.motion == HostScaffoldMotion::Still;
                moved.push((anim.id, next, finished_sink));
                !finished_sink
            });
            if !span.scaffold_anims.is_empty()
                && span
                    .scaffold_anims
                    .iter()
                    .all(|a| a.motion == HostScaffoldMotion::Still)
            {
                span.scaffold_motion_frames = 0;
            }
        }
        moved
    }

    pub fn remove_scaffolding(&mut self, bridge: ObjectId) -> Vec<ObjectId> {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return Vec::new();
        };
        if !span.scaffold_present {
            return Vec::new();
        }
        // CPP BridgeBehavior1262 / BridgeScaffoldBehavior87: reverse course,
        // keep the live animation until its sink explicitly requests deletion.
        for anim in &mut span.scaffold_anims {
            anim.motion = match anim.motion {
                HostScaffoldMotion::Still | HostScaffoldMotion::BuildAcross => {
                    HostScaffoldMotion::TearDownAcross
                }
                HostScaffoldMotion::Rise => HostScaffoldMotion::Sink,
                HostScaffoldMotion::TearDownAcross => HostScaffoldMotion::BuildAcross,
                HostScaffoldMotion::Sink => HostScaffoldMotion::Rise,
            };
        }
        span.scaffold_present = false;
        span.scaffold_motion_frames = 0;
        std::mem::take(&mut span.scaffold_ids)
    }

    /// Bind live object ids to tiled rise/build-across anims.

    pub fn bind_scaffold_anims(&mut self, bridge: ObjectId, anims: Vec<HostScaffoldAnim>) {
        if let Some(span) = self.spans.get_mut(&bridge.0) {
            span.scaffold_anims = anims;
        }
    }

    /// C++ `createScaffolding` tiles from both ends toward center.
    pub fn tiled_scaffold_sites(&self, bridge: ObjectId) -> Vec<HostScaffoldTile> {
        let Some(span) = self.spans.get(&bridge.0) else {
            return Vec::new();
        };
        let left_start = (span.from_left + span.from_right) * 0.5;
        let right_start = (span.to_left + span.to_right) * 0.5;
        let mut left_vector = right_start - left_start;
        let tile_distance = left_vector.length();
        let height = BRIDGE_SCAFFOLD_HEIGHT;
        if tile_distance <= f32::EPSILON {
            let mid = (left_start + right_start) * 0.5;
            return vec![HostScaffoldTile::from_rise_build(
                mid, mid, 0.0, height, false,
            )];
        }
        let spacing = BRIDGE_SCAFFOLD_TILE_SPACING.max(1.0);
        let num_objects = (tile_distance / spacing).ceil() as usize + 1;
        let num_iterations = ((num_objects as f32) / 2.0).ceil() as usize;
        left_vector = left_vector.normalize();
        let right_vector = -left_vector;
        let left_angle = (right_start.z - left_start.z).atan2(right_start.x - left_start.x);
        let right_angle = left_angle + std::f32::consts::TAU;
        let mut tiles = Vec::with_capacity(num_objects * 2);
        let mut created = 0usize;
        for i in 0..num_iterations {
            if created >= num_objects {
                break;
            }
            let offset = spacing * (i as f32);
            let left_build = left_start + left_vector * offset + Vec3::new(0.1, 0.0, 0.0);
            tiles.push(HostScaffoldTile::from_rise_build(
                left_start, left_build, left_angle, height, false,
            ));
            let mut support_rise = left_start;
            support_rise.y -= height;
            let mut support_build = left_build;
            support_build.y -= height;
            tiles.push(HostScaffoldTile::from_rise_build(
                support_rise,
                support_build,
                left_angle,
                height,
                true,
            ));
            created += 1;
            if created >= num_objects {
                break;
            }
            let right_build = right_start + right_vector * offset + Vec3::new(0.1, 0.0, 0.0);
            tiles.push(HostScaffoldTile::from_rise_build(
                right_start,
                right_build,
                right_angle,
                height,
                false,
            ));
            let mut support_rise = right_start;
            support_rise.y -= height;
            let mut support_build = right_build;
            support_build.y -= height;
            tiles.push(HostScaffoldTile::from_rise_build(
                support_rise,
                support_build,
                right_angle,
                height,
                true,
            ));
            created += 1;
        }
        tiles
    }

    /// Enter `BODY_RUBBLE`: restamp deck + collect occupants to splat.
    pub fn on_enter_rubble(&mut self, bridge: ObjectId, occupants: &[ObjectId]) -> bool {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return false;
        };
        if span.was_rubble {
            return false;
        }
        span.was_rubble = true;
        self.rubble_restamps = self.rubble_restamps.saturating_add(1);
        self.splat_kills = self.splat_kills.saturating_add(occupants.len() as u32);
        true
    }

    pub fn on_leave_rubble(&mut self, bridge: ObjectId) {
        if let Some(span) = self.spans.get_mut(&bridge.0) {
            span.was_rubble = false;
            span.death_frame = 0;
            span.die_fx_fired_mask = 0;
            span.die_ocl_fired_mask = 0;
        }
    }

    pub fn mark_death(&mut self, bridge: ObjectId, frame: u32) {
        if let Some(span) = self.spans.get_mut(&bridge.0) {
            span.death_frame = frame;
            span.die_fx_fired_mask = 0;
            span.die_ocl_fired_mask = 0;
        }
    }

    /// C++ `BridgeBehavior::update` delayed FX: fire each authored delay once.
    pub fn take_due_die_fx(&mut self, bridge: ObjectId, frame: u32) -> usize {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return 0;
        };
        if span.death_frame == 0 {
            return 0;
        }
        let death_time = frame.wrapping_sub(span.death_frame);
        let mut n = 0usize;
        for (i, delay) in BRIDGE_DIE_FX_DELAYS.iter().copied().enumerate() {
            let bit = 1u8 << i;
            if death_time == delay && (span.die_fx_fired_mask & bit) == 0 {
                span.die_fx_fired_mask |= bit;
                n += 1;
            }
        }
        if n > 0 {
            self.die_fx = self.die_fx.saturating_add(n as u32);
        }
        n
    }

    /// C++ `BridgeBehavior::update` delayed OCL: fire each authored delay once.
    pub fn take_due_die_ocl(&mut self, bridge: ObjectId, frame: u32) -> usize {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return 0;
        };
        if span.death_frame == 0 {
            return 0;
        }
        let death_time = frame.wrapping_sub(span.death_frame);
        let mut n = 0usize;
        for (i, delay) in BRIDGE_DIE_FX_DELAYS.iter().copied().enumerate() {
            let bit = 1u8 << i;
            if death_time == delay && (span.die_ocl_fired_mask & bit) == 0 {
                span.die_ocl_fired_mask |= bit;
                n += 1;
            }
        }
        if n > 0 {
            self.die_ocl = self.die_ocl.saturating_add(n as u32);
        }
        n
    }

    /// C++ `onBodyDamageStateChange` — true when the synced state changed.
    pub fn note_body_state(&mut self, bridge: ObjectId, state: u8) -> bool {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return false;
        };
        if span.last_body_state == state {
            return false;
        }
        span.last_body_state = state;
        if state != 3 {
            span.death_frame = 0;
            span.die_fx_fired_mask = 0;
            span.die_ocl_fired_mask = 0;
        }
        if state == 1 || state == 2 {
            self.damaged_syncs = self.damaged_syncs.saturating_add(1);
        }
        true
    }

    pub fn record_mirror_applied(&mut self) {
        self.mirrors = self.mirrors.saturating_add(1);
    }

    pub fn record_death_link_applied(&mut self) {
        self.death_links = self.death_links.saturating_add(1);
    }

    pub fn occupants_on_deck(
        &self,
        bridge: ObjectId,
        positions: &[(ObjectId, Vec3)],
    ) -> Vec<ObjectId> {
        let Some(span) = self.spans.get(&bridge.0) else {
            return Vec::new();
        };
        positions
            .iter()
            .filter_map(|(id, p)| {
                if span.point_on_deck(*p) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }
}

pub fn is_bridge_or_tower_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("bridgetower")
        || n.contains("bridge_tower")
        || (n.contains("bridge") && !n.contains("scaffold") && !n.contains("bridger"))
}

pub fn is_bridge_span_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("bridge")
        && !n.contains("tower")
        && !n.contains("scaffold")
        && !n.contains("waterwave"))
        || n.eq_ignore_ascii_case("bridge")
}

fn point_in_quad(x: f32, z: f32, corners: &[Vec3; 4]) -> bool {
    let mut inside = false;
    let mut j = 3;
    for i in 0..4 {
        let yi = corners[i].z;
        let yj = corners[j].z;
        let xi = corners[i].x;
        let xj = corners[j].x;
        if ((yi > z) != (yj > z)) && (x < (xj - xi) * (z - yi) / (yj - yi + f32::EPSILON) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}
