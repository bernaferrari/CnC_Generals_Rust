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
    PoliceCar,
    ScienceModel,
}

impl PresentationSpecializedDrawKind {
    pub fn from_module_name(name: &str) -> Option<Self> {
        match name {
            "W3DTankDraw" => Some(Self::Tank),
            "W3DTruckDraw" => Some(Self::Truck),
            "W3DPoliceCarDraw" => Some(Self::PoliceCar),
            "W3DTankTruckDraw" => Some(Self::TankTruck),
            "W3DOverlordTankDraw" => Some(Self::OverlordTank),
            "W3DOverlordTruckDraw" => Some(Self::OverlordTruck),
            "W3DOverlordAircraftDraw" => Some(Self::OverlordAircraft),
            "W3DLaserDraw" => Some(Self::Laser),
            "W3DDebrisDraw" => Some(Self::Debris),
            "W3DScienceModelDraw" => Some(Self::ScienceModel),
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
            Self::PoliceCar => "W3DPoliceCarDraw",
            Self::ScienceModel => "W3DScienceModelDraw",
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

    /// C++ `W3DTankDraw` / `W3DOverlordTankDraw` TrackDebrisDirt emitters.
    /// TankTruck's `SHOW_TANK_DEBRIS` is compiled out in shipped C++.
    pub fn has_tread_debris(self) -> bool {
        matches!(self, Self::Tank | Self::OverlordTank)
    }

    pub fn spins_wheels(self) -> bool {
        matches!(
            self,
            Self::Truck | Self::TankTruck | Self::OverlordTruck | Self::PoliceCar
        )
    }

    /// C++ `W3DTruckDraw` Dust/DirtSpray/PowerslideSpray + landing/slide audio.
    pub fn has_truck_dust(self) -> bool {
        matches!(
            self,
            Self::Truck | Self::TankTruck | Self::OverlordTruck | Self::PoliceCar
        )
    }

    pub fn has_police_light(self) -> bool {
        self == Self::PoliceCar
    }

    pub fn has_science_hide(self) -> bool {
        self == Self::ScienceModel
    }
}

/// Frozen live residual consumed by the Main WGPU collect pass.
#[derive(Debug, Clone, PartialEq)]
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
    /// C++ `W3DScienceModelDraw::doDrawModule` setHidden residual.
    pub science_hidden: bool,
}

impl PresentationSpecializedDrawSnapshot {
    /// C++ `W3DTankDraw.cpp:235-260` TREADS* leaf + left/right sign.
    pub fn tread_uv_for_mesh(&self, mesh_name: &str) -> Option<[f32; 2]> {
        if !self.kind.scrolls_treads() {
            return None;
        }
        let leaf = mesh_name.rsplit('.').next().unwrap_or(mesh_name);
        if !leaf
            .as_bytes()
            .get(..6)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"TREADS"))
        {
            return None;
        }
        let u = match leaf.as_bytes().get(6) {
            Some(b'L' | b'l') => self.tread_uv,
            Some(b'R' | b'r') => {
                let v = 1.0 - self.tread_uv;
                if v >= 1.0 { 0.0 } else { v }
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

    /// Leftover RequiredScience hide — skip this science-gated mesh.
    pub fn is_science_hidden(&self) -> bool {
        self.science_hidden && self.kind.has_science_hide()
    }
}

/// Default C++ `W3DLaserDrawModuleData` OuterBeamWidth when INI omitted.
const DEFAULT_LASER_OUTER_BEAM_WIDTH: f32 = 1.0;
/// C++ `W3DDebrisDraw` MIN_FINAL_FRAMES before landing can freeze FINAL.
const DEBRIS_MIN_FINAL_FRAMES: u32 = 3;
/// C++ `W3DTankDrawModuleData` default drive-scroll when INI rate is 0.
const DEFAULT_TREAD_SCROLL: f32 = 0.05;

/// Runtime-only residual belongs to one exact Drawable lifetime. Constructing
/// another client cannot publish it, and dropping/replacing a Drawable also
/// drops its prior pose. C++ retains these fields on its Draw modules.
#[derive(Debug)]
pub(crate) struct PresentationSpecializedDrawState {
    snapshot: PresentationSpecializedDrawSnapshot,
    last_pos: [f32; 3],
    last_orientation: f32,
}

impl GameClient {
    pub fn presentation_specialized_draw_snapshot(
        &self,
        object_id: u32,
    ) -> Option<&PresentationSpecializedDrawSnapshot> {
        let drawable_id = self.drawable_object_map.get(&object_id)?;
        let drawable = self.drawable_map.get(drawable_id)?;
        let basic = drawable.as_any().downcast_ref::<BasicDrawable>()?;
        basic
            .presentation_specialized_draw
            .as_ref()
            .map(|state| &state.snapshot)
    }

    /// Explicit client→renderer freeze source; values remain borrowed until
    /// Main captures a small completed visual bundle, once per Drawable.
    pub fn presentation_specialized_draw_snapshots(
        &self,
        host_epoch: u64,
    ) -> impl Iterator<Item = (u32, &PresentationSpecializedDrawSnapshot)> {
        self.drawable_object_map
            .iter()
            .filter_map(move |(&object_id, drawable_id)| {
                let binding = self
                    .presentation_direct_drawable_bindings
                    .get(drawable_id)?;
                if binding.binding_key.host_epoch != host_epoch
                    || binding.binding_key.object_id != object_id
                    || binding.binding_key.drawable_id != *drawable_id
                {
                    return None;
                }
                let drawable = self.drawable_map.get(drawable_id)?;
                if drawable.get_object_id() != Some(object_id) {
                    return None;
                }
                let basic = drawable.as_any().downcast_ref::<BasicDrawable>()?;
                let state = basic.presentation_specialized_draw.as_ref()?;
                Some((object_id, &state.snapshot))
            })
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
    if t.contains("policecar") || t.contains("police_car") || t.contains("civiliansedans") {
        return vec!["W3DPoliceCarDraw".to_string()];
    }
    if t.contains("science")
        && (t.contains("model") || t.contains("particle") || t.contains("uplink"))
    {
        return vec!["W3DScienceModelDraw".to_string()];
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
    if t.contains("police") {
        return vec!["W3DPoliceCarDraw".to_string()];
    }
    if t.contains("truck") || t.contains("humvee") || t.contains("convoy") || t.contains("dozer") {
        return vec!["W3DTruckDraw".to_string()];
    }
    Vec::new()
}

fn wrap_uv(offset: f32) -> f32 {
    offset - offset.floor()
}

fn leftover_truck_draw_module_data(template_name: &str) -> Option<W3DTruckDrawModuleData> {
    if template_name.is_empty() {
        return None;
    }
    let Ok(guard) = get_thing_factory() else {
        return None;
    };
    let factory = guard.as_ref()?;
    let template = factory.find_template(template_name, false)?;
    for entry in template.get_draw_module_info().iter() {
        if let Some(data) = entry.data.as_any().downcast_ref::<W3DTruckDrawModuleData>() {
            return Some(data.clone());
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<W3DOverlordTruckDrawModuleData>()
        {
            return Some(data.base.clone());
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<W3DTankTruckDrawModuleData>()
        {
            return Some(data.base.clone());
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<W3DPoliceCarDrawModuleData>()
        {
            return Some(data.base.clone());
        }
    }
    None
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
    science_hidden: bool,
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
            science_hidden: false,
            scene_line_id: None,
        }
    }

    fn into_snapshot(self, module_name: String) -> PresentationSpecializedDrawSnapshot {
        PresentationSpecializedDrawSnapshot {
            kind: self.kind,
            module_name,
            object_id: self.object_id,
            tread_uv: self.tread_uv,
            wheel_angle: self.wheel_angle,
            laser_width: self.laser_width,
            debris_state: self.debris_state,
            debris_anim_time: self.debris_anim_time,
            model_name: self.model_name,
            science_hidden: self.science_hidden,
        }
    }

    fn tick(&mut self, e: &PresentationDrawableSync) {
        let pos = e.position;
        let mut vel_mag_sq = 0.0;
        if self.has_last_pose {
            let dx = pos[0] - self.last_pos[0];
            let dy = pos[1] - self.last_pos[1];
            vel_mag_sq = dx * dx + dy * dy;
            let ground_speed = vel_mag_sq.sqrt();
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
                self.wheel_angle =
                    wrap_uv((self.wheel_angle + ground_speed * 0.25) / std::f32::consts::TAU)
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
        let visual = if e.visual_template_name.is_empty() {
            e.template_name.as_str()
        } else {
            e.visual_template_name.as_str()
        };
        if leftover_science_model_data(visual).is_some() || self.kind.has_science_hide() {
            self.science_hidden = tick_live_host_science_model_hide(
                visual,
                leftover_science_model_data(visual).as_ref(),
            );
        }
        if leftover_template_uses_animated_particle_sys_bones(visual) {
            tick_live_host_animated_particle_sys_bones(e.object_id);
        }
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

    fn do_draw(&mut self, _transform: &Matrix4, _view: &Matrix4, _projection: &Matrix4) {}

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
        .filter_map(|raw| raw.split_whitespace().next().map(|token| token.to_string()))
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
        Self::tick_specialized_from_sync(drawable, e);
    }

    fn tick_presentation_specialized_draw_modules(
        drawable: &mut BasicDrawable,
        e: &PresentationDrawableSync,
    ) {
        Self::tick_specialized_from_sync(drawable, e);
    }

    fn tick_specialized_from_sync(drawable: &mut BasicDrawable, e: &PresentationDrawableSync) {
        let names = presentation_draw_module_names_for(e);
        let Some(name) = names.first() else {
            return;
        };
        let Some(kind) = PresentationSpecializedDrawKind::from_module_name(name) else {
            return;
        };
        let visual = if e.visual_template_name.is_empty() {
            e.template_name.as_str()
        } else {
            e.visual_template_name.as_str()
        };
        // Preserve the current residual transition/tick order while changing
        // ownership. Move retained strings through the temporary tick module;
        // render projection only borrows the resulting immutable snapshot.
        let previous = drawable.presentation_specialized_draw.take();
        let previous_pose = previous
            .as_ref()
            .map(|state| (state.last_pos, state.last_orientation));
        let mut module =
            PresentationSpecializedDrawModule::new(String::new(), kind, e.object_id, String::new());
        let module_name = if let Some(previous) = previous {
            let snapshot = previous.snapshot;
            module.tread_uv = snapshot.tread_uv;
            module.wheel_angle = snapshot.wheel_angle;
            module.laser_width = snapshot.laser_width;
            module.debris_state = snapshot.debris_state;
            module.debris_anim_time = snapshot.debris_anim_time;
            // Full authored debris timing remains hq-q6sza. This migration
            // intentionally retains the existing presentation counter rule.
            module.debris_frames = if snapshot.debris_state > 0 { 4 } else { 0 };
            module.model_name = snapshot.model_name;
            module.last_pos = previous.last_pos;
            module.last_orientation = previous.last_orientation;
            module.has_last_pose = true;
            if snapshot.kind == kind {
                snapshot.module_name
            } else {
                kind.module_name().to_string()
            }
        } else {
            kind.module_name().to_string()
        };
        if module.model_name.is_empty() {
            module.model_name = visual.to_string();
        }
        module.tick(e);
        drawable.presentation_specialized_draw = Some(PresentationSpecializedDrawState {
            snapshot: module.into_snapshot(module_name),
            last_pos: e.position,
            last_orientation: e.orientation,
        });
        tick_persistent_live_host_draws(drawable, e, previous_pose);
    }
}

enum HostedDrawRole {
    Tank,
    Truck,
    Police,
}

struct LiveHostTick {
    object_id: u32,
    position: [f32; 3],
    vel_mag_sq: Real,
    hidden: bool,
    visual: String,
    physics: TruckDrawLivePhysics,
}

fn live_host_tick_from_sync(
    e: &PresentationDrawableSync,
    previous_pose: Option<([f32; 3], f32)>,
) -> LiveHostTick {
    let pos = e.position;
    let mut vel_mag_sq = 0.0;
    let mut had_pose = false;
    let mut last_pos = pos;
    let mut last_ori = e.orientation;
    if let Some((previous_pos, previous_orientation)) = previous_pose {
        had_pose = true;
        last_pos = previous_pos;
        last_ori = previous_orientation;
        let dx = pos[0] - last_pos[0];
        let dy = pos[1] - last_pos[1];
        vel_mag_sq = dx * dx + dy * dy;
    }
    let speed = vel_mag_sq.sqrt();
    let heading_x = e.orientation.cos();
    let heading_y = e.orientation.sin();
    let turning = if had_pose {
        e.orientation - last_ori
    } else {
        0.0
    };
    let airborne = pos[2] > 2.0 || (had_pose && pos[2] > last_pos[2] + 0.35);
    let visual = if e.visual_template_name.is_empty() {
        e.template_name.clone()
    } else {
        e.visual_template_name.clone()
    };
    LiveHostTick {
        object_id: e.object_id,
        position: pos,
        vel_mag_sq,
        hidden: e.scene_hidden_by_stealth || e.destroyed,
        visual,
        physics: TruckDrawLivePhysics {
            speed,
            vel_x: heading_x * speed,
            vel_y: heading_y * speed,
            accel_x: heading_x * speed,
            accel_y: heading_y * speed,
            is_motive: speed > 0.01,
            airborne,
            frames_airborne: 0,
            turning,
        },
    }
}

fn hosted_draw_role(
    module: &mut dyn game_engine::common::thing::module::Module,
) -> Option<HostedDrawRole> {
    use game_engine::common::thing::module::Module;
    if Module::as_any_mut(module)
        .downcast_mut::<W3DTankDraw>()
        .is_some()
    {
        return Some(HostedDrawRole::Tank);
    }
    if Module::as_any_mut(module)
        .downcast_mut::<W3DOverlordTankDraw>()
        .is_some()
    {
        return Some(HostedDrawRole::Tank);
    }
    if Module::as_any_mut(module)
        .downcast_mut::<W3DPoliceCarDraw>()
        .is_some()
    {
        return Some(HostedDrawRole::Police);
    }
    if Module::as_any_mut(module)
        .downcast_mut::<W3DTruckDraw>()
        .is_some()
    {
        return Some(HostedDrawRole::Truck);
    }
    if Module::as_any_mut(module)
        .downcast_mut::<W3DOverlordTruckDraw>()
        .is_some()
    {
        return Some(HostedDrawRole::Truck);
    }
    if Module::as_any_mut(module)
        .downcast_mut::<W3DTankTruckDraw>()
        .is_some()
    {
        return Some(HostedDrawRole::Truck);
    }
    None
}

fn live_host_roles(drawable: &mut BasicDrawable) -> (bool, bool, bool) {
    let mut tank = false;
    let mut truck = false;
    let mut police = false;
    for module in drawable.get_draw_modules_mut() {
        let Some(logic) = module.logic_module_mut() else {
            continue;
        };
        match hosted_draw_role(logic) {
            Some(HostedDrawRole::Tank) => tank = true,
            Some(HostedDrawRole::Truck) => truck = true,
            Some(HostedDrawRole::Police) => police = true,
            None => {}
        }
    }
    (tank, truck, police)
}

fn push_logic_draw(
    drawable: &mut BasicDrawable,
    name: &str,
    module: Box<dyn game_engine::common::thing::module::Module>,
) {
    drawable.add_draw_module(Box::new(LogicDrawModuleSnapshotAdapter::draw_module(
        name, module,
    )));
}

fn ensure_live_host_draw(
    drawable: &mut BasicDrawable,
    e: &PresentationDrawableSync,
    kind: PresentationSpecializedDrawKind,
) {
    if !kind.has_tread_debris() && !kind.has_truck_dust() && !kind.has_police_light() {
        return;
    }
    let (has_tank, has_truck, has_police) = live_host_roles(drawable);
    let visual = if e.visual_template_name.is_empty() {
        e.template_name.as_str()
    } else {
        e.visual_template_name.as_str()
    };
    let mut has_police = has_police;
    if kind.has_police_light() && !has_police {
        let mut data = W3DPoliceCarDrawModuleData::new();
        if let Some(truck) = leftover_truck_draw_module_data(visual) {
            data.base = truck;
        }
        let mut draw = W3DPoliceCarDraw::new(data);
        draw.bind_owner_id(e.object_id);
        push_logic_draw(drawable, "W3DPoliceCarDraw", Box::new(draw));
        has_police = true;
    }
    if kind.has_truck_dust() && !has_truck && !has_police {
        let data =
            leftover_truck_draw_module_data(visual).unwrap_or_else(W3DTruckDrawModuleData::new);
        let mut draw = W3DTruckDraw::new(data);
        draw.bind_owner_id(e.object_id);
        draw.bind_sounds_from_template(visual);
        push_logic_draw(drawable, "W3DTruckDraw", Box::new(draw));
    }
    if kind.has_tread_debris() && !has_tank {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData::new());
        draw.bind_owner_id(e.object_id);
        push_logic_draw(drawable, "W3DTankDraw", Box::new(draw));
    }
}

fn apply_live_host_draw(
    module: &mut dyn game_engine::common::thing::module::Module,
    kind: PresentationSpecializedDrawKind,
    ctx: &LiveHostTick,
) {
    use game_engine::common::thing::module::Module;
    if kind.has_tread_debris() {
        if let Some(draw) = Module::as_any_mut(module).downcast_mut::<W3DTankDraw>() {
            draw.bind_owner_id(ctx.object_id);
            draw.tick_live_move_debris(ctx.position, ctx.vel_mag_sq, ctx.hidden, false);
            return;
        }
        if let Some(draw) = Module::as_any_mut(module).downcast_mut::<W3DOverlordTankDraw>() {
            draw.bind_owner_id(ctx.object_id);
            draw.tick_live_tread_debris(ctx.position, ctx.vel_mag_sq, ctx.hidden, false);
            return;
        }
    }
    if kind.has_police_light() {
        if let Some(draw) = Module::as_any_mut(module).downcast_mut::<W3DPoliceCarDraw>() {
            draw.bind_owner_id(ctx.object_id);
            draw.tick_live_host_light(ctx.position, ctx.hidden);
            if kind.has_truck_dust() {
                draw.tick_live_host_dust(&ctx.visual, ctx.physics, ctx.hidden);
            }
        }
        return;
    }
    if kind.has_truck_dust() {
        if let Some(draw) = Module::as_any_mut(module).downcast_mut::<W3DTruckDraw>() {
            draw.bind_owner_id(ctx.object_id);
            draw.tick_live_host(&ctx.visual, ctx.physics, ctx.hidden);
            return;
        }
        if let Some(draw) = Module::as_any_mut(module).downcast_mut::<W3DOverlordTruckDraw>() {
            draw.bind_owner_id(ctx.object_id);
            draw.tick_live_host_dust(&ctx.visual, ctx.physics, ctx.hidden);
            return;
        }
        if let Some(draw) = Module::as_any_mut(module).downcast_mut::<W3DTankTruckDraw>() {
            draw.bind_owner_id(ctx.object_id);
            draw.tick_live_host_dust(&ctx.visual, ctx.physics, ctx.hidden);
        }
    }
}

fn tick_persistent_live_host_draws(
    drawable: &mut BasicDrawable,
    e: &PresentationDrawableSync,
    previous_pose: Option<([f32; 3], f32)>,
) {
    let names = presentation_draw_module_names_for(e);
    let Some(name) = names.first() else {
        return;
    };
    let Some(kind) = PresentationSpecializedDrawKind::from_module_name(name) else {
        return;
    };
    ensure_live_host_draw(drawable, e, kind);
    let ctx = live_host_tick_from_sync(e, previous_pose);
    for module in drawable.get_draw_modules_mut() {
        let Some(logic) = module.logic_module_mut() else {
            continue;
        };
        apply_live_host_draw(logic, kind, &ctx);
    }
}
