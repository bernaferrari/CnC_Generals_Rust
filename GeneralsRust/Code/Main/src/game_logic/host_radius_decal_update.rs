//! Host RadiusDecalUpdate residual (superweapon delivery targeting decal).
//!
//! C++: `RadiusDecalUpdate` + OCL `Attack` DeliveryDecal create path
//! (`ObjectCreationList.cpp` → `createRadiusDecal` + `killWhenNoLongerAttacking`).
//!
//! Retail peels:
//! - Module on `GLAScudStorm` / faction SW buildings (empty body)
//! - `SUPERWEAPON_ScudStorm` OCL: DeliveryDecalRadius **200**, texture
//!   `SCCScudStorm_GLA`, OpacityMin **25%**, OpacityMax **50%**,
//!   OpacityThrobTime **500**ms → **15**f, OnlyVisibleToOwningPlayer
//! - Nuclear missile DeliveryDecalRadius **210**
//! - SpecialPower ScudStorm RadiusCursorRadius **200**
//!
//! Live path: `HostRadiusDecal::create` calls
//! `game_client::radius_decal::enqueue_delivery_decal` so
//! `ProjectedShadowManager::collect_render_items` / `forward_render`
//! draws strike rings (C++ `RadiusDecal.cpp:61` addDecal).

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const RADIUS_DECAL_LOGIC_FPS: f32 = 30.0;

/// Retail SCUD storm OCL delivery decal radius.
pub const SCUD_STORM_DELIVERY_DECAL_RADIUS: f32 = 200.0;
/// Retail nuclear missile DeliveryDecalRadius residual.
pub const NUCLEAR_MISSILE_DELIVERY_DECAL_RADIUS: f32 = 210.0;
/// Retail OpacityThrobTime 500ms.
pub const DELIVERY_DECAL_THROB_MS: u32 = 500;
pub const DELIVERY_DECAL_THROB_FRAMES: u32 = 15;
/// Opacity min/max residual (0..1).
pub const DELIVERY_DECAL_OPACITY_MIN: f32 = 0.25;
pub const DELIVERY_DECAL_OPACITY_MAX: f32 = 0.50;
/// Retail SCUD texture peel.
pub const SCUD_STORM_DECAL_TEXTURE: &str = "SCCScudStorm_GLA";
/// Retail nuke texture peel.
pub const NUCLEAR_MISSILE_DECAL_TEXTURE: &str = "SCCNuclearMissile_China";

pub fn radius_decal_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * RADIUS_DECAL_LOGIC_FPS / 1000.0).round() as u32
}

/// Template residual for a delivery decal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostRadiusDecalTemplate {
    pub name: String,
    pub texture: String,
    pub opacity_min: f32,
    pub opacity_max: f32,
    pub throb_frames: u32,
    pub only_visible_to_owner: bool,
    /// RGB residual 0..255.
    pub color_rgb: [u8; 3],
}

impl HostRadiusDecalTemplate {
    pub fn scud_storm() -> Self {
        Self {
            name: "SUPERWEAPON_ScudStorm".into(),
            texture: SCUD_STORM_DECAL_TEXTURE.into(),
            opacity_min: DELIVERY_DECAL_OPACITY_MIN,
            opacity_max: DELIVERY_DECAL_OPACITY_MAX,
            throb_frames: DELIVERY_DECAL_THROB_FRAMES,
            only_visible_to_owner: true,
            color_rgb: [33, 255, 67],
        }
    }

    pub fn nuclear_missile() -> Self {
        Self {
            name: "NuclearMissile".into(),
            texture: NUCLEAR_MISSILE_DECAL_TEXTURE.into(),
            opacity_min: DELIVERY_DECAL_OPACITY_MIN,
            opacity_max: DELIVERY_DECAL_OPACITY_MAX,
            throb_frames: DELIVERY_DECAL_THROB_FRAMES,
            only_visible_to_owner: true,
            color_rgb: [255, 0, 0],
        }
    }

    pub fn valid(&self) -> bool {
        !self.name.is_empty() && !self.texture.is_empty()
    }
}

/// Live RadiusDecal residual instance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostRadiusDecal {
    pub empty: bool,
    pub position: Vec3,
    pub radius: f32,
    pub opacity: f32,
    pub template: Option<HostRadiusDecalTemplate>,
    pub birth_frame: u32,
    /// C++ `RadiusDecal::m_decal` — not serialized (C++ xfer clears on load).
    #[cfg(feature = "game_client")]
    #[serde(skip)]
    projected: Option<game_client::radius_decal::ShadowHandle>,
}

impl HostRadiusDecal {
    #[cfg(feature = "game_client")]
    fn release_projected(&mut self) {
        if let Some(handle) = self.projected.take() {
            handle.release();
            game_client::radius_decal::get_projected_shadow_manager()
                .write()
                .cleanup();
        }
    }

    pub fn clear(&mut self) {
        #[cfg(feature = "game_client")]
        self.release_projected();
        *self = Self {
            empty: true,
            ..Self::default()
        };
        self.empty = true;
    }

    pub fn is_empty(&self) -> bool {
        self.empty || self.template.is_none()
    }

    /// True when C++ `m_decal` is live in `TheProjectedShadowManager`.
    pub fn has_projected_shadow(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            self.projected.is_some()
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    pub fn create(tmpl: HostRadiusDecalTemplate, radius: f32, pos: Vec3, frame: u32) -> Self {
        let opacity = tmpl.opacity_min;
        let radius = radius.max(0.0);
        let empty = !tmpl.valid();
        #[cfg(feature = "game_client")]
        let projected = if empty {
            None
        } else {
            // C++ RadiusDecal.cpp:53-66 TheProjectedShadowManager->addDecal.
            game_client::radius_decal::enqueue_delivery_decal(
                &tmpl.texture,
                radius,
                pos.x,
                pos.y,
                pos.z,
                tmpl.color_rgb,
                opacity,
            )
        };
        Self {
            empty,
            position: pos,
            radius,
            opacity,
            template: if tmpl.valid() { Some(tmpl) } else { None },
            birth_frame: frame,
            #[cfg(feature = "game_client")]
            projected,
        }
    }

    /// C++ RadiusDecal::update — opacity throb residual, pushed to m_decal.
    pub fn update(&mut self, frame: u32) {
        if self.is_empty() {
            return;
        }
        let Some(tmpl) = self.template.as_ref() else {
            return;
        };
        let period = tmpl.throb_frames.max(1);
        let phase = frame.saturating_sub(self.birth_frame) % (period * 2);
        let t = if phase <= period {
            phase as f32 / period as f32
        } else {
            2.0 - (phase as f32 / period as f32)
        };
        self.opacity = tmpl.opacity_min + (tmpl.opacity_max - tmpl.opacity_min) * t;
        #[cfg(feature = "game_client")]
        if let Some(handle) = &self.projected {
            handle.set_opacity((self.opacity.clamp(0.0, 1.0) * 255.0).trunc() as i32);
        }
    }
}

impl Drop for HostRadiusDecal {
    fn drop(&mut self) {
        #[cfg(feature = "game_client")]
        self.release_projected();
    }
}

/// Per-object RadiusDecalUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostRadiusDecalUpdateData {
    pub delivery_decal: HostRadiusDecal,
    pub kill_when_no_longer_attacking: bool,
    pub awake: bool,
}

impl Default for HostRadiusDecalUpdateData {
    fn default() -> Self {
        Self {
            delivery_decal: HostRadiusDecal {
                empty: true,
                ..HostRadiusDecal::default()
            },
            kill_when_no_longer_attacking: false,
            awake: false,
        }
    }
}

impl HostRadiusDecalUpdateData {
    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_radius_decal_update_template(template_name) {
            Some(Self::default())
        } else {
            None
        }
    }

    pub fn create_radius_decal(
        &mut self,
        tmpl: HostRadiusDecalTemplate,
        radius: f32,
        pos: Vec3,
        frame: u32,
    ) {
        self.delivery_decal = HostRadiusDecal::create(tmpl, radius, pos, frame);
        self.awake = !self.delivery_decal.is_empty();
    }

    pub fn kill_radius_decal(&mut self) {
        self.delivery_decal.clear();
        self.kill_when_no_longer_attacking = false;
        self.awake = false;
    }

    pub fn set_kill_when_no_longer_attacking(&mut self, v: bool) {
        self.kill_when_no_longer_attacking = v;
    }

    /// One frame residual. `is_attacking` maps OBJECT_STATUS_IS_ATTACKING.
    /// Returns true if decal was killed this frame.
    pub fn tick(&mut self, frame: u32, is_attacking: bool) -> bool {
        if !self.awake {
            return false;
        }
        if self.kill_when_no_longer_attacking && !is_attacking {
            self.kill_radius_decal();
            return true;
        }
        self.delivery_decal.update(frame);
        false
    }
}

/// Superweapon buildings carrying RadiusDecalUpdate.
pub fn is_radius_decal_update_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("scudstorm")
        || n.contains("particlecannon")
        || n.contains("nuclearmissile")
        || n.contains("spectregunship") // deployment may also use cursor residual
}

/// Default OCL peel radius for a host template.
pub fn default_delivery_decal_radius_for_template(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("scudstorm") {
        SCUD_STORM_DELIVERY_DECAL_RADIUS
    } else if n.contains("nuclear")
        || n.contains("nuke")
        || n.contains("neutron")
        || n.contains("cruisemissile")
    {
        NUCLEAR_MISSILE_DELIVERY_DECAL_RADIUS
    } else {
        SCUD_STORM_DELIVERY_DECAL_RADIUS
    }
}

pub fn default_delivery_decal_template_for_host(name: &str) -> HostRadiusDecalTemplate {
    let n = name.to_ascii_lowercase();
    if n.contains("nuclear")
        || n.contains("nuke")
        || n.contains("neutron")
        || n.contains("cruisemissile")
        || (n.contains("china") && n.contains("missile"))
    {
        HostRadiusDecalTemplate::nuclear_missile()
    } else {
        HostRadiusDecalTemplate::scud_storm()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostRadiusDecalUpdateRegistry {
    pub installed: u32,
    pub creates: u32,
    pub kills: u32,
    pub attack_kills: u32,
    pub updates: u32,
}

impl HostRadiusDecalUpdateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_create(&mut self) {
        self.creates = self.creates.saturating_add(1);
    }
    pub fn record_kill(&mut self, from_attack_end: bool) {
        self.kills = self.kills.saturating_add(1);
        if from_attack_end {
            self.attack_kills = self.attack_kills.saturating_add(1);
        }
    }
    pub fn record_update(&mut self) {
        self.updates = self.updates.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.creates > 0
    }
}

pub fn honesty_radius_decal_update_residual_ok() -> bool {
    radius_decal_ms_to_frames(DELIVERY_DECAL_THROB_MS) == DELIVERY_DECAL_THROB_FRAMES
        && SCUD_STORM_DELIVERY_DECAL_RADIUS == 200.0
        && NUCLEAR_MISSILE_DELIVERY_DECAL_RADIUS == 210.0
        && is_radius_decal_update_template("GLAScudStorm")
        && is_radius_decal_update_template("ChinaNuclearMissileLauncher")
        && !is_radius_decal_update_template("AmericaTankCrusader")
        && HostRadiusDecalTemplate::scud_storm().valid()
        && HostRadiusDecalTemplate::scud_storm().texture == SCUD_STORM_DECAL_TEXTURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_radius_decal_update_residual_ok());
    }

    #[test]
    fn create_throb_and_kill_when_not_attacking() {
        let mut d = HostRadiusDecalUpdateData::default();
        d.create_radius_decal(
            HostRadiusDecalTemplate::scud_storm(),
            SCUD_STORM_DELIVERY_DECAL_RADIUS,
            Vec3::new(10.0, 0.0, 20.0),
            0,
        );
        d.set_kill_when_no_longer_attacking(true);
        assert!(!d.delivery_decal.is_empty());
        assert!(d.awake);
        d.tick(5, true);
        assert!((d.delivery_decal.opacity - DELIVERY_DECAL_OPACITY_MIN).abs() < 0.3);
        let killed = d.tick(10, false);
        assert!(killed);
        assert!(d.delivery_decal.is_empty());
    }

    /// C++ RadiusDecal.cpp:61 addDecal — host create must enqueue so
    /// `ProjectedShadowManager::collect_render_items` (forward_render.rs)
    /// draws the strike ring.
    #[cfg(feature = "game_client")]
    #[test]
    fn create_enqueues_projected_shadow_for_forward_render() {
        let mut d = HostRadiusDecalUpdateData::default();
        d.create_radius_decal(
            HostRadiusDecalTemplate::scud_storm(),
            SCUD_STORM_DELIVERY_DECAL_RADIUS,
            Vec3::new(1111.0, 2.0, 2222.0),
            0,
        );
        assert!(d.delivery_decal.has_projected_shadow());
        let items = game_client::radius_decal::get_projected_shadow_manager()
            .read()
            .collect_render_items();
        assert!(
            items.iter().any(|it| {
                (it.position.x - 1111.0).abs() < 0.01
                    && (it.position.y - 2.0).abs() < 0.01
                    && (it.position.z - 2222.0).abs() < 0.01
                    && (it.size - SCUD_STORM_DELIVERY_DECAL_RADIUS * 2.0).abs() < 0.01
            }),
            "forward_render collect_render_items must see host delivery ring"
        );
        d.tick(5, true);
        let items = game_client::radius_decal::get_projected_shadow_manager()
            .read()
            .collect_render_items();
        let ring = items.iter().find(|it| {
            (it.position.x - 1111.0).abs() < 0.01 && (it.position.z - 2222.0).abs() < 0.01
        });
        assert!(ring.is_some());
        assert!(ring.unwrap().color[3] > 0.0);

        d.set_kill_when_no_longer_attacking(true);
        assert!(d.tick(10, false));
        assert!(!d.delivery_decal.has_projected_shadow());
        let items = game_client::radius_decal::get_projected_shadow_manager()
            .read()
            .collect_render_items();
        assert!(!items.iter().any(|it| {
            (it.position.x - 1111.0).abs() < 0.01 && (it.position.z - 2222.0).abs() < 0.01
        }));
    }
}
