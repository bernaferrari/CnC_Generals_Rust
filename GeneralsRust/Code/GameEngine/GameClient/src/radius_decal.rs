//! Radius decal rendering helpers.
//!
//! Port of `GameClient/RadiusDecal.cpp` using a lightweight projected shadow manager.

use game_engine::common::ini::{FieldParse, INIError, INIResult, INI};
use game_engine::common::system::{Coord3D, Xfer, XferMode, XferVersion};
use gamelogic::common::{
    AsciiString, Bool, Real, UnsignedInt, LOGICFRAMES_PER_SECOND, SHADOW_ALPHA_DECAL, SHADOW_NAMES,
};
use gamelogic::helpers::TheGameLogic;
use gamelogic::player::{Player, ThePlayerList};
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

fn real_to_int(value: Real) -> i32 {
    value.trunc() as i32
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
        if info.shadow_name.is_empty() || info.size_x <= 0.0 || info.size_y <= 0.0 {
            return None;
        }

        let decal = ShadowDecal::new(info.clone());
        let handle = ShadowHandle(Arc::new(Mutex::new(decal)));
        self.decals.push(handle.clone());
        Some(handle)
    }

    pub fn cleanup(&mut self) {
        self.decals.retain(|handle| handle.0.lock().active);
    }
}

static PROJECTED_SHADOW_MANAGER: OnceCell<RwLock<ProjectedShadowManager>> = OnceCell::new();

pub fn get_projected_shadow_manager() -> &'static RwLock<ProjectedShadowManager> {
    PROJECTED_SHADOW_MANAGER.get_or_init(|| RwLock::new(ProjectedShadowManager::new()))
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

    pub fn create_radius_decal(
        &self,
        pos: &Coord3D,
        radius: Real,
        owning_player: Option<Arc<RwLock<Player>>>,
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

        let owner_index = Some(owner.read().get_player_index());
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
                    owner.read().get_player_color().to_argb_u32()
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
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.color = token.parse().map_err(|_| INIError::InvalidData)?;
    Ok(())
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
}
