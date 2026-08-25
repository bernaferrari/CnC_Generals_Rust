//! Radius decal rendering helpers.
//!
//! Port of `GameClient/RadiusDecal.cpp` using a lightweight projected shadow manager.

use crate::effects::decals::DecalRenderItem;
use game_engine::common::game_lod;

use game_engine::common::ini::{FieldParse, INI, INIError, INIResult};
use game_engine::common::system::{Coord3D, Xfer, XferMode, XferVersion};
use gamelogic::common::{
    AsciiString, Bool, LOGICFRAMES_PER_SECOND, Real, SHADOW_ALPHA_DECAL, SHADOW_NAMES, UnsignedInt,
};
use gamelogic::helpers::TheGameLogic;
use gamelogic::player::{Player, ThePlayerList};
use nalgebra::Point3;
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock as StdRwLock};

fn real_to_int(value: Real) -> i32 {
    value.trunc() as i32
}

/// C++ `GameMakeColor` (`Color.h:37-39`): `(A << 24) | (R << 16) | (G << 8) | B`.
fn game_make_color(color: gamelogic::common::Color) -> u32 {
    ((color.a as u32) << 24) | ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32)
}

fn player_color_argb(player: &Player) -> u32 {
    let color = player.get_player_color();
    if color.r != 0 || color.g != 0 || color.b != 0 || color.a != 0 {
        return game_make_color(color);
    }
    ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_player(player.get_player_index()).cloned())
        .and_then(|real| {
            real.read()
                .ok()
                .map(|p| game_make_color(p.get_player_color()))
        })
        .unwrap_or(0)
}

fn argb_u32_to_rgba(color: u32, opacity: Real) -> [f32; 4] {
    [
        ((color >> 16) & 0xFF) as f32 / 255.0,
        ((color >> 8) & 0xFF) as f32 / 255.0,
        (color & 0xFF) as f32 / 255.0,
        opacity,
    ]
}

#[derive(Debug, Clone)]
pub struct ShadowTypeInfo {
    pub allow_updates: Bool,
    pub allow_world_align: Bool,
    pub shadow_type: u32,
    pub shadow_name: AsciiString,
    pub size_x: Real,
    pub size_y: Real,
}

#[derive(Debug, Clone)]
pub struct ShadowDecal {
    info: ShadowTypeInfo,
    angle: Real,
    color: u32,
    position: Coord3D,
    opacity: i32,
    active: Bool,
    /// C++ `addShadow` / `m_shadowList` blob, not `addDecal` / `m_decalList`.
    is_unit_blob: bool,
}

impl ShadowDecal {
    fn new(info: ShadowTypeInfo) -> Self {
        Self {
            info,
            angle: 0.0,
            color: 0xFFFF_FFFF,
            position: Coord3D::new(0.0, 0.0, 0.0),
            opacity: 255,
            active: true,
            is_unit_blob: false,
        }
    }

    fn set_angle(&mut self, angle: Real) {
        self.angle = angle;
    }

    fn set_color(&mut self, color: u32) {
        self.color = color;
    }

    fn set_position(&mut self, x: Real, y: Real, z: Real) {
        self.position = Coord3D::new(x, y, z);
    }

    fn set_opacity(&mut self, opacity: i32) {
        self.opacity = opacity;
    }

    fn release(&mut self) {
        self.active = false;
    }
}

#[derive(Clone, Debug)]
pub struct ShadowHandle(Arc<Mutex<ShadowDecal>>);

impl ShadowHandle {
    pub fn set_angle(&self, angle: Real) {
        self.0.lock().set_angle(angle);
    }

    pub fn set_color(&self, color: u32) {
        self.0.lock().set_color(color);
    }

    pub fn set_position(&self, x: Real, y: Real, z: Real) {
        self.0.lock().set_position(x, y, z);
    }

    pub fn set_opacity(&self, opacity: i32) {
        self.0.lock().set_opacity(opacity);
    }

    pub fn release(&self) {
        self.0.lock().release();
    }
}

#[derive(Debug, Default)]
pub struct ProjectedShadowManager {
    decals: Vec<ShadowHandle>,
}

impl ProjectedShadowManager {
    pub fn new() -> Self {
        Self { decals: Vec::new() }
    }

    pub fn add_decal(&mut self, info: &ShadowTypeInfo) -> Option<ShadowHandle> {
        self.insert_projected(info, false)
    }

    /// C++ `W3DProjectedShadowManager::addShadow` (`W3DProjectedShadow.cpp:1723`).
    /// Returns None when `!TheGlobalData->m_useShadowDecals`.
    pub fn add_shadow(&mut self, info: &ShadowTypeInfo) -> Option<ShadowHandle> {
        if !game_lod::use_shadow_decals() {
            return None;
        }
        self.insert_projected(info, true)
    }

    fn insert_projected(
        &mut self,
        info: &ShadowTypeInfo,
        is_unit_blob: bool,
    ) -> Option<ShadowHandle> {
        if info.shadow_name.is_empty() || info.size_x <= 0.0 || info.size_y <= 0.0 {
            return None;
        }

        let mut decal = ShadowDecal::new(info.clone());
        decal.is_unit_blob = is_unit_blob;
        let handle = ShadowHandle(Arc::new(Mutex::new(decal)));
        self.decals.push(handle.clone());
        Some(handle)
    }

    pub fn cleanup(&mut self) {
        self.decals.retain(|handle| handle.0.lock().active);
    }

    /// Convert stored projected decals into the live GPU decal items.
    ///
    /// C++ `W3DProjectedShadowManager::queueDecal` / `flushDecals` issues these
    /// as textured projected decals. The live wgpu path reuses
    /// `ParticleRenderer::render_decals`.
    ///
    /// C++ `renderShadows` (`W3DProjectedShadow.cpp:1303`) draws `m_shadowList`
    /// only when `m_useShadowDecals`. `m_decalList` / `addDecal` stays ungated.
    pub fn collect_render_items(&self) -> Vec<DecalRenderItem> {
        let blobs_on = game_lod::use_shadow_decals();
        let mut items = Vec::new();
        for handle in &self.decals {
            let decal = handle.0.lock();
            if !decal.active {
                continue;
            }
            if decal.is_unit_blob && !blobs_on {
                continue;
            }
            let opacity = decal.opacity.clamp(0, 255) as f32 / 255.0;
            if opacity <= 0.0 {
                continue;
            }
            let size_x = decal.info.size_x.max(0.0);
            let size_y = decal.info.size_y.max(0.0);
            let size = size_x.max(size_y);
            if size <= 0.0 {
                continue;
            }
            items.push(DecalRenderItem {
                position: Point3::new(decal.position.x, decal.position.y, decal.position.z),
                size,
                size_x,
                size_y,
                rotation: decal.angle,
                color: argb_u32_to_rgba(decal.color, opacity),
                texture_name: decal.info.shadow_name.as_str().to_string(),
                shadow_type: decal.info.shadow_type,
            });
        }
        items
    }
}

static PROJECTED_SHADOW_MANAGER: OnceCell<RwLock<ProjectedShadowManager>> = OnceCell::new();
static LEFTOVER_DECAL_HANDLES: OnceCell<RwLock<HashMap<u64, ShadowHandle>>> = OnceCell::new();
static LEFTOVER_DECAL_NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn leftover_decal_handles() -> &'static RwLock<HashMap<u64, ShadowHandle>> {
    LEFTOVER_DECAL_HANDLES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn leftover_add_decal(
    texture: &str,
    radius: f32,
    x: f32,
    y: f32,
    z: f32,
    color_argb: u32,
    opacity: f32,
    _shadow_type: u32,
) -> Option<u64> {
    let handle = enqueue_delivery_decal_argb(texture, radius, x, y, z, color_argb, opacity)?;
    let id = LEFTOVER_DECAL_NEXT_ID.fetch_add(1, Ordering::Relaxed);
    leftover_decal_handles().write().insert(id, handle);
    Some(id)
}

fn leftover_set_position(id: u64, x: f32, y: f32, z: f32) {
    if let Some(handle) = leftover_decal_handles().read().get(&id) {
        handle.set_position(x, y, z);
    }
}

fn leftover_set_opacity(id: u64, opacity: f32) {
    if let Some(handle) = leftover_decal_handles().read().get(&id) {
        handle.set_opacity(real_to_int(opacity.clamp(0.0, 1.0) * 255.0));
    }
}

fn leftover_set_color(id: u64, color_argb: u32) {
    if let Some(handle) = leftover_decal_handles().read().get(&id) {
        handle.set_color(color_argb);
    }
}

fn leftover_release(id: u64) {
    if let Some(handle) = leftover_decal_handles().write().remove(&id) {
        handle.release();
        get_projected_shadow_manager().write().cleanup();
    }
}

fn ensure_leftover_projected_hooks() {
    let _ = gamelogic::common::register_projected_radius_decal_hooks(
        gamelogic::common::ProjectedRadiusDecalHooks {
            add_decal: leftover_add_decal,
            set_position: leftover_set_position,
            set_opacity: leftover_set_opacity,
            set_color: leftover_set_color,
            release: leftover_release,
        },
    );
}

pub fn get_projected_shadow_manager() -> &'static RwLock<ProjectedShadowManager> {
    ensure_leftover_projected_hooks();
    PROJECTED_SHADOW_MANAGER.get_or_init(|| RwLock::new(ProjectedShadowManager::new()))
}

/// C++ `RadiusDecalTemplate::createRadiusDecal` (`RadiusDecal.cpp:53-66`):
/// `TheProjectedShadowManager->addDecal` then setAngle/setColor/setPosition.
/// Live host delivery rings must land here so `forward_render` flushDecals
/// (`collect_render_items`) can draw strike rings.
pub fn enqueue_delivery_decal(
    texture: &str,
    radius: Real,
    x: Real,
    y: Real,
    z: Real,
    color_rgb: [u8; 3],
    opacity: Real,
) -> Option<ShadowHandle> {
    let color =
        ((color_rgb[0] as u32) << 16) | ((color_rgb[1] as u32) << 8) | (color_rgb[2] as u32);
    enqueue_delivery_decal_argb(texture, radius, x, y, z, color, opacity)
}

pub fn enqueue_delivery_decal_argb(
    texture: &str,
    radius: Real,
    x: Real,
    y: Real,
    z: Real,
    color: u32,
    opacity: Real,
) -> Option<ShadowHandle> {
    if texture.is_empty() || radius <= 0.0 {
        return None;
    }
    ensure_leftover_projected_hooks();
    let info = ShadowTypeInfo {
        allow_updates: false,
        allow_world_align: true,
        shadow_type: SHADOW_ALPHA_DECAL,
        shadow_name: AsciiString::from(texture),
        size_x: radius * 2.0,
        size_y: radius * 2.0,
    };
    let handle = get_projected_shadow_manager().write().add_decal(&info)?;
    handle.set_angle(0.0);
    handle.set_color(color);
    handle.set_position(x, y, z);
    handle.set_opacity(real_to_int(opacity.clamp(0.0, 1.0) * 255.0));
    Some(handle)
}

/// Template for radius decals (mirrors GameClient/RadiusDecalTemplate).
#[derive(Debug, Clone)]
pub struct RadiusDecalTemplate {
    name: AsciiString,
    shadow_type: u32,
    min_opacity: Real,
    max_opacity: Real,
    opacity_throb_time: UnsignedInt,
    color: u32,
    only_visible_to_owning_player: Bool,
}

impl Default for RadiusDecalTemplate {
    fn default() -> Self {
        Self {
            name: AsciiString::TheEmptyString(),
            shadow_type: SHADOW_ALPHA_DECAL,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND,
            color: 0,
            only_visible_to_owning_player: true,
        }
    }
}

impl RadiusDecalTemplate {
    pub fn valid(&self) -> Bool {
        self.name.is_not_empty()
    }

    pub fn set_texture(&mut self, name: &str) {
        self.name.set(name);
    }

    pub fn with_texture(name: &str) -> Self {
        let mut template = Self::default();
        template.set_texture(name);
        template
    }

    pub fn xfer_radius_decal_template(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("xfer version failed: {e}"))?;

        let mut name = self.name.as_str().to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| format!("xfer name failed: {e}"))?;
        self.name.set(&name);

        xfer.xfer_unsigned_int(&mut self.shadow_type)
            .map_err(|e| format!("xfer shadow_type failed: {e}"))?;
        xfer.xfer_real(&mut self.min_opacity)
            .map_err(|e| format!("xfer min_opacity failed: {e}"))?;
        xfer.xfer_real(&mut self.max_opacity)
            .map_err(|e| format!("xfer max_opacity failed: {e}"))?;
        xfer.xfer_unsigned_int(&mut self.opacity_throb_time)
            .map_err(|e| format!("xfer opacity_throb_time failed: {e}"))?;
        xfer.xfer_unsigned_int(&mut self.color)
            .map_err(|e| format!("xfer color failed: {e}"))?;
        xfer.xfer_bool(&mut self.only_visible_to_owning_player)
            .map_err(|e| format!("xfer only_visible_to_owning_player failed: {e}"))?;

        Ok(())
    }

    pub fn min_opacity(&self) -> Real {
        self.min_opacity
    }

    pub fn max_opacity(&self) -> Real {
        self.max_opacity
    }

    pub fn opacity_throb_time(&self) -> UnsignedInt {
        self.opacity_throb_time
    }

    pub fn color(&self) -> u32 {
        self.color
    }

    /// C++ `parseRadiusDecalTemplate` retail InGameUI.ini *RadiusCursor throb peel.
    pub fn apply_retail_radius_cursor_parse(&mut self) {
        let mut ini = INI::new();
        let _ = parse_opacity_min(&mut ini, self, &["25%"]);
        let _ = parse_opacity_max(&mut ini, self, &["50%"]);
        let _ = parse_opacity_throb_time(&mut ini, self, &["500"]);
    }

    pub fn from_radius_cursor_texture(texture: &str) -> Self {
        if texture.is_empty() {
            return Self::default();
        }
        let mut template = Self::with_texture(texture);
        template.apply_retail_radius_cursor_parse();
        template
    }

    pub fn create_radius_decal(
        &self,
        pos: &Coord3D,
        radius: Real,
        owning_player: Option<Arc<StdRwLock<Player>>>,
        result: &mut RadiusDecal,
    ) {
        result.clear();

        let Some(owner) = owning_player else {
            log::error!("RadiusDecalTemplate::create_radius_decal requires owning player");
            return;
        };

        if self.name.is_empty() || radius <= 0.0 {
            return;
        }

        result.empty = false;

        let owner_index = owner.read().ok().map(|player| player.get_player_index());
        let local_index = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index());

        let allow_decal = if self.only_visible_to_owning_player {
            matches!((local_index, owner_index), (Some(local), Some(owner)) if local == owner)
        } else {
            true
        };

        if allow_decal {
            let decal_info = ShadowTypeInfo {
                allow_updates: false,
                allow_world_align: true,
                shadow_type: self.shadow_type,
                shadow_name: self.name.clone(),
                size_x: radius * 2.0,
                size_y: radius * 2.0,
            };

            let decal = get_projected_shadow_manager()
                .write()
                .add_decal(&decal_info);

            if let Some(handle) = decal {
                handle.set_angle(0.0);
                let color = if self.color == 0 {
                    owner
                        .read()
                        .ok()
                        .map(|player| player_color_argb(&player))
                        .unwrap_or(0)
                } else {
                    self.color
                };
                handle.set_color(color);
                handle.set_position(pos.x, pos.y, pos.z);
                result.decal = Some(handle);
                result.template = Some(self.clone());
            } else {
                log::error!(
                    "RadiusDecalTemplate: unable to add decal {}",
                    self.name.as_str()
                );
            }
        }
    }

    pub fn parse_radius_decal_template(
        ini: &mut INI,
        template: &mut RadiusDecalTemplate,
    ) -> INIResult<()> {
        ini.init_from_ini_with_fields(template, RADIUS_DECAL_FIELD_PARSE_TABLE)
    }
}

fn parse_texture(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.name = AsciiString::from(&INI::parse_ascii_string(token)?);
    Ok(())
}

fn parse_style(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.shadow_type = INI::parse_bit_string_32(tokens, &SHADOW_NAMES)?;
    Ok(())
}

fn parse_opacity_min(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.min_opacity = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_opacity_max(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.max_opacity = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_opacity_throb_time(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.opacity_throb_time = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_color(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.color = parse_color_int_tokens(tokens)?;
    Ok(())
}

/// C++ `INI::parseColorInt` / `GameMakeColor` (`R:G:B:[A:]` → ARGB).
fn parse_color_int_tokens(tokens: &[&str]) -> INIResult<u32> {
    if tokens.len() == 1 {
        if let Ok(value) = tokens[0].parse::<u32>() {
            return Ok(value);
        }
    }

    let mut r = None;
    let mut g = None;
    let mut b = None;
    let mut a = None;
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        let (key, value) = if let Some((left, right)) = token.split_once(':') {
            if right.is_empty() {
                i += 1;
                if i >= tokens.len() {
                    return Err(INIError::InvalidData);
                }
                (left, tokens[i])
            } else {
                (left, right)
            }
        } else {
            i += 1;
            if i >= tokens.len() {
                return Err(INIError::InvalidData);
            }
            (token, tokens[i])
        };
        let value: i32 = value.parse().map_err(|_| INIError::InvalidData)?;
        if !(0..=255).contains(&value) {
            return Err(INIError::InvalidData);
        }
        match key.to_ascii_uppercase().as_str() {
            "R" => r = Some(value as u8),
            "G" => g = Some(value as u8),
            "B" => b = Some(value as u8),
            "A" => a = Some(value as u8),
            _ => {}
        }
        i += 1;
    }
    let r = r.ok_or(INIError::InvalidData)?;
    let g = g.ok_or(INIError::InvalidData)?;
    let b = b.ok_or(INIError::InvalidData)?;
    let a = a.unwrap_or(255);
    Ok(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
}

fn parse_only_visible_to_owner(
    _ini: &mut INI,
    template: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.only_visible_to_owning_player = INI::parse_bool(token)?;
    Ok(())
}

const RADIUS_DECAL_FIELD_PARSE_TABLE: &[FieldParse<RadiusDecalTemplate>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_texture,
    },
    FieldParse {
        token: "Style",
        parse: parse_style,
    },
    FieldParse {
        token: "OpacityMin",
        parse: parse_opacity_min,
    },
    FieldParse {
        token: "OpacityMax",
        parse: parse_opacity_max,
    },
    FieldParse {
        token: "OpacityThrobTime",
        parse: parse_opacity_throb_time,
    },
    FieldParse {
        token: "Color",
        parse: parse_color,
    },
    FieldParse {
        token: "OnlyVisibleToOwningPlayer",
        parse: parse_only_visible_to_owner,
    },
];

#[derive(Debug, Default, Clone)]
pub struct RadiusDecal {
    template: Option<RadiusDecalTemplate>,
    decal: Option<ShadowHandle>,
    empty: Bool,
}

impl RadiusDecal {
    pub fn new() -> Self {
        Self {
            template: None,
            decal: None,
            empty: true,
        }
    }

    pub fn is_empty(&self) -> Bool {
        self.empty
    }

    pub fn xfer_radius_decal(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if xfer.get_xfer_mode() == XferMode::Load {
            self.clear();
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.template = None;
        if let Some(decal) = &self.decal {
            decal.release();
        }
        self.decal = None;
        self.empty = true;
    }

    pub fn update(&mut self) {
        let (Some(template), Some(decal)) = (self.template.as_ref(), self.decal.as_ref()) else {
            return;
        };

        if template.opacity_throb_time == 0 {
            return;
        }

        let now = TheGameLogic::get_frame();
        let theta = (2.0 * std::f32::consts::PI)
            * ((now % template.opacity_throb_time) as f32 / template.opacity_throb_time as f32);
        let percent = 0.5 * (theta.sin() + 1.0);
        let opac = if TheGameLogic::get_draw_icon_ui() {
            real_to_int(
                (template.min_opacity + percent * (template.max_opacity - template.min_opacity))
                    * 255.0,
            )
        } else {
            0
        };
        decal.set_opacity(opac);
    }

    pub fn set_opacity(&mut self, opacity: Real) {
        if let Some(decal) = &self.decal {
            decal.set_opacity(real_to_int(255.0 * opacity));
        }
    }

    pub fn set_position(&mut self, pos: &Coord3D) {
        if let Some(decal) = &self.decal {
            decal.set_position(pos.x, pos.y, pos.z);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn test_shadow_handle() -> ShadowHandle {
        ShadowHandle(Arc::new(Mutex::new(ShadowDecal::new(ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: SHADOW_ALPHA_DECAL,
            shadow_name: AsciiString::from("test"),
            size_x: 1.0,
            size_y: 1.0,
        }))))
    }

    #[test]
    fn set_opacity_truncates_like_cpp_real_to_int() {
        let handle = test_shadow_handle();
        let mut radius_decal = RadiusDecal {
            template: None,
            decal: Some(handle.clone()),
            empty: false,
        };

        radius_decal.set_opacity(0.5);
        assert_eq!(handle.0.lock().opacity, 127);

        radius_decal.set_opacity(0.999);
        assert_eq!(handle.0.lock().opacity, 254);
    }

    #[test]
    fn real_to_int_truncates_toward_zero() {
        assert_eq!(real_to_int(127.9), 127);
        assert_eq!(real_to_int(-127.9), -127);
    }

    #[test]
    fn xfer_radius_decal_save_writes_no_bytes_like_cpp_todo() {
        let handle = test_shadow_handle();
        let mut radius_decal = RadiusDecal {
            template: None,
            decal: Some(handle),
            empty: false,
        };
        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut xfer = XferSave::new(cursor, 0);
            radius_decal.xfer_radius_decal(&mut xfer).unwrap();
        }

        assert!(bytes.is_empty());
        assert!(!radius_decal.empty);
        assert!(radius_decal.decal.is_some());
    }

    #[test]
    fn xfer_radius_decal_load_clears_without_reading_like_cpp_todo() {
        let handle = test_shadow_handle();
        let mut radius_decal = RadiusDecal {
            template: Some(RadiusDecalTemplate::default()),
            decal: Some(handle.clone()),
            empty: false,
        };
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut xfer = XferLoad::new(cursor, 0);

        radius_decal.xfer_radius_decal(&mut xfer).unwrap();

        assert!(radius_decal.empty);
        assert!(radius_decal.template.is_none());
        assert!(radius_decal.decal.is_none());
        assert!(!handle.0.lock().active);
    }

    /// C++ `W3DProjectedShadowManager::queueDecal`/`flushDecals` emits the
    /// stored ShadowHandle as a projected decal. Live path must produce a
    /// `DecalRenderItem` so `ParticleRenderer::render_decals` can draw it.
    #[test]
    fn projected_shadow_manager_collects_active_radius_decals() {
        let mut manager = ProjectedShadowManager::new();
        let info = ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: SHADOW_ALPHA_DECAL,
            shadow_name: AsciiString::from("EXScudStorm"),
            size_x: 80.0,
            size_y: 80.0,
        };
        let handle = manager.add_decal(&info).expect("valid decal");
        handle.set_position(10.0, 20.0, 3.0);
        handle.set_color(0x00FF_3300);
        handle.set_opacity(128);
        handle.set_angle(0.5);

        let items = manager.collect_render_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].position, Point3::new(10.0, 20.0, 3.0));
        assert_eq!(items[0].size, 80.0);
        assert_eq!(items[0].size_x, 80.0);
        assert_eq!(items[0].size_y, 80.0);
        assert_eq!(items[0].texture_name, "EXScudStorm");
        assert_eq!(items[0].shadow_type, SHADOW_ALPHA_DECAL);
        assert!((items[0].rotation - 0.5).abs() < f32::EPSILON);
        assert!((items[0].color[0] - 1.0).abs() < 0.01);
        assert!((items[0].color[1] - 0.2).abs() < 0.01);
        assert!((items[0].color[3] - 128.0 / 255.0).abs() < 0.01);

        handle.release();
        assert!(manager.collect_render_items().is_empty());
    }

    /// C++ addShadow NULL + renderShadows skip when !m_useShadowDecals.
    /// addDecal radius/terrain items stay visible.
    #[test]
    fn add_shadow_and_blob_collect_none_when_use_shadow_decals_off() {
        let prev = game_engine::common::global_data::read_safe()
            .map(|g| g.writable.use_shadow_decals)
            .unwrap_or(true);
        let restore = || {
            if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
                runtime.writable.use_shadow_decals = prev;
            }
        };

        let blob_info = ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: gamelogic::common::SHADOW_DECAL,
            shadow_name: AsciiString::from("shadow"),
            size_x: 20.0,
            size_y: 20.0,
        };
        let ring_info = ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: SHADOW_ALPHA_DECAL,
            shadow_name: AsciiString::from("SCCScudStorm_GLA"),
            size_x: 80.0,
            size_y: 80.0,
        };

        if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
            runtime.writable.use_shadow_decals = true;
        }
        let mut manager = ProjectedShadowManager::new();
        let blob = manager.add_shadow(&blob_info).expect("blob while 2D on");
        blob.set_position(1.0, 2.0, 3.0);
        let ring = manager.add_decal(&ring_info).expect("addDecal ungated");
        ring.set_position(9.0, 8.0, 7.0);
        assert_eq!(manager.collect_render_items().len(), 2);

        if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
            runtime.writable.use_shadow_decals = false;
        }
        assert!(
            manager.add_shadow(&blob_info).is_none(),
            "C++ addShadow returns NULL when !UseShadowDecals"
        );
        let still_ring = manager.add_decal(&ring_info);
        assert!(still_ring.is_some(), "addDecal stays ungated");
        if let Some(extra) = still_ring {
            extra.release();
        }
        let items = manager.collect_render_items();
        assert_eq!(items.len(), 1, "2D Shadows off hides unit blobs only");
        assert_eq!(items[0].texture_name, "SCCScudStorm_GLA");

        blob.release();
        ring.release();
        restore();
    }

    /// C++ RadiusDecal.cpp:61 addDecal — global manager must expose the ring
    /// to Display/forward_render collect_render_items.
    #[test]
    fn enqueue_delivery_decal_is_visible_to_collect_render_items() {
        let handle = enqueue_delivery_decal(
            "SCCScudStorm_GLA",
            200.0,
            4242.5,
            3.0,
            4243.5,
            [33, 255, 67],
            0.25,
        )
        .expect("enqueue");
        let items = get_projected_shadow_manager().read().collect_render_items();
        assert!(
            items.iter().any(|it| {
                (it.position.x - 4242.5).abs() < 0.01
                    && (it.position.y - 3.0).abs() < 0.01
                    && (it.position.z - 4243.5).abs() < 0.01
                    && (it.size - 400.0).abs() < 0.01
                    && (it.color[3] - 0.25).abs() < 0.02
            }),
            "forward_render collect_render_items must see delivery ring"
        );
        handle.release();
        get_projected_shadow_manager().write().cleanup();
        let items = get_projected_shadow_manager().read().collect_render_items();
        assert!(!items.iter().any(|it| {
            (it.position.x - 4242.5).abs() < 0.01 && (it.position.z - 4243.5).abs() < 0.01
        }));
    }

    #[test]
    fn parse_color_int_uses_game_make_color_argb() {
        assert_eq!(
            parse_color_int_tokens(&["R:255", "G:0", "B:128", "A:64"]).unwrap(),
            0x40FF_0080
        );
        assert_eq!(
            parse_color_int_tokens(&["R:255", "G:0", "B:0"]).unwrap(),
            0xFFFF_0000
        );
    }

    #[test]
    fn retail_radius_cursor_parse_sets_throb() {
        let template = RadiusDecalTemplate::from_radius_cursor_texture("SCCAttackDamageArea");
        assert!((template.min_opacity() - 0.25).abs() < f32::EPSILON);
        assert!((template.max_opacity() - 0.50).abs() < f32::EPSILON);
        assert_eq!(template.opacity_throb_time(), 15);
        assert_eq!(template.color(), 0);
    }

    #[test]
    fn game_make_color_packs_argb_not_abgr() {
        let color = gamelogic::common::Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        assert_eq!(game_make_color(color), 0xFFFF_0000);
        let rgba = argb_u32_to_rgba(game_make_color(color), 1.0);
        assert!((rgba[0] - 1.0).abs() < 0.01);
        assert!(rgba[1].abs() < 0.01);
        assert!(rgba[2].abs() < 0.01);
    }
}
