//! Live-host FXList nugget dispatch + Sound-nugget audio names.
//!
//! C++ `FXList::doFXPos` / `doFXObj` (FXList.cpp:782-805) run every nugget.
//! `SoundFXNugget` plays `AudioEventRTS(m_soundName)` — the name *inside*
//! the FXList, never the FXList template name (FXList.cpp:79-100).
//!
//! The GameClient runner is registered via `register_fx_list_manager_bridge`.
//! This module is the live-host call into that runner.

use glam::Vec3;

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
    publish_host_fx_object_pose(
        id,
        pos,
        orientation,
        player_index,
        bounding_circle_radius,
        host_object_is_shrouded_for_local(id),
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
    let leftover_pos = host_to_leftover_coord(pos);
    let transform = glam::Mat4::from_translation(glam::Vec3::new(
        leftover_pos.x,
        leftover_pos.y,
        leftover_pos.z,
    )) * glam::Mat4::from_rotation_z(orientation);
    gamelogic::helpers::set_host_fx_object_pose(gamelogic::helpers::HostFxObjectPose {
        id,
        position: leftover_pos,
        transform,
        player_index,
        bounding_circle_radius,
        is_shrouded,
    });
}

fn host_object_is_shrouded_for_local(id: u32) -> bool {
    use gamelogic::common::types::ObjectShroudStatus;
    let player = gamelogic::player::player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())
        .unwrap_or(-1);
    if player < 0 {
        return false;
    }
    let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() else {
        return false;
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
    let mut seen = std::collections::HashSet::new();
    for object in &frame.objects {
        seen.insert(object.id.0);
        let is_shrouded = (object.drawable_shroud.raw_status as u8)
            >= (crate::presentation_frame::PresentationObjectShroudStatus::Fogged as u8);
        publish_host_fx_object_pose(
            object.id.0,
            object.position,
            object.orientation,
            object
                .owner_player_id
                .map(|player| player as i32)
                .unwrap_or(-1),
            0.0,
            is_shrouded,
        );
    }
    gamelogic::helpers::retain_host_fx_object_poses(|id| seen.contains(&id));
}

fn host_object_fx_radius(obj: &crate::game_logic::Object) -> f32 {
    crate::game_logic::host_supply_gather::host_bounding_circle_radius(
        obj.thing.template.geometry_info.authored,
        obj.thing.template.geometry_info.bounding_circle_radius(),
        obj.thing.geometry.radius.max(obj.selection_radius),
    )
}

impl crate::game_logic::GameLogic {
    /// C++ `FXList::doFXObj` using the live host object, not leftover registry.
    pub fn dispatch_fx_list_at_host_object(
        &self,
        name: &str,
        primary_id: crate::game_logic::ObjectId,
        secondary_id: Option<crate::game_logic::ObjectId>,
    ) -> bool {
        if let Some(obj) = self.host_object(primary_id) {
            publish_host_fx_object_ex(
                obj.id.0,
                obj.get_position(),
                obj.get_orientation(),
                obj.owner_player_id.map(|p| p as i32).unwrap_or(-1),
                host_object_fx_radius(obj),
            );
        }
        if let Some(sid) = secondary_id {
            if let Some(obj) = self.host_object(sid) {
                publish_host_fx_object_ex(
                    obj.id.0,
                    obj.get_position(),
                    obj.get_orientation(),
                    obj.owner_player_id.map(|p| p as i32).unwrap_or(-1),
                    host_object_fx_radius(obj),
                );
            }
        }
        dispatch_fx_list_at_object(name, primary_id.0, secondary_id.map(|id| id.0))
    }
}

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
    fn death_fx_passes_killer_and_armor_fx_runs_once() {
        let death = include_str!("object/death.rs");
        assert!(
            death.contains("let killer = self.last_damage_source.map(|id| id.0)"),
            "FXListDie extra modules must pass the killer as doFXObj secondary"
        );
        assert!(death.contains("dispatch_fx_list_at_object(&name, self.id.0, killer)"));
        let tick = include_str!("world_tick/ai.rs");
        assert!(tick.contains("dispatch_fx_list_at_host_object(&fx, object_id, death_killer)"));
        let shadow = include_str!("../gameworld_shadow/session.rs");
        assert!(shadow.contains("dispatch_fx_list_at_host_object(&fx, id, killer)"));
        let armor = include_str!("host_transition_damage_fx.rs");
        let start = armor
            .find("pub fn dispatch_armor_damage_fx")
            .expect("armor dispatch");
        let end = armor
            .find("pub fn take_dispatched_armor_damage_fx")
            .expect("armor take");
        let body = &armor[start..end];
        assert!(body.contains("dfx.do_damage_fx"));
        assert!(
            !body.contains("dispatch_fx_list_at_object"),
            "armor DamageFX must not leftover-doFXObj twice"
        );
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
    fn fire_fx_dispatch_threads_drawable_matrix() {
        let discharge = include_str!("world_combat/weapon_discharge.rs");
        assert!(
            discharge.contains("dispatch_fx_list_at_pos_oriented"),
            "FireFX fallback must pass drawable transform"
        );
        assert!(
            discharge.contains("spawn_weapon_fire_fx_named_ocl_oriented"),
            "play_dispatch_fire_fx must pass drawable transform"
        );
        assert!(discharge.contains("get_transform_matrix()"));
        let particles = include_str!("combat_particles.rs");
        let start = particles
            .find("pub fn spawn_weapon_fire_fx_named_ocl_oriented")
            .expect("oriented spawn");
        let body = &particles[start..start + 3200];
        assert!(body.contains("dispatch_fx_list_at_pos_oriented"));
        assert!(body.contains("drawable_matrix"));
        let leftover = include_str!("../../../GameEngine/GameLogic/src/helpers/particles.rs");
        let start = leftover
            .find("pub fn do_fx_at_position_ex")
            .expect("do_fx_at_position_ex");
        let body = &leftover[start..start + 600];
        assert!(
            !body.contains("do_fx_pos_ex(fx_id, pos, None,"),
            "leftover do_fx_at_position_ex must forward drawable matrix"
        );
        assert!(body.contains("matrix"));
    }

    #[test]
    fn crushing_fx_uses_terrain_height() {
        let src = include_str!("world_objects/create_destroy_die.rs");
        let start = src
            .find("pub(crate) fn apply_structure_topple_crush_samples")
            .expect("crush samples");
        let body = &src[start..start + 1800];
        assert!(
            body.contains("terrain_height_at"),
            "CrushingFX must sit on terrain, not height 0"
        );
        assert!(
            body.contains("glam::Vec3::new(s.x, height, s.z)"),
            "CrushingFX dispatch must use sampled terrain height"
        );
    }

    #[test]
    fn bone_fx_tick_drains_leftover_authored() {
        let tick = include_str!("world_tick/ai.rs");
        assert!(tick.contains("play_bone_fx_event"));
        assert!(tick.contains("bfx.tick(self.frame as i32)"));
        let bone = include_str!("host_bone_fx_damage.rs");
        // Scan only the production prefix: that module's own test asserts
        // mention the invented name and must not match the scan.
        let bone = &bone[..bone.find("#[cfg(test)]").unwrap_or(bone.len())];
        assert!(bone.contains("peel_authored_bone_fx"));
        assert!(!bone.contains("FX_ScudDamagedBoneFX"));
    }

    #[test]
    fn combat_drop_kill_fx_looks_up_unit_specific() {
        let chinook = include_str!("host_combat_chinook.rs");
        assert!(chinook.contains("leftover_combat_drop_kill_fx_name"));
        assert!(chinook.contains("COMBAT_DROP_KILL_FX_KEY"));
        let regs = include_str!("world_combat/registries.rs");
        assert!(regs.contains("leftover_combat_drop_kill_fx_name"));
        assert!(regs.contains("dispatch_fx_list_at_host_object(&fx, bldg_id, None)"));
        let gps = include_str!("world_combat/gps_and_fields.rs");
        let pulse = gps.find("if do_fx {").expect("propaganda pulse");
        let pulse_body = &gps[pulse..pulse + 450];
        assert!(
            pulse_body.contains("dispatch_fx_list_at_host_object(fx, tower.id, None)"),
            "Propaganda PulseFX must doFXObj the tower object"
        );
        let bunker = include_str!("world_combat/missile_defenders.rs");
        let bust = bunker
            .find("fn apply_bunker_buster_to_target")
            .expect("apply_bunker_buster_to_target");
            let bust_body = &bunker[bust..bust + 6200];
        assert!(
            bust_body.contains("BUNKER_BUSTER_DETONATION_FX"),
            "bust must play leftover DetonationFX on the bunker"
        );
        assert!(
            bust_body.contains("dispatch_fx_list_at_host_object"),
            "DetonationFX must use doFXObj on the bunker object"
        );
        let stealth = include_str!("world_combat/air_and_mig.rs");
        assert!(
            stealth.contains("BUNKER_BUSTER_CRASH_THROUGH_FX"),
            "kill-self hold must play leftover CrashThroughBunkerFX on the missile"
        );
        assert!(
            stealth.contains("should_play_crash_through_fx"),
            "crash FX must gate on MISSILE_KILLING_SELF cadence"
        );
    }

    #[test]
    fn garrison_hit_kill_fx_uses_do_fx_obj_on_building() {
        let combat = include_str!("combat/resolution.rs");
        let helper = combat
            .find("fn play_garrison_hit_kill_fx(")
            .expect("garrison doFXObj helper");
        let helper_body = &combat[helper..helper + 700];
        assert!(
            helper_body.contains("dispatch_fx_list_at_object(fx_name, building_id.0, None)"),
            "GarrisonHitKillFX must doFXObj the building, not a pos form"
        );
        assert!(
            !helper_body.contains("dispatch_fx_list_at_pos"),
            "GarrisonHitKillFX must not use Weapon.cpp detonation doFXPos"
        );
        assert!(
            !helper_body.contains("ProjectileImpactFx"),
            "GarrisonHitKillFX must not queue detonation impact FX"
        );
        let mut rest = combat;
        let mut windows = 0usize;
        while let Some(i) = rest.find("apply_garrison_hit_kill") {
            let chunk = &rest[i..];
            let end = chunk.find("projectiles_to_remove").unwrap_or(600).min(600);
            let window = &chunk[..end];
            assert!(
                window.contains("play_garrison_hit_kill_fx"),
                "garrison clear must play doFXObj, got {window}"
            );
            assert!(
                !window.contains("ProjectileImpactFx"),
                "garrison clear must not queue ProjectileImpactFx, got {window}"
            );
            windows += 1;
            rest = &rest[i + 20..];
        }
        assert_eq!(windows, 2, "structure and target garrison paths");
        let leftover_dumb = include_str!(
            "../../../GameEngine/GameLogic/src/object/behavior/dumb_projectile_behavior.rs"
        );
        assert!(leftover_dumb.contains("do_fx_obj_ids(other_id, None, None)"));
        let leftover_missile =
            include_str!("../../../GameEngine/GameLogic/src/object/update/missile_ai_update.rs");
        assert!(leftover_missile.contains("do_fx_obj(&other_arc, None)"));
    }
}
