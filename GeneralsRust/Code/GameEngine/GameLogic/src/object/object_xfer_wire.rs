//! Primitive encodings and named-bit tables from CPP Object.cpp / Xfer.cpp.
//! Object phase traversal and owner-dependent restore remain in object_xfer.
use super::*;

pub(super) fn xfer_matrix3d(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) -> std::io::Result<()> {
    // C++ Xfer::xferMatrix3D: version byte + 3 rows × 4 Reals (Object.cpp:4025, Xfer.cpp:818-843).
    let current_version: u8 = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer_matrix3d_rows(xfer, matrix)
}

pub(super) fn xfer_matrix3d_rows(
    xfer: &mut dyn Xfer,
    matrix: &mut Matrix3D,
) -> std::io::Result<()> {
    let cols = matrix.to_cols_array();
    let mut row0 = [cols[0], cols[4], cols[8], cols[12]];
    let mut row1 = [cols[1], cols[5], cols[9], cols[13]];
    let mut row2 = [cols[2], cols[6], cols[10], cols[14]];

    for value in &mut row0 {
        xfer.xfer_real(value)?;
    }
    for value in &mut row1 {
        xfer.xfer_real(value)?;
    }
    for value in &mut row2 {
        xfer.xfer_real(value)?;
    }

    let rebuilt_cols = [
        row0[0], row1[0], row2[0], 0.0, row0[1], row1[1], row2[1], 0.0, row0[2], row1[2], row2[2],
        0.0, row0[3], row1[3], row2[3], 1.0,
    ];
    *matrix = Matrix3D::from_cols_array(&rebuilt_cols);
    Ok(())
}

/// Object.cpp:4020-4045 has two distinct wire and observation sequences.
/// The old position callback runs before orientation is read, not after both
/// values have been buffered. Current records retain their full matrix phase.
pub(super) enum SavedTransformChange {
    Matrix(Matrix3D),
    Position(Coord3D),
    Orientation(Real),
}

pub(super) fn xfer_transform_phase(
    xfer: &mut dyn Xfer,
    version: u8,
    transform: Matrix3D,
    apply: &mut dyn FnMut(SavedTransformChange) -> Result<(), String>,
) -> Result<(), String> {
    if version >= 7 {
        let mut matrix = transform;
        xfer_matrix3d(xfer, &mut matrix).map_err(|error| error.to_string())?;
        if !matrix.is_finite() {
            return Err("non-finite saved Object matrix".into());
        }
        apply(SavedTransformChange::Matrix(matrix))
    } else {
        let mut position = transform.w_axis.truncate();
        xfer.xfer_real(&mut position.x)
            .map_err(|error| error.to_string())?;
        xfer.xfer_real(&mut position.y)
            .map_err(|error| error.to_string())?;
        xfer.xfer_real(&mut position.z)
            .map_err(|error| error.to_string())?;
        if !position.is_finite() {
            return Err("non-finite saved Object position".into());
        }
        apply(SavedTransformChange::Position(position))?;
        let mut orientation = crate::object::object_triggers::cpp_matrix_z_rotation(&transform);
        xfer.xfer_real(&mut orientation)
            .map_err(|error| error.to_string())?;
        if !orientation.is_finite() {
            return Err("non-finite saved Object orientation".into());
        }
        // Original saves write Thing::getOrientation(), which is canonical
        // [-PI, PI]. Reject malformed finite angles too: repeated subtraction
        // in the compatibility setter can make no progress at f32::MAX.
        if orientation.abs() > std::f32::consts::PI {
            return Err("non-canonical saved Object orientation".into());
        }
        apply(SavedTransformChange::Orientation(orientation))
    }
}

/// C++ `Object::crc` → `xferSnapshot(thisWeapon)` → `Weapon::crc`.
pub(super) fn xfer_weapon_crc_like_cpp(xfer: &mut dyn Xfer, weapon: &crate::weapon::Weapon) {
    let snap = weapon.crc_snapshot_fields();
    let mut name = snap.template_name;
    let _ = xfer.xfer_ascii_string(&mut name);

    let mut wslot = snap.wslot;
    // SAFETY: `wslot` is a fully initialized stack local of exactly
    // `size_of::<i32>()` bytes; `xfer_user` only reads/writes that many bytes
    // through the pointer for the duration of the call.
    unsafe {
        let _ = xfer.xfer_user(
            (&mut wslot as *mut i32).cast::<u8>(),
            std::mem::size_of::<i32>(),
        );
    }

    let mut ammo = snap.ammo_in_clip;
    let _ = xfer.xfer_unsigned_int(&mut ammo);
    let mut when_fire = snap.when_we_can_fire_again;
    let _ = xfer.xfer_unsigned_int(&mut when_fire);
    let mut when_pre = snap.when_pre_attack_finished;
    let _ = xfer.xfer_unsigned_int(&mut when_pre);
    let mut when_reload = snap.when_last_reload_started;
    let _ = xfer.xfer_unsigned_int(&mut when_reload);
    let mut last_fire = snap.last_fire_frame;
    let _ = xfer.xfer_unsigned_int(&mut last_fire);

    let mut stream_id = snap.projectile_stream_id;
    let _ = xfer.xfer_object_id(&mut stream_id);
    let mut laser_id_unused: ObjectID = crate::common::INVALID_ID;
    let _ = xfer.xfer_object_id(&mut laser_id_unused);

    let mut max_shots = snap.max_shot_count;
    let _ = xfer.xfer_int(&mut max_shots);
    let mut cur_barrel = snap.cur_barrel;
    let _ = xfer.xfer_int(&mut cur_barrel);
    let mut shots_for_barrel = snap.num_shots_for_cur_barrel;
    let _ = xfer.xfer_int(&mut shots_for_barrel);

    let mut scatter_count = snap.scatter_targets_unused.len() as u16;
    let _ = xfer.xfer_unsigned_short(&mut scatter_count);
    for target in &snap.scatter_targets_unused {
        let mut int_data = *target;
        let _ = xfer.xfer_int(&mut int_data);
    }

    let mut pitch_limited = snap.pitch_limited;
    let _ = xfer.xfer_bool(&mut pitch_limited);
    let mut leech = snap.leech_weapon_range_active;
    let _ = xfer.xfer_bool(&mut leech);
}

/// C++ `xferUser((Matrix3D *)getTransformMatrix(), sizeof(Matrix3D))`.
/// WWMath `Matrix3D` is 3×4 Reals (48 bytes), row-major.
pub(super) fn xfer_matrix3d_user_blob(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) {
    const CPP_MATRIX3D_FLOATS: usize = 12;
    let cols = matrix.to_cols_array();
    let mut blob = [0f32; CPP_MATRIX3D_FLOATS];
    for row in 0..3 {
        for col in 0..4 {
            blob[row * 4 + col] = cols[col * 4 + row];
        }
    }
    // SAFETY: `blob` is a fully initialized `[f32; 12]` stack array; the
    // xfer reads/writes exactly its 48 bytes within this call.
    unsafe {
        let _ = xfer.xfer_user(
            blob.as_mut_ptr().cast::<u8>(),
            CPP_MATRIX3D_FLOATS * std::mem::size_of::<f32>(),
        );
    }
    let mut cols = [0f32; 16];
    for row in 0..3 {
        for col in 0..4 {
            cols[col * 4 + row] = blob[row * 4 + col];
        }
    }
    cols[15] = 1.0;
    *matrix = Matrix3D::from_cols_array(&cols);
}

pub(super) fn xfer_u128_bits(xfer: &mut dyn Xfer, value: &mut u128) -> std::io::Result<()> {
    let mut lo = (*value & 0xFFFF_FFFF_FFFF_FFFF) as u64;
    let mut hi = (*value >> 64) as u64;
    xfer.xfer_u64(&mut lo)?;
    xfer.xfer_u64(&mut hi)?;
    *value = ((hi as u128) << 64) | (lo as u128);
    Ok(())
}

pub(super) fn xfer_coord3d_values(xfer: &mut dyn Xfer, value: &mut Coord3D) -> std::io::Result<()> {
    xfer.xfer_real(&mut value.x)?;
    xfer.xfer_real(&mut value.y)?;
    xfer.xfer_real(&mut value.z)?;
    Ok(())
}

pub(super) fn xfer_sighting_info(
    xfer: &mut dyn Xfer,
    sighting: &mut SightingInfo,
) -> std::io::Result<()> {
    // C++ SightingInfo::xfer (PartitionManager.cpp:5786-5805): version + Coord3D + howFar + PlayerMask + data.
    let current_version: u8 = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer_coord3d_values(xfer, &mut sighting.where_pos)?;
    xfer.xfer_real(&mut sighting.how_far)?;
    let mut for_whom = sighting.for_whom.bits() as u16;
    xfer.xfer_unsigned_short(&mut for_whom)?;
    sighting.for_whom = PlayerMaskType::from_bits_retain(for_whom as u32);
    xfer.xfer_unsigned_int(&mut sighting.data)?;
    Ok(())
}

pub(super) fn xfer_geometry_info(
    xfer: &mut dyn Xfer,
    geometry: &mut GeometryInfo,
) -> std::io::Result<()> {
    // Transfer the canonical original value, including serialized caches.
    game_engine::common::system::Snapshotable::xfer(&mut geometry.shape, xfer)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

/// C++ `BitFlags<NUMBITS>::xfer` (BitFlagsIO.h:134-205): version + Int count + set-bit names.
pub(super) fn xfer_named_bits(
    xfer: &mut dyn Xfer,
    bits: &mut u128,
    names: &[&str],
) -> std::io::Result<()> {
    let current_version: u8 = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    match xfer.get_xfer_mode() {
        game_engine::system::XferMode::Save => {
            let mut count = 0i32;
            for i in 0..names.len() {
                if (*bits & (1u128 << i)) != 0 {
                    count += 1;
                }
            }
            xfer.xfer_int(&mut count)?;
            for (i, name) in names.iter().enumerate() {
                if (*bits & (1u128 << i)) != 0 {
                    let mut token = (*name).to_string();
                    xfer.xfer_ascii_string(&mut token)?;
                }
            }
        }
        game_engine::system::XferMode::Load => {
            *bits = 0;
            let mut count = 0i32;
            xfer.xfer_int(&mut count)?;
            if count < 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "negative bit-name count",
                ));
            }
            for _ in 0..count {
                let mut token = String::new();
                xfer.xfer_ascii_string(&mut token)?;
                match names
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(&token))
                {
                    Some(index) => *bits |= 1u128 << index,
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Object::xfer invalid bit name '{token}'"),
                        ));
                    }
                }
            }
        }
        game_engine::system::XferMode::Crc => {
            xfer_u128_bits(xfer, bits)?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) const OBJECT_STATUS_XFER_NAMES: &[&str] = &[
    "NONE",
    "DESTROYED",
    "CAN_ATTACK",
    "UNDER_CONSTRUCTION",
    "UNSELECTABLE",
    "NO_COLLISIONS",
    "NO_ATTACK",
    "AIRBORNE_TARGET",
    "PARACHUTING",
    "REPULSOR",
    "HIJACKED",
    "AFLAME",
    "BURNED",
    "WET",
    "IS_FIRING_WEAPON",
    "IS_BRAKING",
    "STEALTHED",
    "DETECTED",
    "CAN_STEALTH",
    "SOLD",
    "UNDERGOING_REPAIR",
    "RECONSTRUCTING",
    "MASKED",
    "IS_ATTACKING",
    "USING_ABILITY",
    "IS_AIMING_WEAPON",
    "NO_ATTACK_FROM_AI",
    "IGNORING_STEALTH",
    "IS_CARBOMB",
    "DECK_HEIGHT_OFFSET",
    "STATUS_RIDER1",
    "STATUS_RIDER2",
    "STATUS_RIDER3",
    "STATUS_RIDER4",
    "STATUS_RIDER5",
    "STATUS_RIDER6",
    "STATUS_RIDER7",
    "STATUS_RIDER8",
    "FAERIE_FIRE",
    "KILLING_SELF",
    "REASSIGN_PARKING",
    "BOOBY_TRAPPED",
    "IMMOBILE",
    "DISGUISED",
    "DEPLOYED",
];

pub(super) const DISABLED_MASK_XFER_NAMES: &[&str] = &[
    "DEFAULT",
    "DISABLED_HACKED",
    "DISABLED_EMP",
    "DISABLED_HELD",
    "DISABLED_PARALYZED",
    "DISABLED_UNMANNED",
    "DISABLED_UNDERPOWERED",
    "DISABLED_FREEFALL",
    "DISABLED_AWESTRUCK",
    "DISABLED_BRAINWASHED",
    "DISABLED_SUBDUED",
    "DISABLED_SCRIPT_DISABLED",
    "DISABLED_SCRIPT_UNDERPOWERED",
];

pub(super) const WEAPON_SET_XFER_NAMES: &[&str] = &[
    "VETERAN",
    "ELITE",
    "HERO",
    "PLAYER_UPGRADE",
    "CRATEUPGRADE_ONE",
    "CRATEUPGRADE_TWO",
    "VEHICLE_HIJACK",
    "CARBOMB",
    "MINE_CLEARING_DETAIL",
    "WEAPON_RIDER1",
    "WEAPON_RIDER2",
    "WEAPON_RIDER3",
    "WEAPON_RIDER4",
    "WEAPON_RIDER5",
    "WEAPON_RIDER6",
    "WEAPON_RIDER7",
    "WEAPON_RIDER8",
];

pub(super) const SPECIAL_POWER_XFER_NAMES: &[&str] = &[
    "SPECIAL_INVALID",
    "SPECIAL_DAISY_CUTTER",
    "SPECIAL_PARADROP_AMERICA",
    "SPECIAL_CARPET_BOMB",
    "SPECIAL_CLUSTER_MINES",
    "SPECIAL_EMP_PULSE",
    "SPECIAL_NAPALM_STRIKE",
    "SPECIAL_CASH_HACK",
    "SPECIAL_NEUTRON_MISSILE",
    "SPECIAL_SPY_SATELLITE",
    "SPECIAL_DEFECTOR",
    "SPECIAL_TERROR_CELL",
    "SPECIAL_AMBUSH",
    "SPECIAL_BLACK_MARKET_NUKE",
    "SPECIAL_ANTHRAX_BOMB",
    "SPECIAL_SCUD_STORM",
    "SPECIAL_DEMORALIZE_OBSOLETE",
    "SPECIAL_CRATE_DROP",
    "SPECIAL_A10_THUNDERBOLT_STRIKE",
    "SPECIAL_DETONATE_DIRTY_NUKE",
    "SPECIAL_ARTILLERY_BARRAGE",
    "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES",
    "SPECIAL_REMOTE_CHARGES",
    "SPECIAL_TIMED_CHARGES",
    "SPECIAL_HELIX_NAPALM_BOMB",
    "SPECIAL_HACKER_DISABLE_BUILDING",
    "SPECIAL_TANKHUNTER_TNT_ATTACK",
    "SPECIAL_BLACKLOTUS_CAPTURE_BUILDING",
    "SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK",
    "SPECIAL_BLACKLOTUS_STEAL_CASH_HACK",
    "SPECIAL_INFANTRY_CAPTURE_BUILDING",
    "SPECIAL_RADAR_VAN_SCAN",
    "SPECIAL_SPY_DRONE",
    "SPECIAL_DISGUISE_AS_VEHICLE",
    "SPECIAL_BOOBY_TRAP",
    "SPECIAL_REPAIR_VEHICLES",
    "SPECIAL_PARTICLE_UPLINK_CANNON",
    "SPECIAL_CASH_BOUNTY",
    "SPECIAL_CHANGE_BATTLE_PLANS",
    "SPECIAL_CIA_INTELLIGENCE",
    "SPECIAL_CLEANUP_AREA",
    "SPECIAL_LAUNCH_BAIKONUR_ROCKET",
    "SPECIAL_SPECTRE_GUNSHIP",
    "SPECIAL_GPS_SCRAMBLER",
    "SPECIAL_FRENZY",
    "SPECIAL_SNEAK_ATTACK",
    "SPECIAL_CHINA_CARPET_BOMB",
    "EARLY_SPECIAL_CHINA_CARPET_BOMB",
    "SPECIAL_LEAFLET_DROP",
    "EARLY_SPECIAL_LEAFLET_DROP",
    "EARLY_SPECIAL_FRENZY",
    "SPECIAL_COMMUNICATIONS_DOWNLOAD",
    "EARLY_SPECIAL_REPAIR_VEHICLES",
    "SPECIAL_TANK_PARADROP",
    "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON",
    "AIRF_SPECIAL_DAISY_CUTTER",
    "NUKE_SPECIAL_CLUSTER_MINES",
    "NUKE_SPECIAL_NEUTRON_MISSILE",
    "AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE",
    "AIRF_SPECIAL_SPECTRE_GUNSHIP",
    "INFA_SPECIAL_PARADROP_AMERICA",
    "SLTH_SPECIAL_GPS_SCRAMBLER",
    "AIRF_SPECIAL_CARPET_BOMB",
    "SUPR_SPECIAL_CRUISE_MISSILE",
    "LAZR_SPECIAL_PARTICLE_UPLINK_CANNON",
    "SUPW_SPECIAL_NEUTRON_MISSILE",
    "SPECIAL_BATTLESHIP_BOMBARDMENT",
];

pub(super) fn xfer_coord2d_values(xfer: &mut dyn Xfer, value: &mut Coord2D) -> std::io::Result<()> {
    xfer.xfer_real(&mut value.x)?;
    xfer.xfer_real(&mut value.y)?;
    Ok(())
}

pub(super) fn xfer_color_rgba(xfer: &mut dyn Xfer, value: &mut Color) -> std::io::Result<()> {
    let mut packed = ((value.a as u32) << 24)
        | ((value.b as u32) << 16)
        | ((value.g as u32) << 8)
        | (value.r as u32);
    xfer.xfer_unsigned_int(&mut packed)?;
    value.r = (packed & 0xFF) as u8;
    value.g = ((packed >> 8) & 0xFF) as u8;
    value.b = ((packed >> 16) & 0xFF) as u8;
    value.a = ((packed >> 24) & 0xFF) as u8;
    Ok(())
}

/// Object.cpp:4070-4094: the old 32-bit format has no NONE position.
/// Preserve every original bit, including bit31 -> current enum position32.
pub(super) fn restore_legacy_status(bits: u32) -> ObjectStatusMaskType {
    ObjectStatusMaskType::from_bits_retain(u64::from(bits) << 1)
}

#[cfg(test)]
#[path = "object_xfer_wire/geometry_tests.rs"]
mod geometry_tests;
