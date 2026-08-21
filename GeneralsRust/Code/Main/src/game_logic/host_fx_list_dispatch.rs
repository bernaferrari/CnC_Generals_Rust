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
        && (raw.starts_with("FX_")
            || raw.starts_with("WeaponFX_")
            || name.starts_with("FX:"))
}

fn strip_fx_list_prefix(name: &str) -> &str {
    name.strip_prefix("FX:").unwrap_or(name).trim()
}

fn is_none_fx_list(name: &str) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case("None")
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
    let name = strip_fx_list_prefix(name);
    if is_none_fx_list(name) {
        return false;
    }
    let Some(fx) = gamelogic::helpers::TheFXList::get() else {
        return false;
    };
    fx.do_fx_at_position_ex(name, &pos, secondary.as_ref(), primary_speed, override_radius);
    gamelogic::helpers::get_fx_list_manager().is_some()
}

/// C++ `FXList::doFXObj` — object form used by death, TransitionDamage, DamageFX.
///
/// Passes the primary object so ParticleSystem nuggets get the object transform
/// (`OrientToObject` / `AttachToObject` / `FXListAtBonePos`).
pub fn dispatch_fx_list_at_object(
    name: &str,
    primary_id: u32,
    secondary_id: Option<u32>,
) -> bool {
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
    use game_engine::common::ini::ini_fx_list::{get_fx_list_store, FXNugget};
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
    use game_engine::common::ini::ini_fx_list::{get_fx_list_store, FXNugget};
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
}
