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

/// Event-time receiving player facts, borrowed by value for one create call.
#[derive(Debug, Clone, Copy, Default)]
pub struct RadiusDecalPlayerView {
    pub local_player_index: Option<i32>,
    pub owner_player_index: Option<i32>,
    pub owner_argb: u32,
}

impl crate::game_logic::GameLogic {
    pub(crate) fn radius_decal_player_view(&self, owner: Option<u32>) -> RadiusDecalPlayerView {
        let local = self.local_player_id();
        let admitted_owner = owner.and_then(|id| self.get_player(id));
        let owner_argb = admitted_owner
            .map(|player| {
                let (r, g, b) = player.color_rgb;
                0xff00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
            })
            .unwrap_or(0);
        RadiusDecalPlayerView {
            local_player_index: local.map(|id| id as i32),
            owner_player_index: admitted_owner.map(|player| player.id as i32),
            owner_argb,
        }
    }
}

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
/// Retail SUPERWEAPON_AnthraxBomb DeliveryDecal Texture residual.
pub const ANTHRAX_BOMB_DECAL_TEXTURE: &str = "SCCAnthraxBomb_GLA";
/// Retail SUPERWEAPON_AnthraxBomb DeliveryDecal Color R:33 G:255 B:67 A:255 (ARGB).
pub const ANTHRAX_BOMB_DECAL_COLOR: u32 = 0xFF21_FF43;

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
    /// C++ `m_color`. Retail SW delivery blocks omit Color, so 0 → player color.
    pub color: u32,
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
            color: 0,
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
            color: 0,
        }
    }

    pub fn anthrax_bomb() -> Self {
        Self {
            name: "SUPERWEAPON_AnthraxBomb".into(),
            texture: ANTHRAX_BOMB_DECAL_TEXTURE.into(),
            opacity_min: DELIVERY_DECAL_OPACITY_MIN,
            opacity_max: DELIVERY_DECAL_OPACITY_MAX,
            throb_frames: DELIVERY_DECAL_THROB_FRAMES,
            only_visible_to_owner: true,
            color: ANTHRAX_BOMB_DECAL_COLOR,
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
        self.empty = true;
        self.position = Vec3::ZERO;
        self.radius = 0.0;
        self.opacity = 0.0;
        self.template = None;
        self.birth_frame = 0;
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
        // Detached component/preview convenience has an explicit synthetic
        // local identity. Actual world creates use create_with_player_view.
        Self::create_with_player_view(
            tmpl,
            radius,
            pos,
            frame,
            Some(0),
            RadiusDecalPlayerView {
                local_player_index: Some(0),
                owner_player_index: Some(0),
                owner_argb: 0,
            },
        )
    }

    /// C++ `createRadiusDecal`: `m_empty = false` first, then addDecal only when
    /// `!onlyVisible || owner == local`. Color==0 uses owning player color.
    pub fn create_with_owner(
        tmpl: HostRadiusDecalTemplate,
        radius: f32,
        pos: Vec3,
        frame: u32,
        owner_index: Option<i32>,
    ) -> Self {
        Self::create_with_player_view(
            tmpl,
            radius,
            pos,
            frame,
            owner_index,
            RadiusDecalPlayerView {
                owner_player_index: owner_index,
                ..Default::default()
            },
        )
    }

    pub fn create_with_player_view(
        tmpl: HostRadiusDecalTemplate,
        radius: f32,
        pos: Vec3,
        frame: u32,
        owner_index: Option<i32>,
        players: RadiusDecalPlayerView,
    ) -> Self {
        let radius = radius.max(0.0);
        if owner_index.is_none()
            || owner_index != players.owner_player_index
            || !tmpl.valid()
            || radius <= 0.0
        {
            return Self {
                empty: true,
                position: pos,
                radius,
                opacity: 0.0,
                template: None,
                birth_frame: frame,
                #[cfg(feature = "game_client")]
                projected: None,
            };
        }

        let opacity = tmpl.opacity_min;
        let local_index = players.local_player_index;
        let allow_decal = if !tmpl.only_visible_to_owner {
            true
        } else if let Some(owner) = owner_index {
            local_index == Some(owner)
        } else {
            false
        };
        let color = if tmpl.color == 0 {
            players.owner_argb
        } else {
            tmpl.color
        };

        #[cfg(feature = "game_client")]
        let projected = if allow_decal {
            game_client::radius_decal::enqueue_delivery_decal_argb(
                &tmpl.texture,
                radius,
                pos.x,
                pos.y,
                pos.z,
                color,
                opacity,
            )
        } else {
            None
        };

        Self {
            empty: false,
            position: pos,
            radius,
            opacity,
            template: Some(tmpl),
            birth_frame: frame,
            #[cfg(feature = "game_client")]
            projected,
        }
    }

    /// C++ RadiusDecal::update — sine of the driving frame, gated by getDrawIconUI.
    pub fn update(&mut self, frame: u32, draw_icon_ui: bool) {
        if self.is_empty() {
            return;
        }
        let Some(tmpl) = self.template.as_ref() else {
            return;
        };
        let now = frame;
        let period = tmpl.throb_frames.max(1);
        let theta = 2.0 * std::f32::consts::PI * ((now % period) as f32) / (period as f32);
        let percent = 0.5 * (theta.sin() + 1.0);
        self.opacity = if draw_icon_ui {
            tmpl.opacity_min + percent * (tmpl.opacity_max - tmpl.opacity_min)
        } else {
            0.0
        };
        #[cfg(feature = "game_client")]
        if let Some(handle) = &self.projected {
            handle.set_opacity((self.opacity.clamp(0.0, 1.0) * 255.0).trunc() as i32);
        }
    }

    /// C++ `RadiusDecal::setPosition` — move stored pose and the live projected shadow.
    pub fn set_position(&mut self, pos: Vec3) {
        self.position = pos;
        #[cfg(feature = "game_client")]
        if let Some(handle) = &self.projected {
            handle.set_position(pos.x, pos.y, pos.z);
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
        let mut delivery_decal = HostRadiusDecal::default();
        delivery_decal.empty = true;
        Self {
            delivery_decal,
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
        self.create_radius_decal_with_player_view(
            tmpl,
            radius,
            pos,
            frame,
            Some(0),
            RadiusDecalPlayerView {
                local_player_index: Some(0),
                owner_player_index: Some(0),
                owner_argb: 0,
            },
        );
    }

    pub fn create_radius_decal_for_owner(
        &mut self,
        tmpl: HostRadiusDecalTemplate,
        radius: f32,
        pos: Vec3,
        frame: u32,
        owner_index: Option<i32>,
    ) {
        self.create_radius_decal_with_player_view(
            tmpl,
            radius,
            pos,
            frame,
            owner_index,
            RadiusDecalPlayerView {
                owner_player_index: owner_index,
                ..Default::default()
            },
        );
    }

    pub fn create_radius_decal_with_player_view(
        &mut self,
        tmpl: HostRadiusDecalTemplate,
        radius: f32,
        pos: Vec3,
        frame: u32,
        owner_index: Option<i32>,
        players: RadiusDecalPlayerView,
    ) {
        self.delivery_decal = HostRadiusDecal::create_with_player_view(
            tmpl,
            radius,
            pos,
            frame,
            owner_index,
            players,
        );
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
    pub fn tick(&mut self, frame: u32, is_attacking: bool, draw_icon_ui: bool) -> bool {
        if !self.awake {
            return false;
        }
        if self.kill_when_no_longer_attacking && !is_attacking {
            self.kill_radius_decal();
            return true;
        }
        self.delivery_decal.update(frame, draw_icon_ui);
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
    if n.contains("anthrax") || n.contains("glajetcargoplane") {
        crate::game_logic::host_anthrax_bomb_flight::ANTHRAX_DELIVERY_DECAL_RADIUS
    } else if n.contains("scudstorm") {
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
    if n.contains("anthrax") || n.contains("glajetcargoplane") {
        HostRadiusDecalTemplate::anthrax_bomb()
    } else if n.contains("nuclear")
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
        && HostRadiusDecalTemplate::scud_storm().color == 0
        && HostRadiusDecalTemplate::nuclear_missile().color == 0
        && HostRadiusDecalTemplate::anthrax_bomb().valid()
        && HostRadiusDecalTemplate::anthrax_bomb().texture == ANTHRAX_BOMB_DECAL_TEXTURE
        && HostRadiusDecalTemplate::anthrax_bomb().color == ANTHRAX_BOMB_DECAL_COLOR
        && (default_delivery_decal_radius_for_template("GLAJetCargoPlane") - 200.0).abs() < 0.1
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
        d.tick(5, true, true);
        assert!((d.delivery_decal.opacity - DELIVERY_DECAL_OPACITY_MIN).abs() < 0.3);
        let killed = d.tick(10, false, true);
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
        d.tick(5, true, true);
        let items = game_client::radius_decal::get_projected_shadow_manager()
            .read()
            .collect_render_items();
        let ring = items.iter().find(|it| {
            (it.position.x - 1111.0).abs() < 0.01 && (it.position.z - 2222.0).abs() < 0.01
        });
        assert!(ring.is_some());
        assert!(ring.unwrap().color[3] > 0.0);

        d.set_kill_when_no_longer_attacking(true);
        assert!(d.tick(10, false, true));
        assert!(!d.delivery_decal.has_projected_shadow());
        let items = game_client::radius_decal::get_projected_shadow_manager()
            .read()
            .collect_render_items();
        assert!(!items.iter().any(|it| {
            (it.position.x - 1111.0).abs() < 0.01 && (it.position.z - 2222.0).abs() < 0.01
        }));
    }

    #[test]
    fn host_templates_use_color_zero_for_player_fallback() {
        assert_eq!(HostRadiusDecalTemplate::scud_storm().color, 0);
        assert_eq!(HostRadiusDecalTemplate::nuclear_missile().color, 0);
    }

    #[test]
    fn update_uses_sine_of_global_frame_not_triangle_birth() {
        let mut decal = HostRadiusDecal::create(
            HostRadiusDecalTemplate::scud_storm(),
            SCUD_STORM_DELIVERY_DECAL_RADIUS,
            Vec3::ZERO,
            100,
        );
        decal.update(0, true);
        let period = DELIVERY_DECAL_THROB_FRAMES.max(1);
        let theta = 2.0 * std::f32::consts::PI * (0.0 / period as f32);
        let percent = 0.5 * (theta.sin() + 1.0);
        let expected = DELIVERY_DECAL_OPACITY_MIN
            + percent * (DELIVERY_DECAL_OPACITY_MAX - DELIVERY_DECAL_OPACITY_MIN);
        assert!((decal.opacity - expected).abs() < 0.02);
    }

    #[test]
    fn create_skips_add_decal_when_only_visible_and_not_local() {
        let decal = HostRadiusDecal::create_with_owner(
            HostRadiusDecalTemplate::scud_storm(),
            SCUD_STORM_DELIVERY_DECAL_RADIUS,
            Vec3::new(1.0, 0.0, 2.0),
            0,
            Some(99),
        );
        assert!(!decal.is_empty());
        assert!(!decal.has_projected_shadow());
    }
}

#[cfg(test)]
mod player_owner_tests {
    use super::*;
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};

    fn world(rgb: (u8, u8, u8), local: u32) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut world = GameLogic::new();
        let mut owner = Player::new(0, Team::USA, "SameOwner", local == 0);
        owner.color_rgb = rgb;
        world.add_player(owner);
        world.add_player(Player::new(1, Team::GLA, "SameViewer", local == 1));
        let template = ThingTemplate::new("GLAScudStormOwnedColorProbe");
        world.templates.insert(template.name.clone(), template);
        let object = world
            .create_object("GLAScudStormOwnedColorProbe", Team::USA, Vec3::ZERO)
            .unwrap();
        world.host_object_mut(object).unwrap().owner_player_id = Some(0);
        (world, object)
    }
    #[test]
    fn delivery_create_uses_receiving_local_identity_and_color_after_restore_and_peer_reset() {
        let (mut source, object) = world((11, 22, 33), 0);
        let (mut peer, peer_object) = world((77, 88, 99), 1);
        assert_eq!(object, peer_object);
        assert_eq!(
            source.radius_decal_player_view(Some(0)).owner_argb,
            0xff0b1621
        );
        assert_eq!(
            peer.radius_decal_player_view(Some(0)).owner_argb,
            0xff4d5863
        );
        assert!(source.create_delivery_radius_decal(object, Vec3::new(111.0, 0.0, 222.0)));
        assert!(peer.create_delivery_radius_decal(peer_object, Vec3::new(333.0, 0.0, 444.0)));
        let decal = |world: &GameLogic| {
            world
                .host_object(object)
                .unwrap()
                .radius_decal_update
                .as_ref()
                .unwrap()
                .delivery_decal
                .has_projected_shadow()
        };
        #[cfg(feature = "game_client")]
        {
            assert!(decal(&source));
            assert!(!decal(&peer));
            let items = game_client::radius_decal::get_projected_shadow_manager()
                .read()
                .collect_render_items();
            let ring = items
                .iter()
                .find(|item| {
                    (item.position.x - 111.0).abs() < 0.01 && (item.position.z - 222.0).abs() < 0.01
                })
                .unwrap();
            assert!((ring.color[0] - 11.0 / 255.0).abs() < 0.001);
            assert!((ring.color[1] - 22.0 / 255.0).abs() < 0.001);
            assert!((ring.color[2] - 33.0 / 255.0).abs() < 0.001);
            assert!(
                !items
                    .iter()
                    .any(|item| (item.position.x - 333.0).abs() < 0.01
                        && (item.position.z - 444.0).abs() < 0.01)
            );
        }
        let builder = crate::save_load::SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&source).unwrap();
        let bytes = bincode_legacy::serialize(&saved).unwrap();
        let saved = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, prepared_object) = world((11, 22, 33), 0);
        assert_eq!(prepared_object, object);
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&saved, &mut restored)
            .unwrap();
        assert_eq!(
            restored.radius_decal_player_view(Some(0)).owner_argb,
            0xff0b1621
        );
        assert_eq!(
            restored
                .radius_decal_player_view(Some(0))
                .local_player_index,
            Some(0)
        );
        peer.reset();
        assert_eq!(
            source.radius_decal_player_view(Some(0)).owner_argb,
            0xff0b1621
        );
        assert_eq!(
            restored.radius_decal_player_view(Some(0)).owner_argb,
            0xff0b1621
        );
    }
    #[test]
    fn null_owner_clears_fixed_color_and_public_decal_without_local_color_fallback() {
        let (source, _) = world((11, 22, 33), 0);
        let view = source.radius_decal_player_view(None);
        assert_eq!(view.owner_argb, 0);
        assert_eq!(view.owner_player_index, None);
        let mut template = HostRadiusDecalTemplate::scud_storm();
        template.color = 0xff123456;
        template.only_visible_to_owner = false;
        let decal =
            HostRadiusDecal::create_with_player_view(template, 50.0, Vec3::ZERO, 7, None, view);
        assert!(decal.is_empty());
        assert!(!decal.has_projected_shadow());
        let unresolved = source.radius_decal_player_view(Some(99));
        assert_eq!(unresolved.owner_player_index, None);
        assert!(
            HostRadiusDecal::create_with_player_view(
                HostRadiusDecalTemplate::anthrax_bomb(),
                50.0,
                Vec3::ZERO,
                7,
                Some(99),
                unresolved
            )
            .is_empty()
        );
        assert!(
            HostRadiusDecal::create_with_owner(
                HostRadiusDecalTemplate::anthrax_bomb(),
                50.0,
                Vec3::ZERO,
                7,
                None
            )
            .is_empty()
        );
    }

    #[test]
    fn actual_spectre_create_uses_actor_owner_and_encoded_lazy_recreation_uses_receiver() {
        fn deployment(local: u32) -> (GameLogic, crate::game_logic::ObjectId) {
            let mut world = GameLogic::new();
            world.add_player(Player::new(0, Team::USA, "Owner", local == 0));
            world.add_player(Player::new(1, Team::GLA, "Viewer", local == 1));
            let template = ThingTemplate::new("AmericaCommandCenter");
            world.templates.insert(template.name.clone(), template);
            let caster = world
                .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
                .unwrap();
            (world, caster)
        }
        let (mut source, caster) = deployment(0);
        let (mut peer, peer_caster) = deployment(1);
        let ship = source
            .initiate_spectre_gunship_deployment(caster, Vec3::new(120.0, 0.0, 220.0))
            .unwrap();
        let peer_ship = peer
            .initiate_spectre_gunship_deployment(peer_caster, Vec3::new(320.0, 0.0, 420.0))
            .unwrap();
        assert_eq!(ship, peer_ship);
        assert_eq!(source.host_object(ship).unwrap().owner_player_id, Some(0));
        let data = source
            .host_object(ship)
            .unwrap()
            .spectre_gunship_update
            .as_ref()
            .unwrap();
        assert!(!data.attack_area_decal.is_empty());
        #[cfg(feature = "game_client")]
        {
            assert!(data.attack_area_decal.has_projected_shadow());
            let items = game_client::radius_decal::get_projected_shadow_manager()
                .read()
                .collect_render_items();
            let ring = items
                .iter()
                .find(|item| {
                    (item.position.x - 120.0).abs() < 0.01 && (item.position.z - 220.0).abs() < 0.01
                })
                .unwrap();
            assert!((ring.color[0] - 127.0 / 255.0).abs() < 0.001);
            assert!((ring.color[1] - 177.0 / 255.0).abs() < 0.001);
            assert!((ring.color[2] - 222.0 / 255.0).abs() < 0.001);
            let enemy = peer
                .host_object(peer_ship)
                .unwrap()
                .spectre_gunship_update
                .as_ref()
                .unwrap();
            assert!(!enemy.attack_area_decal.is_empty());
            assert!(!enemy.attack_area_decal.has_projected_shadow());
        }
        let builder = crate::save_load::SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&source).unwrap();
        let encoded = bincode_legacy::serialize(&saved).unwrap();
        let decoded = crate::save_load::decode_bincode_world_snapshot(&encoded).unwrap();
        let (mut restored, prepared_caster) = deployment(0);
        assert_eq!(prepared_caster, caster);
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        #[cfg(feature = "game_client")]
        assert!(
            !restored
                .host_object(ship)
                .unwrap()
                .spectre_gunship_update
                .as_ref()
                .unwrap()
                .attack_area_decal
                .has_projected_shadow()
        );
        // Actual receiving flight update lazily creates the derived projection.
        restored.update();
        #[cfg(feature = "game_client")]
        assert!(
            restored
                .host_object(ship)
                .unwrap()
                .spectre_gunship_update
                .as_ref()
                .unwrap()
                .attack_area_decal
                .has_projected_shadow()
        );
        // An authoritative missing actor owner suppresses both fixed-color rings.
        restored.host_object_mut(ship).unwrap().owner_player_id = None;
        restored
            .host_object_mut(ship)
            .unwrap()
            .spectre_gunship_update
            .as_mut()
            .unwrap()
            .clean_up_decals();
        restored.update();
        let data = restored
            .host_object(ship)
            .unwrap()
            .spectre_gunship_update
            .as_ref()
            .unwrap();
        assert!(data.attack_area_decal.is_empty());
        assert!(data.targeting_reticle_decal.is_empty());
        peer.reset();
        assert_eq!(source.host_object(ship).unwrap().owner_player_id, Some(0));
    }
}
