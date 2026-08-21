// Live Tank/Truck/Overlord/Laser/Debris residuals for presentation drawables.
//
// C++ `W3DModuleFactory::init` registers `W3DTankDraw`, `W3DTruckDraw`,
// `W3DOverlord*Draw`, `W3DLaserDraw`, and `W3DDebrisDraw`. Those modules
// own treads (`W3DTankDraw.cpp:197-379`), truck wheels, Overlord rider
// draw-after (`W3DOverlordTankDraw.cpp:45-78`), laser width
// (`W3DLaserDraw::getLaserTemplateWidth` = OuterBeamWidth * 0.5), and
// debris INITIAL/FLYING/FINAL anims (`W3DDebrisDraw.cpp:127-228`).
//
// `sync_presentation_drawables` used to allocate bare `BasicDrawable`s.
// This residual attaches typed live modules and ticks them on the host
// presentation path so those effects actually run.


/// C++ draw-module class attached to a live presentation drawable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationSpecializedDrawKind {
    Tank,
    Truck,
    TankTruck,
    OverlordTank,
    OverlordTruck,
    OverlordAircraft,
    Laser,
    Debris,
}

impl PresentationSpecializedDrawKind {
    pub fn from_module_name(name: &str) -> Option<Self> {
        match name {
            "W3DTankDraw" => Some(Self::Tank),
            "W3DTruckDraw" | "W3DPoliceCarDraw" => Some(Self::Truck),
            "W3DTankTruckDraw" => Some(Self::TankTruck),
            "W3DOverlordTankDraw" => Some(Self::OverlordTank),
            "W3DOverlordTruckDraw" => Some(Self::OverlordTruck),
            "W3DOverlordAircraftDraw" => Some(Self::OverlordAircraft),
            "W3DLaserDraw" => Some(Self::Laser),
            "W3DDebrisDraw" => Some(Self::Debris),
            _ => None,
        }
    }

    pub fn module_name(self) -> &'static str {
        match self {
            Self::Tank => "W3DTankDraw",
            Self::Truck => "W3DTruckDraw",
            Self::TankTruck => "W3DTankTruckDraw",
            Self::OverlordTank => "W3DOverlordTankDraw",
            Self::OverlordTruck => "W3DOverlordTruckDraw",
            Self::OverlordAircraft => "W3DOverlordAircraftDraw",
            Self::Laser => "W3DLaserDraw",
            Self::Debris => "W3DDebrisDraw",
        }
    }

    pub fn is_overlord(self) -> bool {
        matches!(
            self,
            Self::OverlordTank | Self::OverlordTruck | Self::OverlordAircraft
        )
    }

    pub fn scrolls_treads(self) -> bool {
        matches!(self, Self::Tank | Self::TankTruck | Self::OverlordTank)
    }

    pub fn spins_wheels(self) -> bool {
        matches!(
            self,
            Self::Truck | Self::TankTruck | Self::OverlordTruck
        )
    }
}

/// Frozen live residual consumed by the Main WGPU collect pass.
#[derive(Debug, Clone)]
pub struct PresentationSpecializedDrawSnapshot {
    pub kind: PresentationSpecializedDrawKind,
    pub module_name: String,
    pub object_id: u32,
    /// C++ `W3DTankDraw::updateTreadPositions` U offset in [0, 1).
    pub tread_uv: f32,
    /// C++ `W3DTruckDraw` wheel rotation residual (radians).
    pub wheel_angle: f32,
    /// C++ `W3DLaserDraw::getLaserTemplateWidth()` = OuterBeamWidth * 0.5.
    pub laser_width: f32,
    /// C++ debris INITIAL=0 / FLYING=1 / FINAL=2.
    pub debris_state: u8,
    pub debris_anim_time: f32,
    pub model_name: String,
}

impl PresentationSpecializedDrawSnapshot {
    /// C++ `W3DTankDraw.cpp:235-260` TREADS* leaf + left/right sign.
    pub fn tread_uv_for_mesh(&self, mesh_name: &str) -> Option<[f32; 2]> {
        if !self.kind.scrolls_treads() {
            return None;
        }
        let leaf = mesh_name.rsplit('.').next().unwrap_or(mesh_name);
        if leaf.len() < 6 || !leaf[..6].eq_ignore_ascii_case("TREADS") {
            return None;
        }
        let u = match leaf.as_bytes().get(6) {
            Some(b'L' | b'l') => self.tread_uv,
            Some(b'R' | b'r') => {
                let v = 1.0 - self.tread_uv;
                if v >= 1.0 {
                    0.0
                } else {
                    v
                }
            }
            _ => self.tread_uv,
        };
        Some([u, 0.0])
    }

    pub fn is_debris(&self) -> bool {
        self.kind == PresentationSpecializedDrawKind::Debris
    }

    pub fn is_laser(&self) -> bool {
        self.kind == PresentationSpecializedDrawKind::Laser
    }

    pub fn is_overlord(&self) -> bool {
        self.kind.is_overlord()
    }
}

/// Default C++ `W3DLaserDrawModuleData` OuterBeamWidth when INI omitted.
const DEFAULT_LASER_OUTER_BEAM_WIDTH: f32 = 1.0;
/// C++ `W3DDebrisDraw` MIN_FINAL_FRAMES before landing can freeze FINAL.
const DEBRIS_MIN_FINAL_FRAMES: u32 = 3;
/// C++ `W3DTankDrawModuleData` default drive-scroll when INI rate is 0.
const DEFAULT_TREAD_SCROLL: f32 = 0.05;

static SPECIALIZED_DRAW_STATES: OnceLock<Mutex<HashMap<u32, PresentationSpecializedDrawSnapshot>>> =
    OnceLock::new();

/// Live host query for the WGPU collect / Overlord rider pass.
pub fn presentation_specialized_draw_snapshot(
    object_id: u32,
) -> Option<PresentationSpecializedDrawSnapshot> {
    SPECIALIZED_DRAW_STATES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()?
        .get(&object_id)
        .cloned()
}

pub fn prune_presentation_specialized_draw(object_id: u32) {
    if let Ok(mut map) = SPECIALIZED_DRAW_STATES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        map.remove(&object_id);
    }
}

fn store_specialized_draw_snapshot(snapshot: PresentationSpecializedDrawSnapshot) {
    if let Ok(mut map) = SPECIALIZED_DRAW_STATES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        map.insert(snapshot.object_id, snapshot);
    }
}

/// Infer C++ Draw class when ThingFactory / INI names are unavailable.
pub fn infer_presentation_draw_module_names(
    template_name: &str,
    kind_names: &[String],
) -> Vec<String> {
    let t = template_name.to_ascii_lowercase();
    let kinds: Vec<String> = kind_names.iter().map(|k| k.to_ascii_lowercase()).collect();
    let has_kind = |needle: &str| kinds.iter().any(|k| k.contains(needle));

    if t.contains("laser")
        || t.contains("binarydatastream")
        || t.contains("binary_data_stream")
        || (t.contains("beam") && (t.contains("stream") || has_kind("immobile")))
    {
        return vec!["W3DLaserDraw".to_string()];
    }
    if t.contains("debris") {
        return vec!["W3DDebrisDraw".to_string()];
    }
    if t.contains("helix") || t.contains("spectregunship") {
        return vec!["W3DOverlordAircraftDraw".to_string()];
    }
    if t.contains("overlord") {
        return vec!["W3DOverlordTankDraw".to_string()];
    }
    if has_kind("tank")
        || t.contains("tank")
        || t.contains("crusader")
        || t.contains("paladin")
        || t.contains("battlemaster")
        || t.contains("scorpion")
    {
        return vec!["W3DTankDraw".to_string()];
    }
    if t.contains("truck")
        || t.contains("humvee")
        || t.contains("convoy")
        || t.contains("dozer")
    {
        return vec!["W3DTruckDraw".to_string()];
    }
    Vec::new()
}


fn wrap_uv(offset: f32) -> f32 {
    offset - offset.floor()
}

/// Live residual attached to a presentation `BasicDrawable`.
#[derive(Debug)]
struct PresentationSpecializedDrawModule {
    identifier: String,
    kind: PresentationSpecializedDrawKind,
    object_id: u32,
    last_pos: [f32; 3],
    last_orientation: f32,
    has_last_pose: bool,
    tread_uv: f32,
    wheel_angle: f32,
    laser_width: f32,
    debris_state: u8,
    debris_frames: u32,
    debris_anim_time: f32,
    model_name: String,
    scene_line_id: Option<game_engine::common::system::scene_submission::SceneLineId>,
}

impl PresentationSpecializedDrawModule {
    fn new(
        identifier: impl Into<String>,
        kind: PresentationSpecializedDrawKind,
        object_id: u32,
        model_name: String,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            kind,
            object_id,
            last_pos: [0.0; 3],
            last_orientation: 0.0,
            has_last_pose: false,
            tread_uv: 0.0,
            wheel_angle: 0.0,
            laser_width: DEFAULT_LASER_OUTER_BEAM_WIDTH * 0.5,
            debris_state: 0,
            debris_frames: 0,
            debris_anim_time: 0.0,
            model_name,
            scene_line_id: None,
        }
    }

    fn snapshot(&self) -> PresentationSpecializedDrawSnapshot {
        PresentationSpecializedDrawSnapshot {
            kind: self.kind,
            module_name: self.kind.module_name().to_string(),
            object_id: self.object_id,
            tread_uv: self.tread_uv,
            wheel_angle: self.wheel_angle,
            laser_width: self.laser_width,
            debris_state: self.debris_state,
            debris_anim_time: self.debris_anim_time,
            model_name: self.model_name.clone(),
        }
    }

    fn tick(&mut self, e: &PresentationDrawableSync) {
        let pos = e.position;
        if self.has_last_pose {
            let dx = pos[0] - self.last_pos[0];
            let dy = pos[1] - self.last_pos[1];
            let ground_speed = (dx * dx + dy * dy).sqrt();
            let turning = (e.orientation - self.last_orientation).abs();

            if self.kind.scrolls_treads() {
                // C++ W3DTankDraw.cpp:338-377 — drive scroll when motive;
                // pivot scroll when turning while nearly stationary.
                let delta = if turning > 0.00001 && ground_speed < 0.35 {
                    if e.orientation >= self.last_orientation {
                        DEFAULT_TREAD_SCROLL
                    } else {
                        -DEFAULT_TREAD_SCROLL
                    }
                } else if ground_speed >= 0.05 {
                    -DEFAULT_TREAD_SCROLL
                } else {
                    0.0
                };
                self.tread_uv = wrap_uv(self.tread_uv + delta);
            }
            if self.kind.spins_wheels() {
                // C++ W3DTruckDraw wheel rotation from ground travel.
                self.wheel_angle = wrap_uv((self.wheel_angle + ground_speed * 0.25) / std::f32::consts::TAU)
                    * std::f32::consts::TAU;
            }
        }
        self.last_pos = pos;
        self.last_orientation = e.orientation;
        self.has_last_pose = true;

        if self.kind == PresentationSpecializedDrawKind::Debris {
            self.tick_debris(e);
        }
        if self.kind == PresentationSpecializedDrawKind::Laser {
            // C++ W3DLaserDraw.h getLaserTemplateWidth = m_outerBeamWidth * 0.5.
            self.laser_width = DEFAULT_LASER_OUTER_BEAM_WIDTH * 0.5;
            self.publish_laser_line(e);
        }
        store_specialized_draw_snapshot(self.snapshot());
    }

    fn tick_debris(&mut self, e: &PresentationDrawableSync) {
        // C++ W3DDebrisDraw.cpp:127-228 INITIAL → FLYING on anim complete,
        // FLYING → FINAL once landed after MIN_FINAL_FRAMES.
        self.debris_frames = self.debris_frames.saturating_add(1);
        let airborne = (e.position[2] - self.last_pos[2]).abs() > 0.05 || e.position[2] > 2.0;
        match self.debris_state {
            0 => {
                self.debris_anim_time = (self.debris_anim_time + 1.0 / 30.0).min(1.0);
                if self.debris_anim_time >= 1.0 {
                    self.debris_state = 1;
                    self.debris_anim_time = 0.0;
                }
            }
            1 => {
                self.debris_anim_time = (self.debris_anim_time + 1.0 / 30.0).min(1.0);
                if self.debris_frames > DEBRIS_MIN_FINAL_FRAMES && !airborne {
                    self.debris_state = 2;
                    self.debris_anim_time = 0.0;
                }
            }
            _ => {
                self.debris_anim_time = 1.0;
            }
        }
        if self.model_name.is_empty() {
            self.model_name = if !e.visual_template_name.is_empty() {
                e.visual_template_name.clone()
            } else {
                e.template_name.clone()
            };
        }
    }

    fn publish_laser_line(&mut self, e: &PresentationDrawableSync) {
        use game_engine::common::system::geometry::Coord3D;
        use game_engine::common::system::scene_submission::SceneLineDesc;
        use gamelogic::helpers::{submit_scene_line, update_scene_line};

        let start = Coord3D::new(e.position[0], e.position[1], e.position[2]);
        let heading = e.orientation;
        let end = Coord3D::new(
            e.position[0] + heading.cos() * 8.0,
            e.position[1] + heading.sin() * 8.0,
            e.position[2],
        );
        let desc = SceneLineDesc {
            start,
            end,
            width: self.laser_width.max(DEFAULT_LASER_OUTER_BEAM_WIDTH * 0.5),
            color_r: 1.0,
            color_g: 0.2,
            color_b: 0.2,
            opacity: 1.0,
            texture_name: None,
            tile_factor: 1.0,
            scroll_rate: 0.0,
            visible: true,
        };
        match self.scene_line_id {
            None => {
                self.scene_line_id = submit_scene_line(e.object_id, &desc);
            }
            Some(id) => update_scene_line(id, &desc),
        }
    }
}

impl DrawModule for PresentationSpecializedDrawModule {
    fn snapshot_module_identifier(&self) -> Option<&str> {
        Some(&self.identifier)
    }

    fn drawable_module_type_index(&self) -> usize {
        0
    }

    fn do_draw(&mut self, _transform: &Matrix4, _view: &Matrix4, _projection: &Matrix4) {
        store_specialized_draw_snapshot(self.snapshot());
    }

    /// C++ `ObjectDrawInterface::getCurrentBonePositions` via W3D HTree.
    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Vector3],
        transforms: &mut [Matrix4],
    ) -> i32 {
        if bone_name_prefix.is_empty() || self.model_name.is_empty() {
            return 0;
        }
        let start = start_index.max(0);
        let end_index = if start == 0 { 0 } else { 99 };
        let limit = positions.len().min(transforms.len());
        let mut count = 0;
        for idx in start..=end_index {
            if count >= limit {
                break;
            }
            let bone_name = if idx == 0 {
                bone_name_prefix.to_string()
            } else {
                format!("{bone_name_prefix}{idx:02}")
            };
            let Some((_, mtx)) = crate::drawable::logic_visual_hooks::lookup_w3d_client_bone(
                &self.model_name,
                1.0,
                0,
                &bone_name,
            ) else {
                break;
            };
            let (_, _, translation) = mtx.to_scale_rotation_translation();
            positions[count] = Vector3::new(translation.x, translation.y, translation.z);
            transforms[count] = Matrix4::from_glam(mtx);
            count += 1;
        }
        count as i32
    }
}

fn presentation_draw_module_names_for(e: &PresentationDrawableSync) -> Vec<String> {
    let mut names: Vec<String> = e
        .draw_module_names
        .iter()
        .filter_map(|raw| {
            raw.split_whitespace()
                .next()
                .map(|token| token.to_string())
        })
        .filter(|name| PresentationSpecializedDrawKind::from_module_name(name).is_some())
        .collect();
    if names.is_empty() {
        let visual = if e.visual_template_name.is_empty() {
            e.template_name.as_str()
        } else {
            e.visual_template_name.as_str()
        };
        names = infer_presentation_draw_module_names(visual, &e.kind_names);
    }
    names
}

fn attach_factory_snapshot_modules(drawable: &mut BasicDrawable, template_name: &str) {
    let Ok(guard) = get_thing_factory() else {
        return;
    };
    let Some(factory) = guard.as_ref() else {
        return;
    };
    let Some(template) = factory.find_template(template_name, false) else {
        return;
    };
    for module in GameClient::create_snapshot_modules_from_template(template.as_ref()) {
        drawable.add_draw_module(module);
    }
}

impl GameClient {
    fn attach_presentation_specialized_draw_modules(
        drawable: &mut BasicDrawable,
        e: &PresentationDrawableSync,
    ) {
        let visual = Self::presentation_visual_template_name(e).to_string();
        attach_factory_snapshot_modules(drawable, &visual);

        let existing: Vec<String> = drawable
            .get_draw_modules()
            .iter()
            .filter_map(|module| module.snapshot_module_identifier().map(str::to_string))
            .collect();

        for name in presentation_draw_module_names_for(e) {
            let Some(kind) = PresentationSpecializedDrawKind::from_module_name(&name) else {
                continue;
            };
            if existing.iter().any(|id| {
                id == &name || PresentationSpecializedDrawKind::from_module_name(id) == Some(kind)
            }) {
                continue;
            }
            let model_name = if !visual.is_empty() {
                visual.clone()
            } else {
                e.template_name.clone()
            };
            let residual =
                PresentationSpecializedDrawModule::new(name.clone(), kind, e.object_id, model_name);
            drawable.add_draw_module(Box::new(residual));
        }
        Self::tick_specialized_from_sync(e);
    }

    fn tick_presentation_specialized_draw_modules(e: &PresentationDrawableSync) {
        Self::tick_specialized_from_sync(e);
    }

    fn tick_specialized_from_sync(e: &PresentationDrawableSync) {
        let names = presentation_draw_module_names_for(e);
        let Some(name) = names.first() else {
            return;
        };
        let Some(kind) = PresentationSpecializedDrawKind::from_module_name(name) else {
            return;
        };
        let visual = if e.visual_template_name.is_empty() {
            e.template_name.clone()
        } else {
            e.visual_template_name.clone()
        };
        let prev = presentation_specialized_draw_snapshot(e.object_id);
        let mut module = PresentationSpecializedDrawModule::new(
            name.clone(),
            kind,
            e.object_id,
            prev.as_ref()
                .map(|s| s.model_name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or(visual),
        );
        if let Some(prev) = prev {
            module.tread_uv = prev.tread_uv;
            module.wheel_angle = prev.wheel_angle;
            module.laser_width = prev.laser_width;
            module.debris_state = prev.debris_state;
            module.debris_anim_time = prev.debris_anim_time;
            module.debris_frames = if prev.debris_state > 0 { 4 } else { 0 };
        }
        if let Some(pos) = prev_last_pos(e.object_id) {
            module.last_pos = pos;
            module.last_orientation = prev_last_ori(e.object_id).unwrap_or(e.orientation);
            module.has_last_pose = true;
        }
        module.tick(e);
        store_last_pose(e.object_id, e.position, e.orientation);
    }
}

static LAST_POSE: OnceLock<Mutex<HashMap<u32, ([f32; 3], f32)>>> = OnceLock::new();

fn store_last_pose(object_id: u32, pos: [f32; 3], ori: f32) {
    if let Ok(mut map) = LAST_POSE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        map.insert(object_id, (pos, ori));
    }
}

fn prev_last_pos(object_id: u32) -> Option<[f32; 3]> {
    LAST_POSE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()?
        .get(&object_id)
        .map(|(p, _)| *p)
}

fn prev_last_ori(object_id: u32) -> Option<f32> {
    LAST_POSE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()?
        .get(&object_id)
        .map(|(_, o)| *o)
}
