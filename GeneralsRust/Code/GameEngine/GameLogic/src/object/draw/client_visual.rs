//! Client-side visual hooks used by live W3D draw modules.
//!
//! GameLogic cannot depend on GameClient. Terrain decals, tread marks, texture
//! aspect, and asset preload therefore go through these registered hooks. The
//! GameClient drawable crate installs the live implementations at startup.

use crate::common::*;
use crate::object::ObjectScriptStatusBit;
use crate::object::draw::TerrainDecalType;
use parking_lot::RwLock;
use std::sync::{Arc, OnceLock};

/// Description of a terrain decal to create or replace (C++ `Shadow::ShadowTypeInfo`).
#[derive(Clone, Debug)]
pub struct TerrainDecalDesc {
    pub object_id: ObjectID,
    pub texture_name: String,
    pub size_x: Real,
    pub size_y: Real,
    pub opacity: Real,
    pub position: Coord3D,
    pub angle: Real,
    pub hidden: bool,
    pub shrouded: bool,
    pub shadow_enabled: bool,
    /// C++ `allocateShadows` / `addShadow` blob, not `setTerrainDecal` addDecal.
    pub is_unit_blob: bool,
}

/// GameClient implementation of projected terrain decals.
pub trait TerrainDecalClient: Send + Sync {
    fn set_decal(&self, desc: &TerrainDecalDesc);
    fn set_size(&self, object_id: ObjectID, x: Real, y: Real);
    fn set_opacity(&self, object_id: ObjectID, opacity: Real);
    fn set_pose(&self, object_id: ObjectID, position: Coord3D, angle: Real);
    fn set_shrouded(&self, object_id: ObjectID, shrouded: bool);
    fn set_shadow_enabled(&self, object_id: ObjectID, enabled: bool);
    fn release(&self, object_id: ObjectID);
}

/// GameClient implementation of `TheTerrainTracksRenderObjClassSystem`.
pub trait TerrainTrackClient: Send + Sync {
    fn bind_track(&self, object_id: ObjectID, width: Real, texture: &str) -> Option<u32>;
    fn unbind_track(&self, handle: u32);
    fn add_edge(&self, handle: u32, x: Real, y: Real, sync_time: u32);
    fn add_cap(&self, handle: u32, x: Real, y: Real, sync_time: u32);
    fn set_airborne(&self, handle: u32);
}

/// C++ `TerrainDecalTextureName` table in W3DModelDraw.cpp.
pub fn terrain_decal_texture_name(decal_type: TerrainDecalType) -> &'static str {
    match decal_type {
        TerrainDecalType::Demoralized => "DM_RING",
        TerrainDecalType::Horde => "EXHorde",
        TerrainDecalType::HordeWithNationalismUpgrade => "EXHorde_UP",
        TerrainDecalType::HordeVehicle => "EXHordeB",
        TerrainDecalType::HordeWithNationalismUpgradeVehicle => "EXHordeB_UP",
        TerrainDecalType::Crate => "EXJunkCrate",
        TerrainDecalType::HordeWithFanaticismUpgrade => "EXHordeC_UP",
        TerrainDecalType::ChemSuit => "EXChemSuit",
        TerrainDecalType::None | TerrainDecalType::ShadowTexture => "",
    }
}

/// C++ `ThingTemplate::validate` default when `ShadowTexture` is empty.
pub fn leftover_default_shadow_texture(
    geom: Option<game_engine::system::geometry::GeometryType>,
    authored: &str,
) -> String {
    if !authored.is_empty() {
        return authored.to_string();
    }
    match geom {
        Some(game_engine::system::geometry::GeometryType::Box) => "shadows".to_string(),
        Some(game_engine::system::geometry::GeometryType::Sphere)
        | Some(game_engine::system::geometry::GeometryType::Cylinder) => "shadow".to_string(),
        _ => "shadow".to_string(),
    }
}

static TERRAIN_DECAL_CLIENT: OnceLock<Arc<dyn TerrainDecalClient>> = OnceLock::new();
static TERRAIN_TRACK_CLIENT: OnceLock<Arc<dyn TerrainTrackClient>> = OnceLock::new();
static TEXTURE_ASPECT_HOOK: RwLock<Option<fn(&str) -> Option<Real>>> = RwLock::new(None);
static PRELOAD_ASSET_HOOK: RwLock<Option<fn(&str)>> = RwLock::new(None);
static RECEIVES_DYNAMIC_LIGHTS_HOOK: RwLock<Option<fn(ObjectID, bool)>> = RwLock::new(None);

pub fn register_terrain_decal_client(client: Arc<dyn TerrainDecalClient>) {
    let _ = TERRAIN_DECAL_CLIENT.set(client);
}

pub fn register_terrain_track_client(client: Arc<dyn TerrainTrackClient>) {
    let _ = TERRAIN_TRACK_CLIENT.set(client);
}

pub fn register_texture_aspect_hook(hook: fn(&str) -> Option<Real>) {
    *TEXTURE_ASPECT_HOOK.write() = Some(hook);
}

pub fn register_preload_asset_hook(hook: fn(&str)) {
    *PRELOAD_ASSET_HOOK.write() = Some(hook);
}
pub fn register_receives_dynamic_lights_hook(hook: fn(ObjectID, bool)) {
    *RECEIVES_DYNAMIC_LIGHTS_HOOK.write() = Some(hook);
}

/// C++ `Drawable::setReceivesDynamicLights`.
pub fn set_receives_dynamic_lights(object_id: ObjectID, receives: bool) {
    if let Some(hook) = *RECEIVES_DYNAMIC_LIGHTS_HOOK.read() {
        hook(object_id, receives);
    }
}

pub fn terrain_decal_client() -> Option<&'static Arc<dyn TerrainDecalClient>> {
    TERRAIN_DECAL_CLIENT.get()
}

pub fn terrain_track_client() -> Option<&'static Arc<dyn TerrainTrackClient>> {
    TERRAIN_TRACK_CLIENT.get()
}

pub fn texture_aspect_ratio(texture_name: &str) -> Option<Real> {
    (*TEXTURE_ASPECT_HOOK.read()).and_then(|hook| hook(texture_name))
}

pub fn preload_draw_asset(name: &str) {
    if name.is_empty() {
        return;
    }
    if let Some(hook) = *PRELOAD_ASSET_HOOK.read() {
        hook(name);
    }
}

/// C++ `Drawable::updateHiddenStatus` (`Drawable.cpp:4631-4633`).
/// Hidden = `m_hidden || m_hiddenByStealth`; that is when retail
/// `TheInGameUI->deselectDrawable` runs.
pub fn leftover_hidden_status_deselects(hidden: bool, hidden_by_stealth: bool) -> bool {
    hidden || hidden_by_stealth
}

/// C++ `Drawable::getShouldAnimate(considerPower)` object checks (flag form).
/// Live host drains this — leftover `Object` is a different type.
pub fn object_should_animate_flags(
    consider_power: bool,
    script_underpowered: bool,
    is_disabled: bool,
    produced_at_helipad: bool,
    disabled_hacked: bool,
    disabled_paralyzed: bool,
    disabled_emp: bool,
    disabled_subdued: bool,
    disabled_unmanned: bool,
    disabled_underpowered: bool,
) -> bool {
    if consider_power && script_underpowered {
        return false;
    }
    if is_disabled {
        if !produced_at_helipad
            && (disabled_hacked
                || disabled_paralyzed
                || disabled_emp
                || disabled_subdued
                || disabled_unmanned)
        {
            return false;
        }
        if consider_power && disabled_underpowered {
            return false;
        }
    }
    true
}

/// C++ `Drawable::getShouldAnimate(considerPower)` object checks.
pub fn object_should_animate(obj: &crate::object::Object, consider_power: bool) -> bool {
    object_should_animate_flags(
        consider_power,
        obj.test_script_status_bit(ObjectScriptStatusBit::ScriptUnderpowered),
        obj.is_disabled(),
        obj.is_kind_of(KindOf::ProducedAtHelipad),
        obj.is_disabled_by_type(DisabledType::DisabledHacked),
        obj.is_disabled_by_type(DisabledType::Paralyzed),
        obj.is_disabled_by_type(DisabledType::DisabledEmp),
        obj.is_disabled_by_type(DisabledType::DisabledSubdued),
        obj.is_disabled_by_type(DisabledType::DisabledUnmanned),
        obj.is_disabled_by_type(DisabledType::DisabledUnderpowered),
    )
}

#[cfg(test)]
mod leftover_should_animate_tests {
    use super::*;

    #[test]
    fn emp_hacked_underpowered_pause_like_cpp() {
        assert!(!object_should_animate_flags(
            true, false, true, false, false, false, true, false, false, false
        ));
        assert!(!object_should_animate_flags(
            true, false, true, false, true, false, false, false, false, false
        ));
        assert!(!object_should_animate_flags(
            true, false, true, false, false, false, false, false, false, true
        ));
        assert!(object_should_animate_flags(
            false, false, true, false, false, false, false, false, false, true
        ));
        assert!(object_should_animate_flags(
            true, false, true, true, false, false, true, false, false, false
        ));
        assert!(object_should_animate_flags(
            true, false, false, false, false, false, false, false, false, false
        ));
    }

    #[test]
    fn hidden_or_stealth_invisible_deselects() {
        assert!(leftover_hidden_status_deselects(true, false));
        assert!(leftover_hidden_status_deselects(false, true));
        assert!(!leftover_hidden_status_deselects(false, false));
    }
}
