use super::{
    AnimSet, ApplyRandomForceNugget, AttackNugget, DeliverPayloadNugget, FireWeaponNugget,
    GenericObjectCreationNugget, ObjectCreationList, ObjectCreationNugget, Payload,
};
use crate::debris::DebrisDisposition;
use crate::time::LOGIC_FRAME_TIMESTEP;
use crate::weapon::WeaponSlotType;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::StaticGameLODLevel;
use game_engine::common::thing::radius_decal_data::{
    SHADOW_ADDITIVE_DECAL, SHADOW_ALPHA_DECAL, SHADOW_DECAL, SHADOW_DIRECTIONAL_PROJECTION,
    SHADOW_DYNAMIC_PROJECTION, SHADOW_PROJECTION, SHADOW_VOLUME,
};
use glam::Vec3;

fn parse_bool_token(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_coord3d_value(value: &str) -> Vec3 {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    let mut saw_axis = false;

    for token in value.split_whitespace() {
        if let Some((axis, raw)) = token.split_once(':') {
            saw_axis = true;
            if let Ok(parsed) = raw.parse::<f32>() {
                match axis.to_ascii_uppercase().as_str() {
                    "X" => x = parsed,
                    "Y" => y = parsed,
                    "Z" => z = parsed,
                    _ => {}
                }
            }
        }
    }

    if !saw_axis {
        let numeric: Vec<f32> = value
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<f32>().ok())
            .collect();
        if numeric.len() >= 3 {
            x = numeric[0];
            y = numeric[1];
            z = numeric[2];
        }
    }

    Vec3::new(x, y, z)
}

fn parse_disposition(value: &str) -> DebrisDisposition {
    let mut disposition = DebrisDisposition::new(0);
    for token in value.split_whitespace() {
        match token.to_ascii_uppercase().as_str() {
            "LIKE_EXISTING" => disposition.set(DebrisDisposition::LIKE_EXISTING),
            "ON_GROUND_ALIGNED" => disposition.set(DebrisDisposition::ON_GROUND_ALIGNED),
            "SEND_IT_FLYING" => disposition.set(DebrisDisposition::SEND_IT_FLYING),
            "SEND_IT_UP" => disposition.set(DebrisDisposition::SEND_IT_UP),
            "SEND_IT_OUT" => disposition.set(DebrisDisposition::SEND_IT_OUT),
            "RANDOM_FORCE" => disposition.set(DebrisDisposition::RANDOM_FORCE),
            "FLOATING" => disposition.set(DebrisDisposition::FLOATING),
            "INHERIT_VELOCITY" => disposition.set(DebrisDisposition::INHERIT_VELOCITY),
            "WHIRLING" => disposition.set(DebrisDisposition::WHIRLING),
            _ => {}
        }
    }

    if !disposition.has(
        DebrisDisposition::LIKE_EXISTING
            | DebrisDisposition::ON_GROUND_ALIGNED
            | DebrisDisposition::SEND_IT_FLYING
            | DebrisDisposition::SEND_IT_UP
            | DebrisDisposition::SEND_IT_OUT
            | DebrisDisposition::RANDOM_FORCE
            | DebrisDisposition::FLOATING
            | DebrisDisposition::INHERIT_VELOCITY
            | DebrisDisposition::WHIRLING,
    ) {
        DebrisDisposition::new(DebrisDisposition::ON_GROUND_ALIGNED)
    } else {
        disposition
    }
}

fn parse_name_list(value: &str) -> Vec<String> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.trim().to_string())
        .collect()
}

fn apply_nugget_property(nugget: &mut GenericObjectCreationNugget, key: &str, value: &str) {
    match key.to_ascii_uppercase().as_str() {
        "OBJECTNAMES" | "MODELNAMES" => {
            let names = parse_name_list(value);
            if !names.is_empty() {
                nugget.names = names;
            }
        }
        "COUNT" => {
            if let Ok(count) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<i32>()
            {
                // The nugget constructor supplies C++'s default Count=1;
                // preserve an explicit INI zero instead of treating it as
                // an omitted field.
                nugget.debris_to_generate = count;
            }
        }
        "OFFSET" => nugget.offset = parse_coord3d_value(value),
        "DISPOSITION" => nugget.disposition = parse_disposition(value),
        "DISPOSITIONINTENSITY" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.disposition_intensity = parsed;
            }
        }
        "MASS" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.mass = parsed;
            }
        }
        "MINFORCEMAGNITUDE" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.min_mag = parsed;
            }
        }
        "MAXFORCEMAGNITUDE" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.max_mag = parsed;
            }
        }
        "MINFORCEPITCH" => {
            if let Some(parsed) = parse_angle_token(value) {
                nugget.min_pitch = parsed;
            }
        }
        "MAXFORCEPITCH" => {
            if let Some(parsed) = parse_angle_token(value) {
                nugget.max_pitch = parsed;
            }
        }
        "YAWRATE" => {
            if let Some(parsed) = parse_angular_velocity_token(value) {
                nugget.yaw_rate = parsed;
            }
        }
        "ROLLRATE" => {
            if let Some(parsed) = parse_angular_velocity_token(value) {
                nugget.roll_rate = parsed;
            }
        }
        "PITCHRATE" => {
            if let Some(parsed) = parse_angular_velocity_token(value) {
                nugget.pitch_rate = parsed;
            }
        }
        "SPINRATE" => {
            if let Some(parsed) = parse_angular_velocity_token(value) {
                nugget.spin_rate = parsed;
            }
        }
        "MINLIFETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.min_frames = frames;
            }
        }
        "MAXLIFETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.max_frames = frames;
            }
        }
        "PUTINCONTAINER" => nugget.put_in_container = value.trim().to_string(),
        "IGNOREPRIMARYOBSTACLE" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.ignore_primary_obstacle = parsed;
            }
        }
        "ORIENTINFORCEDIRECTION" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.orient_in_force_direction = parsed;
            }
        }
        "EXTRABOUNCINESS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.extra_bounciness = parsed;
            }
        }
        "EXTRAFRICTION" => {
            if let Some(parsed) = parse_friction_per_sec(value) {
                nugget.extra_friction = parsed;
            }
        }
        "SPREADFORMATION" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.spread_formation = parsed;
            }
        }
        "MINDISTANCEAFORMATION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_distance_a_formation = parsed;
            }
        }
        "MINDISTANCEBFORMATION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_distance_b_formation = parsed;
            }
        }
        "MAXDISTANCEFORMATION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.max_distance_formation = parsed;
            }
        }
        "FADEIN" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.fade_in = parsed;
            }
        }
        "FADEOUT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.fade_out = parsed;
            }
        }
        "FADETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.fade_frames = frames;
            }
        }
        "FADESOUND" => nugget.fade_sound_name = value.trim().to_string(),
        "DIESONBADLAND" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.dies_on_bad_land = parsed;
            }
        }
        "CONTAININSIDESOURCEOBJECT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.contain_inside_source_object = parsed;
            }
        }
        "INHERITSVETERANCY" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.inherit_veterancy = parsed;
            }
        }
        "SKIPIFSIGNIFICANTLYAIRBORNE" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.skip_if_significantly_airborne = parsed;
            }
        }
        "INVULNERABLETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.invulnerable_time = frames;
            }
        }
        "MINHEALTH" => {
            if let Some(parsed) = parse_first_percent(value) {
                nugget.min_health = parsed;
            }
        }
        "MAXHEALTH" => {
            if let Some(parsed) = parse_first_percent(value) {
                nugget.max_health = parsed;
            }
        }
        "REQUIRESLIVEPLAYER" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.requires_live_player = parsed;
            }
        }
        "PRESERVELAYER" => {
            if let Some(parsed) = parse_bool_token(value.split_whitespace().next().unwrap_or("")) {
                nugget.preserve_layer = parsed;
            }
        }
        "ANIMATIONSET" => {
            let mut parts = value.split_whitespace();
            let initial = parts.next().unwrap_or("").to_string();
            let flying = parts.next().unwrap_or("").to_string();
            let final_anim = parts.next().unwrap_or("").to_string();
            if !initial.is_empty() || !flying.is_empty() || !final_anim.is_empty() {
                nugget.anim_sets.push(AnimSet {
                    anim_initial: initial,
                    anim_flying: flying,
                    anim_final: final_anim,
                });
            }
        }
        "FXFINAL" => {
            let fx_name = value.trim();
            nugget.fx_final = if fx_name.is_empty() {
                None
            } else {
                Some(fx_name.to_string())
            };
        }
        "OKTOCHANGEMODELCOLOR" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.ok_to_change_model_color = parsed;
            }
        }
        "MINLODREQUIRED" => {
            let token = value.split_whitespace().next().unwrap_or("");
            let stripped = token
                .strip_prefix("STATIC_GAME_LOD_")
                .or_else(|| token.strip_prefix("static_game_lod_"))
                .unwrap_or(token);
            if let Some(level) = StaticGameLODLevel::from_str(stripped) {
                nugget.min_lod_required = level;
            }
        }
        "SHADOW" => {
            nugget.shadow_type = parse_shadow_type_bits(value);
        }
        "BOUNCESOUND" => nugget.bounce_sound = value.trim().to_string(),
        "PARTICLESYSTEM" => {
            nugget.particle_sys_name = value.trim().to_string();
        }
        _ => {}
    }
}

fn parse_first_real(value: &str) -> Option<f32> {
    value
        .split_whitespace()
        .next()
        .and_then(|token| token.parse::<f32>().ok())
}

/// C++ ObjectCreationList.cpp:703-708 `parseFrictionPerSec`.
fn parse_friction_per_sec(value: &str) -> Option<f32> {
    parse_first_real(value).map(|per_sec| per_sec * LOGIC_FRAME_TIMESTEP)
}

fn parse_angle_token(value: &str) -> Option<f32> {
    let token = value.split_whitespace().next()?;
    game_engine::common::ini::INI::parse_angle_real(token).ok()
}

fn parse_angular_velocity_token(value: &str) -> Option<f32> {
    let token = value.split_whitespace().next()?;
    game_engine::common::ini::INI::parse_angular_velocity_real(token).ok()
}

fn parse_shadow_type_bits(value: &str) -> u32 {
    let mut flags = 0u32;
    for token in value.split_whitespace() {
        match token.to_ascii_uppercase().as_str() {
            "SHADOW_DECAL" => flags |= SHADOW_DECAL,
            "SHADOW_VOLUME" => flags |= SHADOW_VOLUME,
            "SHADOW_PROJECTION" => flags |= SHADOW_PROJECTION,
            "SHADOW_DYNAMIC_PROJECTION" => flags |= SHADOW_DYNAMIC_PROJECTION,
            "SHADOW_DIRECTIONAL_PROJECTION" => flags |= SHADOW_DIRECTIONAL_PROJECTION,
            "SHADOW_ALPHA_DECAL" => flags |= SHADOW_ALPHA_DECAL,
            "SHADOW_ADDITIVE_DECAL" => flags |= SHADOW_ADDITIVE_DECAL,
            _ => {}
        }
    }
    flags
}

fn parse_first_int(value: &str) -> Option<i32> {
    value
        .split_whitespace()
        .next()
        .and_then(|token| token.parse::<i32>().ok())
}

fn parse_first_percent(value: &str) -> Option<f32> {
    let token = value.split_whitespace().next()?;
    if let Some(raw) = token.strip_suffix('%') {
        return raw.parse::<f32>().ok().map(|v| (v * 0.01).clamp(0.0, 1.0));
    }
    token.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_duration_to_frames(value: &str) -> Option<u32> {
    let token = value.split_whitespace().next()?;
    let lowered = token.to_ascii_lowercase();
    if let Some(raw) = lowered.strip_suffix("frames") {
        return raw.parse::<f32>().ok().map(|v| v.round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix("frame") {
        return raw.parse::<f32>().ok().map(|v| v.round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix('f') {
        return raw.parse::<f32>().ok().map(|v| v.round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix("ms") {
        let ms: f32 = raw.parse().ok()?;
        return Some(((ms * 30.0) / 1000.0).round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix('s') {
        let secs: f32 = raw.parse().ok()?;
        return Some((secs * 30.0).round().max(0.0) as u32);
    }

    lowered
        .parse::<f32>()
        .ok()
        .map(|ms| ((ms * 30.0) / 1000.0).round().max(0.0) as u32)
}

// The original DeliverPayload fields use INI::parseDurationUnsignedInt,
// including its ceiling to whole logic frames. Retain the explicit frame
// notation supported by the receiving INI reader.
fn parse_delivery_duration_to_frames(value: &str) -> Option<u32> {
    let token = value.split_whitespace().next()?;
    let duration = game_engine::common::ini::INI::parse_duration_unsigned_int(token);
    duration.ok().or_else(|| {
        let token = token.to_ascii_lowercase();
        (token.ends_with("frames") || token.ends_with("frame") || token.ends_with('f'))
            .then(|| parse_duration_to_frames(&token))
            .flatten()
    })
}

fn parse_color_rgba(value: &str) -> Option<u32> {
    if let Ok(raw) = value.trim().parse::<u32>() {
        return Some(raw);
    }

    let mut r = 255u32;
    let mut g = 255u32;
    let mut b = 255u32;
    let mut a = 255u32;
    for token in value.split_whitespace() {
        if let Some((k, v)) = token.split_once(':') {
            if let Ok(parsed) = v.parse::<u32>() {
                match k.to_ascii_uppercase().as_str() {
                    "R" => r = parsed.min(255),
                    "G" => g = parsed.min(255),
                    "B" => b = parsed.min(255),
                    "A" => a = parsed.min(255),
                    _ => {}
                }
            }
        }
    }

    Some((a << 24) | (b << 16) | (g << 8) | r)
}

fn parse_shadow_style(value: &str) -> u32 {
    let mut flags = 0u32;
    for token in value.split_whitespace() {
        match token.to_ascii_uppercase().as_str() {
            "SHADOW_DECAL" => flags |= SHADOW_DECAL,
            "SHADOW_VOLUME" => flags |= SHADOW_VOLUME,
            "SHADOW_PROJECTION" => flags |= SHADOW_PROJECTION,
            "SHADOW_DYNAMIC_PROJECTION" => flags |= SHADOW_DYNAMIC_PROJECTION,
            "SHADOW_DIRECTIONAL_PROJECTION" => flags |= SHADOW_DIRECTIONAL_PROJECTION,
            "SHADOW_ALPHA_DECAL" => flags |= SHADOW_ALPHA_DECAL,
            "SHADOW_ADDITIVE_DECAL" => flags |= SHADOW_ADDITIVE_DECAL,
            _ => {}
        }
    }

    if flags == 0 {
        SHADOW_ALPHA_DECAL
    } else {
        flags
    }
}

fn parse_attack_weapon_slot(value: &str) -> Option<WeaponSlotType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => Some(WeaponSlotType::Primary),
        "SECONDARY" | "SECONDARY_WEAPON" => Some(WeaponSlotType::Secondary),
        "TERTIARY" | "TERTIARY_WEAPON" => Some(WeaponSlotType::Tertiary),
        _ => None,
    }
}

fn parse_delivery_weapon_slot(value: &str) -> Option<WeaponSlotType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => Some(WeaponSlotType::Primary),
        "SECONDARY" | "SECONDARY_WEAPON" => Some(WeaponSlotType::Secondary),
        "TERTIARY" | "TERTIARY_WEAPON" => Some(WeaponSlotType::Tertiary),
        "NONE" | "-1" => None,
        _ => None,
    }
}

fn parse_payload(value: &str) -> Option<Payload> {
    let mut parts = value.split_whitespace();
    let name = parts.next()?.to_string();
    let count = parts
        .next()
        .and_then(|token| token.parse::<i32>().ok())
        .unwrap_or(1);
    Some(Payload {
        payload_name: name,
        payload_count: count.max(1),
    })
}

fn apply_delivery_property(
    nugget: &mut DeliverPayloadNugget,
    key: &str,
    value: &str,
    in_delivery_decal: bool,
) {
    if in_delivery_decal {
        match key.to_ascii_uppercase().as_str() {
            "TEXTURE" => {
                nugget.data.delivery_decal_template.texture_name = AsciiString::from(value)
            }
            "STYLE" => nugget.data.delivery_decal_template.shadow_type = parse_shadow_style(value),
            "OPACITYMIN" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.data.delivery_decal_template.min_opacity = parsed * 0.01;
                    nugget.data.delivery_decal_template.opacity =
                        nugget.data.delivery_decal_template.min_opacity;
                }
            }
            "OPACITYMAX" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.data.delivery_decal_template.max_opacity = parsed * 0.01;
                    nugget.data.delivery_decal_template.opacity =
                        nugget.data.delivery_decal_template.max_opacity;
                }
            }
            "OPACITYTHROBTIME" => {
                if let Some(frames) = parse_duration_to_frames(value) {
                    nugget.data.delivery_decal_template.opacity_throb_time = frames;
                }
            }
            "COLOR" => {
                if let Some(color) = parse_color_rgba(value) {
                    nugget.data.delivery_decal_template.color = color;
                }
            }
            "ONLYVISIBLETOOWNINGPLAYER" => {
                if let Some(parsed) = parse_bool_token(value) {
                    nugget
                        .data
                        .delivery_decal_template
                        .only_visible_to_owning_player = parsed;
                }
            }
            _ => {}
        }
        return;
    }

    match key.to_ascii_uppercase().as_str() {
        "TRANSPORT" => nugget.transport_name = value.trim().to_string(),
        "STARTATPREFERREDHEIGHT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.start_at_preferred_height = parsed;
            }
        }
        "STARTATMAXSPEED" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.start_at_max_speed = parsed;
            }
        }
        "FORMATIONSIZE" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.formation_size = parsed.max(1) as u32;
            }
        }
        "FORMATIONSPACING" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.formation_spacing = parsed;
            }
        }
        "WEAPONCONVERGENCEFACTOR" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.convergence_factor = parsed;
            }
        }
        "WEAPONERRORRADIUS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.error_radius = parsed;
            }
        }
        "DELAYDELIVERYMAX" => {
            if let Some(frames) = parse_delivery_duration_to_frames(value) {
                nugget.delay_delivery_frames_max = frames;
            }
        }
        "PAYLOAD" => {
            if let Some(payload) = parse_payload(value) {
                nugget.payload.push(payload);
            }
        }
        "PUTINCONTAINER" => nugget.put_in_container_name = value.trim().to_string(),
        "DELIVERYDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.dist_to_target = parsed;
            }
        }
        "PREOPENDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.pre_open_distance = parsed;
            }
        }
        "MAXATTEMPTS" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.data.max_attempts = parsed;
            }
        }
        "DROPOFFSET" => nugget.data.drop_offset = parse_coord3d_value(value),
        "DROPVARIANCE" => nugget.data.drop_variance = parse_coord3d_value(value),
        "DROPDELAY" => {
            if let Some(frames) = parse_delivery_duration_to_frames(value) {
                nugget.data.drop_delay = frames;
            }
        }
        "FIREWEAPON" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.fire_weapon = parsed;
            }
        }
        "SELFDESTRUCTOBJECT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.self_destruct_object = parsed;
            }
        }
        "VISIBLENUMBONES" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.data.visible_num_bones = parsed;
            }
        }
        "VISIBLEDROPBONEBASENAME" => {
            nugget.data.visible_drop_bone_name = AsciiString::from(value.trim())
        }
        "VISIBLESUBOBJECTBASENAME" => {
            nugget.data.visible_sub_object_name = AsciiString::from(value.trim())
        }
        "VISIBLEPAYLOADTEMPLATENAME" => {
            nugget.data.visible_payload_template_name = AsciiString::from(value.trim())
        }
        "VISIBLEITEMSDROPPEDPERINTERVAL" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.data.visible_items_dropped_per_interval = parsed;
            }
        }
        "INHERITTRANSPORTVELOCITY" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.inherit_transport_velocity = parsed;
            }
        }
        "PARACHUTEDIRECTLY" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.is_parachute_directly = parsed;
            }
        }
        "EXITPITCHRATE" => {
            if let Some(parsed) = parse_angular_velocity_token(value) {
                nugget.data.exit_pitch_rate = parsed;
            }
        }
        "STRAFELENGTH" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.strafe_length = parsed;
            }
        }
        "DIVESTARTDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.dive_start_distance = parsed;
            }
        }
        "DIVEENDDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.dive_end_distance = parsed;
            }
        }
        "STRAFINGWEAPONSLOT" => {
            nugget.data.strafing_weapon_slot = parse_delivery_weapon_slot(value)
        }
        "STRAFEWEAPONFX" => nugget.data.strafe_fx = Some(value.trim().to_owned()),
        "VISIBLEPAYLOADWEAPONTEMPLATE" => {
            let name = value.trim();
            nugget.data.visible_payload_weapon_template =
                (!name.is_empty() && !name.eq_ignore_ascii_case("NONE")).then(|| name.to_owned());
        }
        "DELIVERYDECALRADIUS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.delivery_decal_radius = parsed;
            }
        }
        _ => {}
    }
}

fn apply_attack_property(
    nugget: &mut AttackNugget,
    key: &str,
    value: &str,
    in_delivery_decal: bool,
) {
    if in_delivery_decal {
        match key.to_ascii_uppercase().as_str() {
            "TEXTURE" => nugget.delivery_decal_template.texture_name = AsciiString::from(value),
            "STYLE" => nugget.delivery_decal_template.shadow_type = parse_shadow_style(value),
            "OPACITYMIN" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.delivery_decal_template.min_opacity = parsed * 0.01;
                    nugget.delivery_decal_template.opacity =
                        nugget.delivery_decal_template.min_opacity;
                }
            }
            "OPACITYMAX" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.delivery_decal_template.max_opacity = parsed * 0.01;
                    nugget.delivery_decal_template.opacity =
                        nugget.delivery_decal_template.max_opacity;
                }
            }
            "OPACITYTHROBTIME" => {
                if let Some(frames) = parse_duration_to_frames(value) {
                    nugget.delivery_decal_template.opacity_throb_time = frames;
                }
            }
            "COLOR" => {
                if let Some(color) = parse_color_rgba(value) {
                    nugget.delivery_decal_template.color = color;
                }
            }
            "ONLYVISIBLETOOWNINGPLAYER" => {
                if let Some(parsed) = parse_bool_token(value) {
                    nugget.delivery_decal_template.only_visible_to_owning_player = parsed;
                }
            }
            _ => {}
        }
        return;
    }

    match key.to_ascii_uppercase().as_str() {
        "NUMBEROFSHOTS" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.number_of_shots = parsed;
            }
        }
        "WEAPONSLOT" => {
            if let Some(slot) = parse_attack_weapon_slot(value) {
                nugget.weapon_slot = slot;
            }
        }
        "DELIVERYDECALRADIUS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.delivery_decal_radius = parsed;
            }
        }
        _ => {}
    }
}

fn apply_force_property(nugget: &mut ApplyRandomForceNugget, key: &str, value: &str) {
    match key.to_ascii_uppercase().as_str() {
        "SPINRATE" => {
            if let Some(parsed) = parse_angular_velocity_token(value) {
                nugget.spin_rate = parsed;
            }
        }
        "MINFORCEMAGNITUDE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_mag = parsed;
            }
        }
        "MAXFORCEMAGNITUDE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.max_mag = parsed;
            }
        }
        "MINFORCEPITCH" => {
            if let Some(parsed) = parse_angle_token(value) {
                nugget.min_pitch = parsed;
            }
        }
        "MAXFORCEPITCH" => {
            if let Some(parsed) = parse_angle_token(value) {
                nugget.max_pitch = parsed;
            }
        }
        _ => {}
    }
}

fn apply_fire_weapon_property(nugget: &mut FireWeaponNugget, key: &str, value: &str) {
    if key.eq_ignore_ascii_case("Weapon") {
        let weapon_name = value.trim();
        nugget.weapon = if weapon_name.is_empty() {
            None
        } else {
            Some(weapon_name.to_string())
        };
    }
}

fn has_required_nugget_fields(nugget: &ObjectCreationNugget) -> bool {
    match nugget {
        ObjectCreationNugget::Generic(data) => !data.names.is_empty(),
        ObjectCreationNugget::DeliverPayload(data) => !data.transport_name.is_empty(),
        ObjectCreationNugget::FireWeapon(data) => data.weapon.is_some(),
        ObjectCreationNugget::Attack(_) | ObjectCreationNugget::ApplyRandomForce(_) => true,
    }
}

pub fn parse_object_creation_lists_from_str(
    content: &str,
) -> Result<Vec<(String, ObjectCreationList)>, String> {
    let mut parsed: Vec<(String, ObjectCreationList)> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_ocl: Option<Vec<ObjectCreationNugget>> = None;
    let mut current_nugget: Option<ObjectCreationNugget> = None;
    let mut in_delivery_decal = false;

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let mut words = line.split_whitespace();
        if let Some(keyword) = words.next() {
            if keyword.eq_ignore_ascii_case("ObjectCreationList") {
                if current_name.is_some() || current_nugget.is_some() || in_delivery_decal {
                    return Err(format!(
                        "Nested ObjectCreationList block at line {}",
                        line_no
                    ));
                }
                let name = words.collect::<Vec<_>>().join(" ");
                if name.trim().is_empty() {
                    return Err(format!(
                        "ObjectCreationList missing name at line {}",
                        line_no
                    ));
                }
                current_name = Some(name.trim().to_string());
                current_ocl = Some(Vec::new());
                continue;
            }
        }

        if line.eq_ignore_ascii_case("End") {
            if in_delivery_decal {
                in_delivery_decal = false;
                continue;
            }

            if let Some(builder) = current_nugget.take() {
                if let Some(list) = current_ocl.as_mut()
                    && has_required_nugget_fields(&builder)
                {
                    list.push(builder);
                }
                continue;
            }

            if let (Some(name), Some(ocl)) = (current_name.take(), current_ocl.take()) {
                parsed.push((name, ObjectCreationList::new(ocl)));
                continue;
            }

            return Err(format!("Unexpected End at line {}", line_no));
        }

        if current_ocl.is_none() {
            continue;
        }

        if current_nugget.is_none() {
            if line.eq_ignore_ascii_case("CreateObject") {
                let mut nugget = GenericObjectCreationNugget::default();
                nugget.name_are_objects = true;
                current_nugget = Some(ObjectCreationNugget::Generic(nugget));
                continue;
            }
            if line.eq_ignore_ascii_case("CreateDebris") {
                let mut nugget = GenericObjectCreationNugget::default();
                nugget.name_are_objects = false;
                current_nugget = Some(ObjectCreationNugget::Generic(nugget));
                continue;
            }
            if line.eq_ignore_ascii_case("DeliverPayload") {
                current_nugget = Some(ObjectCreationNugget::DeliverPayload(
                    DeliverPayloadNugget::default(),
                ));
                continue;
            }
            if line.eq_ignore_ascii_case("FireWeapon") {
                current_nugget =
                    Some(ObjectCreationNugget::FireWeapon(FireWeaponNugget::default()));
                continue;
            }
            if line.eq_ignore_ascii_case("Attack") {
                current_nugget = Some(ObjectCreationNugget::Attack(AttackNugget::default()));
                continue;
            }
            if line.eq_ignore_ascii_case("ApplyRandomForce") {
                current_nugget = Some(ObjectCreationNugget::ApplyRandomForce(
                    ApplyRandomForceNugget::default(),
                ));
                continue;
            }

            continue;
        }

        if line.eq_ignore_ascii_case("DeliveryDecal") {
            if matches!(
                current_nugget,
                Some(ObjectCreationNugget::DeliverPayload(_))
                    | Some(ObjectCreationNugget::Attack(_))
            ) {
                in_delivery_decal = true;
            }
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if let Some(builder) = current_nugget.as_mut() {
                match builder {
                    ObjectCreationNugget::Generic(nugget) => {
                        apply_nugget_property(nugget, key, value)
                    }
                    ObjectCreationNugget::DeliverPayload(nugget) => {
                        apply_delivery_property(nugget, key, value, in_delivery_decal)
                    }
                    ObjectCreationNugget::FireWeapon(nugget) => {
                        apply_fire_weapon_property(nugget, key, value)
                    }
                    ObjectCreationNugget::Attack(nugget) => {
                        apply_attack_property(nugget, key, value, in_delivery_decal)
                    }
                    ObjectCreationNugget::ApplyRandomForce(nugget) => {
                        apply_force_property(nugget, key, value)
                    }
                }
            }
            continue;
        }
    }

    if in_delivery_decal {
        return Err("Unterminated DeliveryDecal block".to_string());
    }
    if current_nugget.is_some() {
        return Err("Unterminated OCL nugget block".to_string());
    }
    if let Some(name) = current_name {
        return Err(format!("Unterminated ObjectCreationList block '{}'", name));
    }

    Ok(parsed)
}

/// Collect only actual ObjectCreationList blocks from a mixed Map/Solo INI.
/// Common owns token/physical-line handling; the same existing OCL parser
/// below owns every nugget field/default. No store or random owner is touched.
pub fn parse_object_creation_lists_from_ini(
    content: &str,
) -> Result<Vec<(String, ObjectCreationList)>, String> {
    let lines: Vec<_> = content.lines().collect();
    let mut selected = String::new();
    let mut depth = 0usize;
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(content, |ini| {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                break;
            }
            let tokens = ini.get_line_tokens();
            let Some(first) = tokens.first() else {
                continue;
            };
            if depth == 0 && !first.eq_ignore_ascii_case("ObjectCreationList") {
                continue;
            }
            if let Some(raw) = lines.get(ini.get_line_num().saturating_sub(1) as usize) {
                selected.push_str(raw);
                selected.push('\n');
            }
            if first.eq_ignore_ascii_case("ObjectCreationList")
                || [
                    "CreateObject",
                    "CreateDebris",
                    "DeliverPayload",
                    "FireWeapon",
                    "Attack",
                    "ApplyRandomForce",
                    "DeliveryDecal",
                ]
                .iter()
                .any(|key| first.eq_ignore_ascii_case(key))
            {
                depth += 1;
            }
            if first.eq_ignore_ascii_case("End") {
                depth = depth.saturating_sub(1);
            }
        }
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    parse_object_creation_lists_from_str(&selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_references_are_kept_as_authored_names() {
        let lists = parse_object_creation_lists_from_ini(
            "ObjectCreationList Delivery\n\
             DeliverPayload\n\
             Transport = Aircraft\n\
             VisiblePayloadWeaponTemplate = AuthoredGun\n\
             StrafeWeaponFX = AuthoredEffect\n\
             End\nEnd\n",
        )
        .unwrap();
        let [ObjectCreationNugget::DeliverPayload(delivery)] = lists[0].1.nuggets() else {
            panic!("expected a DeliverPayload nugget");
        };
        assert_eq!(
            delivery.data.visible_payload_weapon_template.as_deref(),
            Some("AuthoredGun"),
        );
        assert_eq!(delivery.data.strafe_fx.as_deref(), Some("AuthoredEffect"));
    }

    #[test]
    fn test_duration_parser_matches_ini_duration_semantics() {
        assert_eq!(parse_duration_to_frames("1000"), Some(30));
        assert_eq!(parse_duration_to_frames("1s"), Some(30));
        assert_eq!(parse_duration_to_frames("500ms"), Some(15));
        assert_eq!(parse_duration_to_frames("10frames"), Some(10));
    }

    #[test]
    fn delivery_durations_round_up_to_logic_frames() {
        assert_eq!(parse_delivery_duration_to_frames("1"), Some(1));
        assert_eq!(parse_delivery_duration_to_frames("170"), Some(6));
        assert_eq!(parse_delivery_duration_to_frames("1000"), Some(30));
        assert_eq!(parse_delivery_duration_to_frames("6frames"), Some(6));
    }

    #[test]
    fn extra_friction_parse_converts_per_sec_to_per_frame() {
        // C++ ObjectCreationList.cpp:703-708 parseFrictionPerSec:
        // fricPerFrame = fricPerSec * SECONDS_PER_LOGICFRAME_REAL (1/30).
        assert!((parse_friction_per_sec("-0.3").unwrap() + 0.01).abs() < 1e-6);
        assert!((parse_friction_per_sec("3.0").unwrap() - 0.1).abs() < 1e-6);
        assert!(parse_friction_per_sec("").is_none());
    }

    #[test]
    fn test_decal_color_parser_matches_common_delivery_parser_order() {
        assert_eq!(parse_color_rgba("R:1 G:2 B:3 A:4"), Some(0x0403_0201));
        assert_eq!(parse_color_rgba("16909060"), Some(16909060));
    }
}
