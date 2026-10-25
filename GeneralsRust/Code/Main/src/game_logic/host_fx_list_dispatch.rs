//! Live-host FXList nugget dispatch + Sound-nugget audio names.
//!
//! C++ `FXList::doFXPos` / `doFXObj` (FXList.cpp:782-805) run every nugget.
//! `SoundFXNugget` plays `AudioEventRTS(m_soundName)` — the name *inside*
//! the FXList, never the FXList template name (FXList.cpp:79-100).
//!
//! The GameClient runner is registered via `register_fx_list_manager_bridge`.
//! This module is the live-host call into that runner.

use glam::Vec3;
#[cfg(feature = "game_client")]
#[path = "host_fx_runtime.rs"]
mod receiving;
#[cfg(feature = "game_client")]
pub(crate) use receiving::{HostFxRuntime, HostFxState};
#[cfg(feature = "game_client")]
#[path = "object_fx_runtime.rs"]
mod object_receiving;
#[cfg(feature = "game_client")]
pub(crate) use object_receiving::{ObjectFxEnvironment, ObjectFxServices};

/// Authored FXList template names (`FX_*` / `WeaponFX_*`) or the leftover
/// `FX:{template}` audio-queue encoding. These are not Miles event names.
pub fn is_authored_fx_list_name(name: &str) -> bool {
    let raw = strip_fx_list_prefix(name);
    !raw.is_empty()
        && (raw.starts_with("FX_") || raw.starts_with("WeaponFX_") || name.starts_with("FX:"))
}

fn strip_fx_list_prefix(name: &str) -> &str {
    name.strip_prefix("FX:").unwrap_or(name).trim()
}

fn is_none_fx_list(name: &str) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case("None")
}

/// Host world is Y-up `(x, height, z_ground)`. Leftover / C++ `Coord3D`
/// is Z-up `(x, y_ground, z_height)` so shroud + particle nuggets land
/// on the C++ XY ground plane.
fn host_to_leftover_coord(pos: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y)
}

/// Host Y-up `Mat4` → leftover/C++ Z-up `Mat4` (`P * T * P`, P swaps Y/Z).
///
/// C++ `Weapon.cpp:939` passes `sourceObj->getDrawable()->getTransformMatrix()`
/// so ParticleSystem Offset / OrientToObject follow the unit, not world axes.
pub fn host_to_leftover_mat4(host: glam::Mat4) -> glam::Mat4 {
    let x = host.x_axis;
    let y = host.y_axis;
    let z = host.z_axis;
    let t = host.w_axis;
    glam::Mat4::from_cols(
        glam::Vec4::new(x.x, x.z, x.y, x.w),
        glam::Vec4::new(z.x, z.z, z.y, z.w),
        glam::Vec4::new(y.x, y.z, y.y, y.w),
        glam::Vec4::new(t.x, t.z, t.y, t.w),
    )
}

/// C++ `FXList::doFXPos` — run every nugget via the registered GameClient runner.
///
/// Returns `true` when a manager was registered (the C++ client path owns
/// playback, including Sound nuggets). Returns `false` when the runner is
/// absent so callers can fall back to Sound-nugget audio names.
pub fn dispatch_fx_list_at_pos(name: &str, pos: Vec3) -> bool {
    dispatch_fx_list_at_pos_ex(name, pos, None, 0.0, 0.0)
}

/// C++ `FXList::doFXPos(fxl, pos, mtx, weaponSpeed, victimPos, damageRadius)`.
///
/// Tracer and RayEffect nuggets require a secondary endpoint. The 2-arg helper
/// keeps non-weapon callers unchanged (secondary = None).
pub fn dispatch_fx_list_at_pos_ex(
    name: &str,
    pos: Vec3,
    secondary: Option<Vec3>,
    primary_speed: f32,
    override_radius: f32,
) -> bool {
    dispatch_fx_list_at_pos_oriented(name, pos, secondary, primary_speed, override_radius, None)
}

/// Same as [`dispatch_fx_list_at_pos_ex`] with C++ drawable transform.
pub fn dispatch_fx_list_at_pos_oriented(
    name: &str,
    pos: Vec3,
    secondary: Option<Vec3>,
    primary_speed: f32,
    override_radius: f32,
    matrix: Option<glam::Mat4>,
) -> bool {
    let name = strip_fx_list_prefix(name);
    if is_none_fx_list(name) {
        return false;
    }
    let Some(fx) = gamelogic::helpers::TheFXList::get() else {
        return false;
    };
    let leftover_pos = host_to_leftover_coord(pos);
    let leftover_secondary = secondary.map(host_to_leftover_coord);
    let leftover_matrix = matrix.map(host_to_leftover_mat4);
    fx.do_fx_at_position_ex(
        name,
        &leftover_pos,
        leftover_secondary.as_ref(),
        primary_speed,
        override_radius,
        leftover_matrix.as_ref(),
    );
    gamelogic::helpers::get_fx_list_manager().is_some()
}

/// C++ `FXList::doFXObj` — object form used by death, TransitionDamage, DamageFX.
///
/// Passes the primary object so ParticleSystem nuggets get the object transform
/// (`OrientToObject` / `AttachToObject` / `FXListAtBonePos`).
pub fn dispatch_fx_list_at_object(name: &str, primary_id: u32, secondary_id: Option<u32>) -> bool {
    let name = strip_fx_list_prefix(name);
    if is_none_fx_list(name) {
        return false;
    }
    let Some(fx) = gamelogic::helpers::TheFXList::get() else {
        return false;
    };
    fx.do_fx_obj(name, primary_id, secondary_id);
    gamelogic::helpers::get_fx_list_manager().is_some()
}

/// Publish a live host object's leftover-space pose for `doFXObj`.
///
/// Production never fills leftover `OBJECT_REGISTRY`. GameClient leftover
/// `doFXObj` reads this table (then the live drawable) so death/transition
/// FX still get `OrientToObject` / `AttachToObject` / `FXListAtBonePos`.
pub fn publish_host_fx_object(id: u32, pos: Vec3, orientation: f32, player_index: i32) {
    publish_host_fx_object_ex(id, pos, orientation, player_index, 0.0);
}

/// Same as [`publish_host_fx_object`] with C++ bounding-circle radius.
pub fn publish_host_fx_object_ex(
    id: u32,
    pos: Vec3,
    orientation: f32,
    player_index: i32,
    bounding_circle_radius: f32,
) {
    publish_host_fx_object_with_visibility(
        id,
        pos,
        orientation,
        player_index,
        bounding_circle_radius,
        host_object_is_shrouded_for_local(id),
    );
}

/// Native emission uses the receiving observer's answer, including false.
pub fn publish_host_fx_object_with_visibility(
    id: u32,
    pos: Vec3,
    orientation: f32,
    player_index: i32,
    bounding_circle_radius: f32,
    is_shrouded: bool,
) {
    publish_host_fx_object_pose(
        id,
        pos,
        orientation,
        player_index,
        bounding_circle_radius,
        is_shrouded,
    );
}

fn publish_host_fx_object_pose(
    id: u32,
    pos: Vec3,
    orientation: f32,
    player_index: i32,
    bounding_circle_radius: f32,
    is_shrouded: bool,
) {
    gamelogic::helpers::set_host_fx_object_pose(host_fx_object_pose(
        id,
        pos,
        orientation,
        player_index,
        bounding_circle_radius,
        is_shrouded,
    ));
}

fn host_fx_object_pose(
    id: u32,
    pos: Vec3,
    orientation: f32,
    player_index: i32,
    bounding_circle_radius: f32,
    is_shrouded: bool,
) -> gamelogic::helpers::HostFxObjectPose {
    let leftover_pos = host_to_leftover_coord(pos);
    let transform = glam::Mat4::from_translation(glam::Vec3::new(
        leftover_pos.x,
        leftover_pos.y,
        leftover_pos.z,
    )) * glam::Mat4::from_rotation_z(orientation);
    gamelogic::helpers::HostFxObjectPose {
        id,
        position: leftover_pos,
        transform,
        player_index,
        bounding_circle_radius,
        is_shrouded,
    }
}

pub(crate) fn host_object_is_shrouded_for_local(id: u32) -> bool {
    use gamelogic::common::types::ObjectShroudStatus;
    // C++ PartitionManager.cpp:3017-3023 fails closed: a negative player index
    // or a missing cell reports SHROUDED. An unanswerable query here must
    // suppress host FX, never leak it.
    let player = gamelogic::player::player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())
        .unwrap_or(-1);
    if player < 0 {
        return true;
    }
    let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
    let Ok(shroud) = shroud_manager.lock() else {
        return true;
    };
    match shroud.get_host_object_shroud_status(player as u32, id) {
        Some(status) => (status as u8) >= (ObjectShroudStatus::Fogged as u8),
        None => false,
    }
}

/// Keep leftover attached systems following live host IDs.
///
/// `ParticleSystem::resolve_attached_parent` cannot see leftover
/// OBJECT_REGISTRY on the production host path. Upsert current poses
/// (including dying wrecks still in the frame) and drop IDs that left
/// so the next leftover tick follows or dies.
pub fn refresh_host_fx_object_poses_from_presentation(
    frame: &crate::presentation_frame::PresentationFrame,
) {
    let poses = frame.objects.iter().map(|object| {
        let is_shrouded = (object.drawable_shroud.raw_status as u8)
            >= (crate::presentation_frame::PresentationObjectShroudStatus::Fogged as u8);
        host_fx_object_pose(
            object.id.0,
            object.position,
            object.orientation,
            object
                .owner_player_id
                .map(|player| player as i32)
                .unwrap_or(-1),
            0.0,
            is_shrouded,
        )
    });
    gamelogic::helpers::replace_host_fx_object_poses_for_frame(poses);
}

fn host_object_fx_radius(obj: &crate::game_logic::Object) -> f32 {
    crate::game_logic::host_supply_gather::host_bounding_circle_radius(
        (*obj.geometry_info()).authored,
        (*obj.geometry_info()).bounding_circle_radius(),
        obj.thing().geometry.radius.max(obj.selection_radius),
    )
}

impl crate::game_logic::GameLogic {
    /// CPP doFXPos samples a positional cell, separately from an object's
    /// attached-FX status. Both viewer and shroud belong to this match.
    pub(crate) fn dispatch_fx_list_at_host_position(&mut self, name: &str, position: Vec3) -> bool {
        self.dispatch_fx_list_at_host_position_with_effects(name, position, &mut Default::default())
    }

    pub(crate) fn dispatch_fx_list_at_host_position_with_effects(
        &mut self,
        name: &str,
        position: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        self.dispatch_fx_list_at_host_position_oriented_with_effects(
            name, position, None, 0.0, 0.0, None, effects,
        )
    }

    pub(crate) fn dispatch_fx_list_at_host_position_oriented(
        &mut self,
        name: &str,
        position: Vec3,
        secondary: Option<Vec3>,
        speed: f32,
        radius: f32,
        matrix: Option<glam::Mat4>,
    ) -> bool {
        self.dispatch_fx_list_at_host_position_oriented_with_effects(
            name,
            position,
            secondary,
            speed,
            radius,
            matrix,
            &mut Default::default(),
        )
    }

    pub(crate) fn dispatch_fx_list_at_host_position_oriented_with_effects(
        &mut self,
        name: &str,
        position: Vec3,
        secondary: Option<Vec3>,
        speed: f32,
        radius: f32,
        matrix: Option<glam::Mat4>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let name = strip_fx_list_prefix(name);
        if is_none_fx_list(name) {
            return false;
        }
        #[cfg(feature = "game_client")]
        {
            let source = game_client::fx_list::FxSource::Position {
                primary: Some(host_to_leftover_coord(position)),
                matrix: matrix.map(host_to_leftover_mat4),
                secondary: secondary.map(host_to_leftover_coord),
                speed,
                radius,
            };
            match game_client::fx_list::execute_fx_with_runtime(
                &mut receiving::HostFxRuntime {
                    world: self,
                    effects,
                },
                name,
                source,
            ) {
                Ok(found) => found,
                Err(error) => {
                    log::error!("FX '{name}' failed at receiving nugget: {error}");
                    true
                }
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            dispatch_fx_list_at_pos_oriented(name, position, secondary, speed, radius, matrix)
        }
    }

    /// Borrow only the camera scalar owner across immediate nuggets. Object
    /// observations are already owned before entering here, and no catalog,
    /// shroud or global-rules guard survives a callback.
    #[cfg(all(test, feature = "game_client"))]
    pub(crate) fn with_host_fx_view<R>(
        &mut self,
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        let player = self.local_player_id();
        Self::with_host_fx_view_parts(
            &mut self.session_view,
            player,
            self.world_services.shroud(),
            run,
        )
    }

    #[cfg(all(test, feature = "game_client"))]
    pub(crate) fn with_host_fx_view_and_terrain<R>(
        &mut self,
        terrain: Option<&mut dyn crate::game_logic::NativeTerrainEffects>,
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        let player = self.local_player_id();
        Self::with_host_fx_view_parts_and_terrain(
            &mut self.session_view,
            player,
            self.world_services.shroud(),
            terrain,
            run,
        )
    }

    #[cfg(all(test, feature = "game_client"))]
    fn with_host_fx_view_and_terrain_and_bones<R>(
        &mut self,
        terrain: Option<&mut dyn crate::game_logic::NativeTerrainEffects>,
        bone_transforms: &mut dyn FnMut(u32, &str, i32, usize) -> Vec<glam::Mat4>,
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        let player = self.local_player_id();
        let mut terrain = terrain;
        let mut add_scorch = |position, radius, kind| {
            if let Some(receiver) = terrain.as_mut() {
                receiver.add_scorch(position, radius, kind);
            }
        };
        Self::with_host_fx_view_parts_with_scorch_and_bones(
            &mut self.session_view,
            player,
            self.world_services.shroud(),
            &mut add_scorch,
            bone_transforms,
            run,
        )
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn dispatch_fx_list_at_position_with_view(
        name: &str,
        position: Vec3,
        secondary: Option<Vec3>,
        speed: f32,
        radius: f32,
        matrix: Option<glam::Mat4>,
        view: &mut ObjectFxServices<'_, '_>,
    ) -> bool {
        let name = strip_fx_list_prefix(name);
        if is_none_fx_list(name) {
            return false;
        }
        view.position(name, position, secondary, speed, radius, matrix)
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn dispatch_fx_list_at_object_with_view(
        name: &str,
        primary: &crate::game_logic::Object,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
        view: &mut ObjectFxServices<'_, '_>,
    ) -> bool {
        let name = strip_fx_list_prefix(name);
        if is_none_fx_list(name) {
            return false;
        }
        view.object(name, primary, secondary)
    }

    #[cfg(all(test, feature = "game_client"))]
    pub(crate) fn with_host_fx_view_parts<R>(
        session_view: &mut crate::presentation_frame::SessionView,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        // Standalone compatibility entrypoint; native production roots must
        // pass their actual borrowed terrain via the receiving variant.
        let mut add_scorch = |position, radius, kind| {
            game_client::terrain::scorch_mesh::add_terrain_scorch(position, radius, kind);
        };
        Self::with_host_fx_view_parts_with_scorch(
            session_view,
            player,
            shroud,
            &mut add_scorch,
            run,
        )
    }

    #[cfg(all(test, feature = "game_client"))]
    pub(crate) fn with_host_fx_view_parts_and_terrain<R>(
        session_view: &mut crate::presentation_frame::SessionView,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        mut terrain: Option<&mut dyn crate::game_logic::NativeTerrainEffects>,
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        let mut add_scorch = |position, radius, kind| {
            if let Some(receiver) = terrain.as_mut() {
                receiver.add_scorch(position, radius, kind);
            }
        };
        Self::with_host_fx_view_parts_with_scorch(
            session_view,
            player,
            shroud,
            &mut add_scorch,
            run,
        )
    }

    #[cfg(all(test, feature = "game_client"))]
    pub(crate) fn with_host_fx_view_parts_with_scorch<R>(
        session_view: &mut crate::presentation_frame::SessionView,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        add_scorch: &mut dyn FnMut([f32; 3], f32, i32),
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        let mut no_presentation_bones = |_, _: &str, _, _| Vec::new();
        Self::with_host_fx_view_parts_with_scorch_and_bones(
            session_view,
            player,
            shroud,
            add_scorch,
            &mut no_presentation_bones,
            run,
        )
    }

    #[cfg(all(test, feature = "game_client"))]
    fn with_host_fx_view_parts_with_scorch_and_bones<R>(
        session_view: &mut crate::presentation_frame::SessionView,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        add_scorch: &mut dyn FnMut([f32; 3], f32, i32),
        bone_transforms: &mut dyn FnMut(u32, &str, i32, usize) -> Vec<glam::Mat4>,
        run: impl FnOnce(&mut game_client::fx_list::FxViewServices<'_>) -> R,
    ) -> R {
        let target = session_view.target;
        let rules = session_view.camera.shake_policy.current();
        let impulse = &mut session_view.impulse;
        let mut shake =
            |position: &gamelogic::common::Coord3D,
             kind: game_client::display::view::CameraShakeType| {
                impulse.enqueue_with_rules(
                    Vec3::new(position.x, position.z, position.y),
                    target,
                    kind.into(),
                    &rules,
                    &mut game_engine::common::random_value::get_game_client_random_value_real,
                );
            };
        let mut find_list =
            |name: &str| game_client::fx_list::get_fx_list_store().find_fx_list(name);
        let mut position_visible = |position: Option<&gamelogic::common::Coord3D>| {
            let (Some(player), Some(position)) = (player, position) else {
                return false;
            };
            let Ok(shroud) = shroud.lock() else {
                return false;
            };
            matches!(
                shroud.get_shroud_state(player, position),
                gamelogic::system::shroud_manager::ShroudState::Visible
            )
        };
        run(&mut game_client::fx_list::FxViewServices {
            add_scorch,
            shake: &mut shake,
            find_list: &mut find_list,
            bone_transforms,
            position_visible: &mut position_visible,
        })
    }

    /// Same fail-closed policy as the original emission boundary; no Core local identity.
    pub(crate) fn host_fx_object_is_shrouded(&self, id: u32) -> bool {
        Self::host_fx_visibility_for(self.local_player_id(), self.world_services.shroud(), id)
    }
    pub(crate) fn host_fx_visibility_for(
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        id: u32,
    ) -> bool {
        use gamelogic::common::types::ObjectShroudStatus;
        match (player, shroud.lock()) {
            (Some(player), Ok(shroud)) => shroud
                .get_host_object_shroud_status(player, id)
                .is_some_and(|status| (status as u8) >= ObjectShroudStatus::Fogged as u8),
            _ => true,
        }
    }
    /// Capture the receiving object's event-time pose before any FX callback.
    pub(crate) fn observe_host_fx_object(
        object: &crate::game_logic::Object,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
    ) -> gamelogic::helpers::HostFxObjectPose {
        use gamelogic::common::types::ObjectShroudStatus;
        let is_shrouded = match (player, shroud.lock()) {
            (Some(player), Ok(shroud)) => shroud
                .get_host_object_shroud_status(player, object.id.0)
                .is_some_and(|status| (status as u8) >= ObjectShroudStatus::Fogged as u8),
            _ => true,
        };
        let mut pose = host_fx_object_pose(
            object.id.0,
            object.get_position(),
            object.get_orientation(),
            object.owner_player_id.map(|p| p as i32).unwrap_or(-1),
            host_object_fx_radius(object),
            is_shrouded,
        );
        pose.transform = host_to_leftover_mat4(object.get_transform_matrix());
        pose
    }

    /// C++ `FXList::doFXObj` using the live host object, not leftover registry.
    pub fn dispatch_fx_list_at_host_object(
        &mut self,
        name: &str,
        primary_id: crate::game_logic::ObjectId,
        secondary_id: Option<crate::game_logic::ObjectId>,
    ) -> bool {
        self.dispatch_fx_list_at_host_object_with_effects(
            name,
            primary_id,
            secondary_id,
            &mut Default::default(),
        )
    }

    pub(crate) fn dispatch_fx_list_at_host_object_with_effects(
        &mut self,
        name: &str,
        primary_id: crate::game_logic::ObjectId,
        secondary_id: Option<crate::game_logic::ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let name = strip_fx_list_prefix(name);
        if is_none_fx_list(name) {
            return false;
        }
        #[cfg(feature = "game_client")]
        {
            let source = game_client::fx_list::FxSource::Objects {
                primary: primary_id.0,
                secondary: secondary_id.map(|id| id.0),
            };
            return match game_client::fx_list::execute_fx_with_runtime(
                &mut receiving::HostFxRuntime {
                    world: self,
                    effects,
                },
                name,
                source,
            ) {
                Ok(found) => found,
                Err(error) => {
                    log::error!("FX '{name}' failed at receiving nugget: {error}");
                    true
                }
            };
        }
        #[cfg(not(feature = "game_client"))]
        let (primary, secondary) = {
            let player = self.local_player_id();
            let primary = self.host_object(primary_id).map(|object| {
                Self::observe_host_fx_object(object, player, self.world_services.shroud())
            });
            let secondary = secondary_id
                .and_then(|id| self.host_object(id))
                .map(|object| {
                    Self::observe_host_fx_object(object, player, self.world_services.shroud())
                });
            // Attached systems still use the published scene poses between logic
            // callbacks. Immediate FX receives these owned values directly.
            if let Some(primary) = primary {
                gamelogic::helpers::set_host_fx_object_pose(primary);
            }
            if let Some(secondary) = secondary {
                gamelogic::helpers::set_host_fx_object_pose(secondary);
            }
            let Some(primary) = primary else {
                return false;
            };
            let name = strip_fx_list_prefix(name);
            if is_none_fx_list(name) {
                return false;
            }
            (primary, secondary)
        };

        #[cfg(not(feature = "game_client"))]
        {
            let Some(fx) = gamelogic::helpers::TheFXList::get() else {
                return false;
            };
            fx.do_fx_for_host_objects(name, &primary, secondary.as_ref());
            gamelogic::helpers::get_fx_list_manager().is_some()
        }
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "host_fx_owner_tests.rs"]
mod owner_tests;

/// Sound nugget names (`m_soundName`) authored inside `name`.
///
/// Prefers the live GameClient FXList store (the runner's source of truth).
/// Falls back to the Common INI store when the client store has no entry.
pub fn sound_names_for_fx_list(name: &str) -> Vec<String> {
    let name = strip_fx_list_prefix(name);
    if is_none_fx_list(name) {
        return Vec::new();
    }
    #[cfg(feature = "game_client")]
    {
        let names = game_client::fx_list::sound_names_for_fx_list(name);
        if !names.is_empty() {
            return names;
        }
    }
    common_ini_sound_names(name)
}

fn common_ini_sound_names(name: &str) -> Vec<String> {
    use game_engine::common::ini::ini_fx_list::{FXNugget, get_fx_list_store};
    let store = get_fx_list_store();
    let Some(fx) = store.find_fx_list(name) else {
        return Vec::new();
    };
    fx.nuggets
        .iter()
        .filter_map(|nugget| match nugget {
            FXNugget::Sound { name } => {
                let sound = name.as_str().trim();
                if sound.is_empty() || sound.eq_ignore_ascii_case("None") {
                    None
                } else {
                    Some(sound.to_string())
                }
            }
            _ => None,
        })
        .collect()
}

/// Authored ParticleSystem nugget names inside `name`.
///
/// C++ `ParticleSystemFXNugget::reallyDoFX` creates these templates. The live
/// host uses them instead of a generic `MuzzleFlash` preset.
pub fn particle_template_names_for_fx_list(name: &str) -> Vec<String> {
    let name = strip_fx_list_prefix(name);
    if is_none_fx_list(name) {
        return Vec::new();
    }
    common_ini_particle_names(name)
}

fn common_ini_particle_names(name: &str) -> Vec<String> {
    use game_engine::common::ini::ini_fx_list::{FXNugget, get_fx_list_store};
    let store = get_fx_list_store();
    let Some(fx) = store.find_fx_list(name) else {
        return Vec::new();
    };
    fx.nuggets
        .iter()
        .filter_map(|nugget| match nugget {
            FXNugget::ParticleSystem { name, .. } => {
                let particle = name.as_str().trim();
                if particle.is_empty() || particle.eq_ignore_ascii_case("None") {
                    None
                } else {
                    Some(particle.to_string())
                }
            }
            _ => None,
        })
        .collect()
}

/// Map an audio-queue name to the Miles events that should actually play.
///
/// FXList template names expand to their Sound nuggets. Unknown FXList
/// templates produce no events (never play the template name).
pub fn resolve_audio_event_names(event_type: &str) -> Vec<String> {
    let raw = strip_fx_list_prefix(event_type);
    if is_none_fx_list(raw) {
        return Vec::new();
    }
    let sounds = sound_names_for_fx_list(raw);
    if !sounds.is_empty() {
        return sounds;
    }
    if is_authored_fx_list_name(event_type) {
        return Vec::new();
    }
    vec![event_type.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_fx_list_does_not_dispatch() {
        assert!(!dispatch_fx_list_at_pos("None", Vec3::ZERO));
        assert!(!dispatch_fx_list_at_pos_ex(
            "",
            Vec3::ZERO,
            Some(Vec3::ONE),
            1.0,
            2.0
        ));
        assert!(!dispatch_fx_list_at_object("None", 1, None));
        assert!(particle_template_names_for_fx_list("None").is_empty());
        assert!(particle_template_names_for_fx_list("FX:None").is_empty());
    }

    #[test]
    fn leftover_coord_swizzle_is_z_up() {
        let leftover = host_to_leftover_coord(Vec3::new(10.0, 20.0, 30.0));
        assert!((leftover.x - 10.0).abs() < f32::EPSILON);
        assert!((leftover.y - 30.0).abs() < f32::EPSILON);
        assert!((leftover.z - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn host_to_leftover_mat4_swaps_y_up_to_z_up() {
        let host = glam::Mat4::from_translation(glam::Vec3::new(10.0, 20.0, 30.0));
        let leftover = host_to_leftover_mat4(host);
        let t = leftover.w_axis;
        assert!((t.x - 10.0).abs() < f32::EPSILON);
        assert!((t.y - 30.0).abs() < f32::EPSILON);
        assert!((t.z - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn garrison_hit_kill_fx_uses_do_fx_obj_on_building() {
        // The two collision branches moved with the actual flight step;
        // resolution owns only the shared doFXObj helper.

        let mut windows = 0usize;

        assert_eq!(windows, 2, "structure and target garrison paths");
    }
}

#[cfg(test)]
mod emission_player_owner_tests {
    use super::*;
    use crate::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};
    use gamelogic::common::types::ObjectShroudStatus;
    fn world(status: ObjectShroudStatus) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "SameLocal", true));
        let template = ThingTemplate::new("FxEmissionPlayerOwnerProbe");
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object(
                "FxEmissionPlayerOwnerProbe",
                Team::USA,
                Vec3::new(11.0, 0.0, 22.0),
            )
            .unwrap();
        world.host_object_mut(id).unwrap().owner_player_id = Some(1);
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.init_shroud_grid(128.0, 128.0);
        shroud.set_host_object_shroud_status(1, id.0, status);
        drop(shroud);
        (world, id)
    }
    fn emit(world: &mut GameLogic, id: ObjectId) -> gamelogic::helpers::HostFxObjectPose {
        world.spawn_emp_sparks_on_victim(id, 30);
        gamelogic::helpers::host_fx_object_pose(id.0)
            .expect("actual particle create publishes its attachment pose")
    }
    #[test]
    fn actual_emp_attachment_observes_each_receiving_world_and_encoded_restore() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "actual_emp_attachment_observes_each_receiving_world_and_encoded_restore",
            || {
                let (mut source, id) = world(ObjectShroudStatus::Clear);
                let (mut peer, peer_id) = world(ObjectShroudStatus::Fogged);
                assert_eq!(id, peer_id);
                {
                    let mut foreign = gamelogic::player::ThePlayerList().write().unwrap();
                    foreign.clear();
                    foreign.set_local_player_index(77);
                }
                assert!(!emit(&mut source, id).is_shrouded);
                assert!(emit(&mut peer, id).is_shrouded);
                // A change immediately before another emission must be visible in that operation.
                source
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Shrouded);
                assert!(emit(&mut source, id).is_shrouded);
                source
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Clear);
                assert!(!emit(&mut source, id).is_shrouded);
                let builder = crate::save_load::SnapshotBuilder::new();
                let saved = builder.create_world_snapshot(&source).unwrap();
                let bytes = bincode_legacy::serialize(&saved).unwrap();
                let saved = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
                let (mut restored, prepared_id) = world(ObjectShroudStatus::Clear);
                assert_eq!(prepared_id, id);
                restored.templates = source.templates.clone();
                builder
                    .restore_from_snapshot(&saved, &mut restored)
                    .unwrap();
                assert!(!emit(&mut restored, id).is_shrouded);
                peer.reset();
                assert!(!emit(&mut source, id).is_shrouded);
                assert!(!emit(&mut restored, id).is_shrouded);
            },
        );
    }
    #[test]
    fn receiving_missing_viewer_fails_closed_but_missing_object_status_retains_original_clear_result()
     {
        let world = GameLogic::new();
        assert!(world.host_fx_object_is_shrouded(999));
        assert!(!GameLogic::host_fx_visibility_for(
            Some(1),
            world.world_services.shroud(),
            999
        ));
    }
}

#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    pub(crate) fn with_object_fx_services<R>(
        session: &mut crate::presentation_frame::SessionView,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        environment: &ObjectFxEnvironment,
        terrain: &std::sync::Arc<std::sync::RwLock<gamelogic::terrain::TerrainLogic>>,
        logic_random: &game_engine::common::random_value::SessionRandom,
        run: impl FnOnce(&mut ObjectFxServices<'_, '_>) -> R,
    ) -> R {
        run(&mut ObjectFxServices {
            session,
            player,
            shroud,
            effects,
            environment,
            terrain,
            logic_random,
        })
    }
}
